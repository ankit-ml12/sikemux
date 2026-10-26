# Sikemux v0.6.0-nightly.5

The fifth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts.

## New since nightly.4

- **Chats open with their history.** A sleeping chat opened after a restart shows its last turns again, read from the history the core keeps, with older turns a page back, rather than opening empty.
- **Resuming is quicker and keeps its model.** The core remembers what it read from each saved session, so Recent, search and resuming read only what is new. A resumed Claude chat goes on with the model it last ran, and Codex chats whose history Codex compressed show in Recent and resume again.
- **Limits shows every account.** Rest on the account name for a card of each account with its rings, flip through them with ‹ ›, choose one for new chats, and add one from the +.
- **A plainer chat.** An agent's header is a plain bar with a GUI/TUI switch, the thinking line scrolls with the chat, and @agents, files, issues and pictures sit in the draft and in sent messages as inline chips.
- **Streaming reads cleanly.** A message still being written draws its bold, code and links as if closed, and code blocks keep their colours while they stream.
- **Lighter.** Chats share one memory budget and let go of what nobody is looking at, timers rest while the window is hidden, and pictures from the web are shrunk to the size they are drawn at.
- An agent that opens a page, task, file or simulator while its tab is behind opens its desk there without pulling you away, and page previews no longer open a tab on the desk.
- A core that dies right after a turn no longer sends that turn's prompt again, and a new chat waiting for you no longer holds up an update.
- ⌘⇧N lists open agents and saved chats together, pinned first, then newest.
- The Git pane's review sits on the ground without a frame, with more room in its rows and headers.
- Sign-in screens look as they did before, a file's tab is named by its file, pinned chats unpin from the rail, and a sent message that reads as a list stays in its column.

For the complete patch history, compare [`v0.6.0-nightly.4...v0.6.0-nightly.5`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.4...v0.6.0-nightly.5).
