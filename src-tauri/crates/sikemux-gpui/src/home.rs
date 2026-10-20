use std::collections::{BTreeSet, HashMap};

use chrono::{Datelike, NaiveDate};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{div, px, rgba, Context, Div, FontWeight, IntoElement, ParentElement, Rgba, Styled};
use serde_json::Value;

use crate::theme;
use crate::ui::{agent_mark, text};
use crate::workbench::Workbench;

const WEEKS: i64 = 53;
const CELL_GAP: f32 = 3.;

/// What a new chat opens on: the project, and what its agents did in it.
pub fn stage(bench: &Workbench, _cx: &mut Context<Workbench>) -> impl IntoElement {
    let project = bench.project();
    let provider = bench
        .agent
        .as_deref()
        .and_then(|agent| {
            let project = project?;
            bench
                .agents_of(&project.id)
                .into_iter()
                .find(|row| row.id == agent)
                .map(|row| row.provider)
        })
        .unwrap_or_else(|| "claude".into());
    div().flex_1().min_w_0().flex().justify_center().child(
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
                    .child(div().mb(px(6.)).child(agent_mark(&provider, 40.)))
                    .child(text(
                        project.map(|project| project.name.clone()).unwrap_or_default(),
                        16.,
                        600.,
                        theme::primary(),
                    )),
            )
            .children(bench.activity.as_ref().map(|summary| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(24.))
                    .when(!is_empty(&summary["totals"]), |activity| {
                        activity.child(overview(&summary["totals"]))
                    })
                    .child(calendar(&summary["days"]))
            })),
    )
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.)
}

fn is_empty(totals: &Value) -> bool {
    number(&totals["sessions"]) == 0. && number(&totals["turns"]) == 0. && number(&totals["commits"]) == 0.
}

fn overview(totals: &Value) -> impl IntoElement {
    let tokens = number(&totals["input"]) + number(&totals["cacheWrite"]) + number(&totals["output"]);
    let cost = number(&totals["costUsd"]);
    let saved = number(&totals["cacheSavingsUsd"]);
    let stats = [
        ("Agent time", None, duration(number(&totals["agentMs"])), plural(number(&totals["turns"]), "turn")),
        (
            "Sessions",
            None,
            whole(number(&totals["sessions"])),
            format!("{} resumed", whole(number(&totals["resumed"]))),
        ),
        ("Tokens", None, compact(tokens), format!("{} cached", compact(number(&totals["cacheRead"])))),
        (
            "Cost",
            Some("API est."),
            if cost > 0. { money(cost) } else { "—".into() },
            if saved > 0. { format!("{} saved", money(saved)) } else { "not priced yet".into() },
        ),
        (
            "Commits",
            None,
            whole(number(&totals["commits"])),
            format!("{} by agents", whole(number(&totals["agentCommits"]))),
        ),
    ];
    div().flex().children(stats.into_iter().enumerate().map(|(index, (label, badge, value, detail))| {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(4.))
            .py(px(4.))
            .px(px(16.))
            .when(index == 0, |stat| stat.pl_0())
            .when(index > 0, |stat| stat.border_l_1().border_color(theme::border()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(text(label, 12., 500., theme::tertiary()))
                    .children(badge.map(|badge| text(badge, 10., 500., theme::tertiary()))),
            )
            .child(
                text(value, 20., 600., theme::primary())
                    .line_height(px(22.))
                    .font_features(theme::tabular()),
            )
            .child(
                text(detail, 11., 400., theme::tertiary())
                    .font_family(theme::MONO)
                    .font_features(theme::tabular())
                    .overflow_hidden()
                    .text_ellipsis(),
            )
    }))
}

