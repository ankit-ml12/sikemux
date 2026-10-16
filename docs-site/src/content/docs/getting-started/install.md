---
title: Install
description: Install Sikemux on a Mac or on x86_64 Linux.
sidebar:
  order: 1
---

## One line

```bash
curl -fsSL https://sikemux.com/install | sh
```

On a Mac this puts Sikemux in `/Applications`. On x86_64 Linux it installs the `.deb` or `.rpm`, or the AppImage in `~/Applications`.

For the nightly build, set the channel before `sh`:

```bash
curl -fsSL https://sikemux.com/install | SIKEMUX_CHANNEL=nightly sh
```

## Download

The latest `.dmg` is on [sikemux.com](https://sikemux.com) and on [GitHub Releases](https://github.com/nodelike/sikemux/releases/latest). It is about 16 MB and needs macOS 11 or later on Apple Silicon.

On x86_64 Linux the same release has a `.deb`, an `.rpm` and an `.AppImage`:

```bash
sudo apt install ./Sikemux_<version>_amd64.deb      # Debian, Ubuntu
sudo dnf install ./Sikemux-<version>-1.x86_64.rpm   # Fedora
chmod +x Sikemux_<version>_amd64.AppImage           # any other distribution
```

## Next

Open Sikemux and follow [First launch](/getting-started/first-launch/).
