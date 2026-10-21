use std::f32::consts::FRAC_PI_2;

use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    canvas, div, fill, point, px, radians, size, Transformation, App, Bounds, Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};
use serde_json::Value;

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, Field, Group, IconButton, Row, Segmented};
use crate::workbench::{AgentRow, Workbench};

pub const TAB_RAIL_WIDTH: f32 = 288.;

pub fn tab_rail(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let project = bench.project();
    let agents = bench.listed_agents();
    let rows: Vec<_> = agents.iter().map(|agent| agent_row(bench, agent, cx).into_any_element()).collect();
    let recent: Vec<_> = project
        .map(|project| bench.recent_of(&project.id))
        .unwrap_or_default()
        .into_iter()
        .map(|recent| {
            Row::new(SharedString::from(format!("recent-{}", recent.id)), recent.title.clone())
                .mark(22., ui::mark(&recent.provider, 20.))
                .child(ui::mono(age(recent.active_at), 11., cx))
                .into_any_element()
        })
        .collect();
    let scope = vec![
        (Icon::Folder, SharedString::from(project.map(|project| project.name.clone()).unwrap_or_default())),
        (Icon::Stack, SharedString::from("All projects")),
    ];
    ui::panel(cx)
        .w(px(TAB_RAIL_WIDTH))
        .min_w(px(240.))
        .flex_none()
        .child(header(bench, cx))
        .children(bench.filter.as_ref().map(|filter| {
            div().flex_none().mt(px(10.)).mx(px(12.)).mb(px(2.)).child(Field::new(filter).icon(Icon::Search))
        }))
        .child(
            div()
                .flex_none()
                .pt(px(4.))
                .px(px(10.))
                .child(new_row("new-chat", Icon::Plus, "New chat", "⌘N", cx))
                .child(new_row("new-terminal", Icon::Terminal, "New terminal", "⌘T", cx))
                .child(new_row("new-browser", Icon::Globe, "New browser", "⌘⇧T", cx))
                .child(
                    div().mt(px(8.)).child(
                        Segmented::new("scope", scope, usize::from(bench.all_projects)).on_select(
                            cx.listener(|bench, all: &usize, _, cx| {
                                bench.all_projects = *all == 1;
                                cx.notify();
                            }),
                        ),
                    ),
                ),
        )
        .child(
            div()
                .id("tab-rail-scroll")
                .flex_1()
                .min_h_0()
                .pt(px(8.))
                .px(px(10.))
                .pb(px(16.))
                .overflow_y_scrollbar()
                .child(div().flex().flex_col().gap(px(2.)).children(rows))
                .when(!recent.is_empty(), |scroll| scroll.child(Group::new("Recent", "").children(recent))),
        )
        .children(limits(bench, cx))
        .child(pages(cx))
}

