import { randomBytes } from "node:crypto";
import {
  validator,
  stashTakenValues,
  type StashItem,
  type StashLink,
  type StashList,
  type StashSeen,
  type StashTaken,
  type StashTarget,
} from "@sikemux/protocol";
import { Hono, type Context } from "hono";
import { sql, type Kysely } from "kysely";

import {
  requireIdentity,
  type AuthEnv,
  type Identity,
  type Verifier,
} from "../auth.ts";
import type { Database, Tables } from "../db.ts";
import {
  ApiFailure,
  badRequest,
  limitBody,
  notFound,
  readBody,
} from "../http.ts";
import { limit, type RateLimiter } from "../limits.ts";
import { appendEvent } from "../events/log.ts";
import type { Logger } from "../log.ts";
import type { StashFiles } from "./files.ts";
import { removeStashFiles } from "./files.ts";
import {
  announce,
  checkSpace,
  checkTarget,
  holdAccount,
  ITEM_ID,
  MAX_FILE_BYTES,
  toItem,
  withAttachments,
  type ItemRow,
} from "./items.ts";
import { fetchPreview, isFetchable, linksIn } from "./preview.ts";

const PAGE = 50;
const NAME_MAX = 255;
const NOTE_MAX = 2000;
const MIME = /^[a-z0-9][a-z0-9!#$&^_.+-]*\/[a-z0-9][a-z0-9!#$&^_.+-]*$/;
const CONTROL = /\p{Cc}/u;
const CURSOR_TIME =
  /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}(\.\d{1,6})?[+-]\d{2}(:\d{2})?$/;
/** Links whose pages are read at once; past this a link is stashed without a preview. */
const PREVIEWS_AT_ONCE = 8;
/** How many links in a text's or prompt's words are kept, and how many of their pages are read. */
const LINKS_KEPT = 6;
const LINKS_READ = 3;

const protocol = validator();

function isUpload(c: Context): boolean {
  return (
    c.req.method === "POST" &&
    !/^application\/json\b/i.test(c.req.header("content-type") ?? "")
  );
}

function newId(): string {
  return randomBytes(16).toString("base64url");
}

function itemId(c: Context): string {
  const id = c.req.param("id") ?? "";
  if (!ITEM_ID.test(id)) throw notFound("No stashed item has that id.");
  return id;
}

function header(c: Context, name: string): string | undefined {
  const value = c.req.header(name);
  if (value === undefined) return undefined;
  try {
    return decodeURIComponent(value);
  } catch {
    throw badRequest(`${name} is not URI-encoded.`);
  }
}

function readTarget(c: Context): StashTarget | undefined {
  const text = header(c, "x-stash-target");
  if (text === undefined) return undefined;
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw badRequest("X-Stash-Target is not JSON.");
  }
  const result = protocol.validate("StashTarget", value);
  if (!result.ok)
    throw badRequest(
      `X-Stash-Target is not a valid StashTarget: ${result.problems.join("; ")}.`,
    );
  return result.value;
}

/** The kind, name, type and note of an upload, from its headers. */
function readUpload(c: Context) {
  const mime = (c.req.header("content-type") ?? "")
    .split(";")[0]
    ?.trim()
    .toLowerCase();
  if (!mime || mime.length > NAME_MAX || !MIME.test(mime))
    throw badRequest("A file is sent with its media type in Content-Type.");
  const name = header(c, "x-stash-name")?.trim();
  if (!name || name.length > NAME_MAX || CONTROL.test(name))
    throw badRequest(
      `A file is sent with its name in X-Stash-Name, at most ${NAME_MAX} characters.`,
    );
  const note = header(c, "x-stash-note");
  if (note !== undefined && note.length > NOTE_MAX)
    throw badRequest(`X-Stash-Note is limited to ${NOTE_MAX} characters.`);
  const kind = mime.startsWith("image/")
    ? ("image" as const)
    : mime.startsWith("video/")
      ? ("video" as const)
      : ("file" as const);
  const prompt = c.req.header("x-stash-prompt");
  if (prompt !== undefined && !ITEM_ID.test(prompt))
    throw badRequest("X-Stash-Prompt is not a stashed item's id.");
  const target = readTarget(c);
  if (prompt !== undefined && target)
    throw badRequest(
      "An attachment goes where its prompt goes; send no target.",
    );
  return { kind, mime, name, note: note || null, target, prompt };
}

