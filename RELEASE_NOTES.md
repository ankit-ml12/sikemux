# Sikemux v0.6.0-nightly.3

The third nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts. 0.2.0-nightly.4 shows the host's grain and pane picture again.

## New since nightly.2

- **Closing asks first.** Closing a busy agent, a lead with a team, a project with agents at work, or tabs with unsaved edits asks before anything is lost. The window's close button hides Sikemux as ⌘H does, so the Dock or a link brings it back.
- **Terminal agents stay terminal agents.** Pi and other agents that run in their own terminal start from ⌘⇧N, come back in their terminal after a sleep or a core restart, wake when messaged, and their earlier sessions show in Recent.
- **Recent reaches every chat.** Recent loads older chats as you scroll, its search finds any chat by title, and chats that ran in a worktree show under their project and resume there.
- **Menus.** Every menu, picker and dropdown is one kind: at least 196 wide, the tick at the end of the row, and arrow keys and Enter move through it.
- **Updates in the rail.** The rail says when an update is out and how its download and install are going, and "Update Sikemux" covers the window again when this build no longer works with Sikemux's servers.
- **The desk.** It slides in from the window's edge, ⌘L floats the address over the page again, pages show a spinner while they load, and the iOS Simulator picks and starts a device by itself, saves screenshots and shuts down when asked.
- **Git is quicker.** A change in one project re-reads only that project, the Git pane re-reads only the diffs that changed, and the history shows authors' pictures from GitHub or Bitbucket.
- **Crash reports.** A panic writes a report that `sikemux doctor` shows, a window that stops answering for two seconds leaves a report too, and each release publishes the symbols to read them.
- Pinching zooms chats, editors, terminals and pictures again, and terminals show their folder and how the last command went.
- The core lifts its open-file limit, so a busy session no longer fails with "Too many open files", and `sikemux` is on PATH in every terminal and agent again.
- Turning remote access on keeps the core starting at login, so phones reach this host after a restart.
- Two crashes are fixed: one when a menu or popover asked to repaint, one when scrolling the Git review partway into a file.
- Many smaller fixes: deleting an [Image #n] tag removes the picture, YOLO Mode switches a running session, notification sounds, Settings search, the Jira board's columns, .ico site icons, and more.

For the complete patch history, compare [`v0.6.0-nightly.2...v0.6.0-nightly.3`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.2...v0.6.0-nightly.3).
