# Sikemux v0.6.0-nightly.4

The fourth nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts.

## New since nightly.3

- **Nightly.3 starts again.** It quit at launch while its core held an update ready to install. The app no longer restarts into the build it already runs, and restarting no longer crashes it.
- **Messages reach agents brought back after an update.** A message to an agent whose chat waits to start, as every chat does after an update, starts it and is delivered, rather than waiting until you open it.
- **Linux hosts update themselves.** Once no chat is in a turn, a host installs the newest signed core and hands over in place, keeping its terminals and chats. `sikemux update` updates it now, `sikemux update --off` stops it, and `sikemux status` says when a host is too old for its account.
- A host's row in the rail and in Settings › Devices shows its version, or "update failed" when the last try did not work, with an Update automatically switch in its ⋯.
- A little more room between an agent's thinking line and the composer.

For the complete patch history, compare [`v0.6.0-nightly.3...v0.6.0-nightly.4`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.3...v0.6.0-nightly.4).
