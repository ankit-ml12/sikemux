"""The desk's checks in a real window, on a throwaway core with fake agents only.

python3 -I ci/desk-checks.py --app APP --core CORE --agent FAKE_AGENT --seed FAKE_CHAT --out DIR [--only DK3,DF1] [--behind]

APP is the app built with the `drive` feature, CORE the sikemux-core binary, FAKE_AGENT
sikemux-fake-acp-agent and FAKE_CHAT the fake_chat example, which seeds the chats. Every check
acts through the app's own driver and reads the answer from app state: the driver's `state`,
the core's layout.json and the app's log. Each prints PASS, FAIL or LOCAL (needs a person's
eyes, the clipboard, a device or a click on a spot only a screenshot finds) and leaves a
capture of the window in DIR. Exits 1 when any check fails. --behind opens the window without
taking the screen or the keys from whoever is at the machine, and leaves out the captures.
"""

import argparse
import http.server
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
BUNDLE_ID = "com.nodelike.sikemux.gpui.test"
TOKENS = ("GH_TOKEN", "GITHUB_TOKEN", "GH_ENTERPRISE_TOKEN", "GITHUB_ENTERPRISE_TOKEN")

PAGE_ONE = """<!doctype html><title>Kit page one</title><h1>Kit page one</h1>
<p><a href="page2.html">Go to the second page</a></p>
<p><a href="file.zip" download>Download file.zip</a></p><input placeholder="type here">"""
PAGE_TWO = """<!doctype html><title>Kit page two</title><h1>Kit page two</h1>
<p><a href="page.html">Back to the first page</a></p>"""
SHOWN = "<!doctype html><title>Kit shown page</title><h1>Kit shown page</h1><p>From page_show.</p>"
TASKS = {"version": 1, "tasks": [{"id": "count", "label": "Count", "command": "i=0; while [ $i -lt 900 ]; do i=$((i+1)); echo line $i; sleep 0.2; done"}]}