/** Refuses an item that is not shown on its own: an attachment, or a prompt still receiving them. */
function listed(row: ItemRow): ItemRow {
  if (row.prompt_id !== null)
    throw badRequest("An attachment is changed with its prompt.");
  if (row.attachments_missing > 0)
    throw notFound("That prompt is still receiving its attachments.");
  return row;
}

/** The device the request came from: the one that signed it, or the phone a session belongs to. */
async function fromDevice(
  db: Kysely<Tables>,
  { userId, deviceKey, sessionId }: Identity,
): Promise<string | null> {
  if (deviceKey) return deviceKey;
  if (!sessionId) return null;
  const phone = await db
    .selectFrom("devices")
    .select("key")
    .where("user_id", "=", userId)
    .where("clerk_session_id", "=", sessionId)
    .executeTakeFirst();
  return phone?.key ?? null;
}

function cursorOf(createdAt: string, id: string): string {
  return Buffer.from(`${createdAt}|${id}`).toString("base64url");
}

function readCursor(cursor: string): { createdAt: string; id: string } {
  const [createdAt = "", id = ""] = Buffer.from(cursor, "base64url")
    .toString("utf8")
    .split("|");
  if (!CURSOR_TIME.test(createdAt) || !ITEM_ID.test(id))
    throw badRequest("The cursor is not one this server gave.");
  return { createdAt, id };
}

