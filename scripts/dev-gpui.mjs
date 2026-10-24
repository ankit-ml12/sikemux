#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import {
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  signalProcessTree,
  stopProcessTree,
  withoutTerminalSession,
} from "./dev-desktop.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const debug = join(root, "src-tauri", "target", "debug");
const appBinary = join(
  root,
  "src-tauri",
  "crates",
  "sikemux-gpui",
  "target",
  "debug",
  "sikemux",
);
const bundle = join(debug, "Sikemux Dev.app");
const macos = join(bundle, "Contents", "MacOS");
const ownCore = join(macos, "sikemux-host");
const socket = join(
  process.env.HOME ?? homedir(),
  ".config",
  "sikemux",
  "core.dev.sock",
);
const helpers = ["sikemux-host", "sikemux-voice", "sikemux-sim"];
const notch = "Sikemux Notch Dev.app";

function fail(message) {
  console.error(message);
  process.exit(1);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function buildApp() {
  run("cargo", [
    "build",
    "--manifest-path",
    "src-tauri/crates/sikemux-gpui/Cargo.toml",
    "--bin",
    "sikemux",
  ]);
}

function icon(resources) {
  const source = join(root, "brand", "icon", "icon-dev-rounded-1024.png");
  const target = join(resources, "AppIcon.icns");
  if (
    existsSync(target) &&
    statSync(target).mtimeMs >= statSync(source).mtimeMs
  )
    return;
  const iconset = join(tmpdir(), `sikemux-dev-${process.pid}.iconset`);
  mkdirSync(iconset, { recursive: true });
  for (const size of [16, 32, 128, 256, 512]) {
    for (const [scale, suffix] of [
      [1, ""],
      [2, "@2x"],
    ]) {
      const pixels = String(size * scale);
      run("sips", [
        "-z",
        pixels,
        pixels,
        source,
        "--out",
        join(iconset, `icon_${size}x${size}${suffix}.png`),
      ]);
    }
  }
  run("iconutil", ["-c", "icns", iconset, "-o", target]);
  rmSync(iconset, { recursive: true, force: true });
}

// The app finds its core, and the core its notch, voice and simulator helpers, beside its own
// binary, and the core trusts the app beside it, so all of them sit together in Contents/MacOS.
function assemble() {
  const dev = JSON.parse(
    readFileSync(join(root, "src-tauri", "tauri.dev.conf.json"), "utf8"),
  );
  const resources = join(bundle, "Contents", "Resources");
  mkdirSync(macos, { recursive: true });
  mkdirSync(resources, { recursive: true });
  copyFileSync(appBinary, join(macos, "sikemux"), constants.COPYFILE_FICLONE);
  for (const helper of helpers) {
    const source = join(debug, helper);
    if (!existsSync(source)) fail(`${source} is missing; run pnpm sidecar:dev`);
    copyFileSync(source, join(macos, helper), constants.COPYFILE_FICLONE);
  }
  rmSync(join(macos, notch), { recursive: true, force: true });
  run("cp", ["-cR", join(debug, notch), join(macos, notch)]);
  icon(resources);
  writeFileSync(
    join(bundle, "Contents", "Info.plist"),
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>${dev.productName}</string>
  <key>CFBundleDisplayName</key><string>${dev.productName}</string>
  <key>CFBundleIdentifier</key><string>${dev.identifier}</string>
  <key>CFBundleExecutable</key><string>sikemux</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
`,
  );
}

function executable(pid) {
  const result = spawnSync("ps", ["-p", String(pid), "-o", "comm="], {
    encoding: "utf8",
  });
  return result.stdout.trim();
}

// Another build's core on the dev socket would be asked to replace itself with this one.
function otherCore() {
  if (!existsSync(socket)) return null;
  const holders = spawnSync("lsof", ["-t", socket], { encoding: "utf8" });
  for (const pid of holders.stdout.split("\n").filter(Boolean)) {
    const path = executable(pid);
    if (path.endsWith("/sikemux-host") && path !== ownCore)
      return { pid, path };
  }
  return null;
}

// Kept from the caller: SIKEMUX_LAUNCH_BEHIND opens the window behind the app in use, and
// SIKEMUX_AGENT_SEARCH=path keeps a sandboxed run's core to the agent CLIs on its PATH.
const CALLER_OPTIONS = ["SIKEMUX_LAUNCH_BEHIND", "SIKEMUX_AGENT_SEARCH"];

function appEnvironment() {
  const env = withoutTerminalSession(process.env);
  for (const key of CALLER_OPTIONS)
    if (process.env[key]) env[key] = process.env[key];
  return env;
}

function runningApp() {
  const listing = spawnSync("pgrep", ["-f", `^${join(macos, "sikemux")}$`], {
    encoding: "utf8",
  });
  return listing.stdout.trim().split("\n").filter(Boolean)[0] ?? null;
}

function refusal() {
  const running = runningApp();
  if (running)
    return `Sikemux Dev is already running from this checkout (PID ${running}). Quit it before running make dev again.`;
  const other = otherCore();
  if (other)
    return `Another Sikemux Dev's core runs on ${socket} (PID ${other.pid}, ${other.path}). Quit that Sikemux Dev and run make dev-stop in its checkout first.`;
  return null;
}

export async function runDevGpui({ build }) {
  if (build) {
    const before = refusal();
    if (before) {
      console.error(before);
      return 1;
    }
    run("pnpm", ["sidecar:dev"]);
    buildApp();
  }
  const refused = refusal();
  if (refused) {
    console.error(refused);
    return 1;
  }
  assemble();

  const child = spawn(join(macos, "sikemux"), [], {
    cwd: root,
    detached: true,
    env: appEnvironment(),
    stdio: "inherit",
  });
  let requested = null;
  const handlers = new Map();
  for (const [signal, code] of [
    ["SIGHUP", 129],
    ["SIGINT", 130],
    ["SIGTERM", 143],
  ]) {
    const handler = () => {
      if (requested !== null) return;
      requested = code;
      signalProcessTree(child.pid, signal);
    };
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  const outcome = await new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
  for (const [signal, handler] of handlers) process.off(signal, handler);
  await stopProcessTree(child.pid);
  if (requested !== null) return requested;
  if (outcome.code !== null) return outcome.code;
  return outcome.signal ? 1 : 0;
}

const isMain = process.argv[1]
  ? resolve(process.argv[1]) === fileURLToPath(import.meta.url)
  : false;

if (isMain)
  process.exitCode = await runDevGpui({
    build: !process.argv.includes("--no-build"),
  });
