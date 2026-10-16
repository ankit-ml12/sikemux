# Sikemux v0.5.3-nightly.3

The third nightly on the 0.5.3 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.2

- **Usage limits work again.** The plan limits lookup answers in about half a second instead of timing out, no longer starts your MCP servers or scans a week of transcripts, and stops asking again and again after a failure, which had Anthropic answering 429.
- **Install missing agents from Sikemux.** The welcome screen lists every agent not on this computer with Install, and installing shows its official installer running in a sheet. Add and sign in agent accounts from a dialog, from the rail or Settings.
- **Network drops.** A turn the network cut short is kept and goes again once the network is back, with Retry now and Discard, and plugins say plainly when a service can't be reached instead of clearing the screen. Voice model downloads pause offline and pick up where they stopped.
- **Rundeck.** A job opens its own page with its live deploy, deploy times and history, an Activity page lists every run in the project, a run names its steps and how long is left, and the top bar's deploy chip shows when its job is deploying.
- **Buddies.** Every agent can get a buddy, off by default, picked from packs in Settings.
- **One-line install.** One command installs Sikemux on a Mac or x86_64 Linux.
- **Who wrote what.** Every agent commit ends in a footer the host signs, naming the agent, its CLI and Sikemux's versions, and a commit holding several writers' lines carries a note of whose lines are whose. History, the commit header and a commit's diff show those agents and open their sessions. Changes made outside the edit tools, and files you save yourself, go on record too, and agents can ask `who_wrote` who changed something and how to reach them.
- **Move a chat to another account,** even in the middle of a turn, from an account picker in the composer.
- **Long chats.** The app takes up a long chat instead of restarting its agent, and pages back through it, keeping only what subagents said.
- **The rail** picks the provider from a menu on its name, shows open agents and recent chats only, and shows agent stats as marks with counts. Menus carry icons throughout.
- **Tabs and the rail** lead with the chat's title, the agent's name dim after it, and an agent's project sits on a second line. The Terminal and Agents rows are back in the project tree. Inbox items are plain two-line agent rows.
- **Jira.** An open issue resizes from its edge, reads in a centred column, and has an Activity section with comments, all activity and history.
- Agents see the pull requests a commit came in through on GitHub and Bitbucket.
- SSH tabs are named after the host.
- A Claude prompt sent again after a restart is answered, and editing a message older than the replay keeps the history before it.

For the complete patch history, compare [`v0.5.3-nightly.2...v0.5.3-nightly.3`](https://github.com/nodelike/sikemux/compare/v0.5.3-nightly.2...v0.5.3-nightly.3).
