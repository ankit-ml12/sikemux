import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { createServer } from "vite";

const server = await createServer({
  configFile: resolve(import.meta.dirname, "vite.config.ts"),
  logLevel: "warn",
  server: { strictPort: false },
});
await server.listen();

const url = new URL("showcase/twitter.html", server.resolvedUrls.local[0]).href;
execFileSync("open", [url]);
console.log(`${url}\nPick a view and press Download; it saves to ~/Downloads.`);
