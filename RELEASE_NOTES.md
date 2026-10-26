# Sikemux v0.6.0-nightly.6

The sixth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts.

## New since nightly.5

- **A stuck core can be restarted.** A window whose core stopped answering says so and offers Restart Sikemux instead of waiting forever, and a core that holds its lock but stopped answering can be found and stopped.
- **Agents stay out of your way.** An agent's preview or desk opens in its own tab without pulling you off the one you are on, and coming back to that tab shows the desk at once.
- **Branches read like commits.** Each branch row shows its tip's author, subject and age, Local and remote headers fold on a click, and the Actions area lists every branch's runs, newest first.
- **The browser tools no longer crash the app.** A tool script that throws or returns something other than text is handled, and browser_evaluate tells an expression from statements by parsing it.
- A core no longer spins in a busy loop ten minutes after it starts, and a linked host the account no longer has is forgotten instead of showing as out of reach.
- Text on 1x screens is drawn antialiased without macOS smoothing, so it is no longer heavy.
- File tabs show their file-type icon, Bitbucket and GitLab show their own marks, and a background task named by a long script shows only its first line.
- The desk's address bar opens a new page when none is left, the project tree's spine hangs from the folder icon's centre, and strip tabs' × sits closer to the edge.

For the complete patch history, compare [`v0.6.0-nightly.5...v0.6.0-nightly.6`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.5...v0.6.0-nightly.6).
