import { createServer as createNetServer } from "node:net";
import { resolve } from "node:path";
import { chromium } from "playwright-core";
import { createServer } from "vite";

export const MAC_USER_AGENT =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";
export const FIXED_TIME = new Date("2026-09-26T09:41:00");

/** `pnpm showcase:serve` keeps 1471, so a capture takes whichever port is free. */
const freePort = () =>
  new Promise((settle, fail) => {
    const probe = createNetServer().listen(0, "localhost", () => {
      const { port } = probe.address();
      probe.close(() => settle(port));
    });
    probe.on("error", fail);
  });

/** The showcase's own Vite server on a free port, with the markdown parser already built. */
export async function startShowcase() {
  const server = await createServer({
    configFile: resolve(import.meta.dirname, "vite.config.ts"),
    logLevel: "warn",
    // Editing the demo while a capture runs must not reload the page under it.
    server: { port: await freePort(), strictPort: true, hmr: false },
  });
  await server.listen();
  const origin = `http://localhost:${server.config.server.port}`;
  // The chat's markdown comes from the app's Rust parser, which may need building first;
  // without this the first scene is taken before its prose arrives.
  await fetch(`${origin}/__showcase/markdown`, {
    method: "POST",
    body: JSON.stringify({ requests: [{ text: "ready" }] }),
  });
  return { origin, close: () => server.close() };
}

export const launchChrome = () => chromium.launch({ channel: "chrome" });
