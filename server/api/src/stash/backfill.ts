import { setTimeout as sleep } from "node:timers/promises";
import { sql, type Kysely } from "kysely";

import type { Tables } from "../db.ts";
import type { Logger } from "../log.ts";
import { announce } from "./items.ts";
import { linksIn, readLinks } from "./preview.ts";

const BATCH = 100;
/** How long to wait for room to read pages while requests are reading theirs. */
const ROOM_WAIT_MS = 1_000;

/**
 * Keeps the links in the words of the prompts and text stashed before links were kept, one item
 * at a time, reading their pages within the server's limit on pages read at once.
 */
export async function backfillLinks(
  db: Kysely<Tables>,
  log: Logger,
): Promise<void> {
  let filled = 0;
  for (;;) {
    const rows = await db
      .selectFrom("stash_items")
      .select(["id", "user_id", "text"])
      .where("links", "is", null)
      .where("kind", "in", ["text", "prompt"])
      .where("taken_at", "is", null)
      .where("attachments_missing", "=", 0)
      .where(sql<boolean>`text ~* 'https?://'`)
      .orderBy("created_at", "desc")
      .limit(BATCH)
      .execute();
    if (rows.length === 0) break;
    for (const row of rows) {
      const urls = linksIn(row.text ?? "");
      let links = await readLinks(urls);
      while (!links) {
        await sleep(ROOM_WAIT_MS);
        links = await readLinks(urls);
      }
      const kept = links;
      await db.transaction().execute(async (trx) => {
        const updated = await trx
          .updateTable("stash_items")
          .set({ links: JSON.stringify(kept) })
          .where("id", "=", row.id)
          .where("links", "is", null)
          .executeTakeFirst();
        if (updated.numUpdatedRows > 0n && kept.length)
          await announce(trx, row.user_id, "stash.changed", row.id);
      });
      filled += 1;
    }
  }
  if (filled)
    log.info({ items: filled }, "kept the links of earlier stashed items");
}
