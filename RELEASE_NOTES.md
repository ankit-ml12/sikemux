# Sikemux v0.5.4-nightly.6

The sixth nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.5

- **A new top bar.** The middle of the top bar says where the window is: the chosen host, then the project in front, each one a picker. Tabs move to a row of their own under it, edge to edge.
- **Projects belong to their host.** Each host keeps the project last open there, a project's ⋯ opens it on another host, and a server's project opens on this host too. Opening a chat on another host switches to that host.
- **SSH hosts without Sikemux** open as a shell, with an offer to install it.
- **Teams fold together.** A lead and the agents it started fold together in the rail like a file group, and hovering an agent shows a card instead of branch lines.
- New agents are named after anime characters, unless you pick Nature in Settings › Agents › Agent names.
- The install and sign-in sheet starts its terminal again.
- Settings stays at the foot of the rail while you're signed out.
- When an agent's git command can't hear back from Sikemux in time, it says so and asks you to retry.

For the complete patch history, compare [`v0.5.4-nightly.5...v0.5.4-nightly.6`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.5...v0.5.4-nightly.6).