fn calendar(days: &Value) -> impl IntoElement {
    let today = chrono::Local::now().date_naive();
    let by_day: HashMap<i64, f64> = days
        .as_array()
        .into_iter()
        .flatten()
        .map(|day| (day["day"].as_i64().unwrap_or(0), number(&day["tokens"])))
        .collect();
    let today_index = (today - epoch()).num_days();
    let start = today_index - weekday(today_index) - (WEEKS - 1) * 7;
    let thresholds = thresholds(by_day.values().copied());
    let active: BTreeSet<i64> = days
        .as_array()
        .into_iter()
        .flatten()
        .filter(|day| {
            number(&day["sessions"]) + number(&day["agentMs"]) + number(&day["commits"]) + number(&day["tokens"]) > 0.
        })
        .filter_map(|day| day["day"].as_i64())
        .collect();
    let levels: Vec<Option<usize>> = (0..WEEKS * 7)
        .map(|offset| {
            let day = start + offset;
            (day <= today_index).then(|| level(by_day.get(&day).copied().unwrap_or(0.), &thresholds))
        })
        .collect();
    let columns = levels.chunks(7).map(|week| {
        div().flex_1().flex().flex_col().gap(px(CELL_GAP)).children(week.iter().map(|level| {
            let cell = div().w_full().aspect_ratio(1.).rounded(px(2.));
            match level {
                Some(level) => cell.bg(level_colour(*level)),
                None => cell,
            }
        }))
    });
    let mut last_month = None;
    let months = (0..WEEKS).map(|week| {
        let first = epoch() + chrono::Duration::days(start + week * 7);
        let month = first.month();
        let label = (last_month != Some(month) && week <= WEEKS - 3).then(|| first.format("%b").to_string());
        last_month = Some(month);
        div()
            .flex_1()
            .relative()
            .children(label.map(|label| small(label).absolute().bottom_0().left_0()))
    });
    let months: Vec<Div> = months.collect();
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
                .child(text(readout(&active, today_index), 12., 400., theme::secondary()))
                .child(div().flex().gap(px(4.)).children(
                    ["Tokens", "Cost", "Commits", "Agent time", "Sessions"].into_iter().enumerate().map(
                        |(index, label)| {
                            div()
                                .h(px(22.))
                                .px(px(8.))
                                .flex()
                                .items_center()
                                .border_1()
                                .border_color(if index == 0 { theme::border() } else { rgba(0) })
                                .when(index == 0, |metric| metric.bg(theme::chosen()))
                                .text_size(px(11.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if index == 0 { theme::primary() } else { theme::tertiary() })
                                .child(label)
                        },
                    ),
                )),
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
                            div().flex_1().flex().items_center().child(small(label.into()))
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(CELL_GAP))
                        .child(div().h(px(14.)).flex().items_end().gap(px(CELL_GAP)).children(months))
                        .child(div().flex().gap(px(CELL_GAP)).children(columns)),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(CELL_GAP))
                .child(small("Less".into()).mr(px(4.)))
                .children((0..5).map(|level| div().size(px(10.)).rounded(px(2.)).bg(level_colour(level))))
                .child(small("More".into()).ml(px(4.))),
        )
}

fn small(label: String) -> Div {
    div()
        .text_size(px(10.))
        .line_height(px(10.))
        .text_color(theme::tertiary())
        .whitespace_nowrap()
        .child(label)
}

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(1970, 1, 1).unwrap_or_default()
}

/// Days counted from the epoch, which was a Thursday; weeks start on Monday.
fn weekday(day: i64) -> i64 {
    (day + 3).rem_euclid(7)
}

fn level_colour(level: usize) -> Rgba {
    match level {
        0 => rgba(0xe7e5ef12),
        1 => rgba(0xa277ff4d),
        2 => rgba(0xa277ff85),
        3 => rgba(0xa277ffc2),
        _ => theme::accent(),
    }
}

// The quartiles and streaks below are what the app's calendar works out for
// itself today; the core should send them with the days.
fn thresholds(values: impl Iterator<Item = f64>) -> Vec<f64> {
    let mut sorted: Vec<f64> = values.filter(|value| *value > 0.).collect();
    sorted.sort_by(f64::total_cmp);
    if sorted.is_empty() {
        return Vec::new();
    }
    [0.25, 0.5, 0.75]
        .iter()
        .map(|quantile| sorted[((quantile * sorted.len() as f64) as usize).min(sorted.len() - 1)])
        .collect()
}

fn level(value: f64, thresholds: &[f64]) -> usize {
    if value <= 0. {
        return 0;
    }
    1 + thresholds.iter().filter(|threshold| value > **threshold).count()
}

fn readout(active: &BTreeSet<i64>, today: i64) -> String {
    if active.is_empty() {
        return "No activity yet".into();
    }
    let (mut longest, mut run, mut previous) = (0, 0, None);
    for day in active {
        run = if previous == Some(day - 1) { run + 1 } else { 1 };
        longest = longest.max(run);
        previous = Some(*day);
    }
    let mut current = 0;
    let mut cursor = if active.contains(&today) { today } else { today - 1 };
    while active.contains(&cursor) {
        current += 1;
        cursor -= 1;
    }
    let mut readout = format!(
        "{} · longest streak {}",
        plural(active.len() as f64, "active day"),
        plural(f64::from(longest), "day")
    );
    if current > 1 {
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
    format!("{} {word}{}", whole(count), if count == 1. { "" } else { "s" })
}

fn compact(value: f64) -> String {
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

fn money(usd: f64) -> String {
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