class Run:
    def __init__(self, options):
        self.options = options
        self.out = Path(options.out).resolve()
        self.root = self.out / "run"
        self.results = []
        self.app = None
        self.processes = []
        self.preferences = {}
        self.desk_body = None

    # The world the app runs in.

    def set_up(self):
        if self.root.exists():
            shutil.rmtree(self.root)
        for folder in ("home", "data", "project", "site", "bin"):
            (self.root / folder).mkdir(parents=True)
        project = self.root / "project"
        (project / "a.txt").write_text("".join(f"line {n}\n" for n in range(1, 11)))
        (project / "sikemux.json").write_text(json.dumps(TASKS))
        git = ["git", "-C", str(project), "-c", "user.name=test", "-c", "user.email=test@invalid"]
        subprocess.run([*git, "init", "-q"], check=True)
        subprocess.run([*git, "add", "-A"], check=True)
        subprocess.run([*git, "commit", "-qm", "seed"], check=True)
        site = self.root / "site"
        (site / "page.html").write_text(PAGE_ONE)
        (site / "page2.html").write_text(PAGE_TWO)
        (site / "shown.html").write_text(SHOWN)
        (site / "file.zip").write_bytes(os.urandom(2048))
        self.port = serve(site)
        self.site = f"http://127.0.0.1:{self.port}"

        # An unsigned core inside an app bundle trusts that bundle as its app, as a release's does,
        # and a dev core trusts examples/remote beside it, which seeds the chats.
        bundle = self.root / "Sikemux GPUI Test.app"
        (bundle / "Contents/MacOS").mkdir(parents=True)
        (bundle / "Contents/Info.plist").write_text(plist())
        self.app_exe = bundle / "Contents/MacOS/sikemux"
        shutil.copy(self.options.app, self.app_exe)
        helpers = bundle / "Contents/Helpers/Sikemux Core.app/Contents/MacOS"
        (helpers / "examples").mkdir(parents=True)
        self.core_exe = helpers / "sikemux-host"
        shutil.copy(self.options.core, self.core_exe)
        self.seed_exe = helpers / "examples/remote"
        shutil.copy(self.options.seed, self.seed_exe)
        self.agent_exe = self.root / "bin/sikemux-fake-acp-agent"
        shutil.copy(self.options.agent, self.agent_exe)

        version = json.loads(subprocess.run([self.core_exe, "--upgrade-info"], capture_output=True, text=True).stdout)["version"]
        (self.root / "data/preferences.json").write_text(json.dumps({
            "notificationsIntroduced": True, "keptRunningNoticeShown": True, "voiceDictation": False,
            "onboardingComplete": True, "lastSeenVersion": version, **self.preferences,
        }))
        home = self.root / "home"
        (home / "Downloads").mkdir()
        self.env = {key: value for key, value in os.environ.items() if key not in TOKENS}
        self.env.update(HOME=str(home), CFFIXED_USER_HOME=str(home), SIKEMUX_CONFIG_DIR=str(home / ".config/sikemux-gpui"),
                        SIKEMUX_APP_EXECUTABLE="/usr/bin/true")
        self.socket = self.root / "core.sock"
        self.drive_socket = self.root / "drive.sock"
        core = subprocess.Popen([self.core_exe, "--socket", self.socket, "--data-dir", self.root / "data"],
                                env=self.env, stdout=open(self.root / "core.log", "w"), stderr=subprocess.STDOUT)
        self.processes.append(core)
        until(lambda: self.socket.exists(), 15) or sys.exit("the core did not start; see run/core.log")
        self.agents = []
        for said in ("hello", "again"):
            seeded = subprocess.run([self.seed_exe, self.socket, self.agent_exe, said], env={**self.env, "FAKE_PROJECT": str(project)},
                                    capture_output=True, text=True, timeout=60)
            (self.root / "seed.log").open("a").write(seeded.stdout + seeded.stderr)
            self.agents += [line.split()[1] for line in seeded.stdout.splitlines() if line.startswith("chat ")]
        self.front = self.agents[-1] if self.agents else None
        self.launch()

    def launch(self):
        log = open(self.root / "app.log", "a")
        env = {**self.env, "SIKEMUX_GPUI_DRIVE": str(self.drive_socket), "SIKEMUX_CORE_SOCKET": str(self.socket)}
        if self.options.behind:
            env["SIKEMUX_GPUI_DRIVE_BEHIND"] = "1"
        self.app = subprocess.Popen([self.app_exe], env=env, stdout=log, stderr=subprocess.STDOUT)
        until(lambda: self.drive_socket.exists() and self.state(), 30) or sys.exit("the app's driver did not answer; see run/app.log")
        # The app must be the last client to say it is the window, or the seeding's browser calls go to a client that left.
        self.drive("register" if self.options.behind else "activate", "wait 1.5")

    def tear_down(self):
        for process in [self.app, *self.processes]:
            if process and process.poll() is None:
                process.terminate()
                try:
                    process.wait(5)
                except subprocess.TimeoutExpired:
                    process.kill()

    # Talking to the app.

    def drive(self, *steps):
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
                sock.settimeout(90)
                sock.connect(str(self.drive_socket))
                sock.sendall(("\n".join(steps) + "\nend\n").encode())
                said = b""
                while not said.endswith(b"done\n"):
                    chunk = sock.recv(65536)
                    if not chunk:
                        break
                    said += chunk
        except OSError as error:
            return [f"driver: {error}"]
        return said.decode(errors="replace").splitlines()

    def state(self):
        for line in self.drive("state"):
            if line.startswith("did: state "):
                return json.loads(line[len("did: state "):])
        return None

    @property
    def agent(self):
        """The chat in front, which the composer's prompts go to, else the last one that was."""
        active = self.strip().get("active") or ""
        if active.startswith("agent:"):
            self.front = active.removeprefix("agent:")
        return self.front

    def desk(self, agent=None):
        desk = ((self.state() or {}).get("desks") or {}).get(agent or self.agent) or {}
        if desk.get("pageBounds"):
            self.desk_body = desk["pageBounds"]
        return desk

    def layout(self):
        try:
            return json.loads((self.root / "data/layout.json").read_text())
        except (OSError, ValueError):
            return {}

    def strip(self):
        projects = self.layout().get("projects") or {}
        return next((strip for strip in projects.values() if strip.get("tabs")), {})

    def log(self):
        return (self.root / "app.log").read_text(errors="replace")

    def answers(self):
        """What the fake agent said each tool answered, oldest first, by chat."""
        found = {}
        for path in sorted((self.root / "data/chat-history").glob("*.jsonl")):
            for line in path.read_text(errors="replace").splitlines():
                try:
                    said = [text for text in strings(json.loads(line)) if text.startswith(("tool answered: ", "tool failed: "))]
                except ValueError:
                    continue
                found.setdefault(path.name, []).extend(said)
        return found

    def composer(self):
        width, height = (self.state() or {}).get("window", [1340, 881])
        return width * 0.32, height - 120

    def prompt(self, text, seconds=20):
        """Types `text` into the front chat's composer and sends it; answers the tool's reply."""
        before = {chat: len(said) for chat, said in self.answers().items()}
        x, y = self.composer()
        self.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", f"text {text}", "wait 0.2", "key enter")

        def new():
            for chat, said in self.answers().items():
                if len(said) > before.get(chat, 0):
                    return said[before.get(chat, 0)]
            return None

        return until(new, seconds) or ""

    def go(self, url):
        """Types `url` into the desk's address and presses Enter, which hands the page the keys."""
        self.drive("key cmd+l", "wait 0.6", f"text {url}", "wait 0.3", "key enter")
        return bool(until(lambda: self.desk().get("pageFocused"), 8))

    def focus_page(self):
        return self.go(active_page(self.desk()).get("url", ""))

    def desks_per_agent(self):
        counts = {}
        def walk(pane):
            if pane.get("kind") == "desk":
                counts[pane.get("target")] = counts.get(pane.get("target"), 0) + 1
            for inner in pane.get("panes") or []:
                walk(inner)
        for strip in (self.layout().get("projects") or {}).values():
            for tab in strip.get("tabs") or []:
                walk(tab.get("pane") or {})
        return counts

    def capture(self, name):
        if self.options.behind:
            return
        number = subprocess.run(["swift", HERE / "window-number.swift", str(self.app.pid)], capture_output=True, text=True).stdout.strip()
        if number:
            subprocess.run(["screencapture", "-x", "-o", "-l", number, self.out / f"{name}.png"])

    # Reporting.

    def report(self, name, ok, detail):
        self.say(name, "PASS" if ok else "FAIL", detail)
        self.capture(name)

    def leave(self, name, why):
        self.say(name, "LOCAL", why)

    def say(self, name, word, detail):
        self.results.append((name, word, detail))
        print(f"{word} {name}: {detail}", flush=True)


