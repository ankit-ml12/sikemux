//! ⌘⇧P, the command deck: every command the window can do and the person's own
//! commands, the ones used last first.

use gpui_kit::component::notification::Notification;
use gpui_kit::{Context, SharedString, Window};
use serde_json::json;
use sikemux_wire::protocol::{PaneKind, SessionId};
use tokio::sync::oneshot;

use crate::core;
use crate::keymap;
use crate::palette::{Row, Spec};
use crate::workbench::{Strip, Workbench};

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "palette.commands")]
pub struct OpenCommands;

const REMEMBERED: usize = 20;
const BUILTIN: &str = "builtin:";
const CUSTOM: &str = "custom:";
const STANDALONE: &str = "standalone:";

/// A command no shortcut does: its id, name, what it does and its category.
type Standalone = (&'static str, &'static str, &'static str, &'static str);

impl Workbench {
    pub fn toggle_commands(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut rows: Vec<Row> = keymap::shortcuts()
            .iter()
            .filter(|shortcut| shortcut.id != "palette.commands")
            .filter(|shortcut| cx.build_action(shortcut.id, None).is_ok())
            .map(|shortcut| {
                Row::new(format!("{BUILTIN}{}", shortcut.id), shortcut.label)
                    .meta(shortcut.category)
                    .keys(keymap::hint(shortcut.id, cx))
                    .also(shortcut.detail)
                    .also(shortcut.category)
                    .also(shortcut.id)
            })
            .collect();
        rows.extend(
            self.standalone()
                .into_iter()
                .map(|(id, title, detail, category)| {
                    Row::new(format!("{STANDALONE}{id}"), title)
                        .meta(category)
                        .also(detail)
                        .also(category)
                }),
        );
        if let Some(Strip::Project(_)) = self.strip() {
            let custom = self
                .preferences()
                .custom_commands
                .into_iter()
                .filter(|command| {
                    command.contexts.is_empty()
                        || command.contexts.iter().any(|kind| kind == "project")
                });
            rows.extend(custom.map(|command| {
                Row::new(format!("{CUSTOM}{}", command.id), command.title)
                    .meta("custom")
                    .also(&command.detail)
                    .also(&command.command)
            }));
        }
        let recent = |row: &Row| {
            self.recent_commands
                .iter()
                .position(|key| *key == row.key)
                .unwrap_or(usize::MAX)
        };
        rows.sort_by_key(recent);
        let spec = Spec::new(
            "palette.commands",
            "type a command…",
            rows,
            |row, bench, window, cx| {
                bench.remember_command(row.key.clone());
                if let Some(id) = row.key.strip_prefix(BUILTIN) {
                    keymap::dispatch(id, window, cx);
                } else if let Some(id) = row.key.strip_prefix(CUSTOM) {
                    bench.run_custom_command(id, cx);
                } else if let Some(id) = row.key.strip_prefix(STANDALONE) {
                    bench.run_standalone(id, window, cx);
                }
            },
        )
        .empty("no commands match");
        self.toggle_palette(spec, window, cx);
    }

    fn standalone(&self) -> Vec<Standalone> {
        let mut commands = Vec::new();
        if let Some(project) = self.project() {
            let path = project.path.to_string_lossy();
            commands.push((
                "project.settings",
                "Project settings",
                "Tasks, the default git repository, worktrees and language servers",
                "Project",
            ));
            commands.push(match crate::editor::language_servers_allowed(self, &path) {
                true => (
                    "project.language-servers.stop",
                    "Stop language servers for this project",
                    "Stop them now and do not start them again",
                    "Project · Language servers",
                ),
                false => (
                    "project.language-servers.allow",
                    "Allow language servers for this project",
                    "Start them when you open a file here",
                    "Project · Language servers",
                ),
            });
        }
        commands.extend([
            (
                "support.diagnostics",
                "Open runtime diagnostics",
                "Inspect redacted runtime and agent-detection health",
                "Support",
            ),
            (
                "support.whats-new",
                "Open What’s New",
                "Review the latest Sikemux release notes",
                "Support",
            ),
            (
                "support.onboarding",
                "Show welcome",
                "Open the first-run screen",
                "Support",
            ),
        ]);
        if self.terminal_in_front().is_some() {
            commands.push((
                "window.duplicate",
                "Duplicate active terminal",
                "Clone the active window into a new terminal tab",
                "Window",
            ));
        }
        commands.extend([
            (
                "app.quit-and-stop-everything",
                "Quit and Stop Everything",
                "Stop every terminal, terminal agent and task, then quit. Plain Quit leaves them running.",
                "Application",
            ),
            (
                "agents.reload-manifests",
                "Reload agent manifests",
                "Reload agent-state detection rules from disk",
                "Agents",
            ),
        ]);
        commands
    }

