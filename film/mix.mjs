import { execFile } from "node:child_process";
import { readFile, rename } from "node:fs/promises";
import { resolve } from "node:path";
import { parseArgs, promisify } from "node:util";

const run = promisify(execFile);
const ROOT = resolve(import.meta.dirname, "..");
const SOUNDS = resolve(ROOT, "marketing/x/sfx");

/** When the S mark lands after the closing card starts, where the riser peaks and the music's last hit goes. */
const MARK_LANDS = 0.45;
const RISER_SECONDS = 1.8;

function seeded(seed) {
  let state = seed;
  return () => {
    state = (state * 1664525 + 1013904223) % 4294967296;
    return state / 4294967296;
  };
}

/** The sounds one event plays: which file, how far after the event, and how loud. */
function soundsFor(event, random) {
  switch (event.kind) {
    case "click":
      return [["click", 0, 0.32]];
    case "key":
      return [["tick", 0, 0.28 + random() * 0.22]];
    case "send":
      return [["pop", 0, 0.4]];
    case "swipe":
      return [["swish", 0, 0.42]];
    case "zoom-in":
      return [
        ["whoosh", 0, Math.min(0.32, 0.1 + 0.12 * (event.detail.by - 1))],
      ];
    case "zoom-out":
      return [
        ["whoosh", 0, Math.min(0.18, 0.06 + 0.08 * (event.detail.by - 1))],
      ];
    case "message":
      return [["message", 0, 0.6]];
    case "pinch":
      return [["whoosh", 0, Math.min(0.22, 0.08 + 0.1 * Math.abs(Math.log2(event.detail.by)))]];
    case "pan":
    case "drag":
      return [["swish", 0, 0.22]];
    case "card":
      return [["pop", 0.05, 0.55]];
    case "shot":
      return [["pop", 0, 0.45]];
    case "allow":
      return [["chime", 0.02, 0.28]];
    case "island":
      if (event.detail.state === "P1-permission")
        return [
          ["pop", 0, 0.5],
          ["message", 0.08, 0.45],
        ];
      if (event.detail.state === "P2-finished") return [["chime", 0, 0.24]];
      return [];
    case "end-card":
      return [
        ["riser", MARK_LANDS - RISER_SECONDS, 0.5],
        ["impact", MARK_LANDS, 0.75],
      ];
    default:
      return [];
  }
}

async function seconds(file) {
  const { stdout } = await run("ffprobe", [
    "-v",
    "error",
    "-show_entries",
    "format=duration",
    "-of",
    "csv=p=0",
    file,
  ]);
  return Number(stdout.trim());
}

/** Integrated loudness and true peak of the finished mix, as ffmpeg's loudnorm measures them. */
async function measure(file) {
  const { stderr } = await run(
    "ffmpeg",
    [
      "-hide_banner",
      "-i",
      file,
      "-af",
      "loudnorm=I=-14:TP=-1:print_format=json",
      "-f",
      "null",
      "-",
    ],
    { maxBuffer: 1 << 24 },
  );
  const json = JSON.parse(stderr.match(/\{[^{}]*"input_i"[^{}]*\}/)[0]);
  return {
    lufs: Number(json.input_i),
    peak: Number(json.input_tp),
    range: Number(json.input_lra),
  };
}

/**
 * Lays one sound per event under `video` and writes `out`. With `music`, the
 * track sits about 6 LU under the sounds, ducks a little beneath them, fades
 * in, and is placed so that `musicHit` seconds into it, its last hit, lands as
 * the closing card's mark does; the clip holds its last frame for any tail.
 */
