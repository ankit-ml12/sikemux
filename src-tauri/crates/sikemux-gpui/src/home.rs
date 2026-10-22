use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use gpui_kit::component::button::{Button as KitButton, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, Anchor, AnyElement, App, Context, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
};
use serde_json::Value;
use sikemux_wire::protocol::AgentStatus;

use crate::agent_menu::AgentMenu;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, Button, IconButton, PaneHeader, Segmented, Toggle, Variant};
use crate::workbench::{AgentRow, Workbench};
use crate::{chat, terminal};

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

/// The stage: the tab in front, its views side by side, or what a project with nothing
/// open says.
pub fn stage(bench: &Workbench, window: &Window, cx: &mut Context<Workbench>) -> AnyElement {
    if bench.settings.is_some() {
        return crate::settings::view(bench, window, cx);
    }
    if bench
        .strip()
        .is_some_and(|strip| bench.tabs(&strip).is_empty())
    {
        return div()
            .flex_1()
            .min_w_0()
            .child(ui::pane(cx).child(empty_stage(cx)))
            .into_any_element();
    }
    let shown = match bench.tab() {
        Some(tab) => crate::track::stage(bench, tab, window, cx),
        None => ui::pane(cx)
            .child(
                div()
                    .size_full()
                    .flex()
                    .justify_center()
                    .child(welcome(bench, None, cx)),
            )
            .into_any_element(),
    };
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .child(shown)
        .into_any_element()
}

/// One agent's view of a tab: its terminal or its chat under its header, or what a new
/// chat opens on.
pub fn agent_view(
    bench: &Workbench,
    tab: &str,
    view: &str,
    agent: &str,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let project = bench.project();
    let agent = project.and_then(|project| {
        bench
            .agents_of(&project.id)
            .into_iter()
            .find(|row| row.id == agent)
    });
    let header = agent
        .as_ref()
        .filter(|_| project.is_some())
        .map(|agent| pane_header(bench, agent, tab, view, cx));
    if let Some(session) = agent
        .as_ref()
        .and_then(|agent| bench.agent_terminal(&agent.id))
    {
        return ui::pane(cx)
            .child(div().size_full().pt(px(40.)).child(terminal::pane(
                bench,
                &cx.entity(),
                &session.to_string(),
                window,
            )))
            .children(header)
            .into_any_element();
    }
    if let Some((agent, chat)) = agent
        .as_ref()
        .and_then(|agent| Some((agent, bench.chats.get(&agent.id)?)))
    {
        let welcome = chat
            .is_empty(cx)
            .then(|| welcome(bench, Some(agent), cx).into_any_element());
        return ui::pane(cx)
            .child(chat::pane(bench, agent, chat, welcome, cx))
            .children(header)
            .into_any_element();
    }
    ui::pane(cx)
        .child(
            div()
                .size_full()
                .flex()
                .justify_center()
                .child(welcome(bench, agent.as_ref(), cx)),
        )
        .children(header)
        .into_any_element()
}

/// A project with every tab closed, which would otherwise show nothing to click.
fn empty_stage(cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let hint = |id: &str| crate::keymap::hint(id, cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .text_color(t.text_tertiary)
        .child(
            ui::text("Nothing open in this project", 12.5, FontWeight::NORMAL)
                .text_color(t.text_secondary),
        )
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(
                    Button::new("empty-new-agent", "New agent", Variant::Outline)
                        .keys(hint("agent.new"))
                        .on_click(cx.listener(|bench, _, _, _| bench.new_chat())),
                )
                .child(
                    Button::new("empty-new-terminal", "New terminal", Variant::Outline)
                        .keys(hint("terminal.new"))
                        .on_click(cx.listener(|bench, _, _, _| bench.new_terminal())),
                )
                .child(
                    Button::new("empty-open-file", "Open file", Variant::Outline)
                        .keys(hint("palette.files"))
                        .on_click(cx.listener(|bench, _, window, cx| {
                            bench.toggle_finder(window, cx);
                            cx.notify();
                        })),
                ),
        )
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
                .child(calendar(summary, bench, cx))
        }))
}

