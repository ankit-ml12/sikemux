#!/usr/bin/env node
// Writes the agent faces from the app's own art in src/buddies: the catalogue
// the core resolves each agent's face from, and every drawing of each shape for
// the GPUI app. `--check` fails when either is out of date instead of writing.

import { mkdir, readdir, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { gunzipSync, gzipSync } from "node:zlib";
import { createServer } from "vite";

const root = resolve(import.meta.dirname, "..");
const manifestPath = resolve(
  root,
  "src-tauri/crates/sikemux-core/src/assets/faces.json",
);
const drawingsDir = resolve(root, "src-tauri/crates/sikemux-gpui/assets/faces");
const check = process.argv.includes("--check");
const STATES = ["idle", "working", "waiting", "done", "error"];

const server = await createServer({
  root,
  logLevel: "error",
  server: { middlewareMode: true, hmr: false },
  appType: "custom",
  optimizeDeps: { noDiscovery: true, include: [] },
});
const wanted = new Map();
try {
  const { BUDDY_PACKS } = await server.ssrLoadModule(
    "/src/buddies/catalogue.ts",
  );
  const { buddyMarkup, FULL_BOX } = await server.ssrLoadModule(
    "/src/buddies/render.ts",
  );
  const packs = BUDDY_PACKS.map((pack) => ({
    id: pack.id,
    shapes: pack.shapes.map((shape) => shape.id),
    colors: pack.colors.length,
  }));
  wanted.set(
    manifestPath,
    Buffer.from(`${JSON.stringify({ packs }, null, 2)}\n`),
  );
  for (const pack of BUDDY_PACKS) {
    for (const shape of pack.shapes) {
      const drawings = pack.colors.map((color) =>
        Object.fromEntries(
          STATES.map((state) => [
            state,
            buddyMarkup(pack, shape, color, state, "face", FULL_BOX).replace(
              "<svg ",
              '<svg xmlns="http://www.w3.org/2000/svg" ',
            ),
          ]),
        ),
      );
      wanted.set(
        resolve(drawingsDir, `${pack.id}.${shape.id}.json.gz`),
        gzipSync(JSON.stringify(drawings), { level: 9 }),
      );
    }
  }
} finally {
  await server.close();
}

const present = (await readdir(drawingsDir).catch(() => [])).map((file) =>
  resolve(drawingsDir, file),
);
const extra = present.filter((path) => !wanted.has(path));
// Compressed files are compared by what they hold, since zlib versions differ in the bytes they write.
const unpacked = (path, bytes) =>
  path.endsWith(".gz") ? gunzipSync(bytes) : bytes;
const stale = [];
for (const [path, bytes] of wanted) {
  const current = await readFile(path).catch(() => null);
  if (!current || !unpacked(path, current).equals(unpacked(path, bytes)))
    stale.push(path);
}
if (check) {
  const wrong = [...stale, ...extra].map((path) => path.slice(root.length + 1));
  if (wrong.length) {
    console.error(
      `Agent faces are out of date; run node scripts/generate-faces.mjs:\n  ${wrong.join("\n  ")}`,
    );
    process.exit(1);
  }
} else {
  await mkdir(drawingsDir, { recursive: true });
  await mkdir(resolve(manifestPath, ".."), { recursive: true });
  for (const path of extra) await rm(path);
  for (const path of stale) await writeFile(path, wanted.get(path));
}
