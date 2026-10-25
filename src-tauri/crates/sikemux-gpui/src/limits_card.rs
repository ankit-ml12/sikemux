//! Every account the person has with an agent, in a card that opens from the Limits panel's
//! account name: the accounts down one side, the one under the pointer in full on the other.
//! Looking changes nothing; only "Use for new chats" moves new chats onto an account.

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    anchored, deferred, div, point, px, relative, Anchor, AnchoredPositionMode, AnyElement,
    Context, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Task,
};
use serde_json::Value;
use sikemux_core::agent_accounts::Profile;

use crate::accounts::{self, Status};
use crate::agents::{age, countdown, reset_at, signal};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, Button, IconButton, Variant};
use crate::workbench::Workbench;

const OPEN_AFTER: Duration = Duration::from_millis(250);
const CLOSE_AFTER: Duration = Duration::from_millis(160);
const LIST_W: f32 = 176.;
const DETAIL_W: f32 = 344.;
const COLUMN_GAP: f32 = 16.;

#[derive(Default)]
pub struct Card {
    open: bool,
    /// The account the detail side shows, when the pointer has picked one.
    focus: Option<String>,
    timer: Option<Task<()>>,
}

impl Workbench {
    /// The pointer came onto the account name, or left it or the card: the card opens after a
    /// short rest and closes a moment after the pointer leaves both.
    pub fn rest_on_limits(&mut self, inside: bool, cx: &mut Context<Self>) {
        let card = &mut self.accounts.limits_card;
        if inside && card.open {
            card.timer = None;
            return;
        }
        let wait = if inside { OPEN_AFTER } else { CLOSE_AFTER };
        card.timer = Some(cx.spawn(async move |bench, cx| {
            cx.background_executor().timer(wait).await;
            bench
                .update(cx, |bench, cx| match inside {
                    true => bench.open_limits(cx),
                    false => bench.close_limits(cx),
                })
                .ok();
        }));
    }

    pub fn open_limits(&mut self, cx: &mut Context<Self>) {
        let card = &mut self.accounts.limits_card;
        card.timer = None;
        if !card.open {
            card.open = true;
            card.focus = None;
            cx.notify();
        }
    }

    pub fn close_limits(&mut self, cx: &mut Context<Self>) {
        if self.accounts.limits_card.open {
            self.accounts.limits_card = Card::default();
            cx.notify();
        }
    }

    fn focus_account(&mut self, id: &str, cx: &mut Context<Self>) {
        let card = &mut self.accounts.limits_card;
        card.timer = None;
        if card.focus.as_deref() != Some(id) {
            card.focus = Some(id.to_owned());
            cx.notify();
        }
    }
}

/// What the card knows of one account, read once per frame.
struct Seen<'a> {
    profile: &'a Profile,
    name: String,
    status: Option<&'a Status>,
    usage: &'a Value,
    current: bool,
}

impl Seen<'_> {
    fn signed_out(&self) -> bool {
        self.status.is_some_and(|status| !status.signed_in)
    }

    fn windows(&self) -> &[Value] {
        self.usage["usage"]["windows"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
    }

    fn peak(&self) -> Option<f64> {
        self.windows()
            .iter()
            .filter_map(|window| window["usedPercent"].as_f64())
            .map(|used| used.clamp(0., 100.))
            .reduce(f64::max)
    }

    fn plan(&self) -> Option<String> {
        let plan = self.usage["usage"]["plan"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| self.status?.plan.clone())?;
        let mut letters = plan.chars();
        let first = letters.next()?;
        Some(first.to_uppercase().chain(letters).collect())
    }
}

