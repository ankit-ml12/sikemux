//! ⌘⇧P, the command deck: every command the window can do and the person's own
//! commands, the ones used last first.

use gpui_kit::component::notification::Notification;
use gpui_kit::{Context, SharedString, Window};
use serde_json::json;

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
                }
            },
        )
        .empty("no commands match");
        self.toggle_palette(spec, window, cx);
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
        let ran = core::request(
            &self.asks,
            "runCustomCommand",
            json!({ "project": project, "id": id }),
        );
        cx.spawn(async move |this, cx| {
            let ran = ran.await;
            let _ = this.update(cx, |bench, cx| match ran {
                Ok(ran) => {
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
