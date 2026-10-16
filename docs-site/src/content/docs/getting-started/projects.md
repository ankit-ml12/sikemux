---
title: Open a project
description: Projects, and the sikemux.json that adds actions and tasks to one.
sidebar:
  order: 3
---

A project is a folder: its editor, terminals, Git and search all work in that directory. Open one with `⌘O`, or from a terminal with the `sikemux` CLI:

```bash
sikemux .
sikemux src/App.tsx:42:5
```

Install the CLI from **Settings → CLI**.

## Project config

Check a `sikemux.json` into a project to add actions, tasks and a preview to the command deck (`⌘⇧P`). Sikemux shows each command and asks before running it.

```json
{
  "version": 1,
  "actions": [
    {
      "id": "check",
      "label": "Run checks",
      "command": "pnpm check",
      "placement": "terminal",
      "contexts": ["project"]
    }
  ],
  "tasks": [{ "id": "dev", "label": "Dev server", "command": "pnpm dev" }],
  "preview": { "url": "http://localhost:5173", "command": "pnpm dev" }
}
```

Agents can start, read and stop these tasks too. Tasks keep running in Sikemux's background process while the window is closed.

## Quitting

Terminals, agents and tasks keep running when you quit, and come back where they were. **Quit and Stop Everything** (`⌥⌘Q`) ends them.
