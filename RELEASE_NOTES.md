# Sikemux v0.6.0-nightly.10

The tenth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts. 0.2.0-nightly.11 brings the phone's chat up to the Mac's: steering, edits, find, questions and mentions.

## New since nightly.9

- **Chats pick up exactly where they left off.** A chat's changes are numbered by its log, so an app coming back is sent exactly what it missed, and a delivery that never arrived is noticed and caught up rather than leaving a stale copy.
- **The phone's chat matches the Mac's.** Sending while an agent works steers into its turn, sent messages can be edited, find works in a chat, agents' questions get the Mac's dock with recommended answers marked, @, +, # and ! work in the composer, a new chat can start in its own worktree, and a chat in trouble offers Reconnect, Retry or Discard.
- **HTML files preview.** An HTML file's editor has Source and Preview, as markdown's does, and Preview draws the page in WebKit.
- **Triggers say where they came from.** A trigger's message sits in your own bubble under a line naming the trigger, with the webhook request that fired it below.
- **Videos and sound play.** Media the Mac can play opens in Quick Look's viewer with its player rather than a card.
- Pinching a picture no longer crashes the app, and a zoomed picture pans to every edge.
- The notch island updates properly, so clicking an agent there opens its tab again.
- The composer fits a narrow chat, shortening the model name first, and no longer shakes sideways while typing.
- A long code block in a chat scrolls only when the gesture starts over it, so scrolling the chat past it stays smooth.
- File tabs carry git's letter for the file, in the colour its row in the tree uses.
- The Stash lives in the composer's Stash button and tray, no longer in the side rail.
- A browser tab leaves fullscreen before it navigates, reloads or closes.

For the complete patch history, compare [`v0.6.0-nightly.9...v0.6.0-nightly.10`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.9...v0.6.0-nightly.10).
