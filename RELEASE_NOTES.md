# Sikemux v0.6.0-nightly.7

The seventh nightly of the new Sikemux app. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

Your phone needs the nightly phone app, 0.2.0-nightly.3 or later, from TestFlight on iPhone or the Google Play testing track on Android, and nightly hosts link only with other nightly hosts.

## New since nightly.6

- **Bring your own model provider.** Add any provider with its base URL, an API key (kept in the Keychain) and a name, and OMP and OpenCode chats run on its models. The model picker groups models by who serves them, lists only what the picked agent can run, and adds a provider from its footer.
- **Pull requests in the app read like GitHub's.** One layout at every width: the author's face and title, Conversation, Files, Commits and Checks tabs, a conversation line with faces, mentions and pushed commits, folding diffs, and merging pinned at the foot with its reviews, checks and a split Merge button. Pull requests close from beside Comment.
- **Links open where you are.** A pull request, issue, Actions run or Jira issue link in an agent's reply opens on its desk as the app draws it, not as a web page.
- **CI at a glance.** The top bar's CI chip stands for the branch's newest commit across every workflow, with a count, a spinning mark while runs go, and a card of every run on hover.
- **Settings regrouped.** 23 pages in six groups, your account at the top, a flat search row, and Escape closes Settings from anywhere.
- **A new chat greets you.** The agent's face asks "What are we making in" the project, with the composer at the foot of the pane.
- OMP and OpenCode chats show tool-server calls, edits, searches and commands as their own rows, resume on the provider and model they last ran, and show their past conversation while waiting.
- Clicking anywhere in the app takes the keyboard back from a page, so typing no longer beeps after clicking into a page in a chat.
- Clicking a run in the CI card no longer crashes the app.
- YOLO shows a filled bolt and SAFE a closed lock, code-host and plugin panes draw no grain behind them, and markdown follows Appearance › Text size.

For the complete patch history, compare [`v0.6.0-nightly.6...v0.6.0-nightly.7`](https://github.com/nodelike/sikemux/compare/v0.6.0-nightly.6...v0.6.0-nightly.7).
