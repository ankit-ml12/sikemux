---
title: Chats and terminals
description: "An agent's chat, its own terminal UI, and how to talk to it while it works."
sidebar:
  order: 2
---

Start an agent with `⌘N`, or from **Start an agent** on the welcome screen. Each agent opens in the chat Sikemux draws for it.

## GUI and TUI

The **GUI / TUI** switch at the top of an agent swaps between:

- **GUI**: Sikemux's chat, with the transcript, tool calls, questions and the composer.
- **TUI**: the agent's own terminal interface, running in a Sikemux terminal.

Switch while the agent is idle. A switch waits until the agent finishes its reply.

## The composer

- `@` attaches files from the project.
- `#` hands over a GitHub, GitLab or Bitbucket issue or pull request.
- Selections from terminals, problems, logs and pull request lines can be sent to any agent.

## While the agent works

| You press | What happens |
|---|---|
| `Enter` | Your message waits and is sent when the agent finishes its reply |
| `⌘Enter` | Sends it now, steering the agent in the middle of its reply |
| `⌘Enter` in an empty box | Sends the messages that were waiting, now |

## Its desk

Each agent has its own browser tabs on its desk. `⌘J` shows or hides the desk of the agent in front. See [Browser](/browser/desk-tabs/).
