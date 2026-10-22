//! Asks over the whole window whether a phone waiting to connect may, the one that has waited
//! longest first, since each gives up after two minutes. A phone that starts waiting while
//! Sikemux is in the background is announced with a notification and a bounce.

use std::collections::HashSet;
use std::time::Duration;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    actions, div, px, AnyElement, App, Context, FocusHandle, FontWeight, InteractiveElement,
    IntoElement, KeyBinding, ParentElement, SharedString, StatefulInteractiveElement, Styled,
    Window,
};
use sikemux_wire::protocol::{DeviceAccess, PendingDevice};

use crate::core::{Ask, RemoteAsk};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, Button, Variant};
use crate::workbench::Workbench;

actions!(pairing, [Allow, Decline, OtherAccess]);

const CONTEXT: &str = "Pairing";

pub fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Allow, Some(CONTEXT)),
        KeyBinding::new("escape", Decline, Some(CONTEXT)),
        KeyBinding::new("up", OtherAccess, Some(CONTEXT)),
        KeyBinding::new("down", OtherAccess, Some(CONTEXT)),
    ]);
}

/// The question while a phone waits: the access it would get, and the phones already told of.
pub struct Pairing {
    access: DeviceAccess,
    announced: HashSet<String>,
    ticking: bool,
    focus: FocusHandle,
}

impl Pairing {
    pub fn new(cx: &mut App) -> Self {
        Self {
            access: DeviceAccess::Full,
            announced: HashSet::new(),
            ticking: false,
            focus: cx.focus_handle(),
        }
    }
}

const ACCESS: [(DeviceAccess, Icon, &str, &str); 2] = [
    (
        DeviceAccess::Full,
        Icon::Pointer,
        "Full control",
        "Start chats, answer agents, type in terminals",
    ),
    (
        DeviceAccess::Watch,
        Icon::Eye,
        "Watch only",
        "See chats and terminals, answer what agents ask",
    ),
];

impl Workbench {
    pub(crate) fn waiting_phone(&self) -> Option<&PendingDevice> {
        self.remote
            .as_ref()?
            .pending
            .iter()
            .min_by_key(|request| request.expires_at)
    }

    fn answer_pairing(&mut self, allow: bool, cx: &mut Context<Self>) {
        let Some(id) = self.waiting_phone().map(|request| request.id.clone()) else {
            return;
        };
        let access = std::mem::replace(&mut self.pairing.access, DeviceAccess::Full);
        let _ = self
            .asks
            .send(Ask::Remote(RemoteAsk::Answer { id, allow, access }));
        self.parts.redraw(&self.parts.over, cx);
    }
}

/// Hears what the core says of phones waiting: tells the person of new ones while Sikemux is in
/// the background, and keeps the countdown moving while one waits.
pub fn remote_moved(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    let waiting: Vec<PendingDevice> = bench
        .remote
        .as_ref()
        .map_or(Vec::new(), |remote| remote.pending.clone());
    let fresh: Vec<&PendingDevice> = waiting
        .iter()
        .filter(|request| !bench.pairing.announced.contains(&request.id))
        .collect();
    if !fresh.is_empty() && cx.active_window().is_none() {
        if let Some(window) = cx.windows().first().copied() {
            let _ = window.update(cx, |_, window, _| window.request_attention());
        }
        for request in &fresh {
            let name = if request.name.is_empty() {
                "A phone"
            } else {
                &request.name
            };
            crate::notifications::post(
                format!("{name} wants to connect to this computer"),
                "It is signed in to your Sikemux account. Allow or decline it in Sikemux.".into(),
                None,
                cx,
            );
        }
    }
    bench.pairing.announced = waiting.iter().map(|request| request.id.clone()).collect();
    if !waiting.is_empty() && !bench.pairing.ticking {
        bench.pairing.ticking = true;
        cx.spawn(async move |bench, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let going = bench.update(cx, |bench, cx| {
                let going = bench.waiting_phone().is_some();
                bench.pairing.ticking = going;
                match going {
                    true => bench.parts.redraw(&bench.parts.over, cx),
                    false => cx.notify(),
                }
                going
            });
            if !matches!(going, Ok(true)) {
                return;
            }
        })
        .detach();
    }
    cx.notify();
}

/// What is left of the two minutes, as m:ss.
fn waits(expires_at: u64) -> String {
    let now = chrono::Utc::now().timestamp_millis().max(0) as u64;
    let left = expires_at.saturating_sub(now).div_ceil(1000);
    format!("{}:{:02}", left / 60, left % 60)
}

