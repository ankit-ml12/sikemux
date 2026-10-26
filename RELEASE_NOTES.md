# Sikemux v0.6.0-nightly.6

The sixth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts. 0.2.0-nightly.6 opens chats with long folded turns again.

## New since nightly.5

- **Lighter in the background.** A chat, Git tab, simulator or plugin pane out of front stops following the core, so agents streaming in other tabs no longer cost the one you're on. Terminals resize at most every 50ms, the file tree and Git review stop re-reading on every repository change, and the code host is asked at most every 5 seconds.
- **A stuck core can be restarted.** A window whose core stopped answering offers Restart Sikemux instead of waiting forever, and a core that holds its lock but stopped answering can be found and stopped. A core no longer spins in a busy loop ten minutes after it starts.
- **Agents stay out of your way.** An agent's preview or desk opens in its own tab without pulling you off the one you are on.
- **Chats start sooner.** A message sent while the agent starts shows at once, a new chat's effort picker works before the first message, and a Claude chat's pickers name the model it runs instead of Default.
- **Pasted pictures reach agents.** A picture pasted into a terminal is kept and its path typed, and a TIFF is kept as a PNG. Thanks to @ankit-ml12 (#171).
- **Branches read like commits.** Branch rows show the tip's author, subject and age, Fetch, Pull and Push are disabled with the reason while git runs, and Actions lists every branch's runs.
- **Lists show what matched.** Lists that narrow as you type mark the matched letters, and a double click in a chat selects a word, a triple click its paragraph.
- Fixed crashes from a ⌘ shortcut pressed while a desk page has the keys, and from browser tool scripts that throw.
- Diffs, search matches and markdown headings follow the theme, dialogs appear at once, and the context meter's tooltip is a card with tokens and session cost.
- GPUI Kit 0.8.1, and the file editor rubber-bands at its ends again; ⌘⇧[ and ⌘⇧] move along the tab strip.
- The leftovers of the old React UI and Tauri shell are gone.

For the complete patch history, compare [`v0.6.0-nightly.5...v0.6.0-nightly.6`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.5...v0.6.0-nightly.6).