fn header(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let (provider, label) = bench
        .launcher().map_or(("", "Agents"), |launcher| (launcher.provider.as_str(), launcher.label.as_str()));
    div()
        .flex_none()
        .pt(px(6.))
        .pr(px(6.))
        .pb(px(6.))
        .pl(px(10.))
        .border_b_1()
        .border_color(t.line_soft)
        .child(
            div()
                .min_h(px(28.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .child(ui::mark(provider, 16.))
                        .child(ui::text(label.to_owned(), 13., FontWeight::SEMIBOLD).text_color(t.text_primary)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(2.))
                        .child(
                            IconButton::new("search", Icon::Search, "Filter recent chats")
                                .pressed(bench.filter.is_some())
                                .on_click(cx.listener(|bench, _, window, cx| {
                                    bench.toggle_filter(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(IconButton::new("start-agent", Icon::Plus, "New chat"))
                        .child(IconButton::new("hide-tab-rail", Icon::PanelRight, "Hide the agent rail")),
                ),
        )
}

fn new_row(id: &'static str, glyph: Icon, label: &'static str, keys: &'static str, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    Row::new(id, label)
        .mark(22., ui::icon(glyph, 15.).text_color(t.text_secondary).group_hover(ui::ROW, move |style| style.text_color(t.text_primary)))
        .quiet()
        .child(ui::kbd(keys, cx))
}

fn agent_row(bench: &Workbench, agent: &AgentRow, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let project = bench
        .all_projects
        .then(|| bench.projects().iter().find(|project| project.id == agent.project))
        .flatten()
        .map(|project| project.name.clone());
    let chosen = bench.agent().is_some_and(|front| front.id == agent.id);
    let (id, home) = (agent.id.clone(), agent.project.clone());
    Row::new(SharedString::from(format!("agent-{}", agent.id)), agent.title.clone())
        .mark(22., ui::mark(&agent.provider, 20.))
        .chosen(chosen)
        .when_some(project, |row, project| {
            row.subtitle(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .line_height(px(14.))
                    .child(ui::icon(Icon::Folder, 11.).text_color(t.live))
                    .child(ui::meta(project, cx)),
            )
        })
        .status(ui::status_mark(agent.status, cx))
        .on_click(cx.listener(move |bench, _, _, _| {
            if let Some(agent) = bench.agents_of(&home).into_iter().find(|agent| agent.id == id) {
                bench.activate_agent(&agent);
            }
        }))
        .children(agent.name.clone().map(|name| ui::meta(format!("@{name}"), cx).pl(px(8.)).pr(px(4.))))
        .children(counts(agent, cx))
}

fn counts(agent: &AgentRow, cx: &App) -> Option<impl IntoElement> {
    let t = Tokens::get(cx).clone();
    (agent.subagents > 0 || agent.tasks > 0).then(|| {
        let count = |glyph: Icon, size: f32, color, value: u32| {
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .child(ui::icon(glyph, size).text_color(color))
                .child(ui::text(value.to_string(), 11., FontWeight::MEDIUM).font_features(crate::theme::tabular()))
        };
        div()
            .flex()
            .gap(px(6.))
            .text_color(t.text_secondary)
            .when(agent.subagents > 0, |row| row.child(count(Icon::Agent, 10., t.live, agent.subagents)))
            .when(agent.tasks > 0, |row| row.child(count(Icon::Command, 11., t.warn, agent.tasks)))
    })
}

/// How long ago, in a tight column: `now`, `5m`, `3h`, `4d`, `2mo`, `1y`.
fn age(at: u64) -> String {
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let seconds = now.saturating_sub(at) / 1000;
    match seconds {
        0..60 => "now".into(),
        60..3600 => format!("{}m", seconds / 60),
        3600..86_400 => format!("{}h", seconds / 3600),
        86_400..2_592_000 => format!("{}d", seconds / 86_400),
        2_592_000..31_536_000 => format!("{}mo", seconds / 2_592_000),
        _ => format!("{}y", seconds / 31_536_000),
    }
}

/// The plan limits of the account new chats here start on, as its agent last reported them.
fn limits(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<impl IntoElement> {
    let t = Tokens::get(cx).clone();
    let launcher = bench.launcher()?;
    let account = bench.read("usage")?.as_array()?.iter().find(|account| account["launcher"] == launcher.id.as_str())?;
    let name = account["label"].as_str().unwrap_or_default().to_owned();
    let refresh = format!("Refresh {} plan limits", launcher.label);
    let windows = account["usage"]["windows"].as_array().map_or(&[][..], Vec::as_slice);
    let quiet = |text: String| ui::text(text, 10.5, FontWeight::NORMAL).italic().text_color(t.text_tertiary).px(px(8.));
    let body = if account["signedIn"] == false {
        div()
            .flex()
            .gap(px(4.))
            .child(quiet(format!("{name} is signed out.")).pr_0())
            .child(ui::text("Sign in", 10.5, FontWeight::NORMAL).text_color(t.text_secondary).underline())
            .into_any_element()
    } else if !account["problem"].is_null() {
        quiet("Could not read plan limits. Refresh to try again.".into()).into_any_element()
    } else if windows.is_empty() {
        let reason = account["usage"]["unavailableReason"].as_str().unwrap_or("This account did not report plan limits.");
        quiet(reason.to_owned()).into_any_element()
    } else {
        div().flex().flex_col().gap(px(6.)).children(windows.iter().map(|window| limit_row(window, cx))).into_any_element()
    };
    Some(
        div()
            .flex_none()
            .mt(px(12.))
            .mx(px(10.))
            .mb(px(16.))
            .child(
                div()
                    .h(px(18.))
                    .mb(px(9.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::text("LIMITS", 11., FontWeight::MEDIUM).letter_spacing(0.08).text_color(t.text_tertiary))
                    .child(div().flex_1().h(px(1.)).bg(t.border))
                    .child(
                        div()
                            .h(px(20.))
                            .pl(px(3.))
                            .pr(px(5.))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .rounded(theme::RADIUS_2)
                            .child(ui::text(name, 11., FontWeight::MEDIUM).text_color(t.text_primary))
                            .child(ui::icon(Icon::Chevron, 9.).with_transformation(Transformation::rotate(radians(FRAC_PI_2))).text_color(t.text_tertiary)),
                    )
                    .child(
                        IconButton::new("refresh-limits", Icon::Refresh, refresh)
                            .small()
                            .on_click(cx.listener(|bench, _, _, _| bench.read_usage())),
                    ),
            )
            .child(body),
    )
}

/// One of a plan's rolling windows: how much of it is used, and when it starts again.
fn limit_row(window: &Value, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let label = window["label"].as_str().unwrap_or_default().to_owned();
    let percent = window["usedPercent"].as_f64().unwrap_or(0.).clamp(0., 100.);
    let signal = if percent >= 90. {
        t.danger
    } else if percent >= 70. {
        t.warn
    } else {
        Hsla { a: 0.62, ..t.text_primary }
    };
    let reset = reset_at(&window["resetsAt"]);
    let note = match reset {
        Some(at) => format!("Resets {}", at.with_timezone(&chrono::Local).format("%-m/%-d/%Y, %-I:%M:%S %p")),
        None => "Reset time unavailable".into(),
    };
    let unlit = Hsla { a: 0.10, ..t.text_primary };
    div()
        .id(SharedString::from(format!("limit-{label}")))
        .h(px(18.))
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(10.))
        .tooltip(ui::tooltip(format!("{label}: {}% used. {note}", percent.round())))
        .child(ui::meta(label, cx).w(px(58.)).flex_none().overflow_hidden().text_ellipsis().whitespace_nowrap())
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let lit = bounds.size.width * (percent / 100.) as f32;
                    let mut x = px(0.);
                    while x < bounds.size.width {
                        let tick = Bounds::new(point(bounds.origin.x + x, bounds.origin.y), size(px(1.5), bounds.size.height));
                        window.paint_quad(fill(tick, if x < lit { signal } else { unlit }));
                        x += px(4.);
                    }
                },
            )
            .flex_1()
            .h(px(10.)),
        )
        .child(
            ui::text(format!("{}%", percent.round()), 11., FontWeight::NORMAL)
                .w(px(28.))
                .flex_none()
                .text_right()
                .font_features(theme::tabular())
                .text_color(if percent < 70. { t.text_primary } else { signal }),
        )
        .child(
            ui::text(countdown(reset), 10., FontWeight::NORMAL)
                .w(px(42.))
                .flex_none()
                .text_right()
                .font_features(theme::tabular())
                .text_color(t.text_tertiary),
        )
}

/// When a window starts again: Unix seconds, as a number or digits, or an ISO time.
fn reset_at(value: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    let seconds = value.as_i64().or_else(|| value.as_str().filter(|text| text.bytes().all(|byte| byte.is_ascii_digit())).and_then(|text| text.parse().ok()));
    match seconds {
        Some(seconds) => chrono::DateTime::from_timestamp(seconds, 0),
        None => chrono::DateTime::parse_from_rfc3339(value.as_str()?).ok().map(|at| at.to_utc()),
    }
}

fn countdown(reset: Option<chrono::DateTime<chrono::Utc>>) -> String {
    let Some(reset) = reset else {
        return "unknown".into();
    };
    let minutes = ((reset - chrono::Utc::now()).num_seconds().max(0) as f64 / 60.).ceil() as i64;
    let (hours, days) = (minutes / 60, minutes / 1440);
    let with = |big: i64, unit: &str, small: i64, small_unit: &str| match small {
        0 => format!("{big}{unit}"),
        small => format!("{big}{unit} {small}{small_unit}"),
    };
    match minutes {
        0 => "resetting now".into(),
        1..=59 => format!("{minutes}m"),
        _ if hours < 24 => with(hours, "h", minutes % 60, "m"),
        _ if days < 7 => with(days, "d", hours % 24, "h"),
        _ => reset.with_timezone(&chrono::Local).format("%b %-d").to_string(),
    }
}

fn pages(cx: &App) -> impl IntoElement {
    let border = Tokens::get(cx).border;
    div()
        .flex_none()
        .h(px(35.))
        .px(px(8.))
        .py(px(4.))
        .flex()
        .justify_center()
        .gap(px(4.))
        .border_t_1()
        .border_color(border)
        .child(IconButton::new("page-agents", Icon::Agents, "Agents").width(32.).pressed(true))
        .child(IconButton::new("page-terminals", Icon::Terminal, "Terminals").width(32.))
        .child(IconButton::new("page-files", Icon::File, "Files").width(32.))
}
