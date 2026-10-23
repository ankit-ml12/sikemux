# Sikemux v0.5.4-nightly.6

The sixth nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.5

- **A new top bar.** The middle of the top bar says where the window is: the chosen host, then the project in front, each a picker. Tabs move to a row of their own under it, and the bar is thinner.
- **The composer strip.** Host, project, branch or worktree, and account sit in one strip docked on the composer, always shown, so the pane header keeps only the agent and its view. The account picker works on new chats too, and the model picker lists each agent once. One project picker serves the top bar and the strip, listing open projects and then folders found on disk.
- **Projects belong to their host.** Each host keeps the project last open there, a project's ⋯ opens it on another host, and opening a chat on another host switches to it.
- **SSH hosts.** A host without Sikemux opens as a shell with an offer to install it, and a stopped or older one is offered Start or Update. Install, Update and Sign in say first when this app isn't signed in.
- **Teams.** A lead and the agents it started fold together in the rail, and a teammate's message is headed like "Worked for", its name a link to that agent's chat. A teammate that fails to start leaves no worktree or branch behind.
- **Warm worktrees.** A new checkout starts from copy-on-write clones of the main checkout's build folders, Auto's card worktrees included, and builds across the machine share one pool of jobs.
- **New chats** show the tokens burnt in their project over a heat map, and until a chat has a title, its @name is the title.
- New agents are named after anime characters, unless you pick Nature in Settings › Agents › Agent names.
- A theme can be drawn from the pane image as well as the wallpaper.
- Text dictated or dropped into a terminal is pasted only when the program supports it, and typed on one line otherwise.
- Waking a sleeping agent keeps its place in the list, and turning buddies off shows every agent's logo again.
- The install and sign-in sheet starts its terminal again, and an install keeps its colours.

For the complete patch history, compare [`v0.5.4-nightly.5...v0.5.4-nightly.6`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.5...v0.5.4-nightly.6).
