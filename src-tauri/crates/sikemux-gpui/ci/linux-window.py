"""The GPUI app on Linux in a real X window, on a throwaway core with fake agents only.

python3 -I ci/linux-window.py --app APP --core CORE --agent FAKE_AGENT --seed FAKE_CHAT --out DIR [--browser]

The arguments are desk-checks.py's, run under an X server and a window manager (DISPLAY set). L1 opens the app
and checks it shows a window with the seeded chat. With --browser, L2 has the fake agent open a desk browser
tab on a local page through the agent browser tool, and L3 has it click a link there with the same tools. Each
leaves a capture of the window in DIR. Exits 1 when any check fails.
"""

import argparse
import importlib.util
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("desk_checks", HERE / "desk-checks.py")
desk = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desk)


def windows(pid):
    shown = subprocess.run(["xdotool", "search", "--onlyvisible", "--pid", str(pid)], capture_output=True, text=True)
    return shown.stdout.split()


def launch(run):
    time.sleep(3)
    state = run.state() or {}
    pane = (state.get("spots") or {}).get(f"pane-Agent-{run.agent}")
    shown = windows(run.app.pid)
    run.report("L1", bool(shown) and bool(pane), f"X windows of the app: {shown or 'none'}; chat pane {pane or 'not drawn'}; window {state.get('window')}")


def page_from_tool(run):
    said = run.prompt(desk.json_tool("browser.navigate", {"url": f"{run.site}/page.html"}))
    page = desk.until(lambda: (lambda page: page if page.get("url", "").endswith("/page.html") and not page.get("loading") else None)(desk.active_page(run.desk())), 20) or {}
    run.report("L2", bool(page), f"browser.navigate answered {said[:160]!r}; the desk's page {page.get('url')} titled {page.get('title')!r}")


def click_from_tool(run):
    said = run.prompt(desk.json_tool("browser.click", {"text": "Go to the second page"}))
    page = desk.until(lambda: (lambda page: page if page.get("url", "").endswith("/page2.html") else None)(desk.active_page(run.desk())), 20) or {}
    run.report("L3", bool(page), f"browser.click answered {said[:160]!r}; the desk's page after it {page.get('url')}")


def main():
    parser = argparse.ArgumentParser()
    for name in ("app", "core", "agent", "seed", "out"):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--browser", action="store_true")
    options = parser.parse_args()
    options.only, options.window, options.behind = "", None, False
    run = desk.Run(options)
    try:
        run.set_up()
        launch(run)
        if options.browser:
            page_from_tool(run)
            click_from_tool(run)
        else:
            run.leave("L2, L3", "the browser checks wait for the Linux browser (pass --browser once it draws a page)")
    finally:
        run.tear_down()
    (run.out / "results.json").write_text(json.dumps(run.results, indent=1))
    sys.exit(1 if any(word == "FAIL" for _, word, _ in run.results) else 0)


if __name__ == "__main__":
    main()
