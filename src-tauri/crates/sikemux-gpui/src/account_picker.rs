//! Which of the person's accounts with its agent a chat runs on. One that has started moves
//! there, even in the middle of a turn, to those signed in that keep their chats where this
//! one does; one that has not simply starts there, on any signed in.

use gpui_kit::base::{Button as BaseButton, Popover};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, Anchor, AnyElement, Context, Entity, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled,
};
use sikemux_core::agent_accounts::{chat_account, Profile};

use crate::accounts;
use crate::core::Ask;
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, Menu, MenuItem};
use crate::workbench::Workbench;

/// Carries the chat `agent` on to `profile`; a turn at work stops and carries on there.
fn move_chat(bench: &Workbench, agent: &str, profile: &Profile) {
    let account = chat_account(
        &profile.provider,
        &profile.id,
        &profile.name,
        profile.config_path.as_deref(),
    );
    let _ = bench.asks.send(Ask::MoveChat {
        agent: agent.to_owned(),
        account,
    });
}

/// A chat: its agent's id, which agent it is, the account it runs on, and whether it has
/// started.
pub struct Chat<'a> {
    pub agent: &'a str,
    pub provider: &'a str,
    pub on: &'a str,
    pub started: bool,
}

/// The picker for the chat, or none while no other account can take it over.
pub fn picker(
    bench: &Workbench,
    chat: Chat,
    running: bool,
    disabled: bool,
    this: &Entity<Workbench>,
    cx: &mut Context<Workbench>,
) -> Option<AnyElement> {
    let Chat {
        agent,
        provider,
        on,
        started,
    } = chat;
    let t = Tokens::get(cx).clone();
    let current = bench.accounts.profile(on)?.clone();
    let signed_in = |profile: &Profile| {
        accounts::status(bench, profile)
            .and_then(|status| status.as_ref().ok())
            .filter(|status| status.signed_in)
            .cloned()
    };
    let sessions = signed_in(&current).and_then(|status| status.sessions);
    let movable: Vec<(Profile, accounts::Status)> = bench
        .accounts
        .of(provider)
        .into_iter()
        .filter_map(|profile| Some((profile.clone(), signed_in(profile)?)))
        .filter(|(_, status)| !started || sessions.is_some() && status.sessions == sessions)
        .collect();
    if !movable.iter().any(|(profile, _)| profile.id != current.id) {
        return None;
    }
    let name = |profile: &Profile, status: Option<&accounts::Status>| {
        accounts::name(bench, profile, status)
    };
    let label = name(&current, signed_in(&current).as_ref());
    let about = match running {
        true => "Moving stops this turn and carries it on in the other account".to_owned(),
        false => label.clone(),
    };
    let trigger = BaseButton::new(SharedString::from(format!("account-picker-{agent}")))
        .accessibility_label("Account")
        .disabled(disabled)
        .h(px(26.))
        .max_w(px(200.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(crate::theme::ui_text(11.))
        .text_color(t.text_secondary)
        .when(disabled, |trigger| trigger.opacity(0.45))
        .when(!disabled, |trigger| {
            trigger.hover(move |style| {
                style.bg(Hsla {
                    a: 0.07,
                    ..t.text_primary
                })
            })
        })
        .tooltip(ui::tooltip(about))
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label),
        )
        .child(ui::icon(Icon::Chevron, 10.).text_color(t.text_tertiary));
    if disabled {
        return Some(trigger.into_any_element());
    }
    let rows: Vec<(Profile, String, Option<String>, Option<f64>)> = movable
        .iter()
        .map(|(profile, status)| {
            let detail = status.email.clone().or_else(|| status.organization.clone());
            (
                profile.clone(),
                name(profile, Some(status)),
                detail,
                accounts::used(bench, profile),
            )
        })
        .collect();
    let (this, agent) = (this.clone(), agent.to_owned());
    let on = current.id.clone();
    Some(
        Popover::new(SharedString::from(format!("account-menu-{agent}")))
            .anchor(Anchor::BottomRight)
            .offset(px(4.))
            .trigger(trigger)
            .content(move |_, _, cx| {
                let popover = cx.entity();
                Menu::new().children(rows.iter().map(|(profile, name, detail, used)| {
                    let (this, agent, popover, chosen) = (
                        this.clone(),
                        agent.clone(),
                        popover.clone(),
                        profile.clone(),
                    );
                    let hint: Vec<String> = detail
                        .iter()
                        .cloned()
                        .chain(used.map(|used| format!("{}%", used.round())))
                        .collect();
                    let mut item = MenuItem::new(
                        SharedString::from(format!("account-{}", profile.id)),
                        name.clone(),
                    )
                    .mark(accounts::avatar(profile, name, 16.))
                    .checked(profile.id == on);
                    if !hint.is_empty() {
                        item = item.hint(hint.join(" · "));
                    }
                    let here = profile.id == on;
                    item.on_click(move |_, window, cx| {
                        if !here {
                            this.update(cx, |bench, _| match started {
                                true => move_chat(bench, &agent, &chosen),
                                false => bench.start_as(
                                    &agent,
                                    serde_json::json!({ "profileId": chosen.id }),
                                    &chosen.name,
                                ),
                            });
                        }
                        popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    })
                }))
            })
            .into_any_element(),
    )
}
