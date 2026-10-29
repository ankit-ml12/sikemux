//! Small parts the pane's pages share: headings, chips and marks.

use super::*;

pub(super) fn heading(label: &str, cx: &App) -> Div {
    div()
        .px(px(8.))
        .pt(px(4.))
        .pb(px(2.))
        .child(ui::caps(label, cx))
}

pub(super) fn chevron(open: bool, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    let chevron = ui::icon(Icon::Chevron, 10.).text_color(t.text_tertiary);
    match open {
        true => chevron
            .with_transformation(Transformation::rotate(radians(std::f32::consts::FRAC_PI_2)))
            .into_any_element(),
        false => chevron.into_any_element(),
    }
}

pub(super) fn tag(text: &str, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .px(px(6.))
        .py(px(1.))
        .rounded(RADIUS_1)
        .bg(t.surface_active)
        .text_size(crate::theme::ui_text(10.5))
        .text_color(t.text_secondary)
        .child(text.to_owned())
        .into_any_element()
}

/// A status, coloured by the bucket Jira puts it in: to do, in progress or done.
pub(super) fn status_chip(status: &str, category: &str, cx: &App) -> Div {
    let t = Tokens::get(cx);
    let ink = match category {
        "indeterminate" => jira_blue(),
        "done" => t.live,
        _ => t.text_secondary,
    };
    let ground = match category {
        "indeterminate" | "done" => ink.opacity(0.16),
        _ => t.surface_hover,
    };
    div()
        .flex_none()
        .px(px(6.))
        .py(px(1.))
        .rounded(RADIUS_1)
        .bg(ground)
        .text_size(crate::theme::ui_text(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .whitespace_nowrap()
        .child(status.to_owned())
}

pub(super) fn type_mark(kind: Option<&str>, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    let Some(kind) = kind else {
        return div().into_any_element();
    };
    let ink = match kind.to_lowercase().as_str() {
        "bug" => t.danger,
        "story" => t.live,
        "epic" => t.accent,
        _ => jira_blue(),
    };
    div()
        .id(SharedString::from(format!("jira-type-{kind}")))
        .flex_none()
        .tooltip(ui::tooltip(kind.to_owned()))
        .child(ui::icon(Icon::Issue, 13.).text_color(ink))
        .into_any_element()
}

pub(super) fn priority_mark(priority: &str, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    let (icon, ink) = match priority.to_lowercase().as_str() {
        "highest" | "high" | "critical" | "blocker" => (Icon::ArrowUp, t.danger),
        "low" | "lowest" | "minor" | "trivial" => (Icon::ArrowDown, jira_blue()),
        _ => (Icon::Minus, t.warn),
    };
    div()
        .id(SharedString::from(format!("jira-priority-{priority}")))
        .flex_none()
        .tooltip(ui::tooltip(format!("{priority} priority")))
        .child(ui::icon(icon, 12.).text_color(ink))
        .into_any_element()
}

pub(super) fn epic_chip(parent: &Parent, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .id(SharedString::from(format!("jira-parent-{}", parent.key)))
        .max_w(px(200.))
        .px(px(6.))
        .py(px(1.))
        .rounded(RADIUS_1)
        .bg(t.accent.opacity(0.14))
        .text_size(crate::theme::ui_text(10.5))
        .text_color(t.text_primary)
        .truncate()
        .tooltip(ui::tooltip(format!("{} {}", parent.key, parent.summary)))
        .child(parent.summary.clone())
        .into_any_element()
}

/// The day an issue is due, red once it has passed and the issue is not done.
/// When an issue is due: red once past and not done, green once done, plain while there is
/// time. `long` says it as the details do, with a calendar and how late it is, rather than
/// as a card's `Due 23 Oct`.
pub(super) fn due_chip(due: &str, done: bool, long: bool, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    let day = chrono::NaiveDate::parse_from_str(due, "%Y-%m-%d").ok();
    let today = chrono::Local::now().date_naive();
    let late = (!done).then_some(day).flatten().filter(|day| *day < today);
    let (ink, ground) = match (late, done) {
        (Some(_), _) => (t.danger, t.danger.opacity(0.16)),
        (None, true) if long => (t.live, t.live.opacity(0.16)),
        _ => (t.text_secondary, t.surface_active),
    };
    let said = match (day, long) {
        (Some(day), false) => day.format("Due %-d %b").to_string(),
        (Some(day), true) => match late.map(|day| (today - day).num_days()) {
            Some(1) => format!("{} · 1 day late", day.format("%-d %b")),
            Some(days) => format!("{} · {days} days late", day.format("%-d %b")),
            None => day.format("%-d %b").to_string(),
        },
        (None, _) => due.to_owned(),
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(if long { 7. } else { 6. }))
        .py(px(1.))
        .rounded(RADIUS_1)
        .bg(ground)
        .text_size(crate::theme::ui_text(if long { 11.5 } else { 10.5 }))
        .text_color(ink)
        .when(long, |chip| {
            chip.child(ui::icon(Icon::Clock, 11.).text_color(ink))
        })
        .child(said)
        .into_any_element()
}

/// Someone's comment or change: their picture, their name and when, and what they did.
pub(super) fn said(
    host: &Host,
    author: &str,
    avatar: Option<&str>,
    at: &str,
    body: AnyElement,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .flex()
        .gap(px(10.))
        .child(host.face(author, avatar, 24., cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            ui::text(author.to_owned(), 12.5, FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .min_w_0()
                                .truncate(),
                        )
                        .child(ui::meta(crate::ui::time::ago_written(at), cx).flex_none()),
                )
                .child(body),
        )
        .into_any_element()
}
