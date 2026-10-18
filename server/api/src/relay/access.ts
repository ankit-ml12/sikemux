import { createHash, timingSafeEqual } from "node:crypto";
import { Hono } from "hono";

import type { Database } from "../db.ts";
import type { Env } from "../http.ts";

/** The header iroh-relay names the connecting device in. */
const ENDPOINT_HEADER = "x-iroh-nodeid";

function sameSecret(given: string, expected: string): boolean {
  const digest = (text: string) => createHash("sha256").update(text).digest();
  return timingSafeEqual(digest(given), digest(expected));
}

/**
 * POST /internal/relay-access, which relay.sikemux.com asks before it carries a device's
 * traffic: only devices on an account that stands may use it. Caddy keeps /internal off the
 * internet, and the relay proves itself with the token both are given.
 */
export function relayAccessRoutes({ db }: Database, token: string | null) {
  return new Hono<Env>().post("/relay-access", async (c) => {
    const bearer = /^Bearer\s+(\S+)$/i.exec(
      c.req.header("authorization") ?? "",
    );
    if (!token || !bearer?.[1] || !sameSecret(bearer[1], token))
      return c.text("false", 401);
    const endpoint = c.req.header(ENDPOINT_HEADER) ?? "";
    if (!/^[0-9a-f]{64}$/.test(endpoint)) return c.text("false", 403);
    const device = await db
      .selectFrom("devices")
      .innerJoin("users", "users.id", "devices.user_id")
      .select("devices.key")
      .where("devices.key", "=", endpoint)
      .where("users.deleted_at", "is", null)
      .executeTakeFirst();
    return device ? c.text("true") : c.text("false", 403);
  });
}
