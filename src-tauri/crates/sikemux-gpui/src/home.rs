use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use gpui_kit::base::Popover;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, Anchor, AnyElement, App, Context, Div, FontWeight, Hsla, IntoElement, ParentElement,
    SharedString, Styled, Window,
};
use serde_json::{json, Value};
use sikemux_wire::protocol::PaneKind;

use crate::core::Ask;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, IconButton, Menu, MenuItem, PaneHeader, Segmented, Toggle};
use crate::workbench::{AgentRow, Workbench};
use crate::{chat, tabs, terminal};

const WEEKS: i64 = 53;
const CELL_GAP: f32 = 3.;

/// The calendar's metrics, as the activity summary names them, in the order its buttons show.
pub const METRICS: [(&str, &str); 5] = [
    ("tokens", "Tokens"),
    ("costUsd", "Cost"),
    ("commits", "Commits"),
    ("agentMs", "Agent time"),
    ("sessions", "Sessions"),
];

/// The stage: the chosen agent's pane, on what a new chat opens on: the project, and
/// what its agents did in it.
pub fn stage(bench: &Workbench, window: &Window, cx: &mut Context<Workbench>) -> AnyElement {
    if let Some((PaneKind::Terminal, session)) =
        bench.tab().and_then(|tab| tabs::first_view(&tab.pane))
    {
        let pane = ui::pane(cx).child(terminal::pane(bench, &cx.entity(), session, window));
        return div().flex_1().min_w_0().child(pane).into_any_element();
    }
    let project = bench.project();
    let agent = bench.agent();
    let header = agent
        .as_ref()
        .zip(project)
        .map(|(agent, project)| pane_header(bench, agent, &project.id));
    if let Some((agent, chat)) = agent
        .as_ref()
        .and_then(|agent| Some((agent, bench.chats.get(&agent.id)?)))
    {
        let welcome = chat
            .is_empty(cx)
            .then(|| welcome(bench, Some(agent), cx).into_any_element());
        let pane = ui::pane(cx)
            .child(chat::pane(bench, agent, chat, welcome, cx))
            .children(header);
        return div().flex_1().min_w_0().child(pane).into_any_element();
    }
    div()
        .flex_1()
        .min_w_0()
        .child(
            ui::pane(cx)
                .child(div().size_full().flex().justify_center().child(welcome(
                    bench,
                    agent.as_ref(),
                    cx,
                )))
                .children(header),
        )
        .into_any_element()
}

/// What a chat opens on before its first message: the agent's face, the project, and what
/// its agents did in it.
pub fn welcome(
    bench: &Workbench,
    agent: Option<&AgentRow>,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let project = bench.project();
    div()
        .w_full()
        .max_w(px(756.))
        .mx(px(22.))
        .pt(px(70.))
        .flex()
        .flex_col()
        .gap(px(32.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .child(div().mb(px(6.)).child(match agent {
                    Some(agent) => ui::agent_mark(agent, 40., cx),
                    None => ui::mark("claude", 40.),
                }))
                .child(
                    ui::text(
                        project
                            .map(|project| project.name.clone())
                            .unwrap_or_default(),
                        16.,
                        FontWeight::SEMIBOLD,
                    )
                    .text_color(t.text_primary),
                ),
        )
        .children(bench.read("activity").map(|summary| {
            div()
                .flex()
                .flex_col()
                .gap(px(24.))
                .when(!is_empty(&summary["totals"]), |activity| {
                    activity.child(overview(&summary["totals"], cx))
                })
                .child(calendar(summary, bench.metric, cx))
        }))
}

fn pane_header(bench: &Workbench, agent: &AgentRow, project: &str) -> impl IntoElement {
    let asks = bench.asks.clone();
    let tab = bench.tab().map(|tab| tab.id.clone()).unwrap_or_default();
    let close = json!({ "project": project, "tab": tab });
    let menu = Popover::new("agent-menu")
        .anchor(Anchor::TopLeft)
        .offset(px(4.))
        .trigger(
            IconButton::new("agent-more", Icon::More, "More")
                .round()
                .glyph(14.),
        )
        .content(move |_, _, cx| {
            let popover = cx.entity();
            Menu::new().child(
                MenuItem::new("close-agent", "Close")
                    .icon(Icon::Close)
                    .on_click(move |_, window, cx| {
                        let _ = asks.send(Ask::Call {
                            method: "closeTab",
                            params: close.clone(),
                        });
                        popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    }),
            )
        });
    let views = vec![
        (Icon::Agent, SharedString::from("GUI")),
        (Icon::Terminal, SharedString::from("TUI")),
    ];
    PaneHeader::new(agent.title.clone())
        .mark(ui::mark(&agent.provider, 16.))
        .name(
            agent
                .name
                .as_ref()
                .map(|name| SharedString::from(format!("@{name}"))),
        )
        .action(IconButton::new("agent-share", Icon::Share, "Share").round())
        .action(menu)
        .controls(Segmented::new("agent-view", views, 0).pill())
        .side(IconButton::new("agent-desk", Icon::PanelRight, "Open the desk").glyph(13.))
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.)
}

