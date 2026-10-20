#!/usr/bin/env node
// GPUI Kit depends on Zed's crates as published under `gpui-pre-*`, at one
// exact version. Cargo patches a crate only with one of the same name and
// version, so this checks out the sonorahq fork, applies fork.patch and renames
// its crates to match.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "https://github.com/sonorahq/gpui";
const REV = "85954e34d5a6bf808237d82757852312749b99b2";
const VERSION = "0.3.9";

const fork = join(dirname(fileURLToPath(import.meta.url)), "target", "gpui-fork");
const git = (...args) => execFileSync("git", ["-C", fork, ...args], { stdio: "inherit" });

if (!existsSync(join(fork, ".git"))) {
  execFileSync("git", ["clone", "--quiet", REPO, fork], { stdio: "inherit" });
}
git("fetch", "--quiet", "origin", REV);
git("checkout", "--quiet", "--force", REV);
// What GPUI Kit needs of the fork to build against it as it does against gpui-pre.
git("apply", join(dirname(fileURLToPath(import.meta.url)), "fork.patch"));

// Zed's `util` is unpublished and would take `gpui_util`'s name.
const RENAMED = { gpui: "gpui-pre", util: "gpui-pre-zed-util" };
const published = (name) =>
  RENAMED[name] ?? `gpui-pre-${name.replace(/^gpui_/, "").replaceAll("_", "-")}`;

const root = join(fork, "Cargo.toml");
const workspace = readFileSync(root, "utf8");
const members = [...workspace.matchAll(/^\s*"((?:crates|tooling)\/[^"]+)",$/gm)].map((m) => m[1]);
const names = new Set();

for (const member of members) {
  const manifest = join(fork, member, "Cargo.toml");
  let text = readFileSync(manifest, "utf8");
  const name = text.match(/^\[package\]\nname = "([^"]+)"/m)[1];
  names.add(name);
  text = text
    .replace(/^(\[package\]\nname = )"[^"]+"/m, `$1"${published(name)}"`)
    .replace(/^version = "[^"]+"/m, `version = "${VERSION}"`);
  text = /^\[lib\]$/m.test(text)
    ? text.replace(/^\[lib\]$/m, `[lib]\nname = "${name}"`)
    : `${text.trimEnd()}\n\n[lib]\nname = "${name}"\n`;
  writeFileSync(manifest, text);
}

writeFileSync(
  root,
  workspace.replace(/^([a-z_]+) = \{ path = ("[^"]+")(?:, version = "[^"]+")?/gm, (line, name, path) =>
    names.has(name) ? `${name} = { package = "${published(name)}", path = ${path}` : line,
  ),
);

git("diff", "--stat");
