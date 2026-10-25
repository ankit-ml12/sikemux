# Sikemux v0.6.0-nightly.1

The first nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break.

## What's new

- **Drawn natively on the GPU**, no longer in a web page.
- **Faster and lighter on memory** with many agents at work and long chats.
- **Plugin panels** for Jira, Linear, Slack, databases and more open as panes.
- **Agents start only when you write.** Opening, restoring or showing a chat starts nothing.
- **On Linux too.** The new app ships for x86_64 as a .deb, an .rpm and an AppImage. The AppImage updates itself; for the .deb and .rpm, Settings → About shows the one line that installs each new release.

## Before you update

- **Your phone needs the nightly phone app.** Install Sikemux 0.2.0-nightly.1 from TestFlight on iPhone, or from the Google Play testing track on Android. Phone apps from before it, 0.1.0 and its nightlies, keep working with stable hosts only.
- **A nightly host and a stable host can't link.** Hosts on this nightly link with each other, and hosts on stable Sikemux link with each other, but not across the two until the new app ships stable.
- **There is no going back to an older build.** This build moves saved chats to a new format, and older Sikemux, stable 0.5.x or earlier nightlies, can't read chats saved after the update. Switching back to stable in Settings → About keeps this build until a stable release passes it, so nothing is downgraded, but installing an older build by hand isn't supported.

## What carries over

- Your projects, chats, terminals, sign-ins and settings, and the work the core is doing, carry over. This host stays the same device on your account.

For the complete patch history, compare [`v0.5.4-nightly.8...v0.6.0-nightly.1`](https://github.com/nodelike/sikemux/compare/v0.5.4-nightly.8...v0.6.0-nightly.1).