def until(check, seconds=8):
    end = time.time() + seconds
    while time.time() < end:
        found = check()
        if found:
            return found
        time.sleep(0.25)
    return None


def serve(folder):
    handler = lambda *a: http.server.SimpleHTTPRequestHandler(*a, directory=str(folder))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
    server.RequestHandlerClass.log_message = lambda *a: None
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server.server_address[1]


def plist():
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
<key>CFBundleExecutable</key><string>sikemux</string>
<key>CFBundleName</key><string>Sikemux GPUI Test</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>"""


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def pages(desk):
    return desk.get("pages") or []


def active_page(desk):
    return next((page for page in pages(desk) if page.get("active")), {})


# The checks, in the order they run: each leaves the window as the next expects.

def dp5(run):
    every = answered(run.prompt('tool app.console {}'))
    errors = answered(run.prompt('tool app.console {"errors":true}'))
    texts = [message.get("text", "") for message in errors.get("messages", [])]
    only = all(text.startswith("sikemux") or "panicked" in text for text in texts)
    run.report("DP5", bool(every.get("messages")) and only and errors.get("matched", 0) < errors.get("recorded", 0),
               f"all: {every.get('matched')} of {every.get('recorded')} lines; errors only: {errors.get('matched')} of {errors.get('recorded')}, all sikemux lines or panics {only}")


def dp3(run):
    said = run.prompt(json_tool("page.show", {"path": str(run.root / "site/shown.html"), "title": "Kit shown"}))
    on_desk = any(page.get("title") == "Kit shown page" for page in pages(run.desk()))
    run.report("DP3", said.startswith("tool answered") and not on_desk, f"answer {said[:160]!r}; on the desk too: {on_desk}")


def d5(run):
    run.prompt(json_tool("browser.navigate", {"url": f"{run.site}/page.html"}))
    first = until(lambda: (lambda page: page if page.get("url", "").endswith("/page.html") and not page.get("loading") else None)(active_page(run.desk())), 15) or {}
    fresh = run.desk().get("open") and not first.get("back") and not first.get("forward")
    focused = run.go(f"{run.site}/page2.html")
    second = until(lambda: (lambda page: page if page.get("url", "").endswith("/page2.html") and page.get("back") else None)(active_page(run.desk())), 15) or {}
    run.drive("key cmd+[", "wait 1.5")
    back = active_page(run.desk())
    run.drive("key cmd+]", "wait 1.5")
    forward = active_page(run.desk())
    run.report("D5", bool(fresh and second and back.get("url", "").endswith("/page.html") and back.get("forward") and forward.get("url", "").endswith("/page2.html")),
               f"first {first.get('url')} back/forward {first.get('back')}/{first.get('forward')}; then {second.get('url')}; page focused {focused}; ⌘[ → {back.get('url')}; ⌘] → {forward.get('url')}")


def dk3(run):
    run.focus_page()
    run.drive("key cmd+r", "wait 0.3")
    reloading = active_page(run.desk())
    url = reloading.get("url")
    x, y = run.composer()
    run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key cmd+[", "wait 1.5")
    still = active_page(run.desk()).get("url")
    run.report("DK3", url == still and url and url.endswith("/page2.html"),
               f"⌘R in the page kept {url} (loading {reloading.get('loading')}); ⌘[ in the chat left {still}")


def d6_dk4_dk5(run):
    before = len(pages(run.desk()))
    run.drive("key cmd+shift+t", "wait 1.5")
    opened = run.desk()
    run.report("D6", len(pages(opened)) == before + 1, f"pages {before} → {len(pages(opened))} after ⌘⇧T, active {active_page(opened).get('url')!r}")
    run.drive("key cmd+l", "wait 0.5")
    at = active_page(run.desk()).get("url")
    run.drive("key ctrl+tab", "wait 0.8")
    stepped = active_page(run.desk()).get("url")
    run.drive("key ctrl+shift+tab", "wait 0.8")
    back = active_page(run.desk()).get("url")
    tab = run.strip().get("active")
    x, y = run.composer()
    run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key ctrl+tab", "wait 1")
    strip_stepped = run.strip().get("active")
    run.drive("key ctrl+shift+tab", "wait 1")
    run.report("DK5", stepped != at and back == at and strip_stepped != tab,
               f"in the address ⌃Tab {at!r} → {stepped!r}, ⌃⇧Tab → {back!r}; in the chat ⌃Tab moved the strip {tab} → {strip_stepped}")
    run.drive("key cmd+l", "wait 0.5", "key cmd+w", "wait 1")
    closed = run.desk()
    run.report("DK4", closed.get("open") and len(pages(closed)) == before,
               f"⌘W in the address closed its page: pages {before + 1} → {len(pages(closed))}, desk open {closed.get('open')}")


def dk6(run):
    before = run.strip().get("active")
    focused = run.focus_page()
    run.drive("key cmd+shift+[", "wait 1")
    after = run.strip().get("active")
    run.drive("key cmd+shift+]", "wait 1")
    run.report("DK6", focused and after != before, f"page focused {focused}; ⌘⇧[ moved the strip {before} → {after}")


def d7(run):
    x, y = run.composer()
    if not run.desk().get("open"):
        run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key cmd+j", "wait 1.5")
    urls = [page.get("url") for page in pages(run.desk())]
    run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key cmd+j", "wait 1")
    closed = run.desk().get("open")
    run.drive("key cmd+j", "wait 1.5")
    again = run.desk()
    run.report("D7", closed is False and again.get("open") and [page.get("url") for page in pages(again)] == urls,
               f"⌘J closed it: {closed is False}; ⌘J again: open {again.get('open')}, pages kept {urls == [page.get('url') for page in pages(again)]}")


def dk8(run):
    x, y = run.composer()
    if run.desk().get("open"):
        run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key cmd+j", "wait 1")
    closed = run.desk().get("open") is False
    run.prompt(json_tool("browser.navigate", {"url": f"{run.site}/page.html"}))
    opened = until(lambda: run.desk().get("open"), 10)
    run.report("DK8", closed and bool(opened), f"desk closed first {closed}; the agent's navigate brought it out {bool(opened)}")


def dk2(run):
    x, y = run.composer()
    run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key cmd+j", "wait 1")
    closed = run.desk().get("open") is False
    held = len(pages(run.desk()))
    run.drive("key cmd+l")
    desk = until(lambda: (lambda desk: desk if desk.get("addressFocused") else None)(run.desk()), 5) or run.desk()
    page = active_page(desk)
    run.report("DK2", closed and desk.get("open") and desk.get("addressFocused") and (held or page.get("url") == "about:blank"),
               f"closed first {closed}; ⌘L: open {desk.get('open')}, address focused {desk.get('addressFocused')}, on {page.get('url')!r} with {held} pages held")


def one_desk(run):
    """Every way a desk opens, all at once with it closed, still leaves one desk."""
    x, y = run.composer()
    run.drive(f"click {x:.0f} {y:.0f}", "wait 0.3", "key cmd+j", "wait 1")
    run.drive("key cmd+l", "key cmd+shift+t", "key cmd+j")
    run.prompt(json_tool("ui.open", {"kind": "file", "path": str(run.root / "project/a.txt")}))
    run.prompt(json_tool("browser.navigate", {"url": f"{run.site}/page2.html"}))
    time.sleep(2)
    counts = run.desks_per_agent()
    run.report("ONE-DESK", counts.get(run.agent) == 1 and all(count == 1 for count in counts.values()), f"desks per agent {counts}")


def dk7(run):
    run.prompt(json_tool("browser.navigate", {"url": f"{run.site}/page.html"}))
    until(lambda: active_page(run.desk()).get("url", "").endswith("/page.html") and not active_page(run.desk()).get("loading"), 15)
    said = run.prompt(json_tool("browser.click", {"text": "Download file.zip"}))
    finished = until(lambda: "download Finished" in run.log() or "download finished" in run.log(), 20)
    started = "download Started" in run.log() or "download started" in run.log()
    run.report("DK7", started and bool(finished), f"click answer {said[:120]!r}; started {started}, finished {bool(finished)} (Keep in Downloads is checked locally)")


def df1(run):
    path = str(run.root / "project/a.txt")
    said = run.prompt(json_tool("ui.open", {"kind": "file", "path": path, "line": 3}))
    desk = until(lambda: (lambda desk: desk if (desk.get("front") or {}).get("file") == path else None)(run.desk()), 10) or {}
    line = next((file.get("line") for file in (run.state() or {}).get("files", []) if file.get("path") == path), None)
    run.report("DF1", bool(desk.get("open")) and line == 3, f"answer {said[:120]!r}; front {desk.get('front')}, cursor line {line}")


def df3(run):
    path = str(run.root / "project/a.txt")
    if run.desk_body:
        left, top, width, height = run.desk_body
        run.drive(f"click {left + width / 2:.0f} {top + height / 2:.0f}", "wait 0.5")
    contexts = (run.state() or {}).get("contexts", [])
    tabs = len(run.strip().get("tabs", []))
    run.drive("key cmd+w", "wait 1")
    desk = run.desk()
    gone = all(item.get("file") != path for item in desk.get("items") or [])
    run.report("DF3", gone and desk.get("open") and len(run.strip().get("tabs", [])) == tabs,
               f"keys in {contexts[-2:]}; ⌘W took the file off {gone}, desk open {desk.get('open')}, tabs {tabs} → {len(run.strip().get('tabs', []))}")


def df4(run):
    command = "i=0; while [ $i -lt 900 ]; do i=$((i+1)); echo line $i; sleep 0.2; done"
    said = run.prompt(json_tool("task.start", {"command": command, "label": "Count", "idempotencyKey": "desk-checks-df4"}))
    desk = until(lambda: (lambda desk: desk if (desk.get("front") or {}).get("terminal") else None)(run.desk()), 15) or {}
    run.execution = answered(said).get("executionId")
    run.report("DF4", desk.get("open") and (desk.get("front") or {}).get("terminal") == "Count",
               f"answer {said[:160]!r}; front {desk.get('front')}")


def df5(run):
    run.drive("key cmd+q")
    try:
        run.app.wait(15)
    except subprocess.TimeoutExpired:
        run.app.terminate()
        run.app.wait(5)
    run.launch()
    desk = until(lambda: (lambda desk: desk if any(item.get("terminal") for item in desk.get("items") or []) else None)(run.desk()), 20) or {}
    behind = not (desk.get("front") or {}).get("terminal")
    said = run.prompt(json_tool("ui.open", {"kind": "terminal", "executionId": run.execution})) if getattr(run, "execution", None) else "no executionId from DF4"
    front = until(lambda: (run.desk().get("front") or {}).get("terminal"), 10)
    run.report("DF5", bool(desk) and behind and bool(front),
               f"after the restart the terminal is on the desk {bool(desk)}, behind {behind}; ui.open brought it in front {bool(front)} ({said[:100]!r})")


def dk1(run):
    run.drive("key cmd+t", "wait 2")
    width, height = (run.state() or {}).get("window", [1340, 881])
    # A new terminal tab takes the keys only once it is clicked, as a person does to type in it.
    run.drive(f"click {width * 0.55:.0f} {height * 0.5:.0f}", "wait 0.5")
    terminal = run.strip().get("active")
    run.drive("key cmd+shift+t", "wait 2")
    after = run.strip().get("active")
    desk = run.desk()
    run.report("DK1", after != terminal and desk.get("open") and active_page(desk).get("url") is not None,
               f"terminal tab {terminal}; ⌘⇧T brought {after} forward, desk open {desk.get('open')}, pages {len(pages(desk))}")


def local(run):
    for name, why in [
        ("DP1", "page_preview needs a terminal agent, which the fake agent is not"),
        ("DP2", "page_show from a terminal agent, as DP1"),
        ("DP4", "a theme switch goes through Settings' pickers"),
        ("D8", "the driver cannot resize the window"),
        ("D9", "the More menu's spot is found from a screenshot"),
        ("DF2", "the desk's tab pills are clicked at spots only a screenshot finds"),
        ("S1", "needs a booted simulator and a person to judge the stream"),
        ("5.5", "⌘C and ⌘V use the clipboard"),
        ("5.6", "resizing needs a person's eyes"),
        ("DK7 Keep", "the notice's button is clicked at a spot only a screenshot finds"),
    ]:
        run.leave(name, why)


def json_tool(method, params):
    return f"tool {method} {json.dumps(params)}"


def answered(said):
    """The JSON a tool answered with, or an empty one when it failed or said none."""
    try:
        value, _ = json.JSONDecoder().raw_decode(said.removeprefix("tool answered: "))
        return value if isinstance(value, dict) else {}
    except ValueError:
        return {}


CHECKS = [("DP5", dp5), ("DP3", dp3), ("D5", d5), ("DK3", dk3), ("D6", d6_dk4_dk5), ("DK6", dk6), ("D7", d7), ("DK8", dk8),
          ("DK2", dk2), ("ONE-DESK", one_desk), ("DK7", dk7), ("DF1", df1), ("DF3", df3), ("DF4", df4), ("DF5", df5), ("DK1", dk1)]


def main():
    parser = argparse.ArgumentParser()
    for name in ("app", "core", "agent", "seed", "out"):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--only", default="")
    parser.add_argument("--behind", action="store_true")
    options = parser.parse_args()
    only = {name.strip() for name in options.only.split(",") if name.strip()}
    run = Run(options)
    signal.signal(signal.SIGTERM, lambda *_: (run.tear_down(), sys.exit(1)))
    try:
        run.set_up()
        print(f"agents {run.agents}; site {run.site}", flush=True)
        run.capture("start")
        for name, check in CHECKS:
            if only and name not in only:
                continue
            try:
                check(run)
            except Exception as error:
                run.report(name, False, f"the check broke: {error!r}")
            doubled = {agent: count for agent, count in run.desks_per_agent().items() if count > 1}
            if doubled:
                run.report(f"{name} one desk", False, f"agents with more than one desk after {name}: {doubled}")
        if not only:
            local(run)
    finally:
        run.tear_down()
    (run.out / "results.json").write_text(json.dumps(run.results, indent=1))
    sys.exit(1 if any(word == "FAIL" for _, word, _ in run.results) else 0)


if __name__ == "__main__":
    main()
