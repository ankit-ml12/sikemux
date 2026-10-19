# Sikemux v0.5.4-nightly.1

The first nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since 0.5.3

- **The core runs everything.** The background core now keeps the workspace: projects, spaces, pinned chats and every agent with how it runs, the same on the Mac, your phone and the web. Git, tasks, ports, CI checks, notifications, the activity record and AI commit messages all run in the core, so they work with no window open and every device sees the same state. Clients hear when something changes instead of asking again and again.
- **Terminal agents without the window.** The core starts terminal agents, starts a crashed one again on its saved session below its last screen, and runs an agent's terminal workers with no window needed. Terminals are named by the title their program sets.
- **Idle chats sleep.** A chat nobody is looking at goes to sleep and wakes when it comes back on screen, and a resumed chat shows how full its context was on every device.
- **Teams.** A teammate can work in a checkout of its own and land its commits on its lead's branch, `agent_send` to "team" reaches every teammate at once, and a ticket goes on the board in one call with its subtasks and blockers. The project's check runs on each commit as committed, and you hear only once it stays broken.
- **Names stick.** An agent you renamed keeps its name and buddy after it closes, and @ opens it again under them. Every agent mark shows its buddy while buddies are on, in the app and the notch.
- **Security.** Secrets move to the data protection keychain, which only Sikemux's own signed code can reach, and a Linux host keeps its secrets where only its own core can. Hosts sign their own requests, adding a host takes a code you type after seeing where it asked from, and agents a linked host started use none of this host's sign-ins until you allow that host. Agents can no longer sign the host in to another account, start a plugin's browser sign-in, or run a production Rundeck job without your say. A desk tab's download waits for you to keep it.
- **Phone.** A signed-in phone asks for Face ID, Touch ID or the passcode before it shows anything, and again after a minute away.
- Pinch to zoom and pan in image tabs.
- The Run button and the task picker share one pill, and agents in All projects open their menu on right click and rename on double click.
- A chat that ran in its worktree lists under its project in Recent and resumes back in the worktree.
- `git config sikemux.attribution false` keeps the agent footer and notes out of a repository's commits.

For the complete patch history, compare [`v0.5.3...v0.5.4-nightly.1`](https://github.com/nodelike/sikemux/compare/v0.5.3...v0.5.4-nightly.1).
