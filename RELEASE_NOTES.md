# Sikemux v0.5.4-nightly.3

The third nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.2

- **Your other hosts, in the app.** The app reaches the other hosts on your account as a device of its own: their projects, chats and terminals show beside this host's. The file tree and the editor open, save and watch files on another host, and a file a chat on another host names opens there. A terminal tab always stays on the host it was opened on.
- Each project says what it's known by on every host, so the same project lines up across them.
- A terminal opened from another device gets this host's CLI.
- Agents start again for someone on the built-in account, and when a launcher can't be found the error says why.
- A save that arrives in pieces waits in the temporary folder rather than in the project.

For the complete patch history, compare [`v0.5.4-nightly.2...v0.5.4-nightly.3`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.2...v0.5.4-nightly.3).
