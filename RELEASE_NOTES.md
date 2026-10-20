# Sikemux v0.5.4-nightly.2

The second nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.1

- **Project settings.** A project's right-click menu opens Project settings in a tab of its own. Tasks you added can be edited in place or removed, Add a task opens a dialog, and the project's check command is set there as `check` in `sikemux.json`.
- **Agent board.** The agents' board is called Agent board, so it isn't mistaken for Jira's. It groups cards into lanes under their parent card, which shows how many of its cards are done. A card's Jira ticket moves by the workflow's own status categories. Auto's agent and reviewer are each one picker: agent, account, who pays and model. A dropped card lands where the pointer is.
- **GitLab.** Auto and who_wrote work on GitLab projects, and every code host says when a repository won't merge the way you asked.
- **Faster chats.** The core reads each answer's markdown once and sends the parsed result to the Mac and the phone, so neither parses every message again. The device view rebuilds only when something changes, and a killed terminal is cleaned up as soon as its shell exits.
- **Buddies.** An agent you named wears its buddy on every client, even with buddies off.
- **Open With.** Sikemux appears under Open With for folders and text files.
- **Phone.** Tables in answers fill the message's width with headings that fit, an answer you open mid-stream stays one answer, and the lock stays off until you turn on Lock Sikemux in the account sheet.
- **Signing in.** Signing in the Mac from the browser is one run of full-width cards ending on a signed-in page, and adding a host or deleting the account asks you to confirm it's you on Sikemux's own card.
- A permission card leaves when its request is answered anywhere, and answering one that's already gone just clears it.
- A spare shell starts at the terminal's real size, so what `.zshrc` prints isn't wrapped at 80 columns.
- The agent palette shows open agents' state, the way the rail does.
- `sikemux pair` and `login` on a server ask about linking hosts as well as devices, and the server install works on Ubuntu 26.04.

For the complete patch history, compare [`v0.5.4-nightly.1...v0.5.4-nightly.2`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.1...v0.5.4-nightly.2).
