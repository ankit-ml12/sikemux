# Sikemux v0.5.4-nightly.7

The seventh nightly on the 0.5.4 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.6

- **New chats wait for you.** A new chat starts its agent only once you begin writing in it, and bringing an agent on screen no longer wakes it.
- **Lighter under load.** Pages swiped away in the rail, git and code host lists are no longer laid out, so moving between pages costs milliseconds. Reconnecting to a chat keeps every message that came back unchanged instead of redrawing the transcript. The view the phone, the notch and the app watch rebuilds only when something it shows changes, and workspace saves no longer stall calls while they write.
- **Agents stay available.** An agent whose version check timed out is checked again in the background instead of showing as broken, agent lists reload when a new sign-in is heard, and the core asks each agent's CLI again only when something changed.
- A notification left unclicked no longer costs the app anything.
- Clicking a lead's row opens it without folding its team; only the chevron folds.
- Messages from Sikemux itself, the board, Auto or hosts carry the Sikemux mark.
- A teammate of an agent already in a worktree gets its checkout in the main repository's `.worktrees` folder.

For the complete patch history, compare [`v0.5.4-nightly.6...v0.5.4-nightly.7`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.6...v0.5.4-nightly.7).
