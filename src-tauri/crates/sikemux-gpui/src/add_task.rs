//! The dialog that saves a person's own start command as one of the project's tasks,
//! for the team in sikemux.json or for them alone, or changes one they saved.

use std::rc::Rc;

use gpui_kit::component::input::InputState;
use gpui_kit::component::WindowExt;
use gpui_kit::{
    div, AnyElement, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement,
    SharedString, Styled, Window,
};
use serde_json::{json, Value};

use crate::icons::Icon;
use crate::theme::{Tokens, SPACE_1H, SPACE_3, TEXT_2XS};
use crate::toast;
use crate::ui::{self, dialog, Segmented};
use crate::workbench::Workbench;

pub struct AddTask {
    project: String,
    label: Entity<InputState>,
    command: Entity<InputState>,
    folder: Entity<InputState>,
    team: bool,
    /// The task this is an edit of.
    replaces: Option<String>,
}

impl AddTask {
    pub fn new(project: String, window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        Self::filled(project, None, window, cx)
    }

    /// The dialog holding the person's own `task`, to change it.
    pub fn edit(
        project: String,
        task: &Value,
        window: &mut Window,
        cx: &mut Context<Workbench>,
    ) -> Self {
        Self::filled(project, Some(task), window, cx)
    }

    fn filled(
        project: String,
        task: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Workbench>,
    ) -> Self {
        let text = |key: &str| {
            task.and_then(|task| task[key].as_str())
                .filter(|value| *value != ".")
                .unwrap_or_default()
                .to_owned()
        };
        let label = field("Backend", text("label"), window, cx);
        label.update(cx, |state, cx| state.focus(window, cx));
        Self {
            project,
            label,
            command: field("bash tool/run.sh", text("command"), window, cx),
            folder: field("The project root", text("cwd"), window, cx),
            team: task.is_none_or(|task| task["foundIn"].is_null()),
            replaces: task.and_then(|task| task["id"].as_str()).map(str::to_owned),
        }
    }
}

fn field(
    placeholder: &'static str,
    value: String,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    })
}

/// The dialog, while one is open.
pub fn view(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let adding = bench.overlays.adding_task.as_ref()?;
    let editing = adding.replaces.is_some();
    let t = Tokens::get(cx).clone();
    let note = if adding.team {
        "Goes in sikemux.json, which you commit."
    } else {
        "Kept in .sikemux/, which Git ignores."
    };
    let scope = vec![
        (Icon::Agents, SharedString::from("For the team")),
        (Icon::User, SharedString::from("Only me")),
    ];
    let body = dialog::marked_body()
        .pt(SPACE_3)
        .gap(SPACE_3)
        .child(dialog::field("Name", &adding.label, false, cx))
        .child(dialog::field("Command", &adding.command, true, cx))
        .child(dialog::field("Folder", &adding.folder, false, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(SPACE_1H)
                .child(ui::text("Save it", 12., FontWeight::MEDIUM).text_color(t.text_secondary))
                .child(
                    Segmented::new("task-scope", scope, usize::from(!adding.team)).on_select(
                        cx.listener(|bench, picked: &usize, _, cx| {
                            if let Some(adding) = bench.overlays.adding_task.as_mut() {
                                adding.team = *picked == 0;
                            }
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    div()
                        .text_size(crate::theme::ui_text(TEXT_2XS))
                        .text_color(t.text_tertiary)
                        .child(note),
                ),
        )
        .into_any_element();
    let this = cx.entity();
    let (saving, closing) = (this.clone(), this);
    let save: dialog::Answer =
        Rc::new(move |window, cx| saving.update(cx, |bench, cx| save_task(bench, window, cx)));
    let close: dialog::Answer = Rc::new(move |_, cx| {
        closing.update(cx, |bench, cx| {
            bench.overlays.adding_task = None;
            cx.notify();
        })
    });
    let foot = dialog::hinted_foot(dialog::escape("cancel", cx), cx)
        .child(dialog::button(
            "task-cancel",
            "Cancel".into(),
            None,
            close.clone(),
            cx,
        ))
        .child(dialog::button(
            "task-add",
            if editing { "Save" } else { "Add task" }.into(),
            Some(t.accent),
            save.clone(),
            cx,
        ));
    let title = if editing { "Edit task" } else { "Add a task" };
    let tile = if editing { Icon::Pencil } else { Icon::Plus };
    Some(dialog::marked_sheet(tile, title.into(), body, foot, save, close, cx).into_any_element())
}

/// Saves the task the dialog holds, once it has a name and a command.
fn save_task(bench: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>) {
    let Some(adding) = bench.overlays.adding_task.as_ref() else {
        return;
    };
    let read = |input: &Entity<InputState>| input.read(cx).value().trim().to_owned();
    let (label, command, folder) = (
        read(&adding.label),
        read(&adding.command),
        read(&adding.folder),
    );
    if label.is_empty() || command.is_empty() {
        return;
    }
    let (team, project, replaces) = (adding.team, adding.project.clone(), adding.replaces.clone());
    let edited = replaces.is_some();
    let params = json!({ "project": project, "label": label, "command": command, "cwd": folder, "team": team, "replaces": replaces });
    let adding = bench.target().request("addTask", params);
    bench.overlays.adding_task = None;
    cx.notify();
    cx.spawn_in(window, async move |bench, cx| {
        let added = adding.await;
        let _ = bench.update_in(cx, |bench, window, cx| match added {
            Ok(answer) => {
                if let Some(task) = answer["taskId"].as_str() {
                    bench.pick_task(task, cx);
                }
                let done = if edited { "Saved" } else { "Added" };
                let saved = if team {
                    format!("{done} {label} in sikemux.json; commit it to share it")
                } else {
                    format!("{done} {label} for you only")
                };
                window.push_notification(toast::success(saved), cx);
            }
            Err(error) => window.push_notification(
                toast::error(format!("Could not save the task: {error}")),
                cx,
            ),
        });
    })
    .detach();
}
