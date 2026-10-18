import { spawn } from "node:child_process";
import { mkdir, readdir, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { parseArgs } from "node:util";
import { FIXED_TIME, launchChrome, startShowcase } from "../harness.mjs";
import { Director, FPS } from "./director.mjs";
import { STAGE } from "../stage.mjs";

const root = resolve(import.meta.dirname, "../..");
const sceneDir = resolve(import.meta.dirname, "scenes");
const { values: options, positionals: only } = parseArgs({
  allowPositionals: true,
  options: {
    out: { type: "string", default: resolve(root, "marketing/x/posts") },
    height: { type: "string", default: "1440" },
    draft: { type: "boolean", default: false },
    at: { type: "string" },
  },
});

const available = (await readdir(sceneDir))
  .filter((name) => name.endsWith(".mjs"))
  .map((name) => name.slice(0, -4));
const chosen = only.length ? only : available;
for (const name of chosen)
  if (!available.includes(name))
    throw new Error(`no scene ${name}; there are ${available.join(", ")}`);

// Pages are drawn at twice the size and shrunk, so text stays sharp while the camera is zoomed in.
// A draft is a quick, softer look at the timing.
const scale = options.draft ? 1 : 2;
const height = options.draft ? 1080 : Number(options.height);

// `--at 0.5,9,30` saves the frames at those seconds as pictures, for judging a look without a whole render.
const stillsAt = options.at
  ?.split(",")
  .map((seconds) => Math.round(Number(seconds) * FPS));

function picture(png, path) {
  const ffmpeg = spawn(
    "ffmpeg",
    [
      "-y",
      "-loglevel",
      "error",
      "-i",
      "-",
      "-vf",
      `scale=-2:${height}:flags=lanczos`,
      path,
    ],
    { stdio: ["pipe", "inherit", "inherit"] },
  );
  ffmpeg.stdin.end(png);
  return new Promise((done) => ffmpeg.on("exit", done));
}

function encoder(path) {
  const ffmpeg = spawn(
    "ffmpeg",
    [
      ...[
        "-y",
        "-loglevel",
        "error",
        "-f",
        "image2pipe",
        "-framerate",
        String(FPS),
      ],
      ...["-c:v", "png", "-i", "-", "-vf", `scale=-2:${height}:flags=lanczos`],
      ...["-c:v", "libx264", "-preset", options.draft ? "veryfast" : "slow"],
      ...["-crf", options.draft ? "24" : "14", "-pix_fmt", "yuv420p"],
      ...["-profile:v", "high", "-movflags", "+faststart", path],
    ],
    { stdio: ["pipe", "inherit", "inherit"] },
  );
  const finished = new Promise((done, fail) =>
    ffmpeg.on("exit", (code) =>
      code === 0 ? done() : fail(new Error(`ffmpeg exited with ${code}`)),
    ),
  );
  return {
    write: (png) =>
      ffmpeg.stdin.write(png)
        ? Promise.resolve()
        : new Promise((done) => ffmpeg.stdin.once("drain", done)),
    close: () => {
      ffmpeg.stdin.end();
      return finished;
    },
  };
}

const showcase = await startShowcase();
const browser = await launchChrome();

for (const name of chosen) {
  const { default: scene } = await import(resolve(sceneDir, `${name}.mjs`));
  const context = await browser.newContext({
    viewport: STAGE,
    deviceScaleFactor: scale,
    colorScheme: "dark",
  });
  const start = FIXED_TIME.getTime();
  await context.clock.install({ time: start });
  await context.clock.pauseAt(start + 1000);
  const page = await context.newPage();
  page.on("pageerror", (error) =>
    console.warn(`${name}: ${error.message.split("\n")[0]}`),
  );
  const cdp = await context.newCDPSession(page);
  await page.goto(
    `${showcase.origin}/showcase/twitter.html?capture&video&notch=${scene.notch ?? "closed"}`,
  );
  await cdp.send("Animation.enable");
  await cdp.send("Animation.setPlaybackRate", { playbackRate: 0 });

  // A draft stays out of the posts, in the showcase's own ignored output.
  const folder = options.draft
    ? resolve(root, "showcase/out/video")
    : resolve(options.out, name);
  await mkdir(folder, { recursive: true });
  const path = resolve(folder, options.draft ? `${name}.mp4` : "clip.mp4");
  const stills = [];
  const video = stillsAt ? null : encoder(path);
  const director = new Director({
    page,
    app: null,
    start: start + 1000,
    capture: async () => {
      if (stillsAt && !stillsAt.includes(director.recorded)) return;
      const { data } = await cdp.send("Page.captureScreenshot", {
        format: "png",
        optimizeForSpeed: !stillsAt,
      });
      const png = Buffer.from(data, "base64");
      if (video) return video.write(png);
      const still = resolve(
        root,
        "showcase/out/video",
        `${name}-${(director.recorded / FPS).toFixed(1)}s.png`,
      );
      await mkdir(resolve(root, "showcase/out/video"), { recursive: true });
      await picture(png, still);
      stills.push(still);
    },
  });
  const began = Date.now();
  await director.until(
    async () =>
      page.evaluate(() => Boolean(window.video?.ready())).catch(() => false),
    { label: "the app to start", limit: 120_000 },
  );
  director.appFrame = page.frames().find((frame) => frame !== page.mainFrame());
  await director.app(() => document.fonts.ready);
  await director.settle(1200);
  await scene.setup?.(director);
  await director.settle(scene.settle ?? 600);
  director.recording = true;
  try {
    await scene.play(director);
  } catch (error) {
    const shot = resolve(root, "showcase/out", `video-${name}-failed.png`);
    await page.screenshot({ path: shot });
    console.error(
      `✗ ${name} failed ${director.seconds.toFixed(1)} s in; the screen then is ${shot}`,
    );
    throw error;
  }
  director.background.clear();
  await director.still();
  if (!video) {
    console.log(`✓ ${name}  ${stills.length} stills\n  ${stills.join("\n  ")}`);
    await context.close();
    continue;
  }
  await video.close();
  const { size } = await stat(path);
  console.log(
    `✓ ${name}  ${director.seconds.toFixed(1)} s, ${(size / 1e6).toFixed(1)} MB, rendered in ${((Date.now() - began) / 1000).toFixed(0)} s\n  ${path}`,
  );
  await context.close();
}

await browser.close();
await showcase.close();
