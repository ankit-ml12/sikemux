use std::f32::consts::FRAC_PI_2;
use std::path::Path;
use std::rc::Rc;

use gpui::Anchor;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::Side;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, radians, AnyElement, App, Context, FontWeight, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Transformation, WeakEntity,
    WindowControlArea,
};
use serde_json::Value;
use sikemux_wire::protocol::{Pane, PaneKind, Tab as LayoutTab};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, IconButton, Tab};
use crate::workbench::{Strip, Workbench};

/// The bar over the stage: the project's tabs, then the clock.
pub fn top_bar(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let tabs: Vec<_> = bench
        .strip()
        .map(|strip| {
            let front = bench.tab().map(|tab| tab.id.clone());
            bench
                .tabs(&strip)
                .iter()
                .map(|tab| {
                    strip_tab(
                        bench,
                        &strip,
                        tab,
                        front.as_deref() == Some(tab.id.as_str()),
                        cx,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    div()
        .id("top-bar")
        .flex()
        .flex_none()
        .items_center()
        .h(px(34.))
        .window_control_area(WindowControlArea::Drag)
        .child(div().w(px(80.)).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .pl(px(20.))
                .pr(px(12.))
                .flex()
                .items_center()
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .min_w_0()
                        .overflow_hidden()
                        .px(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .children(tabs),
                )
                .child(
                    IconButton::new("new-tab", Icon::Plus, "New tab")
                        .glyph(13.)
                        .on_click(|_, window, cx| {
                            crate::keymap::dispatch("palette.newTab", window, cx)
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .pr(px(16.))
                .children(
                    git_chip(bench, cx)
                        .map(|chip| [chip, separator(cx)])
                        .into_iter()
                        .flatten(),
                )
                .children(
                    run_chip(bench, cx)
                        .map(|chip| [chip, separator(cx)])
                        .into_iter()
                        .flatten(),
                )
                .children(
                    ports_chip(bench, cx)
                        .map(|chip| [chip, separator(cx)])
                        .into_iter()
                        .flatten(),
                )
                .children(ci_glyph(bench, cx))
                .children(battery_chip(bench, cx))
                .child(clock(cx))
                .child(
                    IconButton::new(
                        "focus",
                        Icon::Focus,
                        match crate::keymap::hint("view.focusMode", cx) {
                            Some(keys) => format!("Focus mode — hide rails — {keys}"),
                            None => "Focus mode — hide rails".to_owned(),
                        },
                    )
                    .width(30.)
                    .pressed(!bench.side_rail_open && !bench.tab_rail_open)
                    .on_click(cx.listener(|bench, _, _, cx| bench.toggle_focus_mode(cx))),
                ),
        )
}

fn separator(cx: &App) -> AnyElement {
    div()
        .w(px(1.))
        .h(px(16.))
        .bg(Tokens::get(cx).line)
        .into_any_element()
}

/// Where the project's repository stands: its branch, and how far it is from its upstream.
fn git_chip(bench: &Workbench, cx: &App) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let project = bench.project()?;
    let git = project.git.as_ref()?;
    let host = project.code_host.as_ref();
    let mark = match host {
        Some(host) if host.host == "github" => {
            ui::icon(Icon::Github, 12.).text_color(t.text_primary)
        }
        _ => ui::icon(Icon::Git, 12.).text_color(if git.dirty { t.warn } else { t.live }),
    };
    let mut about: Vec<String> = host
        .map(|host| vec![host_label(&host.host).to_owned(), host.repo.clone()])
        .unwrap_or_default();
    about.push(git.branch.clone());
    about.push(if git.dirty {
        "changed".into()
    } else {
        "clean".into()
    });
    about.extend((git.ahead > 0).then(|| format!("ahead {}", git.ahead)));
    about.extend((git.behind > 0).then(|| format!("behind {}", git.behind)));
    let count = |arrow: &str, count: u32, color| {
        (count > 0)
            .then(|| ui::text(format!("{arrow}{count}"), 10., FontWeight::NORMAL).text_color(color))
    };
    Some(
        div()
            .id("git-chip")
            .h(px(24.))
            .max_w(px(200.))
            .px(px(4.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(crate::theme::RADIUS_1)
            .font_family(crate::theme::MONO)
            .text_size(px(11.))
            .text_color(t.text_tertiary)
            .hover(move |style| style.text_color(t.text_primary))
            .tooltip(ui::tooltip(about.join(" · ")))
            .child(mark)
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(t.text_primary)
                    .child(git.branch.clone()),
            )
            .when(git.ahead > 0 || git.behind > 0, |chip| {
                chip.child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .children(count("↑", git.ahead, t.live))
                        .children(count("↓", git.behind, t.warn)),
                )
            })
            .into_any_element(),
    )
}

/// How many ports the project's terminals, tasks and agents listen on.
fn ports_chip(bench: &Workbench, cx: &App) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let count = bench.read("ports")?.as_array()?.len();
    if count == 0 {
        return None;
    }
    let about = if count == 1 {
        "1 listening port".to_owned()
    } else {
        format!("{count} listening ports")
    };
    Some(
        div()
            .id("ports-chip")
            .h(px(24.))
            .px(px(4.))
            .flex()
            .items_center()
            .gap(px(5.))
            .rounded(crate::theme::RADIUS_1)
            .font_family(crate::theme::MONO)
            .text_size(px(11.))
            .text_color(t.text_tertiary)
            .hover(move |style| style.text_color(t.text_primary))
            .tooltip(ui::tooltip(about))
            .child(ui::icon(Icon::Plug, 12.).text_color(t.text_tertiary))
            .child(div().text_color(t.text_primary).child(count.to_string()))
            .into_any_element(),
    )
}

/// How the branch's latest CI run went, in the space of one mark.
fn ci_glyph(bench: &Workbench, cx: &App) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let run = bench.latest_run()?;
    let outcome = run["outcome"].as_str()?;
    let number = run["runNumber"].as_u64()?;
    let ago = chrono::DateTime::parse_from_rfc3339(run["createdAt"].as_str()?)
        .ok()?
        .timestamp_millis()
        .max(0) as u64;
    let ago = match crate::agents::age(ago) {
        now if now == "now" => now,
        age => format!("{age} ago"),
    };
    let about = format!(
        "{} · {} #{number} · {ago}",
        outcome_label(outcome),
        run["name"].as_str().unwrap_or_default()
    );
    Some(
        div()
            .id("ci-glyph")
            .h(px(24.))
            .px(px(4.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(crate::theme::RADIUS_1)
            .text_size(px(11.))
            .text_color(t.text_secondary)
            .hover(move |style| style.text_color(t.text_primary))
            .tooltip(ui::tooltip(about))
            .child(ui::status_of_run(outcome, 12., cx))
            .child(
                ui::mono(format!("#{number}"), 11., cx)
                    .font_features(crate::theme::tabular())
                    .text_color(t.text_secondary),
            )
            .into_any_element(),
    )
}

/// How full this device's battery is, drawn as the app's battery icon.
fn battery_chip(bench: &Workbench, cx: &App) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let battery = bench.battery?;
    let percent = battery.percent;
    let tint = match percent {
        _ if battery.charging => t.live,
        0..=10 => t.danger,
        11..=20 => t.warn,
        _ => t.accent,
    };
    let time = battery
        .minutes
        .map(|minutes| format!("{}:{:02}", minutes / 60, minutes % 60));
    let about = match (battery.charging, time) {
        (true, Some(time)) => format!("{percent}% · charging · {time} until full"),
        (true, None) => format!("{percent}% · charging"),
        (false, Some(time)) => format!("{percent}% · {time} remaining"),
        (false, None) => format!("{percent}%"),
    };
    let unit = 12. / 13.;
    let at = |units: f32| px(units * unit);
    let level = f32::from(percent) / 100.;
    let fill = if level > 0. {
        (20.5 * level).max(2.2)
    } else {
        0.
    };
    let frame = Hsla { a: 0.45, ..tint };
    let bolt = |glyph: Icon, color: Hsla| {
        ui::icon(glyph, 19. * unit)
            .w(at(12.))
            .absolute()
            .left(at(7.))
            .top(at(-3.))
            .text_color(color)
    };
    let icon = div()
        .relative()
        .flex_none()
        .w(at(28.))
        .h(at(13.))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .w(at(24.5))
                .h(at(13.))
                .border(at(1.))
                .border_color(frame)
                .rounded(at(4.1)),
        )
        .child(
            div()
                .absolute()
                .left(at(25.4))
                .top(at(4.4))
                .w(at(1.6))
                .h(at(4.2))
                .rounded_r(at(1.6))
                .bg(frame),
        )
        .child(
            div()
                .absolute()
                .left(at(2.))
                .top(at(2.))
                .w(at(fill))
                .h(at(9.))
                .rounded(at(2.2))
                .bg(tint),
        )
        .when(battery.charging, |icon| {
            icon.child(bolt(Icon::BatteryBoltGap, t.surface_sunken))
                .child(bolt(Icon::BatteryBolt, tint))
        });
    Some(
        div()
            .id("battery")
            .flex()
            .items_center()
            .gap(px(5.))
            .tooltip(ui::tooltip(about))
            .child(icon)
            .child(
                ui::text(format!("{percent}%"), 11., FontWeight::SEMIBOLD)
                    .font_features(crate::theme::tabular())
                    .text_color(tint),
            )
            .into_any_element(),
    )
}

fn outcome_label(outcome: &str) -> &str {
    match outcome {
        "running" => "Running",
        "queued" => "Queued",
        "success" => "Passed",
        "failure" => "Failed",
        "cancelled" => "Cancelled",
        "skipped" => "Skipped",
        "blocked" => "Waiting for approval",
        _ => "Unknown",
    }
}

fn host_label(host: &str) -> &str {
    match host {
        "github" => "GitHub",
        "gitlab" => "GitLab",
        "bitbucket" => "Bitbucket",
        other => other,
    }
}

/// Runs the project's task, or stops the run going, and says how the last one went.
fn run_chip(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let tasks = bench.read("tasks")?["tasks"].as_array()?;
    let project = bench.watched_project()?;
    let ports = bench
        .read("ports")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let (running, last) = bench.person_runs();
    let task_of = |run: &Value| tasks.iter().find(|task| task["id"] == run["taskId"]);
    let picked = bench
        .picked_tasks
        .get(project)
        .and_then(|id| tasks.iter().find(|task| task["id"] == id.as_str()));
    let selected = running.and_then(task_of).or(picked).or(tasks.first())?;
    let id = selected["id"].as_str()?.to_owned();
    let label = selected["label"].as_str()?.to_owned();
    let last = last.filter(|run| running.is_none() && run["taskId"] == id.as_str());
    let failed =
        last.filter(|run| run["status"] == "failed")
            .map(|run| match run["exitCode"].as_i64() {
                Some(code) => format!("exited {code}"),
                None => "failed".into(),
            });
    let port = running.and_then(|run| {
        ports
            .iter()
            .find(|port| port["owner"]["executionId"] == run["executionId"])
    });
    let port_number = port.and_then(|port| port["port"].as_u64());
    let since = running.map(|run| {
        run["startedAt"].as_i64().map_or(0, |at| {
            (chrono::Utc::now().timestamp_millis() - at).max(0) / 1000
        })
    });
    let needs_java = selected["java"]["runsOn"]
        .is_null()
        .then(|| &selected["java"]["required"])
        .filter(|required| !required.is_null())
        .map(|required| format!("needs Java {}", plain(required)));
    let status = match (port_number, since, &failed) {
        (Some(port), _, _) => Some((format!(":{port}"), t.text_tertiary)),
        (None, Some(since), _) => Some((format!("{since}s"), t.text_tertiary)),
        (None, None, Some(failed)) => Some((failed.clone(), t.danger)),
        _ => needs_java.map(|text| (text, t.warn)),
    };
    let dot = match (running, &failed) {
        (Some(_), _) if port.is_some() => Some(div().size(px(7.)).rounded_full().bg(t.live)),
        (Some(_), _) => Some(
            div()
                .size(px(7.))
                .rounded_full()
                .border(px(1.5))
                .border_color(t.warn),
        ),
        (None, Some(_)) => Some(div().size(px(7.)).rounded_full().bg(t.danger)),
        _ => None,
    };
    let execution = running
        .and_then(|run| run["executionId"].as_str())
        .map(str::to_owned);
    let menu = Rc::new(RunMenu {
        bench: cx.entity().downgrade(),
        running: running.map(|_| RunningTask {
            id: id.clone(),
            label: label.clone(),
            meta: match (port_number, since) {
                (Some(port), _) => format!("up · :{port}"),
                (None, since) => format!("running · {}s", since.unwrap_or(0)),
            },
            port: port_number,
        }),
        output: running
            .or(last.filter(|_| failed.is_some()))
            .and_then(|run| run["ptyId"].as_u64())
            .map(|session| (session, label.clone())),
        groups: task_groups(tasks, project),
        selected: id.clone(),
        others: ports
            .iter()
            .filter(|other| other["owner"]["kind"] != "terminal" && Some(*other) != port)
            .filter_map(OtherPort::from_port)
            .collect(),
    });
    let primary = if running.is_some() {
        format!("Stop {label}")
    } else {
        format!("Run {label}")
    };
    let (glyph, word, tint) = if running.is_some() {
        (Icon::Stop, "Stop", t.danger)
    } else {
        (Icon::Run, "Run", t.live)
    };
    Some(
        div()
            .h(px(24.))
            .flex()
            .rounded(crate::theme::RADIUS_2)
            .bg(Hsla {
                a: 0.94,
                ..t.surface
            })
            .border_1()
            .border_color(Hsla {
                a: 0.06,
                ..t.text_primary
            })
            .child(
                div()
                    .id("run-task")
                    .pl(px(7.))
                    .pr(px(9.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .text_size(px(12.))
                    .text_color(t.text_secondary)
                    .hover(move |style| style.text_color(t.text_primary).bg(t.surface_hover))
                    .tooltip(ui::tooltip(primary))
                    .on_click(cx.listener(move |bench, _, _, _| match &execution {
                        Some(execution) => bench.stop_run(execution),
                        None => bench.start_task(&id),
                    }))
                    .child(ui::icon(glyph, 11.).text_color(tint))
                    .child(word),
            )
            .child(
                Button::new("choose-task")
                    .ghost()
                    .compact()
                    .rounded(px(0.))
                    .h_full()
                    .max_w(px(320.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .gap(px(5.))
                    .border_l_1()
                    .border_color(Hsla {
                        a: 0.08,
                        ..t.text_primary
                    })
                    .font_family(crate::theme::MONO)
                    .text_size(px(11.))
                    .text_color(t.text_tertiary)
                    .children(dot)
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(label),
                    )
                    .children(
                        status.map(|(text, color)| {
                            div().text_color(color).child(format!("· {text}"))
                        }),
                    )
                    .child(
                        ui::icon(Icon::Chevron, 10.)
                            .with_transformation(Transformation::rotate(radians(FRAC_PI_2)))
                            .text_color(t.text_tertiary),
                    )
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |popup, _, _| {
                        menu.build(popup)
                    }),
            )
            .into_any_element(),
    )
}

/// What the Run chip's menu lists, as it was when the chip was drawn.
struct RunMenu {
    bench: WeakEntity<Workbench>,
    running: Option<RunningTask>,
    /// The terminal the task runs in, or ran in when it failed, and the task's name.
    output: Option<(u64, String)>,
    groups: Vec<TaskGroup>,
    selected: String,
    /// Other things listening in the project, that tasks or agents started.
    others: Vec<OtherPort>,
}

struct RunningTask {
    id: String,
    label: String,
    meta: String,
    port: Option<u64>,
}

struct TaskGroup {
    title: String,
    meta: Option<String>,
    /// Each task's id, and its label without the folder the group's heading names.
    tasks: Vec<(String, String)>,
}

struct OtherPort {
    port: u64,
    process: String,
    owner: String,
    session: Option<u64>,
    agent: Option<String>,
    execution: Option<String>,
}

impl OtherPort {
    fn from_port(port: &Value) -> Option<Self> {
        let owner = &port["owner"];
        let text = |value: &Value| value.as_str().map(str::to_owned);
        Some(Self {
            port: port["port"].as_u64()?,
            process: text(&port["process"])
                .filter(|process| !process.is_empty())
                .unwrap_or_else(|| "process".into()),
            owner: match owner["kind"].as_str() {
                Some("agent") => text(&owner["label"]).unwrap_or_default(),
                _ => "You".into(),
            },
            session: owner["sessionId"].as_u64(),
            agent: text(&owner["agentId"]),
            execution: text(&owner["executionId"]),
        })
    }
}

impl RunMenu {
    fn build(&self, popup: PopupMenu) -> PopupMenu {
        let mut popup = popup
            .check_side(Side::Right)
            .scrollable(true)
            .min_w(px(220.))
            .max_w(px(360.))
            .max_h(px(480.));
        if let Some(running) = &self.running {
            popup = popup.label(format!("{} · {}", running.label, running.meta));
            if let Some(port) = running.port {
                let url = format!("http://localhost:{port}");
                popup = popup.item(
                    PopupMenuItem::new(format!("Open localhost:{port}"))
                        .icon(Icon::Globe)
                        .on_click(move |_, _, cx| cx.open_url(&url)),
                );
            }
            let (bench, task) = (self.bench.clone(), running.id.clone());
            popup = popup.item(PopupMenuItem::new("Restart").icon(Icon::Refresh).on_click(
                move |_, _, cx| {
                    let _ = bench.update(cx, |bench, _| bench.start_task(&task));
                },
            ));
        }
        if let Some((session, label)) = &self.output {
            let (bench, session, label) = (self.bench.clone(), *session, label.clone());
            popup = popup.item(
                PopupMenuItem::new("Show output")
                    .icon(Icon::Terminal)
                    .on_click(move |_, _, cx| {
                        let _ = bench.update(cx, |bench, _| {
                            bench.show("terminal", &session.to_string(), Some(&label))
                        });
                    }),
            );
        }
        for group in &self.groups {
            popup = popup.separator().label(match &group.meta {
                Some(meta) => format!("{} · {meta}", group.title),
                None => group.title.clone(),
            });
            for (task, label) in &group.tasks {
                let (bench, task) = (self.bench.clone(), task.clone());
                popup = popup.item(
                    PopupMenuItem::new(label.clone())
                        .icon(Icon::Run)
                        .checked(task == self.selected)
                        .on_click(move |_, _, cx| {
                            let _ = bench.update(cx, |bench, cx| bench.pick_task(&task, cx));
                        }),
                );
            }
        }
        if !self.others.is_empty() {
            popup = popup.separator().label("Running now");
        }
        for other in &self.others {
            popup = popup.item(other.item(&self.bench));
        }
        popup
    }
}

impl OtherPort {
    fn item(&self, bench: &WeakEntity<Workbench>) -> PopupMenuItem {
        let (port, process, owner) = (self.port, self.process.clone(), self.owner.clone());
        let stop = self
            .execution
            .clone()
            .map(|execution| (bench.clone(), execution));
        let reveal = match (self.session, &self.agent) {
            (Some(session), _) => Some(("terminal", session.to_string())),
            (None, Some(agent)) => Some(("agent", agent.clone())),
            _ => None,
        };
        let shown = bench.clone();
        PopupMenuItem::element(move |_, cx| {
            let t = Tokens::get(cx).clone();
            let stop = stop.clone().map(|(bench, execution)| {
                div()
                    .id(("stop-port", port as usize))
                    .p(px(3.))
                    .rounded(crate::theme::RADIUS_2)
                    .hover(move |style| style.bg(t.surface_hover))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        let _ = bench.update(cx, |bench, _| bench.stop_run(&execution));
                    })
                    .child(ui::icon(Icon::Stop, 9.).text_color(t.danger))
            });
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(7.)).rounded_full().bg(t.live))
                .child(div().flex_1().min_w_0().truncate().child(process.clone()))
                .child(
                    div()
                        .font_family(crate::theme::MONO)
                        .text_color(t.text_tertiary)
                        .child(format!(":{port}")),
                )
                .child(div().text_color(t.text_tertiary).child(owner.clone()))
                .children(stop)
        })
        .on_click(move |_, _, cx| {
            if let Some((kind, target)) = &reveal {
                let _ = shown.update(cx, |bench, _| bench.show(kind, target, None));
            }
        })
    }
}

/// sikemux.json's tasks, then one group for each folder the build files' tasks came from.
fn task_groups(tasks: &[Value], project: &str) -> Vec<TaskGroup> {
    let text = |task: &Value, key: &str| task[key].as_str().unwrap_or_default().to_owned();
    let mut groups = Vec::new();
    let configured: Vec<_> = tasks
        .iter()
        .filter(|task| matches!(task["source"].as_str(), Some("project" | "personal")))
        .map(|task| (text(task, "id"), text(task, "label")))
        .collect();
    if !configured.is_empty() {
        groups.push(TaskGroup {
            title: "sikemux.json".into(),
            meta: None,
            tasks: configured,
        });
    }
    let mut folders: Vec<(&str, Vec<&Value>)> = Vec::new();
    for task in tasks
        .iter()
        .filter(|task| matches!(task["source"].as_str(), Some("detected" | "rerun")))
    {
        let cwd = task["cwd"].as_str().unwrap_or(".");
        match folders.iter_mut().find(|(folder, _)| *folder == cwd) {
            Some((_, folder)) => folder.push(task),
            None => folders.push((cwd, vec![task])),
        }
    }
    for (cwd, folder_tasks) in folders {
        let folder = if cwd == "." {
            Path::new(project).file_name().map_or_else(
                || project.to_owned(),
                |name| name.to_string_lossy().into_owned(),
            )
        } else {
            cwd.to_owned()
        };
        let first = folder_tasks[0];
        let tool = first["command"]
            .as_str()
            .and_then(|command| command.split(' ').next())
            .and_then(|program| match program {
                "./mvnw" => Some("Maven wrapper"),
                "mvn" => Some("Maven"),
                "./gradlew" => Some("Gradle wrapper"),
                "gradle" => Some("Gradle"),
                "go" => Some("Go"),
                _ => None,
            })
            .map(str::to_owned);
        let java = &first["java"];
        let java = java.is_object().then(|| match &java["runsOn"] {
            Value::Null => format!("needs Java {}", plain(&java["required"])),
            runs_on => format!("Java {}", plain(runs_on)),
        });
        let meta: Vec<String> = tool.into_iter().chain(java).collect();
        let suffix = format!(" ({folder})");
        groups.push(TaskGroup {
            tasks: folder_tasks
                .iter()
                .map(|task| {
                    let label = text(task, "label");
                    let short = match cwd {
                        "." => label.as_str(),
                        _ => label.strip_suffix(&suffix).unwrap_or(&label),
                    };
                    (text(task, "id"), short.to_owned())
                })
                .collect(),
            title: folder,
            meta: (!meta.is_empty()).then(|| meta.join(" · ")),
        });
    }
    groups
}

/// A number or a string as plain text.
fn plain(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn strip_tab(
    bench: &Workbench,
    strip: &Strip,
    tab: &LayoutTab,
    active: bool,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let tab_id = tab.id.clone();
    let base = Tab::new(
        SharedString::from(format!("tab-{}", tab.id)),
        label(bench, tab),
    )
    .active(active)
    .on_click(cx.listener({
        let (strip, tab) = (strip.clone(), tab_id.clone());
        move |bench, _, _, _| bench.activate_tab(&strip, &tab)
    }))
    .on_close(cx.listener({
        let strip = strip.clone();
        move |bench, _, _, _| bench.close_tab(&strip, &tab_id)
    }));
    let view = first_view(&tab.pane);
    let drawn = match view {
        Some((PaneKind::Agent, target)) => match match strip {
            Strip::Project(project) => bench.agents_of(project),
            Strip::Host(_) => Vec::new(),
        }
        .into_iter()
        .find(|agent| &agent.id == target)
        {
            Some(agent) => base
                .mark(ui::agent_mark(&agent, 19., cx))
                .note(
                    agent
                        .name
                        .as_ref()
                        .map(|name| SharedString::from(format!("@{name}"))),
                )
                .tail(ui::status_mark(agent.status, cx)),
            None => base,
        },
        Some((kind, _)) => base.mark(ui::icon(kind_icon(kind), 13.).text_color(
            if kind == PaneKind::Terminal {
                t.live
            } else {
                t.text_tertiary
            },
        )),
        None => base,
    }
    .into_any_element();
    let index = bench
        .tabs(strip)
        .iter()
        .position(|other| other.id == tab.id)
        .unwrap_or(0);
    let dragged = crate::drag::DraggedTab {
        strip: strip.clone(),
        tab: tab.id.clone(),
        label: label(bench, tab),
    };
    crate::drag::tab(drawn, dragged, index, &cx.entity(), cx).into_any_element()
}

/// A tab's name: the person's, else what its first view shows.
fn label(bench: &Workbench, tab: &LayoutTab) -> SharedString {
    if let Some(title) = &tab.title {
        return title.clone().into();
    }
    match first_view(&tab.pane) {
        Some((PaneKind::Agent, target)) => bench
            .view
            .as_ref()
            .and_then(|view| {
                view.workspace
                    .agents
                    .iter()
                    .find(|agent| &agent.id == target)
            })
            .map_or_else(|| "Agent".into(), |agent| agent.title.clone().into()),
        Some((PaneKind::Terminal, target)) => bench
            .session(target)
            .and_then(|session| {
                session
                    .window_title
                    .clone()
                    .or_else(|| session.title.clone())
            })
            .map_or_else(|| "Terminal".into(), SharedString::from),
        Some((kind, _)) => kind_label(kind).into(),
        None => "".into(),
    }
}

pub fn first_view(pane: &Pane) -> Option<(PaneKind, &String)> {
    match pane {
        Pane::View { kind, target, .. } => Some((*kind, target)),
        Pane::Split { panes, .. } => panes.iter().find_map(first_view),
    }
}

pub fn kind_icon(kind: PaneKind) -> Icon {
    match kind {
        PaneKind::Agent => Icon::Agent,
        PaneKind::Terminal => Icon::Terminal,
        PaneKind::Files => Icon::Editor,
        PaneKind::Git => Icon::Git,
        PaneKind::Search => Icon::Search,
        PaneKind::Board => Icon::Board,
        PaneKind::Browser => Icon::Globe,
        PaneKind::Plugin => Icon::Window,
        PaneKind::Desk => Icon::PanelRight,
    }
}

fn kind_label(kind: PaneKind) -> &'static str {
    match kind {
        PaneKind::Agent => "Agent",
        PaneKind::Terminal => "Terminal",
        PaneKind::Files => "Files",
        PaneKind::Git => "Git",
        PaneKind::Search => "Search",
        PaneKind::Board => "Agent board",
        PaneKind::Browser => "Browser",
        PaneKind::Plugin => "Plugin",
        PaneKind::Desk => "Desk",
    }
}

fn clock(cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx);
    let now = chrono::Local::now();
    div()
        .flex()
        .items_baseline()
        .font_features(crate::theme::tabular())
        .child(
            ui::text(now.format("%-I:%M").to_string(), 12., FontWeight::SEMIBOLD)
                .text_color(t.text_secondary),
        )
        .child(
            ui::text(now.format("%p").to_string(), 10., FontWeight::SEMIBOLD)
                .letter_spacing(0.06)
                .ml(px(4.))
                .text_color(t.text_tertiary),
        )
}
