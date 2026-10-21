use std::f32::consts::FRAC_PI_2;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, radians, AnyElement, App, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Transformation, WindowControlArea,
};
use serde_json::Value;
use sikemux_wire::protocol::{Pane, PaneKind, Tab as LayoutTab};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, IconButton, Tab};
use crate::workbench::Workbench;

/// The bar over the stage: the project's tabs, then the clock.
pub fn top_bar(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let tabs: Vec<_> = bench
        .project()
        .map(|project| {
            let front = bench.tab().map(|tab| tab.id.clone());
            bench
                .tabs(&project.id)
                .iter()
                .map(|tab| strip_tab(bench, &project.id, tab, front.as_deref() == Some(tab.id.as_str()), cx))
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
                .child(div().h_full().min_w_0().overflow_hidden().px(px(4.)).flex().items_center().gap(px(4.)).children(tabs))
                .child(IconButton::new("new-tab", Icon::Plus, "New tab").glyph(13.)),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .pr(px(16.))
                .children(git_chip(bench, cx).map(|chip| [chip, separator(cx)]).into_iter().flatten())
                .children(run_chip(bench, cx).map(|chip| [chip, separator(cx)]).into_iter().flatten())
                .children(ports_chip(bench, cx).map(|chip| [chip, separator(cx)]).into_iter().flatten())
                .children(ci_glyph(bench, cx))
                .child(clock(cx))
                .child(IconButton::new("focus", Icon::Focus, "Focus mode — hide rails — ⌘B").width(30.)),
        )
}

fn separator(cx: &App) -> AnyElement {
    div().w(px(1.)).h(px(16.)).bg(Tokens::get(cx).line).into_any_element()
}

/// Where the project's repository stands: its branch, and how far it is from its upstream.
fn git_chip(bench: &Workbench, cx: &App) -> Option<AnyElement> {
    let t = Tokens::get(cx).clone();
    let project = bench.project()?;
    let git = project.git.as_ref()?;
    let host = project.code_host.as_ref();
    let mark = match host {
        Some(host) if host.host == "github" => ui::icon(Icon::Github, 12.).text_color(t.text_primary),
        _ => ui::icon(Icon::Git, 12.).text_color(if git.dirty { t.warn } else { t.live }),
    };
    let mut about: Vec<String> = host.map(|host| vec![host_label(&host.host).to_owned(), host.repo.clone()]).unwrap_or_default();
    about.push(git.branch.clone());
    about.push(if git.dirty { "changed".into() } else { "clean".into() });
    about.extend((git.ahead > 0).then(|| format!("ahead {}", git.ahead)));
    about.extend((git.behind > 0).then(|| format!("behind {}", git.behind)));
    let count = |arrow: &str, count: u32, color| {
        (count > 0).then(|| ui::text(format!("{arrow}{count}"), 10., FontWeight::NORMAL).text_color(color))
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
            .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(t.text_primary).child(git.branch.clone()))
            .when(git.ahead > 0 || git.behind > 0, |chip| {
                chip.child(div().flex().gap(px(4.)).children(count("↑", git.ahead, t.live)).children(count("↓", git.behind, t.warn)))
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
    let about = if count == 1 { "1 listening port".to_owned() } else { format!("{count} listening ports") };
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
    let ago = chrono::DateTime::parse_from_rfc3339(run["createdAt"].as_str()?).ok()?.timestamp_millis().max(0) as u64;
    let ago = match crate::agents::age(ago) {
        now if now == "now" => now,
        age => format!("{age} ago"),
    };
    let about = format!("{} · {} #{number} · {ago}", outcome_label(outcome), run["name"].as_str().unwrap_or_default());
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
            .child(ui::mono(format!("#{number}"), 11., cx).font_features(crate::theme::tabular()).text_color(t.text_secondary))
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
    let (running, last) = bench.person_runs();
    let task_of = |run: &Value| tasks.iter().find(|task| task["id"] == run["taskId"]);
    let selected = running.and_then(task_of).or(tasks.first())?;
    let id = selected["id"].as_str()?.to_owned();
    let label = selected["label"].as_str()?.to_owned();
    let last = last.filter(|run| running.is_none() && run["taskId"] == id.as_str());
    let failed = last.filter(|run| run["status"] == "failed").map(|run| match run["exitCode"].as_i64() {
        Some(code) => format!("exited {code}"),
        None => "failed".into(),
    });
    let status = match (running, &failed) {
        (Some(run), _) => {
            let since = run["startedAt"].as_i64().map_or(0, |at| (chrono::Utc::now().timestamp_millis() - at).max(0) / 1000);
            Some((format!("{since}s"), t.text_tertiary))
        }
        (None, Some(failed)) => Some((failed.clone(), t.danger)),
        _ => None,
    };
    let dot = match (running, &failed) {
        (Some(_), _) => Some(div().size(px(7.)).rounded_full().border(px(1.5)).border_color(t.warn)),
        (None, Some(_)) => Some(div().size(px(7.)).rounded_full().bg(t.danger)),
        _ => None,
    };
    let execution = running.and_then(|run| run["executionId"].as_str()).map(str::to_owned);
    let primary = if running.is_some() { format!("Stop {label}") } else { format!("Run {label}") };
    let (glyph, word, tint) = if running.is_some() { (Icon::Stop, "Stop", t.danger) } else { (Icon::Run, "Run", t.live) };
    Some(
        div()
            .h(px(24.))
            .flex()
            .rounded(crate::theme::RADIUS_2)
            .bg(Hsla { a: 0.94, ..t.surface })
            .border_1()
            .border_color(Hsla { a: 0.06, ..t.text_primary })
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
                div()
                    .id("choose-task")
                    .max_w(px(320.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .border_l_1()
                    .border_color(Hsla { a: 0.08, ..t.text_primary })
                    .font_family(crate::theme::MONO)
                    .text_size(px(11.))
                    .text_color(t.text_tertiary)
                    .hover(move |style| style.text_color(t.text_primary))
                    .children(dot)
                    .child(div().min_w_0().overflow_hidden().text_ellipsis().whitespace_nowrap().child(label))
                    .children(status.map(|(text, color)| div().text_color(color).child(format!("· {text}"))))
                    .child(ui::icon(Icon::Chevron, 10.).with_transformation(Transformation::rotate(radians(FRAC_PI_2))).text_color(t.text_tertiary)),
            )
            .into_any_element(),
    )
}

fn strip_tab(bench: &Workbench, project: &str, tab: &LayoutTab, active: bool, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let (project_id, tab_id) = (project.to_owned(), tab.id.clone());
    let base = Tab::new(SharedString::from(format!("tab-{}", tab.id)), label(bench, tab))
        .active(active)
        .on_click(cx.listener(move |bench, _, _, _| bench.activate_tab(&project_id, &tab_id)));
    let view = first_view(&tab.pane);
    match view {
        Some((PaneKind::Agent, target)) => match bench.agents_of(project).into_iter().find(|agent| &agent.id == target) {
            Some(agent) => base
                .mark(ui::agent_mark(&agent, 19., cx))
                .note(agent.name.as_ref().map(|name| SharedString::from(format!("@{name}"))))
                .tail(ui::status_mark(agent.status, cx)),
            None => base,
        },
        Some((kind, _)) => base.mark(ui::icon(kind_icon(kind), 13.).text_color(if kind == PaneKind::Terminal { t.live } else { t.text_tertiary })),
        None => base,
    }
    .into_any_element()
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
            .and_then(|view| view.workspace.agents.iter().find(|agent| &agent.id == target))
            .map_or_else(|| "Agent".into(), |agent| agent.title.clone().into()),
        Some((PaneKind::Terminal, target)) => bench
            .session(target)
            .and_then(|session| session.window_title.clone().or_else(|| session.title.clone()))
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
    }
}

fn clock(cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx);
    let now = chrono::Local::now();
    div()
        .flex()
        .items_baseline()
        .font_features(crate::theme::tabular())
        .child(ui::text(now.format("%-I:%M").to_string(), 12., FontWeight::SEMIBOLD).text_color(t.text_secondary))
        .child(ui::text(now.format("%p").to_string(), 10., FontWeight::SEMIBOLD).letter_spacing(0.06).ml(px(4.)).text_color(t.text_tertiary))
}
