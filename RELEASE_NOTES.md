# Sikemux v0.6.0-nightly.9

The ninth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts. 0.2.0-nightly.9 adds Stash and shares a chat's draft with this app.

## New since nightly.8

- **Stash.** Share links, text, photos, files or a whole prompt into Sikemux from your phone's share menu, and they wait in the Stash: a button beside + in the composer counts what is new and opens a tray of preview cards, & finds an item as you type, and each goes into the draft as a chip.
- **Triggers.** Agents set triggers on their own chats: a prompt once, every so often, each day at a time, from a board, or from a webhook address that reaches the host wherever it is. A waiting agent shows a timer until its trigger fires, and a chat's triggers sit over its composer.
- **DBML diagrams.** A `.dbml` file opens as a diagram of its tables with crow's-foot links, and a ```dbml block in a chat reply draws as one too, panned and pinched without leaving the chat.
- **Databases grow up.** A table opens on its rows in a grid with Data and Structure tabs, the console's SQL is highlighted in the code editor, a server lists every database on it, a connection takes a URL first, and an agent can propose a database for you to approve.
- **The chat moves.** A finished turn's work folds into its line, rows glide in as they arrive, waiting cards come up and fold away, and agents react with emoji and send GIFs.
- **One draft everywhere.** A chat's unsent draft is the same on every device.
- **Find in markdown.** ⌘F finds in a markdown preview and in a reply, marking each place. Thanks to @ankit-ml12.
- Pasted photos too large for a chat are sent as a JPEG that fits, and a picture pasted into a sleeping chat is kept for it. Thanks to @ankit-ml12.
- Markdown previews read like GitHub's, the file tree lights the file an agent's desk shows, and closing the desk's last tab shuts the desk.
- The core logs why it stops, and a core started with another HOME can no longer overwrite the host's key and sign it out.
- Agents' tools are clearer: fewer wrong guesses, smaller answers, browser calls that survive re-renders, and an `events_wait` that watches files and processes.

For the complete patch history, compare [`v0.6.0-nightly.8...v0.6.0-nightly.9`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.8...v0.6.0-nightly.9).
