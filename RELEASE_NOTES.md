# Sikemux v0.6.0-nightly.8

The eighth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts.

## New since nightly.7

- **Dither your own way.** The pane picture's dither has its own settings: colours from the theme, the picture or three inks of your own, the pattern and the dot size.
- **Saved chats always open.** An older chat picked from ⌘⇧N or the rail opens, renames and deletes even after the core's list of paged chats has forgotten it, and says why when it can't.
- **Overlays stay on top.** The palette, finder, settings, questions, sheets and peeking rails draw above a page on a desk or in a chat.
- **Commands as chips.** A / command or skill picked from the composer's menu goes in as a chip with a bolt mark, like a mention.
- **Steadier browser tools.** An agent's browser calls run one at a time in the order sent, browser_evaluate waits for a page still loading, and element numbers survive a re-render.
- The CI chip counts a commit's checks as GitHub does: the latest run of each workflow a push or pull request started.
- Claude chats and AI commit messages name Sikemux as their caller, so plans count them against the 5-hour limit like the terminal.
- An open chat takes the new theme's colours when the theme changes, and the agents rail no longer re-reads every preference for each chat on every frame.
- A new database connection picks its engine from PostgreSQL, MySQL and SQLite tiles.
- Agents' commits land more reliably: commits already on the branch are skipped, and a clash says to rebase.

For the complete patch history, compare [`v0.6.0-nightly.7...v0.6.0-nightly.8`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.7...v0.6.0-nightly.8).