export async function mix({ video, events, out, music, musicHit }) {
  const random = seeded(11);
  const length = await seconds(video);
  const placed = events.flatMap((event) =>
    soundsFor(event, random).map(([sound, offset, volume]) => ({
      file: resolve(SOUNDS, `${sound}.wav`),
      at: Math.max(0, event.at + offset),
      volume,
    })),
  );
  const markAt =
    (events.find((event) => event.kind === "end-card")?.at ?? length) +
    MARK_LANDS;

  const inputs = ["-i", video, ...placed.flatMap(({ file }) => ["-i", file])];
  const graph = placed.map(
    ({ at, volume }, index) =>
      `[${index + 1}:a]aresample=48000,aformat=channel_layouts=stereo,adelay=${Math.round(at * 1000)}:all=1,volume=${volume.toFixed(3)}[s${index}]`,
  );
  graph.push(
    `${placed.map((_, index) => `[s${index}]`).join("")}amix=inputs=${placed.length}:normalize=0:dropout_transition=0,apad[sfx]`,
  );

  let total = length;
  let videoFilter = null;
  if (music) {
    const trackLength = await seconds(music);
    const hit = musicHit ?? trackLength;
    const lead = markAt - hit;
    total = Math.max(length, markAt + (trackLength - hit));
    if (total > length)
      videoFilter = `tpad=stop_mode=clone:stop_duration=${(total - length).toFixed(3)}`;
    const musicInput = placed.length + 1;
    inputs.push(...(lead < 0 ? ["-ss", (-lead).toFixed(3)] : []), "-i", music);
    graph.push(
      `[${musicInput}:a]aresample=48000,aformat=channel_layouts=stereo,${lead > 0 ? `adelay=${Math.round(lead * 1000)}:all=1,` : ""}loudnorm=I=-20:TP=-3,afade=t=in:st=0:d=0.5,apad[musicbed]`,
      `[sfx]asplit[sfxmix][sfxkey]`,
      `[musicbed][sfxkey]sidechaincompress=threshold=0.04:ratio=2.5:attack=15:release=350:makeup=1[ducked]`,
      `[sfxmix][ducked]amix=inputs=2:normalize=0,atrim=0:${total.toFixed(3)},loudnorm=I=-14:TP=-1.5:LRA=11,alimiter=limit=0.82:level=false[a]`,
    );
  } else {
    // Sounds alone are too sparse to measure as loudness, so they keep the levels set above, held under -1 dB.
    graph.push(
      `[sfx]atrim=0:${total.toFixed(3)},alimiter=limit=0.89:level=false[a]`,
    );
  }

  await run(
    "ffmpeg",
    [
      "-y",
      "-loglevel",
      "error",
      ...inputs,
      "-filter_complex",
      graph.join(";"),
      "-map",
      "0:v",
      "-map",
      "[a]",
      ...(videoFilter
        ? [
            "-vf",
            videoFilter,
            "-c:v",
            "libx264",
            "-preset",
            "slow",
            "-crf",
            "14",
            "-pix_fmt",
            "yuv420p",
          ]
        : ["-c:v", "copy"]),
      ...[
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-ar",
        "48000",
        "-ac",
        "2",
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        out,
      ],
    ],
    { maxBuffer: 1 << 24 },
  );
  const loudness = await measure(out);
  console.log(
    `  sound: ${placed.length} cues${music ? " and music" : ""}, ${loudness.lufs} LUFS, true peak ${loudness.peak} dBTP`,
  );
}

// `node film/mix.mjs <silent.mp4> <events.json> <out.mp4> [--music track] [--music-hit seconds]`
// lays the sounds the film's events call for, and music when given, under its picture.
if (import.meta.url === `file://${process.argv[1]}`) {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: { music: { type: "string" }, "music-hit": { type: "string" } },
  });
  const [video, timeline, out] = positionals;
  const { events } = JSON.parse(await readFile(timeline, "utf8"));
  await mix({
    video,
    events,
    music: values.music,
    musicHit: values["music-hit"] && Number(values["music-hit"]),
    out: `${out}.partial.mp4`,
  });
  await rename(`${out}.partial.mp4`, out);
  console.log(`film: wrote ${out}`);
}
