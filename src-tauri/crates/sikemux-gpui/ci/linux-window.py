"""The GPUI app on Linux in a real X window, on a throwaway core with fake agents only.

python3 -I ci/linux-window.py --app APP --core CORE --agent FAKE_AGENT --seed FAKE_CHAT --out DIR [--browser [--input]]

The arguments are desk-checks.py's, run under an X server and a window manager (DISPLAY set). L1 opens the app
and checks it shows a window with the seeded chat. With --browser, L2 has the fake agent open a desk browser
tab on a local page through the agent browser tool; with --input as well, L4 has it type into the page's field,
press a key there and read the field back, L5 has the page ask a prompt the person sees in a dialog and the agent
answers, and L3 click a link there, all with the same tools. Each leaves a capture of the window in DIR. Exits 1
when any check fails.
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


def typing_from_tool(run):
    typed = desk.answered(run.prompt(desk.json_tool("browser.type", {"selector": "input", "text": "hello linux"})))
    run.prompt(desk.json_tool("browser.press", {"key": "Backspace"}))
    read = desk.answered(run.prompt(desk.json_tool("browser.evaluate", {"script": "document.querySelector('input').value"})))
    run.report("L4", typed.get("value") == "hello linux" and read.get("result") == "hello linu",
               f"browser.type left {typed.get('value')!r}; after Backspace the field holds {read.get('result')!r}")


def dialog_from_tool(run):
    before = windows(run.app.pid)
    run.prompt(desk.json_tool("browser.evaluate", {"script": "setTimeout(() => { window.answer = prompt('Your name?', 'nobody') }, 500), 'asked'"}))
    shown = desk.until(lambda: [window for window in windows(run.app.pid) if window not in before], 10) or []
    run.capture("L5-shown")
    said = run.prompt(desk.json_tool("browser.dialog", {"accept": True, "text": "linux"}))
    gone = desk.until(lambda: not [window for window in windows(run.app.pid) if window not in before], 10)
    read = desk.answered(run.prompt(desk.json_tool("browser.evaluate", {"script": "window.answer"})))
    run.report("L5", bool(shown) and bool(gone) and read.get("result") == "linux",
               f"the person's dialog {shown or 'never showed'}, {'gone' if gone else 'still open'} after browser.dialog answered "
               f"{said[:120]!r}; the page's prompt returned {read.get('result')!r}")


def click_from_tool(run):
    said = run.prompt(desk.json_tool("browser.click", {"text": "Go to the second page"}))
    page = desk.until(lambda: (lambda page: page if page.get("url", "").endswith("/page2.html") else None)(desk.active_page(run.desk())), 20) or {}
    run.report("L3", bool(page), f"browser.click answered {said[:160]!r}; the desk's page after it {page.get('url')}")


def main():
    parser = argparse.ArgumentParser()
    for name in ("app", "core", "agent", "seed", "out"):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--browser", action="store_true")
    parser.add_argument("--input", action="store_true")
    options = parser.parse_args()
    options.only, options.window, options.behind = "", None, False
    run = desk.Run(options)
    try:
        run.set_up()
        launch(run)
        if options.browser:
            page_from_tool(run)
        else:
            run.leave("L2", "waits for the Linux browser (pass --browser once it draws a page)")
        if options.browser and options.input:
            typing_from_tool(run)
            dialog_from_tool(run)
            click_from_tool(run)
        else:
            run.leave("L4", "waits for input in the Linux browser (pass --browser --input once keys reach a page)")
            run.leave("L5", "waits for input in the Linux browser (pass --browser --input once dialogs reach the agent)")
            run.leave("L3", "waits for input in the Linux browser (pass --browser --input once clicks reach a page)")
    finally:
        run.tear_down()
    (run.out / "results.json").write_text(json.dumps(run.results, indent=1))
    sys.exit(1 if any(word == "FAIL" for _, word, _ in run.results) else 0)


if __name__ == "__main__":
    main()
