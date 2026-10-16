# Sikemux v0.5.3-nightly.5

The fifth nightly on the 0.5.3 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.4

- **Questions with several choices.** An agent's questions open above the composer as tabs, each with a short header, and every answer goes in one Send. A question that takes several choices shows checkboxes: numbers and Space tick, Enter moves on.
- **who_wrote names who broke CI.** Give it a failed CI run and it names the agents whose changes broke it. The core also answers whose uncommitted work a git command would touch, and the git guard refuses only where other agents work.
- **Agents hear what changed in the guide** after a Sikemux update.
- **Themes.** Switch theme previews each theme as you move through the list, and switching repaints only what shows. Command rows in the palettes are one line each.
- **The rail and the notch.** An agent's name sits with its counts and state at the row's end, the open island lists every open agent in the rail's order, and the closed island totals subagents and shells across every agent.
- Buddies reach the notch and your phone too.
- A terminal agent that dies the moment it starts is still resumed.
- The board reads and moves Jira tickets again, and Auto reaches the GitHub and Bitbucket plugins again.

For the complete patch history, compare [`v0.5.3-nightly.4...v0.5.3-nightly.5`](https://github.com/nodelike/sikemux/compare/v0.5.3-nightly.4...v0.5.3-nightly.5).