function contentDisposition(name: string): string {
  const plain = name.replace(/[^\x20-\x7e]|["\\]/g, "_");
  return `attachment; filename="${plain}"; filename*=UTF-8''${encodeURIComponent(name)}`;
}

/** The account's stash: links, text and files people share into Sikemux from other apps. */
export function stashRoutes(
  { db }: Database,
  verifier: Verifier,
  limiter: RateLimiter,
  files: StashFiles | null,
  log: Logger,
) {
  const perUser = (name: string, perMinute: number) =>
    limit<AuthEnv>(limiter, name, perMinute, (c) => c.get("identity").userId);
  const jsonBodies = limitBody<AuthEnv>();
  let previewing = 0;

  const stored = (): StashFiles => {
    if (!files)
      throw new ApiFailure(
        503,
        "unavailable",
        "The stash is not set up on this server.",
      );
    return files;
  };

  const owned = async (
    userId: string,
    id: string,
    from: Kysely<Tables> = db,
  ) => {
    const row = await from
      .selectFrom("stash_items")
      .selectAll()
      .where("id", "=", id)
      .where("user_id", "=", userId)
      .forUpdate()
      .executeTakeFirst();
    if (!row) throw notFound("No stashed item has that id.");
    return row;
  };

  /** The account's prompt that is still waiting for attachments, locked while one is added. */
  const openPrompt = async (
    from: Kysely<Tables>,
    userId: string,
    id: string,
  ) => {
    const row = await from
      .selectFrom("stash_items")
      .select("id")
      .where("id", "=", id)
      .where("user_id", "=", userId)
      .where("kind", "=", "prompt")
      .where("attachments_missing", ">", 0)
      .forUpdate()
      .executeTakeFirst();
    if (!row)
      throw badRequest("That prompt is not waiting for any more attachments.");
  };

  /**
   * Reads up to `wanted` pages in the background, as many as there is room for, and saves what
   * `read` makes of that many.
   */
  const readPages = (
    userId: string,
    id: string,
    wanted: number,
    read: (
      room: number,
    ) => Promise<{ preview: string } | { links: string } | null>,
  ) => {
    const room = Math.min(wanted, PREVIEWS_AT_ONCE - previewing);
    if (room <= 0) return;
    previewing += room;
    read(room)
      .then(async (found) => {
        if (!found) return;
        await db.transaction().execute(async (trx) => {
          const updated = await trx
            .updateTable("stash_items")
            .set(found)
            .where("id", "=", id)
            .executeTakeFirst();
          if (updated.numUpdatedRows > 0n)
            await announce(trx, userId, "stash.changed", id);
        });
      })
      .catch((error: unknown) =>
        log.warn({ err: error, id }, "could not save a link's preview"),
      )
      .finally(() => {
        previewing -= room;
      });
  };

  const previewLink = (userId: string, id: string, url: string) =>
    readPages(userId, id, 1, async () => {
      const found = await fetchPreview(url);
      return found && { preview: JSON.stringify(found) };
    });

  /** Reads the pages of the first links in an item's words. */
  const previewLinks = (userId: string, id: string, urls: string[]) =>
    readPages(userId, id, Math.min(urls.length, LINKS_READ), async (room) => {
      const found = await Promise.all(
        urls.map((url, at) => (at < room ? fetchPreview(url) : null)),
      );
      if (found.every((preview) => preview === null)) return null;
      const links: StashLink[] = urls.map((url, at) => ({
        url,
        preview: found[at] ?? null,
      }));
      return { links: JSON.stringify(links) };
    });

  const add = async (c: Context<AuthEnv>): Promise<StashItem> => {
    const identity = c.get("identity");
    const { userId } = identity;
    const request = await readBody(c, "StashAdd");
    if (request.kind === "link") {
      const url = URL.parse(request.url ?? "");
      if (!url || !(url.protocol === "http:" || url.protocol === "https:"))
        throw badRequest("A link is an http or https url.");
      if (request.text !== undefined)
        throw badRequest("A link has a url and no text.");
    } else if (request.kind === "text" || request.kind === "prompt") {
      if (request.text === undefined || request.url !== undefined)
        throw badRequest("Text and prompts have text and no url.");
    } else {
      throw badRequest(
        "Images, videos and files are sent as their bytes, not as JSON.",
      );
    }
    if (request.attachmentCount !== undefined && request.kind !== "prompt")
      throw badRequest("Only a prompt has attachments.");
    const missing = request.attachmentCount ?? 0;
    const links = linksIn(request.text ?? "").slice(0, LINKS_KEPT);
    if (request.target) await checkTarget(db, userId, request.target);
    const id = newId();
    const row = await db.transaction().execute(async (trx) => {
      await holdAccount(trx, userId);
      const row = await trx
        .insertInto("stash_items")
        .values({
          id,
          user_id: userId,
          kind: request.kind,
          url: request.url ?? null,
          text: request.text ?? null,
          note: request.note || null,
          name: null,
          mime: null,
          size: null,
          preview: null,
          links: links.length
            ? JSON.stringify(links.map((url) => ({ url, preview: null })))
            : null,
          target: request.target ? JSON.stringify(request.target) : null,
          from_device: await fromDevice(trx, identity),
          attachments_missing: missing,
        })
        .returningAll()
        .executeTakeFirstOrThrow();
      if (missing === 0) await announce(trx, userId, "stash.added", id);
      return row;
    });
    if (row.url) {
      const url = URL.parse(row.url);
      if (url && isFetchable(url)) previewLink(userId, id, row.url);
    }
    if (links.length) previewLinks(userId, id, links);
    return toItem(row);
  };

  const upload = async (c: Context<AuthEnv>): Promise<StashItem> => {
    const store = stored();
    const identity = c.get("identity");
    const { userId } = identity;
    const { kind, mime, name, note, target, prompt } = readUpload(c);
    const declared = Number(c.req.header("content-length") ?? 0);
    if (declared > MAX_FILE_BYTES)
      throw new ApiFailure(
        413,
        "payload_too_large",
        `A stashed file is limited to ${MAX_FILE_BYTES} bytes.`,
      );
    await checkSpace(db, userId, declared);
    if (target) await checkTarget(db, userId, target);
    if (prompt) await openPrompt(db, userId, prompt);
    const id = newId();
    const written = await store.write(
      id,
      c.req.raw.body ?? (async function* () {})(),
      MAX_FILE_BYTES,
    );
    try {
      c.get("checkBody")(written.sha256);
      const device = await fromDevice(db, identity);
      const row = await db.transaction().execute(async (trx) => {
        await holdAccount(trx, userId);
        await checkSpace(trx, userId, written.size);
        if (prompt) await openPrompt(trx, userId, prompt);
        const row = await trx
          .insertInto("stash_items")
          .values({
            id,
            user_id: userId,
            kind,
            url: null,
            text: null,
            note,
            name,
            mime,
            size: written.size,
            preview: null,
            target: target ? JSON.stringify(target) : null,
            from_device: device,
            prompt_id: prompt ?? null,
          })
          .returningAll()
          .executeTakeFirstOrThrow();
        if (prompt) {
          const { attachments_missing } = await trx
            .updateTable("stash_items")
            .set((eb) => ({
              attachments_missing: eb("attachments_missing", "-", 1),
            }))
            .where("id", "=", prompt)
            .returning("attachments_missing")
            .executeTakeFirstOrThrow();
          if (attachments_missing === 0)
            await announce(trx, userId, "stash.added", prompt);
        } else await announce(trx, userId, "stash.added", id);
        await store.commit(id);
        return row;
      });
      return toItem(row);
    } catch (error) {
      await store.discard(id);
      throw error;
    }
  };

  return new Hono<AuthEnv>()
    .use((c, next) => (isUpload(c) ? next() : jsonBodies(c, next)))
    .use(
      requireIdentity(verifier, db, {
        accept: ["session", "host", "device"],
        streamed: isUpload,
      }),
    )
    .post("/", perUser("stash-add", 60), async (c) => {
      const item = await (isUpload(c) ? upload(c) : add(c));
      c.get("log").info({ id: item.id, kind: item.kind }, "stashed an item");
      return c.json(item, 201);
    })
    .get("/", perUser("stash-list", 120), async (c) => {
      const { userId } = c.get("identity");
      const taken = (c.req.query("taken") ?? "false") as StashTaken;
      if (!stashTakenValues.includes(taken))
        throw badRequest(`taken is one of ${stashTakenValues.join(", ")}.`);
      const cursor = c.req.query("cursor");
      let query = db
        .selectFrom("stash_items")
        .selectAll()
        .select(sql<string>`created_at::text`.as("created_text"))
        .where("user_id", "=", userId)
        .where("prompt_id", "is", null)
        .where("attachments_missing", "=", 0);
      if (taken === "false") query = query.where("taken_at", "is", null);
      if (taken === "true") query = query.where("taken_at", "is not", null);
      if (cursor) {
        const after = readCursor(cursor);
        query = query.where(
          sql<boolean>`(created_at, id) < (${after.createdAt}::timestamptz, ${after.id})`,
        );
      }
      const [rows, user] = await Promise.all([
        query
          .orderBy("created_at", "desc")
          .orderBy("id", "desc")
          .limit(PAGE + 1)
          .execute(),
        db
          .selectFrom("users")
          .select("stash_seen_at")
          .where("id", "=", userId)
          .executeTakeFirst(),
      ]);
      const page = rows.slice(0, PAGE);
      const last = page.at(-1);
      const body: StashList = {
        items: await withAttachments(db, page),
        seenAt: user?.stash_seen_at?.toISOString() ?? null,
      };
      if (rows.length > PAGE && last)
        body.cursor = cursorOf(last.created_text, last.id);
      return c.json(body);
    })
    .put("/seen", perUser("stash-seen", 120), async (c) => {
      const { userId } = c.get("identity");
      const { at } = await readBody(c, "StashSeen");
      const seenAt = await db.transaction().execute(async (trx) => {
        await holdAccount(trx, userId);
        const moved = await trx
          .updateTable("users")
          .set({ stash_seen_at: sql<Date>`least(${at}::timestamptz, now())` })
          .where("id", "=", userId)
          .where((eb) =>
            eb.or([
              eb("stash_seen_at", "is", null),
              eb(
                "stash_seen_at",
                "<",
                sql<Date>`least(${at}::timestamptz, now())`,
              ),
            ]),
          )
          .returning("stash_seen_at")
          .executeTakeFirst();
        if (moved) {
          await appendEvent(trx, { userId, type: "stash.seen" });
          return moved.stash_seen_at;
        }
        const user = await trx
          .selectFrom("users")
          .select("stash_seen_at")
          .where("id", "=", userId)
          .executeTakeFirstOrThrow();
        return user.stash_seen_at;
      });
      const body: StashSeen = { at: (seenAt ?? new Date(at)).toISOString() };
      return c.json(body);
    })
    .get("/:id", perUser("stash-get", 240), async (c) => {
      const row = await owned(c.get("identity").userId, itemId(c));
      if (row.attachments_missing > 0)
        throw notFound("That prompt is still receiving its attachments.");
      const [item] = await withAttachments(db, [row]);
      return c.json(item);
    })
    .get("/:id/file", perUser("stash-file", 120), async (c) => {
      const row = await owned(c.get("identity").userId, itemId(c));
      if (row.size === null || row.name === null || row.mime === null)
        throw notFound("That item has no file.");
      const stream = await stored().read(row.id, c.get("log"));
      if (!stream) {
        c.get("log").error({ id: row.id }, "a stashed item's file is missing");
        throw notFound("That item's file is gone.");
      }
      return c.body(stream, 200, {
        "content-type": row.mime,
        "content-length": String(row.size),
        "content-disposition": contentDisposition(row.name),
        "content-security-policy": "sandbox; default-src 'none'",
        "cache-control": "private, no-store",
      });
    })
    .patch("/:id", perUser("stash-update", 120), async (c) => {
      const { userId } = c.get("identity");
      const id = itemId(c);
      const update = await readBody(c, "StashUpdate");
      if (update.taken === false)
        throw badRequest("A taken item cannot be untaken.");
      const row = await db.transaction().execute(async (trx) => {
        const before = listed(await owned(userId, id, trx));
        if (update.note === undefined && (!update.taken || before.taken_at))
          return before;
        if (update.taken)
          await trx
            .updateTable("stash_items")
            .set({ taken_at: sql<Date>`coalesce(taken_at, now())` })
            .where("prompt_id", "=", id)
            .execute();
        const row = await trx
          .updateTable("stash_items")
          .set({
            ...(update.note !== undefined ? { note: update.note || null } : {}),
            ...(update.taken
              ? { taken_at: sql<Date>`coalesce(taken_at, now())` }
              : {}),
          })
          .where("id", "=", id)
          .where("user_id", "=", userId)
          .returningAll()
          .executeTakeFirst();
        if (!row) throw notFound("No stashed item has that id.");
        await announce(trx, userId, "stash.changed", id);
        return row;
      });
      const [item] = await withAttachments(db, [row]);
      return c.json(item);
    })
    .delete("/:id", perUser("stash-delete", 120), async (c) => {
      const { userId } = c.get("identity");
      const id = itemId(c);
      const removed = await db.transaction().execute(async (trx) => {
        const row = await trx
          .selectFrom("stash_items")
          .select(["prompt_id", "attachments_missing"])
          .where("id", "=", id)
          .where("user_id", "=", userId)
          .forUpdate()
          .executeTakeFirst();
        if (!row) return [];
        if (row.prompt_id !== null)
          throw badRequest("An attachment is deleted with its prompt.");
        const gone = await trx
          .deleteFrom("stash_items")
          .where("user_id", "=", userId)
          .where((eb) => eb.or([eb("id", "=", id), eb("prompt_id", "=", id)]))
          .returning(["id", "size"])
          .execute();
        if (row.attachments_missing === 0)
          await announce(trx, userId, "stash.removed", id);
        return gone;
      });
      const withFiles = removed
        .filter((row) => row.size !== null)
        .map((row) => row.id);
      if (withFiles.length && files)
        await removeStashFiles(files.dir, withFiles).catch((error: unknown) =>
          c
            .get("log")
            .warn({ err: error, id }, "could not delete a stashed file"),
        );
      return c.body(null, 204);
    });
}
