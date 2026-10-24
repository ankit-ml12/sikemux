"""Whether What's New's sky moves in a real window when What's New opens as the app starts.

python3 -I ci/sky-check.py --app APP --core CORE --agent FAKE_AGENT --seed FAKE_CHAT --out DIR [--behind]

The arguments are desk-checks.py's. Each case starts the app on a throwaway core with a release
the person has not read, so What's New opens before the person's preferences are read, with
the panes' grain on and off. It counts the frames that move the sky on over a few seconds and
compares two captures of the sky half a second apart: a sky that moves changes many of its
dots between them. Exits 1 when any case fails.
"""

import importlib.util
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name, file):
    spec = importlib.util.spec_from_file_location(name, HERE / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


desk = load("desk_checks", "desk-checks.py")
ground = load("ground_check", "ground-check.py")

CASES = [
    ("grain-on", {"paneShader": True, "paneStrength": 1.0}),
    ("grain-off", {"paneShader": False}),
]
SECONDS = 3
# The tick moves the sky at 30 frames a second on power, 20 on battery.
FEWEST_PRESENTS = 15
# Of the sky's pixels, how many must change between two captures half a second apart.
FEWEST_CHANGED = 0.02


def changed(before, after, frame, window):
    """The share of the pixels in `frame`, in the window's points, that differ between two captures."""
    width, _, channels, first = ground.pixels(before)
    _, _, _, second = ground.pixels(after)
    scale = width / window[0]
    left, top, frame_width, frame_height = frame
    xs = range(int(left * scale), int((left + frame_width) * scale))
    ys = range(int(top * scale), int((top + frame_height) * scale))
    differ = sum(first[y][x * channels:x * channels + 3] != second[y][x * channels:x * channels + 3] for y in ys for x in xs)
    return differ / max(1, len(xs) * len(ys))


def main():
    options = desk.argparse.ArgumentParser()
    for name in ("app", "core", "agent", "seed", "out"):
        options.add_argument(f"--{name}", required=True)
    options.add_argument("--behind", action="store_true")
    options = options.parse_args()
    out = Path(options.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    results = []
    for name, preferences in CASES:
        options.out = str(out / name)
        options.only = ""
        options.window = None
        run = desk.Run(options)
        run.preferences = {"lastSeenVersion": "0.0.1", **preferences}
        try:
            run.set_up()
            frame = desk.until(lambda: desk.spot(run, "whats-new-sky"), 15)
            if not frame:
                results.append((name, False, "What's New did not open with its sky"))
                continue
            first = (run.state() or {}).get("sky") or {}
            time.sleep(SECONDS)
            state = run.state() or {}
            last = state.get("sky") or {}
            rate = ((last.get("presents") or 0) - (first.get("presents") or 0)) / SECONDS
            number = ground.window_number(run.app.pid)
            shots = [out / f"{name}-{at}.png" for at in (1, 2)]
            for shot in shots:
                if number:
                    subprocess.run(["screencapture", "-x", "-o", "-l", number, shot])
                time.sleep(0.5)
            if not all(shot.exists() for shot in shots):
                results.append((name, False, f"no capture of the window; {rate:.0f} sky frames a second, skies {last.get('skies')}"))
                continue
            share = changed(*shots, frame, state.get("window", [1340, 881]))
            ok = rate >= FEWEST_PRESENTS and share >= FEWEST_CHANGED
            results.append((name, ok, f"skies {last.get('skies')}, window active {last.get('active')}, visible {last.get('visible')}, "
                                      f"reduce motion {last.get('reduceMotion')}; {rate:.0f} sky frames a second (at least {FEWEST_PRESENTS}); "
                                      f"{share:.1%} of the sky's pixels changed in half a second (at least {FEWEST_CHANGED:.0%})"))
        finally:
            run.tear_down()
    for name, ok, detail in results:
        print(f"{'PASS' if ok else 'FAIL'} {name}: {detail}", flush=True)
    (out / "results.json").write_text(json.dumps(results, indent=1))
    sys.exit(0 if all(ok for _, ok, _ in results) else 1)


if __name__ == "__main__":
    main()
