# Sikemux v0.5.1-nightly.3

The third nightly on the 0.5.1 line. Nightlies are signed and delivered exactly like stable releases, but they carry unreleased work and can break. Switch back to stable in Settings → About whenever you want; you keep the build you are on until a stable release passes it.

## New since nightly.2

- **Databases.** A new Database plugin connects to SQLite, PostgreSQL, MySQL and MariaDB, with passwords in the Keychain. Browse every connection's schemas and tables in one explorer, write SQL in tabs where ⌘↵ runs the statement under the cursor, see results in a grid, copy them as CSV, search the history of every query, and send results to an agent. Agents query through `db_` tools on a connection of their own, read-only unless you allow them to change data.
- **GitLab.** GitLab, on gitlab.com or your company's server, joins the Git pane as a code host: merge requests with their changes, commits, comments and approvals, issues, releases, your To-Do list, and pipelines you can follow, retry, cancel or start. Agents get GitLab tools too.
- **Jira boards.** A board view with its sprint and columns, where cards move between columns, plus lists for what you reported, watch or viewed. Tick the tasks in an issue's description, and open an issue in the browser or copy its link.
- **Rundeck tells you when a deploy ends.** Sikemux follows a run you started and notifies you whether it succeeded or failed, with a switch in the connection menu.
- **The file tree** opens and closes with ⌘E, colours every folder above a changed file, and a closed rail stays closed when Sikemux starts.
- **Claude chats** show the model and effort their settings choose, and changing the model no longer leaves a chat stuck working.

Thanks to Ankit Patidar for the Database and GitLab plugins, Jira boards and tasks, Rundeck run notices and the changed-folder colours.

For the complete patch history, compare [`v0.5.1-nightly.2...v0.5.1-nightly.3`](https://github.com/nodelike/sikemux/compare/v0.5.1-nightly.2...v0.5.1-nightly.3).