    fn run_standalone(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "project.language-servers.allow" | "project.language-servers.stop" => {
                if let Some(project) = self.project() {
                    let path = project.path.to_string_lossy().into_owned();
                    crate::editor::allow_language_servers(self, path, id.ends_with("allow"));
                }
            }
            "project.settings" => {
                if let Some(project) = self.project().map(|project| project.id.clone()) {
                    self.open_project_settings(&project);
                }
            }
            "support.diagnostics" => crate::experience::open_diagnostics(self, cx),
            "support.whats-new" => crate::experience::open_whats_new(self, cx),
            "support.onboarding" => crate::experience::open_welcome(self, cx),
            "window.duplicate" => {
                if let Some((strip, tab)) = self.terminal_in_front() {
                    self.duplicate_tab(&strip, &tab);
                }
            }
            "app.quit-and-stop-everything" => {
                keymap::dispatch("app.quitAndStopEverything", window, cx)
            }
            "agents.reload-manifests" => self.reload_manifests(cx),
            _ => {}
        }
    }

    /// The tab in front, when it is a project's terminal.
    fn terminal_in_front(&self) -> Option<(Strip, String)> {
        let strip @ Strip::Project(_) = self.strip()? else {
            return None;
        };
        let tab = self.tab()?;
        let (kind, _) = crate::tabs::first_view(&tab.pane)?;
        (kind == PaneKind::Terminal).then(|| (strip, tab.id.clone()))
    }

    fn reload_manifests(&self, cx: &mut Context<Self>) {
        let (reloaded, reload) = oneshot::channel();
        let _ = self.asks.send(core::Ask::Detection {
            detection: core::Detection::Reload,
            reply: reloaded,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Err(error)) = reload.await {
                let _ = this.update(cx, |bench, cx| {
                    bench.notify(
                        Notification::error(format!("Agent manifests: {error}")).autohide(false),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    fn remember_command(&mut self, key: SharedString) {
        self.recent_commands.retain(|known| *known != key);
        self.recent_commands.insert(0, key);
        self.recent_commands.truncate(REMEMBERED);
    }

    /// Runs one of the person's commands in the project in front, telling them
    /// what one in the background printed.
    fn run_custom_command(&self, id: &str, cx: &mut Context<Self>) {
        let Some(Strip::Project(project)) = self.strip() else {
            return;
        };
        let title = self
            .preferences()
            .custom_commands
            .into_iter()
            .find(|command| command.id == id)
            .map_or_else(|| id.to_owned(), |command| command.title);
        let tab = self.tab().map(|tab| tab.id.clone());
        let ran = core::request(
            &self.asks,
            "runCustomCommand",
            json!({ "project": project, "id": id, "tab": tab, "pane": self.focused_view() }),
        );
        cx.spawn(async move |this, cx| {
            let ran = ran.await;
            let _ = this.update(cx, |bench, cx| match ran {
                Ok(ran) => {
                    if let Ok(popup) = serde_json::from_value::<SessionId>(ran["popup"].clone()) {
                        return bench.show_command_popup(title, popup, cx);
                    }
                    let Some(result) = ran.get("result") else {
                        return;
                    };
                    let output = result["output"].as_str().unwrap_or_default().trim();
                    let code = result["code"].as_i64().unwrap_or(-1);
                    let said = if output.is_empty() {
                        format!("exit {code}")
                    } else {
                        output.to_owned()
                    };
                    let said = format!("{title}: {said}");
                    let note = if code == 0 {
                        Notification::success(said)
                    } else {
                        Notification::error(said).autohide(false)
                    };
                    bench.notify(note, cx);
                }
                Err(error) => bench.notify(
                    Notification::error(format!("{title}: {error}")).autohide(false),
                    cx,
                ),
            });
        })
        .detach();
    }
}
