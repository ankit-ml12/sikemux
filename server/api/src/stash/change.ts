import type { StashItem, StashUpdate } from "@sikemux/protocol";
import { sql, type Kysely } from "kysely";

import type { Tables } from "../db.ts";
import { badRequest, notFound } from "../http.ts";
import { announce, shownAs, type ItemRow } from "./items.ts";

/** The account's item `id`, locked until the transaction `db` is in ends. */
export async function ownedItem(
  db: Kysely<Tables>,
  userId: string,
  id: string,
): Promise<ItemRow> {
  const row = await db
    .selectFrom("stash_items")
    .selectAll()
    .where("id", "=", id)
    .where("user_id", "=", userId)
    .forUpdate()
    .executeTakeFirst();
  if (!row) throw notFound("No stashed item has that id.");
  return row;
}

/** Refuses an item that is not shown on its own: an attachment, or a prompt still receiving them. */
function listed(row: ItemRow): ItemRow {
  if (row.prompt_id !== null)
    throw badRequest("An attachment is changed with its prompt.");
  if (row.attachments_missing > 0)
    throw notFound("That prompt is still receiving its attachments.");
  return row;
}

/** The account's item `id` as clients see it, once a prompt has all its attachments. */
export async function shownItem(
  db: Kysely<Tables>,
  userId: string,
  id: string,
): Promise<StashItem> {
  const row = await ownedItem(db, userId, id);
  if (row.attachments_missing > 0)
    throw notFound("That prompt is still receiving its attachments.");
  return shownAs(db, row);
}

/** A new note, or taken once it is used, with a prompt's attachments taken alongside it. */
export async function updateItem(
  db: Kysely<Tables>,
  userId: string,
  id: string,
  update: StashUpdate,
): Promise<StashItem> {
  if (update.taken === false)
    throw badRequest("A taken item cannot be untaken.");
  const row = await db.transaction().execute(async (trx) => {
    const before = listed(await ownedItem(trx, userId, id));
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
  return shownAs(db, row);
}

/** Deletes the account's item `id` with a prompt's attachments, answering the ids of those that had files. */
export async function removeItem(
  db: Kysely<Tables>,
  userId: string,
  id: string,
): Promise<string[]> {
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
  return removed.filter((row) => row.size !== null).map((row) => row.id);
}
