# Sikemux v0.4.0

Sikemux 0.4.0 is the release the nightly channel has been building toward since 0.3.5. The agent session is a first-class surface, the browser is a real one, the window is a stage you swipe through, and the whole app was measured and made to do less work.

## Agents

- An agent session has a structured view of its own: a turn's tool calls hang off a tree, an edit shows the hunk it wrote, and a code fence is coloured in the theme's own palette.
- A message written mid-turn joins the running turn on agents that take steering, instead of waiting for it or being refused.
- Background tasks and subagents reach the session view. A live task sits above the composer with its progress and a stop button, a subagent keeps a thread of its own, and a finished task says how it went.
- Harnesses, models and reasoning effort are picked in the composer, each harness wearing its own colour, and a slash part-way through a draft still names a command.
- A picture in a transcript opens and can be kept; a file the transcript names opens where files open.

## The browser

- A tab is a native child webview rather than a stream of screenshots, so a page scrolls, types and renders at the speed the page actually runs.
- The agent drives the same tabs you see, rather than a second browser of its own, and can read what a page asked the server for.
- Page dialogs appear in the pane, downloads land in the Downloads folder, a tab wears the site's own icon, and an agent's tabs are still there after a restart.

## The stage

- A session's screens sit side by side on one track, and a two-finger trackpad swipe moves between them: it follows the hand, lands from a short pull or a light flick, and gives at either end.
- Each screen is a card carrying its own frame, panes stack into a tab strip inside a window, and every pane is its own surface — which is what lets a see-through window read at all.

## Speed

- Commands that read disk or spawn a process no longer run on the main thread, and neither do pty commands or session listings. Terminal output crosses IPC as raw bytes rather than a JSON array of numbers, and WebGL draws the terminal by default.
- CodeMirror leaves the boot bundle and a language pack downloads when a file in it is opened. Git does one status walk per change instead of four. The chat transcript holds a handful of thumbnails rather than half a gigabyte.

## When it goes wrong

- A hang leaves evidence behind, a frozen window can say what the UI was doing, and `sikemux doctor` reads the last autopsy.

For the complete patch history, compare [`v0.3.5...v0.4.0`](https://github.com/nodelike/sikemux/compare/v0.3.5...v0.4.0).
