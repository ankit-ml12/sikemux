# Sikemux v0.5.3-nightly.4

The fourth nightly on the 0.5.3 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.3

- **Linear.** A new Linear plugin signs in with a personal API key and shows your issues in a pane. Agents get tools to read, comment on, move and file issues.
- **Sign in to agents from Sikemux.** A chat refused for sign-in offers its agent's own ways to sign in, the welcome screen offers Sign in for a signed-out agent, and the install sheet runs the agent's own sign-in after installing. Agents learn that Sikemux can run their sign-in in a terminal.
- **Set up agents anywhere.** Settings lists every agent with Install, ready ones first, the agent palette offers agents not on this computer as Install, and an empty rail offers to set one up.
- **Accounts.** Sign out of the current account or remove an added one right on its row in the account menu.
- **The rail.** Only the agent list scrolls, so the new rows and the scope track stay put. A project child's name never truncates for the marks beside it; the marks stack, and the rest are counted as +N. Buddies stack like agent marks.
- **The top bar** drops the zoom pill and the Jira issue chip, has one Rundeck deploy chip, and dictation is a mic that pulses while it listens.
- The Git pane carries the pane picture like the others, and folding a reviewed file keeps every row at its real height.
- A dragged board card keeps moving after it leaves its own column, and a page an agent shows no longer flickers.
- The line under a finished answer's work summary is 1px again.

Thanks to Ankit Patidar for the Linear plugin, signing in to agents from Sikemux and setting up agents from Settings, the rail and the palette.

For the complete patch history, compare [`v0.5.3-nightly.3...v0.5.3-nightly.4`](https://github.com/nodelike/sikemux/compare/v0.5.3-nightly.3...v0.5.3-nightly.4).
