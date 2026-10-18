import { Hono } from "hono";

import { requireIdentity, type AuthEnv, type Verifier } from "../auth.ts";
import type { Database } from "../db.ts";
import { ApiFailure } from "../http.ts";
import { limit, type RateLimiter } from "../limits.ts";
import type { ClerkBackend } from "./clerk.ts";

/** GET /v1/account/profile: who a host belongs to, for the host to show, read from Clerk. */
export function profileRoutes(
  { db }: Database,
  verifier: Verifier,
  limiter: RateLimiter,
  clerk: ClerkBackend | null,
) {
  return new Hono<AuthEnv>().get(
    "/",
    requireIdentity(verifier, db, { accept: ["host"] }),
    limit<AuthEnv>(limiter, "profile", 10, (c) => c.get("identity").userId),
    async (c) => {
      if (!clerk)
        throw new ApiFailure(
          503,
          "unavailable",
          "The account's profile cannot be read right now.",
        );
      return c.json(await clerk.profile(c.get("identity").userId));
    },
  );
}