fn is_empty(totals: &Value) -> bool {
    number(&totals["sessions"]) == 0.
        && number(&totals["turns"]) == 0.
        && number(&totals["commits"]) == 0.
}

fn overview(totals: &Value, cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let tokens =
        number(&totals["input"]) + number(&totals["cacheWrite"]) + number(&totals["output"]);
    let cost = number(&totals["costUsd"]);
    let saved = number(&totals["cacheSavingsUsd"]);
    let stats = [
        (
            "Agent time",
            None,
            duration(number(&totals["agentMs"])),
            plural(number(&totals["turns"]), "turn"),
        ),
        (
            "Sessions",
            None,
            whole(number(&totals["sessions"])),
            format!("{} resumed", whole(number(&totals["resumed"]))),
        ),
        (
            "Tokens",
            None,
            compact(tokens),
            format!("{} cached", compact(number(&totals["cacheRead"]))),
        ),
        (
            "Cost",
            Some("API est."),
            if cost > 0. { money(cost) } else { "—".into() },
            if saved > 0. {
                format!("{} saved", money(saved))
            } else {
                "not priced yet".into()
            },
        ),
        (
            "Commits",
            None,
            whole(number(&totals["commits"])),
            format!("{} by agents", whole(number(&totals["agentCommits"]))),
        ),
    ];
    div().flex().children(stats.into_iter().enumerate().map(
        |(index, (label, badge, value, detail))| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .py(px(4.))
                .px(px(16.))
                .when(index == 0, |stat| stat.pl_0())
                .when(index > 0, |stat| stat.border_l_1().border_color(t.border))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_color(t.text_tertiary)
                        .child(ui::text(label, 12., FontWeight::MEDIUM))
                        .children(badge.map(|badge| ui::text(badge, 10., FontWeight::MEDIUM))),
                )
                .child(
                    ui::text(value, 20., FontWeight::SEMIBOLD)
                        .line_height(px(22.))
                        .font_features(theme::tabular())
                        .text_color(t.text_primary),
                )
                .child(ui::mono(detail, 11., cx).overflow_hidden().text_ellipsis())
        },
    ))
}