/// The card, when it is open, floating above the account name it opened from.
pub fn view(bench: &Workbench, provider: &str, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    if !bench.accounts.limits_card.open {
        return None;
    }
    let t = Tokens::get(cx).clone();
    let current = bench.accounts.current(provider).map(|profile| &profile.id);
    let seen: Vec<Seen> = bench
        .accounts
        .of(provider)
        .into_iter()
        .map(|profile| {
            let status = accounts::status(bench, profile).and_then(|status| status.as_ref().ok());
            Seen {
                profile,
                name: accounts::name(bench, profile, status),
                status,
                usage: accounts::usage(bench, profile).unwrap_or(&Value::Null),
                current: Some(&profile.id) == current,
            }
        })
        .collect();
    let focus = bench.accounts.limits_card.focus.as_ref();
    let shown = seen
        .iter()
        .find(|seen| Some(&seen.profile.id) == focus)
        .or_else(|| seen.iter().find(|seen| seen.current))
        .or(seen.first())?;
    let shown_id = shown.profile.id.clone();
    let add = {
        let provider = provider.to_owned();
        div()
            .id("limits-card-add")
            .h(px(32.))
            .px(px(9.))
            .flex()
            .items_center()
            .gap(px(9.))
            .rounded(theme::RADIUS_2)
            .cursor_pointer()
            .text_color(t.text_tertiary)
            .hover(|row| row.bg(t.surface_hover).text_color(t.text_secondary))
            .child(
                div()
                    .w(px(18.))
                    .flex()
                    .justify_center()
                    .child(ui::icon(Icon::Plus, 11.)),
            )
            .child(ui::text("Add account…", 11.5, FontWeight::NORMAL))
            .on_click(cx.listener(move |bench, _, window, cx| {
                bench.close_limits(cx);
                accounts::open_sign_in(bench, &provider, None, window, cx);
            }))
    };
    let list = div()
        .w(px(LIST_W))
        .flex_none()
        .p(px(6.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .border_r_1()
        .border_color(t.border)
        .children(
            seen.iter()
                .map(|seen| list_row(seen, seen.profile.id == shown_id, cx)),
        )
        .child(div().flex_1())
        .child(add);
    let card = div()
        .id("limits-card")
        .occlude()
        .flex()
        .bg(t.surface_overlay)
        .border_1()
        .border_color(t.border)
        .rounded(theme::RADIUS_4)
        .shadow(t.shadow_float())
        .overflow_hidden()
        .on_hover(cx.listener(|bench, inside: &bool, _, cx| bench.rest_on_limits(*inside, cx)))
        .on_mouse_down_out(cx.listener(|bench, _, _, cx| bench.close_limits(cx)))
        .child(list)
        .child(detail(shown, cx));
    Some(
        deferred(
            anchored()
                .position_mode(AnchoredPositionMode::Local)
                .position(point(px(0.), px(-8.)))
                .anchor(Anchor::BottomLeft)
                .snap_to_window()
                .child(card),
        )
        .with_priority(1)
        .into_any_element(),
    )
}

/// An account on the list side: a ring for its fullest window, its name, and its plan.
fn list_row(seen: &Seen, shown: bool, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let peak = seen.peak().filter(|_| !seen.signed_out());
    let line = match (seen.signed_out(), peak, seen.plan()) {
        (true, ..) => "Signed out".to_owned(),
        (false, Some(peak), Some(plan)) => format!("{}% · {plan}", peak.round()),
        (false, Some(peak), None) => format!("{}%", peak.round()),
        (false, None, Some(plan)) => plan,
        (false, None, None) => "Not read yet".to_owned(),
    };
    let id = seen.profile.id.clone();
    div()
        .id(SharedString::from(format!("limits-card-{id}")))
        .h(px(40.))
        .px(px(9.))
        .flex()
        .items_center()
        .gap(px(9.))
        .rounded(theme::RADIUS_2)
        .cursor_pointer()
        .when(shown, |row| row.bg(t.surface_chosen))
        .when(!shown, |row| row.hover(|row| row.bg(t.surface_hover)))
        .when(seen.signed_out(), |row| row.opacity(0.5))
        .child(ui::ring(
            peak.map(|peak| (peak, signal(peak, &t))),
            18.,
            2.,
            Hsla {
                a: 0.1,
                ..t.text_primary
            },
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            ui::text(seen.name.clone(), 12., FontWeight::MEDIUM)
                                .min_w_0()
                                .truncate()
                                .text_color(t.text_primary),
                        )
                        .when(seen.current, |line| {
                            line.child(div().size(px(5.)).flex_none().rounded_full().bg(t.accent))
                        }),
                )
                .child(
                    ui::text(line, 10., FontWeight::NORMAL)
                        .truncate()
                        .font_features(theme::tabular())
                        .text_color(t.text_tertiary),
                ),
        )
        .on_hover(cx.listener({
            let id = id.clone();
            move |bench, inside: &bool, _, cx| {
                if *inside {
                    bench.focus_account(&id, cx);
                }
            }
        }))
        .on_click(cx.listener(move |bench, _, _, cx| bench.focus_account(&id, cx)))
}

/// The account in full: who it is, every window of its plan, and what can be done with it.
fn detail(seen: &Seen, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let who = seen
        .status
        .and_then(|status| status.email.clone().or_else(|| status.organization.clone()));
    let quiet = |text: String| {
        ui::text(text, 11., FontWeight::NORMAL)
            .italic()
            .text_color(t.text_tertiary)
            .py(px(14.))
    };
    let usage = seen.usage;
    let windows = seen.windows();
    let body = if seen.signed_out() {
        quiet(format!("{} is signed out.", seen.name)).into_any_element()
    } else if usage["usage"].is_null() && usage["problem"].is_null() {
        quiet("Refresh to read plan limits.".into()).into_any_element()
    } else if !usage["problem"].is_null() {
        quiet("Could not read plan limits. Refresh to try again.".into()).into_any_element()
    } else if windows.is_empty() {
        let reason = usage["usage"]["unavailableReason"]
            .as_str()
            .unwrap_or("This account did not report plan limits.");
        quiet(reason.to_owned()).into_any_element()
    } else {
        div()
            .flex()
            .flex_wrap()
            .gap_x(px(COLUMN_GAP))
            .gap_y(px(18.))
            .children(windows.iter().map(|window| column(window, cx)))
            .into_any_element()
    };
    let provider = seen.profile.provider.clone();
    let id = seen.profile.id.clone();
    let profile = seen.profile.clone();
    let name = seen.name.clone();
    let link = |label: String| {
        ui::text(label, 11., FontWeight::NORMAL)
            .text_color(t.text_tertiary)
            .cursor_pointer()
            .hover(|link| link.text_color(t.text_primary))
    };
    let account_action = match seen.signed_out() {
        true => Button::new(
            "limits-card-sign-in",
            format!("Sign in to {name}"),
            Variant::Outline,
        )
        .on_click(cx.listener(move |bench, _, window, cx| {
            bench.close_limits(cx);
            accounts::open_sign_in(bench, &provider, Some(&id), window, cx);
        }))
        .into_any_element(),
        false => link("Sign out".into())
            .id("limits-card-sign-out")
            .on_click(cx.listener(move |bench, _, _, cx| accounts::sign_out(bench, &id, cx)))
            .into_any_element(),
    };
    let choose = match (seen.current, seen.signed_out()) {
        (true, _) => Some(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(5.)).rounded_full().bg(t.accent))
                .child(ui::text("New chats start here", 11., FontWeight::NORMAL))
                .text_color(t.text_secondary)
                .into_any_element(),
        ),
        (false, false) => Some(
            Button::new("limits-card-use", "Use for new chats", Variant::Outline)
                .on_click(cx.listener(move |bench, _, _, cx| accounts::select(bench, &profile, cx)))
                .into_any_element(),
        ),
        (false, true) => None,
    };
    let refresh_provider = seen.profile.provider.clone();
    div()
        .w(px(DETAIL_W))
        .flex_none()
        .pt(px(18.))
        .px(px(20.))
        .pb(px(14.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    ui::text(seen.name.clone(), 15., FontWeight::SEMIBOLD)
                        .min_w_0()
                        .truncate()
                        .text_color(t.text_primary),
                )
                .children(seen.plan().map(|plan| {
                    ui::text(plan, 10., FontWeight::MEDIUM)
                        .flex_none()
                        .h(px(17.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .rounded(theme::RADIUS_1)
                        .bg(t.surface_hover)
                        .text_color(t.text_secondary)
                }))
                .child(div().flex_1())
                .children(usage["readAt"].as_u64().map(|read_at| {
                    ui::text(
                        format!("read {} ago", age(read_at)),
                        10.5,
                        FontWeight::NORMAL,
                    )
                    .flex_none()
                    .text_color(t.text_tertiary)
                    .font_features(theme::tabular())
                }))
                .child(
                    IconButton::new("limits-card-refresh", Icon::Refresh, "Refresh plan limits")
                        .small()
                        .on_click(cx.listener(move |bench, _, _, cx| {
                            bench.read_usage();
                            accounts::ask_again(bench, &refresh_provider, cx);
                        })),
                ),
        )
        .child(
            ui::text(who.unwrap_or_default(), 11., FontWeight::NORMAL)
                .h(px(15.))
                .mt(px(2.))
                .mb(px(18.))
                .truncate()
                .text_color(t.text_tertiary),
        )
        .child(body)
        .child(
            div()
                .mt(px(20.))
                .pt(px(12.))
                .border_t_1()
                .border_color(t.border)
                .flex()
                .items_center()
                .gap(px(12.))
                .child(account_action)
                .child(div().flex_1())
                .children(choose),
        )
}

/// One window of the plan: how much is used, large, and when it starts again.
fn column(window: &Value, cx: &mut Context<Workbench>) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let label = window["label"].as_str().unwrap_or_default().to_owned();
    let percent = window["usedPercent"].as_f64().unwrap_or(0.).clamp(0., 100.);
    let tint = signal(percent, &t);
    let reset = reset_at(&window["resetsAt"]);
    let width = (DETAIL_W - 40. - 2. * COLUMN_GAP) / 3.;
    div()
        .w(px(width))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(1.))
                .child(
                    ui::text(format!("{}", percent.round()), 22., FontWeight::MEDIUM)
                        .line_height(px(24.))
                        .font_features(theme::tabular())
                        .text_color(if percent < 70. { t.text_primary } else { tint }),
                )
                .child(ui::text("%", 11., FontWeight::NORMAL).text_color(t.text_tertiary)),
        )
        .child(
            ui::text(label, 10.5, FontWeight::NORMAL)
                .mt(px(5.))
                .mb(px(8.))
                .truncate()
                .text_color(t.text_secondary),
        )
        .child(
            div()
                .h(px(3.))
                .rounded_full()
                .bg(Hsla {
                    a: 0.08,
                    ..t.text_primary
                })
                .child(
                    div()
                        .h_full()
                        .w(relative((percent / 100.) as f32))
                        .rounded_full()
                        .bg(tint),
                ),
        )
        .child(
            div()
                .mt(px(8.))
                .flex()
                .flex_col()
                .gap(px(1.))
                .text_size(theme::ui_text(10.5))
                .font_features(theme::tabular())
                .text_color(t.text_tertiary)
                .child(match reset {
                    Some(_) => div()
                        .flex()
                        .gap(px(3.))
                        .child("Resets in")
                        .child(div().text_color(t.text_secondary).child(countdown(reset))),
                    None => div().child("Reset time unknown"),
                })
                .children(reset.map(|at| div().child(day_and_time(at)))),
        )
}

/// When a window resets, as a person would say it: today, tomorrow, a weekday or a date.
fn day_and_time(at: chrono::DateTime<chrono::Utc>) -> String {
    let at = at.with_timezone(&chrono::Local);
    let days = (at.date_naive() - chrono::Local::now().date_naive()).num_days();
    let time = at.format("%-I:%M %p");
    match days {
        ..=0 => format!("today {time}"),
        1 => format!("tomorrow {time}"),
        2..=6 => format!("{} {time}", at.format("%a")),
        _ => format!("{}, {time}", at.format("%b %-d")),
    }
}