pub fn overlay(
    bench: &Workbench,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) -> Option<AnyElement> {
    let request = bench.waiting_phone()?;
    let t = Tokens::get(cx).clone();
    if !bench.pairing.focus.is_focused(window) {
        cx.defer_in(window, |bench, window, cx| {
            bench.pairing.focus.focus(window, cx)
        });
    }
    let name = if request.name.is_empty() {
        "A phone".to_owned()
    } else {
        request.name.clone()
    };
    let account = bench
        .view
        .as_ref()
        .and_then(|view| view.account.as_deref())
        .and_then(|account| account.email.clone().or_else(|| account.name.clone()));
    let tile = |icon: Icon| {
        div()
            .size(px(72.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme::RADIUS_4)
            .border_1()
            .border_color(t.border)
            .bg(t.surface_raised)
            .child(ui::icon(icon, 36.).text_color(t.text_primary))
    };
    let choices = ACCESS.iter().map(|&(access, icon, label, detail)| {
        let on = bench.pairing.access == access;
        div()
            .id(SharedString::from(format!("pairing-{label}")))
            .w(px(360.))
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(14.))
            .py(px(10.))
            .rounded(theme::RADIUS_3)
            .border_1()
            .border_color(if on { t.text_secondary } else { t.border })
            .when(on, |choice| choice.bg(t.surface_chosen))
            .child(ui::icon(icon, 16.).text_color(t.text_secondary))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(ui::text(label, 13., FontWeight::MEDIUM).text_color(t.text_primary))
                    .child(ui::text(detail, 11.5, FontWeight::NORMAL).text_color(t.text_tertiary)),
            )
            .on_click(cx.listener(move |bench, _, _, cx| {
                bench.pairing.access = access;
                bench.parts.redraw(&bench.parts.over, cx);
            }))
    });
    let hint = |keys: &'static str, what: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(ui::kbd(keys, cx))
            .child(ui::text(what, 11., FontWeight::NORMAL).text_color(t.text_tertiary))
    };
    let column = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(18.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(tile(Icon::Phone))
                .child(div().w(px(48.)).h(px(1.)).bg(t.border))
                .child(tile(Icon::Laptop)),
        )
        .child(
            div()
                .text_center()
                .text_size(px(22.))
                .text_color(t.text_primary)
                .child(format!("{name} wants to connect"))
                .child(div().child("to this computer")),
        )
        .children(account.map(|account| {
            div()
                .text_size(px(12.))
                .text_color(t.text_tertiary)
                .child(format!("Signed in as {account}"))
        }))
        .child(div().flex().flex_col().gap(px(8.)).children(choices))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(ui::mono(
                    format!("waits {}", waits(request.expires_at)),
                    11.,
                    cx,
                ))
                .child(
                    Button::new("pairing-decline", "Decline", Variant::Outline)
                        .on_click(cx.listener(|bench, _, _, cx| bench.answer_pairing(false, cx))),
                )
                .child(
                    Button::new("pairing-allow", "Allow", Variant::Primary)
                        .on_click(cx.listener(|bench, _, _, cx| bench.answer_pairing(true, cx))),
                ),
        )
        .child(
            div()
                .flex()
                .gap(px(16.))
                .child(hint("esc", "Decline"))
                .child(hint("↑↓", "Choose access"))
                .child(hint("↵", "Allow")),
        );
    Some(
        div()
            .id("pairing")
            .key_context(CONTEXT)
            .track_focus(&bench.pairing.focus)
            .on_action(cx.listener(|bench, _: &Allow, _, cx| bench.answer_pairing(true, cx)))
            .on_action(cx.listener(|bench, _: &Decline, _, cx| bench.answer_pairing(false, cx)))
            .on_action(cx.listener(|bench, _: &OtherAccess, _, cx| {
                bench.pairing.access = match bench.pairing.access {
                    DeviceAccess::Full => DeviceAccess::Watch,
                    DeviceAccess::Watch => DeviceAccess::Full,
                };
                bench.parts.redraw(&bench.parts.over, cx);
            }))
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(t.surface)
            .child(column)
            .child(
                div()
                    .absolute()
                    .bottom(px(24.))
                    .text_size(px(11.5))
                    .text_color(t.text_tertiary)
                    .child("Only allow phones you own. Remove one any time in Settings › Devices."),
            )
            .into_any_element(),
    )
}
