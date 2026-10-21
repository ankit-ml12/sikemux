//! Moves a chat that has started to another of the person's accounts with its agent,
//! even in the middle of a turn: those signed in that keep their chats where this one does.

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

/// The most of any window of its plan the account has used, as the core last read it.
fn used(bench: &Workbench, profile: &Profile) -> Option<f64> {
    let launcher = format!("{}:{}", profile.provider, profile.id);
    bench.usage_peak(&launcher).or_else(|| {
        profile
            .id
            .starts_with("builtin-")
            .then(|| bench.usage_peak(&profile.provider))
            .flatten()
    })
}

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

/// A chat that has started: its agent's id, which agent it is, and the account it runs on.
pub struct Chat<'a> {
    pub agent: &'a str,
    pub provider: &'a str,
    pub on: &'a str,
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
        .filter(|(_, status)| sessions.is_some() && status.sessions == sessions)
        .collect();
    if !movable.iter().any(|(profile, _)| profile.id != current.id) {
        return None;
    }
    let name = |profile: &Profile, status: Option<&accounts::Status>| {
        let unnamed = profile.id.starts_with("builtin-")
            && bench
                .read("catalog")
                .and_then(|kinds| {
                    kinds
                        .as_array()?
                        .iter()
                        .find(|kind| kind["type"] == profile.provider.as_str())?["label"]
                        .as_str()
                        .map(str::to_owned)
                })
                .as_deref()
                == Some(profile.name.as_str());
        status
            .and_then(|status| status.name.clone())
            .filter(|_| unnamed)
            .unwrap_or_else(|| profile.name.clone())
    };
    let label = name(&current, signed_in(&current).as_ref());
    let about = match running {
        true => "Moving stops this turn and carries it on in the other account".to_owned(),
        false => label.clone(),
    };
    let trigger = BaseButton::new(SharedString::from(format!("account-picker-{agent}")))
        .accessibility_label("Account")
        .disabled(disabled)
        .h(px(28.))
        .max_w(px(200.))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(px(11.))
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
                used(bench, profile),
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
                            this.update(cx, |bench, _| move_chat(bench, &agent, &chosen));
                        }
                        popover.update(cx, |popover, cx| popover.dismiss(window, cx));
                    })
                }))
            })
            .into_any_element(),
    )
}
