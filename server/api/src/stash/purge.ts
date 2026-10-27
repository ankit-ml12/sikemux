import { sql, type Kysely } from "kysely";

import type { Tables } from "../db.ts";
import type { Logger } from "../log.ts";
import { removeStashFiles, sweepStashFiles } from "./files.ts";
import { TAKEN_KEPT_DAYS } from "./items.ts";

const BATCH = 1_000;

/**
 * Deletes items taken more than 30 days ago, then every file no item holds, which includes those
 * of accounts purged before this ran.
 */
export async function purgeStash(
  db: Kysely<Tables>,
  dir: string | null,
  log: Logger,
): Promise<void> {
  let taken = 0;
  for (;;) {
    const gone = await db
      .deleteFrom("stash_items")
      .where(
        "id",
        "in",
        db
          .selectFrom("stash_items")
          .select("id")
          .where(
            "taken_at",
            "<",
            sql<Date>`now() - make_interval(days => ${TAKEN_KEPT_DAYS})`,
          )
          .limit(BATCH),
      )
      .returning(["id", "size"])
      .execute();
    taken += gone.length;
    if (dir)
      await removeStashFiles(
        dir,
        gone.filter((row) => row.size !== null).map((row) => row.id),
      );
    if (gone.length < BATCH) break;
  }
  if (taken) log.info({ items: taken }, "purged taken stash items");
  if (!dir) return;
  await sweepStashFiles(
    dir,
    async (ids) =>
      new Set(
        (
          await db
            .selectFrom("stash_items")
            .select("id")
            .where("id", "in", ids)
            .execute()
        ).map((row) => row.id),
      ),
    log,
  );
}
