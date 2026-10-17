# Sikemux v0.5.3-nightly.6

The sixth nightly on the 0.5.3 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.5

- **Switching accounts takes your chats with it.** Picking another Claude or Codex account, from the rail's account menu or with Use in Settings, now moves open and sleeping chats to it too, and a turn in progress carries on. Before, it only changed the account new chats start on. The separate per-agent default in Settings is gone, since it did the same half switch.
- **Long Claude chats.** A reopened Claude chat loads back past its compactions, and faster, and an opened long turn scrolls in a box capped at 60% of the transcript.
- A new Claude chat is named as its first message is sent, not after the first turn ends.
- The notch's agent rows follow the rail: the time after the task, then the name, counts and state together at the end, and idle rows end at their project.

For the complete patch history, compare [`v0.5.3-nightly.5...v0.5.3-nightly.6`](https://github.com/nodelike/sikemux/compare/v0.5.3-nightly.5...v0.5.3-nightly.6).