fn calendar(summary: &Value, metric: usize, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let key = METRICS[metric].0;
    let today = (chrono::Local::now().date_naive() - epoch()).num_days();
    let start = today - (today + 3).rem_euclid(7) - (WEEKS - 1) * 7;
    let levels: HashMap<i64, u64> = summary["days"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|day| {
            (
                day["day"].as_i64().unwrap_or(0),
                day["levels"][key].as_u64().unwrap_or(0),
            )
        })
        .collect();
    let colour = |level: u64| match level {
        0 => Hsla {
            a: 0.07,
            ..t.text_primary
        },
        1 => Hsla {
            a: 0.30,
            ..t.accent
        },
        2 => Hsla {
            a: 0.52,
            ..t.accent
        },
        3 => Hsla {
            a: 0.76,
            ..t.accent
        },
        _ => t.accent,
    };
    let weeks = (0..WEEKS).map(|week| {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(CELL_GAP))
            .children((0..7).map(|weekday| {
                let day = start + week * 7 + weekday;
                div()
                    .w_full()
                    .aspect_ratio(1.)
                    .rounded(px(2.))
                    .when(day <= today, |cell| {
                        cell.bg(colour(levels.get(&day).copied().unwrap_or(0)))
                    })
            }))
    });
    let mut last_month = None;
    let months: Vec<Div> = (0..WEEKS)
        .map(|week| {
            let first = epoch() + chrono::Duration::days(start + week * 7);
            let label = (last_month != Some(first.month()) && week <= WEEKS - 3)
                .then(|| first.format("%b").to_string());
            last_month = Some(first.month());
            div()
                .flex_1()
                .relative()
                .children(label.map(|label| small(label, &t).absolute().bottom_0().left_0()))
        })
        .collect();
    let streaks = &summary["streaks"];
    let toggles: Vec<_> = METRICS
        .iter()
        .enumerate()
        .map(|(index, (_, label))| {
            Toggle::new(("metric", index), *label, index == metric).on_click(cx.listener(
                move |bench, _, _, cx| {
                    bench.metric = index;
                    cx.notify();
                },
            ))
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .min_h(px(24.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    ui::text(readout(streaks), 12., FontWeight::NORMAL)
                        .text_color(t.text_secondary),
                )
                .child(div().flex().gap(px(4.)).children(toggles)),
        )
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .pt(px(14. + CELL_GAP))
                        .gap(px(CELL_GAP))
                        .children(["Mon", "", "Wed", "", "Fri", "", ""].map(|label| {
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .child(small(label.into(), &t))
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(CELL_GAP))
                        .child(div().h(px(14.)).flex().gap(px(CELL_GAP)).children(months))
                        .child(div().flex().gap(px(CELL_GAP)).children(weeks)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(CELL_GAP))
                .child(small("Less".into(), &t).mr(px(4.)))
                .children((0..5).map(|level| div().size(px(10.)).rounded(px(2.)).bg(colour(level))))
                .child(small("More".into(), &t).ml(px(4.))),
        )
}

fn small(label: String, t: &Tokens) -> Div {
    ui::text(label, 10., FontWeight::NORMAL)
        .line_height(px(10.))
        .text_color(t.text_tertiary)
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()
}

fn readout(streaks: &Value) -> String {
    let active = number(&streaks["activeDays"]);
    if active == 0. {
        return "No activity yet".into();
    }
    let current = number(&streaks["current"]);
    let mut readout = format!(
        "{} · longest streak {}",
        plural(active, "active day"),
        plural(number(&streaks["longest"]), "day")
    );
    if current > 1. {
        readout.push_str(&format!(" · {current} days running"));
    }
    readout
}

fn whole(value: f64) -> String {
    let digits = format!("{}", value.round() as i64);
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 && digit != '-' {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

fn plural(count: f64, word: &str) -> String {
    format!(
        "{} {word}{}",
        whole(count),
        if count == 1. { "" } else { "s" }
    )
}

pub fn compact(value: f64) -> String {
    let (scaled, unit) = match value {
        v if v >= 1e9 => (v / 1e9, "B"),
        v if v >= 1e6 => (v / 1e6, "M"),
        v if v >= 1e3 => (v / 1e3, "K"),
        v => (v, ""),
    };
    let rounded = (scaled * 10.).round() / 10.;
    if rounded.fract() == 0. {
        format!("{rounded:.0}{unit}")
    } else {
        format!("{rounded:.1}{unit}")
    }
}

pub fn money(usd: f64) -> String {
    if usd >= 1000. {
        format!("${}", whole(usd))
    } else {
        format!("${usd:.2}")
    }
}

fn duration(ms: f64) -> String {
    let minutes = (ms / 60_000.).round() as i64;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours >= 100 {
        return format!("{}h", whole(hours as f64));
    }
    match minutes % 60 {
        0 => format!("{hours}h"),
        rest => format!("{hours}h {rest}m"),
    }
}
