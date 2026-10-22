//! The dialog that saves a person's own start command as one of the project's tasks,
//! for the team in sikemux.json or for them alone.

use std::rc::Rc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, AppContext, Context, Entity, FontWeight, IntoElement, ParentElement,
    SharedString, Styled, Window,
};
use serde_json::json;

use crate::icons::Icon;
use crate::theme::{self, Tokens, RADIUS_2};
use crate::ui::{self, dialog, Segmented};
use crate::workbench::Workbench;

pub struct AddTask {
    project: String,
    label: Entity<InputState>,
    command: Entity<InputState>,
    folder: Entity<InputState>,
    team: bool,
}

impl AddTask {
    pub fn new(project: String, window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let label = field("Backend", window, cx);
        label.update(cx, |state, cx| state.focus(window, cx));
        Self {
            project,
            label,
            command: field("bash tool/run.sh", window, cx),
            folder: field("The project root", window, cx),
            team: true,
        }
    }
}

fn field(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
}

/// The dialog, while one is open.
pub fn view(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let adding = bench.adding_task.as_ref()?;
    let t = Tokens::get(cx).clone();
    let field = |name: &'static str, input: &Entity<InputState>, mono: bool| {
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(ui::text(name, 12., FontWeight::MEDIUM).text_color(t.text_secondary))
            .child(
                div()
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(t.border_strong)
                    .rounded(RADIUS_2)
                    .bg(t.surface_sunken)
                    .text_size(px(12.))
                    .when(mono, |field| field.font_family(theme::MONO))
                    .child(Input::new(input).appearance(false)),
            )
    };
    let note = if adding.team {
        "Goes in sikemux.json, which you commit."
    } else {
        "Kept in .sikemux/, which Git ignores."
    };
    let scope = vec![
        (Icon::Agents, SharedString::from("For the team")),
        (Icon::User, SharedString::from("Only me")),
    ];
    let body = div()
        .px(px(16.))
        .py(px(14.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(field("Name", &adding.label, false))
        .child(field("Command", &adding.command, true))
        .child(field("Folder", &adding.folder, false))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ui::text("Save it", 12., FontWeight::MEDIUM).text_color(t.text_secondary))
                .child(
                    Segmented::new("task-scope", scope, usize::from(!adding.team)).on_select(
                        cx.listener(|bench, picked: &usize, _, cx| {
                            if let Some(adding) = bench.adding_task.as_mut() {
                                adding.team = *picked == 0;
                            }
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    div()
                        .text_size(px(11.))
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
            bench.adding_task = None;
            cx.notify();
        })
    });
    let foot = div()
        .child(dialog::button(
            "task-cancel",
            "Cancel".into(),
            None,
            close.clone(),
            cx,
        ))
        .child(dialog::button(
            "task-add",
            "Add task".into(),
            Some(t.accent),
            save.clone(),
            cx,
        ));
    Some(dialog::sheet("Add a task".into(), body, foot, save, close, cx).into_any_element())
}

/// Saves the task the dialog holds, once it has a name and a command.
fn save_task(bench: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>) {
    let Some(adding) = bench.adding_task.as_ref() else {
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
    let (team, project) = (adding.team, adding.project.clone());
    let params = json!({ "project": project, "label": label, "command": command, "cwd": folder, "team": team });
    let adding = bench.target().request("addTask", params);
    bench.adding_task = None;
    cx.notify();
    cx.spawn_in(window, async move |bench, cx| {
        let added = adding.await;
        let _ = bench.update_in(cx, |bench, window, cx| match added {
            Ok(answer) => {
                if let Some(task) = answer["taskId"].as_str() {
                    bench.pick_task(task, cx);
                }
                let saved = if team {
                    format!("Added {label} in sikemux.json; commit it to share it")
                } else {
                    format!("Added {label} for you only")
                };
                window.push_notification(Notification::success(saved), cx);
            }
            Err(error) => window.push_notification(
                Notification::error(format!("Could not add the task: {error}")),
                cx,
            ),
        });
    })
    .detach();
}
