---
title: Pi
description: "The Pi coding agent in Sikemux: install, sign-in and running it in a terminal."
sidebar:
  order: 5
---

Pi is a minimal terminal coding agent.

## At a glance

| | |
|---|---|
| Command | `pi` |
| Chat in Sikemux | No, it runs in its own terminal |
| Permission modes | Normal only |
| Effort levels | off, minimal, low, medium, high, xhigh, max |
| Several accounts | No |

## Install

```bash
curl -fsSL https://pi.dev/install.sh | sh
```

Pi needs Node 22.19 or later, and its installer can install Node for you.

## Sign in

```bash
pi --terminal-login
```

Sikemux checks Pi's sign-in with `pi auth check`: ready, not signed in, or credentials that no longer work.

## How Sikemux runs it

Pi has no chat view in Sikemux. It runs in its own terminal, where you use it as you would anywhere else. If Pi crashes, Sikemux starts it again on the same session, below its old screen.