fn pane_header(
    bench: &Workbench,
    agent: &AgentRow,
    tab: &str,
    view: &str,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let menu = AgentMenu::new(bench, &agent.id, cx);
    let more = KitButton::new("agent-more")
        .ghost()
        .compact()
        .rounded_full()
        .w(px(24.))
        .h(px(24.))
        .tooltip("More")
        .child(ui::icon(Icon::More, 14.).text_color(t.text_secondary))
        .dropdown_menu_with_anchor(Anchor::TopRight, move |popup, window, cx| match &menu {
            Some(menu) => menu.build(popup, window, cx),
            None => popup,
        });
    let desk_open = bench.desk_of(tab, &agent.id).is_some();
    let (tab, view, agent_id) = (tab.to_owned(), view.to_owned(), agent.id.clone());
    let views = vec![
        (Icon::Agent, SharedString::from("GUI")),
        (Icon::Terminal, SharedString::from("TUI")),
    ];
    let in_terminal = bench.agent_terminal(&agent.id).is_some();
    let working = matches!(agent.status, AgentStatus::Working | AgentStatus::Blocked);
    let switched = agent.id.clone();
    let shared = agent.id.clone();
    let open_share = cx.listener(move |bench, _, window, cx| bench.open_share(&shared, window, cx));
    let share = match bench.share_of(&agent.id) {
        Some(share) => {
            let viewers = share.viewers();
            div()
                .id("agent-share")
                .h(px(24.))
                .min_w(px(24.))
                .px(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(3.))
                .rounded_full()
                .text_size(px(12.))
                .text_color(t.live)
                .tooltip(ui::tooltip(share.watching()))
                .on_click(open_share)
                .child(ui::icon(Icon::Eye, 13.).text_color(t.live))
                .when(viewers > 0, |button| button.child(viewers.to_string()))
                .into_any_element()
        }
        None => IconButton::new("agent-share", Icon::Share, "Share this chat read-only")
            .round()
            .on_click(open_share)
            .into_any_element(),
    };
    let id = agent.id.clone();
    let editing = bench
        .retitling
        .as_ref()
        .filter(|retitling| retitling.agent == agent.id)
        .map(|retitling| {
            div()
                .key_context("Retitle")
                .min_w(px(160.))
                .on_action(
                    cx.listener(|bench, _: &crate::keymap::CancelRetitle, _, cx| {
                        bench.finish_retitle(false, cx)
                    }),
                )
                .child(
                    Input::new(&retitling.input)
                        .appearance(false)
                        .p_0()
                        .text_size(px(13.))
                        .text_color(t.text_primary),
                )
        });
    PaneHeader::new(agent.title.clone())
        .title_view(editing)
        .on_title_double_click(
            cx.listener(move |bench, _, window, cx| bench.retitle(&id, window, cx)),
        )
        .mark(ui::mark(&agent.provider, 16.))
        .name(
            agent
                .name
                .as_ref()
                .map(|name| SharedString::from(format!("@{name}"))),
        )
        .action(share)
        .action(more)
        .controls(
            div().flex().items_center().gap(px(4.)).child(
                Segmented::new("agent-view", views, usize::from(in_terminal))
                    .pill()
                    .disabled_because(working.then(|| {
                        let other = if in_terminal { "GUI" } else { "TUI" };
                        format!("Wait for the agent to finish before opening {other}")
                    }))
                    .on_select(cx.listener(move |bench, picked: &usize, _, _| {
                        let terminal = *picked == 1;
                        if terminal != in_terminal {
                            bench.switch_view(&switched, terminal);
                        }
                    })),
            ),
        )
        .side(
            IconButton::new(
                "agent-desk",
                Icon::PanelRight,
                if desk_open {
                    "Close the desk"
                } else {
                    "Open the desk"
                },
            )
            .glyph(13.)
            .on_click(cx.listener(move |bench, _, _, _| bench.toggle_desk(&tab, &view, &agent_id))),
        )
}

pub(crate) fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.)
}

pub(crate) fn is_empty(totals: &Value) -> bool {
    number(&totals["sessions"]) == 0.
        && number(&totals["turns"]) == 0.
        && number(&totals["commits"]) == 0.
}

pub(crate) fn overview(totals: &Value, cx: &App) -> impl IntoElement {
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

pub(crate) fn calendar(
    summary: &Value,
    bench: &Workbench,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let metric = bench.metric;
    let key = METRICS[metric].0;
    let today = (chrono::Local::now().date_naive() - epoch()).num_days();
    let start = today - (today + 3).rem_euclid(7) - (WEEKS - 1) * 7;
    let days: HashMap<i64, &Value> = summary["days"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|day| (day["day"].as_i64().unwrap_or(0), day))
        .collect();
    let level = |day: i64| {
        days.get(&day)
            .and_then(|day| day["levels"][key].as_u64())
            .unwrap_or(0)
    };
    let hovered = bench.metric_day.filter(|day| *day <= today);
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
                    .id(("activity-day", day as usize))
                    .w_full()
                    .aspect_ratio(1.)
                    .rounded(px(2.))
                    .when(day <= today, |cell| {
                        cell.bg(colour(level(day)))
                            .when(hovered == Some(day), |cell| {
                                cell.border_1().border_color(t.text_primary)
                            })
                            .on_hover(cx.listener(move |bench, over: &bool, _, cx| {
                                match (*over, bench.metric_day == Some(day)) {
                                    (true, _) => bench.metric_day = Some(day),
                                    (false, true) => bench.metric_day = None,
                                    (false, false) => return,
                                }
                                bench.parts.redraw(&bench.parts.stage, cx);
                            }))
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
                    ui::text(
                        match hovered {
                            Some(day) => format!(
                                "{} · {}",
                                (epoch() + chrono::Duration::days(day)).format("%a, %b %-d, %Y"),
                                metric_text(
                                    key,
                                    days.get(&day).map_or(0., |day| number(&day[key]))
                                )
                            ),
                            None => readout(streaks),
                        },
                        12.,
                        FontWeight::NORMAL,
                    )
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

/// A day's measure as the calendar reads it out.
fn metric_text(key: &str, value: f64) -> String {
    match (key, value > 0.) {
        ("agentMs", true) => format!("{} of agent time", duration(value)),
        ("agentMs", false) => "No agent time".into(),
        ("tokens", true) => format!("{} tokens", compact(value)),
        ("tokens", false) => "No tokens".into(),
        ("sessions", true) => plural(value, "session"),
        ("sessions", false) => "No sessions".into(),
        ("costUsd", true) => format!("{} at API prices", money(value)),
        ("costUsd", false) => "No cost".into(),
        (_, true) => plural(value, "commit"),
        (_, false) => "No commits".into(),
    }
}

pub(crate) fn whole(value: f64) -> String {
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

pub(crate) fn plural(count: f64, word: &str) -> String {
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

pub(crate) fn duration(ms: f64) -> String {
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
