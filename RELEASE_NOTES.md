# Sikemux v0.5.4-nightly.5

The fifth nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.4

- **Anime agent names.** Agents can be named after anime characters, picked in Settings › Agents › Agent names.
- **The agent pane's header** holds the worktree switch and the account picker; model and effort stay in the composer, and the account menu is the rail's.
- **Hosts.** An SSH host that runs Sikemux shows as one row with its account host and keeps the alias you gave it. A host that has left the account says so, and a signed-out one offers Sign in. The app opens on your project, not on a host picked before it quit.
- **Hosts stay signed in.** A failed write, a stop mid-save or a change to the remote file no longer signs a host out, and a host with the systemd service runs its core only under it.
- **A crashed terminal agent comes back once.** A client asking for it after the core restarted it gets that terminal, instead of a second copy of the agent starting.
- A subagent that ended before a chat's opening view no longer shows as running.
- `sikemux status --json` names the host's public key as `coreId`.

For the complete patch history, compare [`v0.5.4-nightly.4...v0.5.4-nightly.5`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.4...v0.5.4-nightly.5).
