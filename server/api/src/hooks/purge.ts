import { sql, type Kysely } from "kysely";

import type { Tables } from "../db.ts";
import type { Logger } from "../log.ts";
import { HOOK_OPTIONS } from "./options.ts";

/** Drops the webhook requests no host took within a day. */
export async function purgeHookDeliveries(
  db: Kysely<Tables>,
  log: Logger,
): Promise<void> {
  const dropped = await db
    .deleteFrom("hook_deliveries")
    .where(
      "received_at",
      "<",
      sql<Date>`now() - make_interval(secs => ${HOOK_OPTIONS.keptMs / 1000})`,
    )
    .executeTakeFirst();
  if (dropped.numDeletedRows > 0n)
    log.info(
      { requests: Number(dropped.numDeletedRows) },
      "dropped webhook requests no host took",
    );
}
