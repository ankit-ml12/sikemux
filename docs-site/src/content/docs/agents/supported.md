---
title: Supported agents
description: The coding agent CLIs Sikemux runs, and what each one can do here.
sidebar:
  order: 1
---

Sikemux runs the coding agent CLIs you already use. It finds them on your `PATH` and in the folders their own installers use.

| Agent | Command | YOLO mode | Several accounts |
|---|---|---|---|
| Claude Code | `claude` | Yes | Yes |
| Codex | `codex` | Yes | Yes |
| OpenCode | `opencode` | No | No |
| Hermes | `hermes` | Yes | No |
| Pi | `pi` | No | No |
| OMP | `omp` | Yes | No |
| Grok | `grok` | Yes | No |
| Muse | `muse` | Yes | No |

An agent that isn't installed shows **Not installed** on the welcome screen with an **Install** button, which runs the agent's own installer in a terminal you can watch. It installs where the agent's own updater expects, so the CLI works in your own terminal too.
