//! Starting one of the project's tasks, and the card its checks raise when one fails, as the
//! Mac's Run chip shows it: what is missing, then Run anyway, or Install and run or Fix and run
//! when the core can set right what is missing.

use std::rc::Rc;

use gpui_kit::component::notification::Notification;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window,
};
use serde_json::{json, Value};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, dialog};
use crate::workbench::Workbench;

/// What an agent asked to find the project's tasks is told, after the line naming the project.
const FIND_TASKS: &str = "Read the README, AGENTS.md, Makefile, package.json and any scripts in folders like tool/, bin/ or scripts/.
Then add a task for each command people really use (starting the app, running the tests, building) to sikemux.json at the project root:
each task has an \"id\", a \"label\", a \"command\" and, when it runs in a subfolder, a \"cwd\". Keep everything already in sikemux.json.
Call guide with the config topic first for the format. Do not start long-running commands; tell me which tasks you added and why.";

/// A task held back by its checks, waiting on the person.
pub struct RunCheck {
    project: String,
    task: String,
    label: String,
    checks: Vec<Value>,
    fixing: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum How {
    Checked,
    Anyway,
    Fixed,
}

impl Workbench {
    /// Starts `task` in the project in front once its checks pass.
    pub fn start_task(&mut self, task: &str, cx: &mut Context<Self>) {
        if let Some(project) = self.watched_project.clone() {
            self.start_task_in(project, task, cx);
        }
    }

    /// Starts `task` in the project at `project` once its checks pass.
    pub fn start_task_in(&mut self, project: String, task: &str, cx: &mut Context<Self>) {
        self.run_task(project, task.to_owned(), How::Checked, cx);
    }

    fn run_task(&mut self, project: String, task: String, how: How, cx: &mut Context<Self>) {
        if self.run_check.as_ref().is_some_and(|check| check.fixing) {
            return;
        }
        let label = self.task_label(&task);
        let asked = self.target().request(
            "taskStart",
            json!({
                "project": project,
                "taskId": task,
                "force": how == How::Anyway,
                "fix": how == How::Fixed,
            }),
        );
        if let Some(check) = self.run_check.as_mut().filter(|_| how == How::Fixed) {
            check.fixing = true;
        }
        cx.notify();
        cx.spawn(async move |bench, cx| {
            let answer = asked.await;
            let _ = bench.update(cx, |bench, cx| {
                match answer {
                    Ok(answer) => match answer["checks"].as_array() {
                        Some(checks) => {
                            bench.run_check = Some(RunCheck {
                                project,
                                task,
                                label,
                                checks: checks.clone(),
                                fixing: false,
                            })
                        }
                        None => {
                            bench.run_check = None;
                            bench.notify(
                                Notification::success(format!("Started task: {label}")),
                                cx,
                            );
                        }
                    },
                    Err(error) => {
                        if let Some(check) = bench.run_check.as_mut() {
                            check.fixing = false;
                        }
                        bench.notify(
                            Notification::error(format!("Start task {label}: {error}")),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Puts in `to`'s composer the ask to find how the project `name` is run, tested and built,
    /// and save those as tasks, starting its chat first when it is a new one.
    pub fn find_tasks_with(
        &mut self,
        to: crate::send_to::Recipient,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = format!(
            "Find how {name} is run, tested and built, and save those as Sikemux tasks.\n{FIND_TASKS}"
        );
        crate::send_to::send(self, to, async move { Some(text) }, window, cx);
    }

    fn task_label(&self, task: &str) -> String {
        self.read("tasks")
            .and_then(|read| read["tasks"].as_array())
            .and_then(|tasks| tasks.iter().find(|listed| listed["id"] == task))
            .and_then(|listed| listed["label"].as_str())
            .unwrap_or(task)
            .to_owned()
    }

    fn close_run_check(&mut self, cx: &mut Context<Self>) {
        self.run_check = None;
        cx.notify();
    }
}

/// The card, while a task waits on its checks.
pub fn view(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let check = bench.run_check.as_ref()?;
    let t = Tokens::get(cx).clone();
    let failed = |item: &&Value| item["ok"] != true;
    let installs = check
        .checks
        .iter()
        .filter(failed)
        .any(|item| item["kind"] != "port");
    let fixable = installs
        || check
            .checks
            .iter()
            .filter(failed)
            .any(|item| item["kind"] == "port" && !item["freePort"].is_null());
    let body = div()
        .px(px(20.))
        .py(px(12.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(check.checks.iter().map(|item| {
            let ok = item["ok"] == true;
            div()
                .flex()
                .items_start()
                .gap(px(8.))
                .child(
                    ui::icon(if ok { Icon::Check } else { Icon::Close }, 11.)
                        .mt(px(2.))
                        .text_color(if ok { t.live } else { t.danger }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .min_w_0()
                        .child(
                            div()
                                .text_size(crate::theme::ui_text(12.))
                                .text_color(t.text_primary)
                                .child(item["title"].as_str().unwrap_or_default().to_owned()),
                        )
                        .child(
                            div()
                                .text_size(crate::theme::ui_text(11.))
                                .text_color(t.text_tertiary)
                                .child(item["detail"].as_str().unwrap_or_default().to_owned()),
                        ),
                )
        }))
        .into_any_element();
    let this = cx.entity();
    let close: dialog::Answer =
        Rc::new(move |_, cx| this.update(cx, |bench, cx| bench.close_run_check(cx)));
    let run = |how: How| -> dialog::Answer {
        let (this, project, task) = (cx.entity(), check.project.clone(), check.task.clone());
        Rc::new(move |_, cx| {
            this.update(cx, |bench, cx| {
                bench.run_task(project.clone(), task.clone(), how, cx)
            })
        })
    };
    let anyway = run(How::Anyway);
    let fix = run(How::Fixed);
    let yes = if fixable { fix.clone() } else { anyway.clone() };
    let foot = div()
        .child(dialog::button(
            "run-check-anyway",
            "Run anyway".into(),
            (!fixable).then_some(t.accent),
            anyway,
            cx,
        ))
        .when(fixable, |foot| {
            let label = match (check.fixing, installs) {
                (true, _) => "Fixing…",
                (false, true) => "Install and run",
                (false, false) => "Fix and run",
            };
            foot.child(dialog::button(
                "run-check-fix",
                SharedString::from(label),
                Some(t.accent),
                fix,
                cx,
            ))
        });
    Some(
        dialog::sheet_of(
            px(440.),
            format!("{} can't start yet", check.label).into(),
            body,
            foot,
            yes,
            close,
            cx,
        )
        .into_any_element(),
    )
}
