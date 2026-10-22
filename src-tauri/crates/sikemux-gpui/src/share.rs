//! Sharing a chat read-only: a link anyone signed in to Sikemux can read it from, as it
//! happens. The core opens and keeps the share; the window lists them and asks.

use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::component::WindowExt;
use gpui_kit::{
    div, px, AppContext, ClipboardItem, Context, FontWeight, IntoElement, ParentElement, Render,
    SharedString, Styled, WeakEntity, Window,
};
use serde::Deserialize;
use serde_json::json;

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui::{self, Button, Segmented, Variant};
use crate::workbench::Workbench;

const HOUR_MS: u64 = 60 * 60 * 1000;
const LIFETIMES: [(&str, u64); 3] = [
    ("1 hour", HOUR_MS),
    ("24 hours", 24 * HOUR_MS),
    ("7 days", 7 * 24 * HOUR_MS),
];

/// A chat this host shares, as the core's `shares` lists it.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    id: String,
    agent_id: String,
    url: String,
    expires_at: u64,
    viewers: u32,
    state: String,
    problem: Option<String>,
}

impl Share {
    /// Who reads it now, in the words beside the chat's name and in the dialog.
    pub fn watching(&self) -> String {
        match (self.state.as_str(), self.viewers) {
            ("connecting", _) => "Connecting…".to_owned(),
            (_, 0) => "No one watching yet".to_owned(),
            (_, 1) => "1 person watching".to_owned(),
            (_, viewers) => format!("{viewers} people watching"),
        }
    }

    pub fn viewers(&self) -> u32 {
        self.viewers
    }
}

impl Workbench {
    fn shares(&self) -> Vec<Share> {
        self.read("shares")
            .and_then(|shares| serde_json::from_value(shares.clone()).ok())
            .unwrap_or_default()
    }

    /// The share of `agent`'s chat that is open or opening.
    pub fn share_of(&self, agent: &str) -> Option<Share> {
        self.shares()
            .into_iter()
            .find(|share| share.agent_id == agent && share.state != "refused")
    }

    /// Why the server turned down the last share of `agent`'s chat, while it stands.
    fn refusal_of(&self, agent: &str) -> Option<String> {
        let refused = self
            .shares()
            .into_iter()
            .find(|share| share.agent_id == agent && share.state == "refused");
        refused.and_then(|share| share.problem)
    }

    /// Opens the dialog that shares `agent`'s chat, or shows the share it has.
    pub fn open_share(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let bench = cx.entity();
        let form = cx.new(|cx| {
            cx.observe(&bench, |_, _, cx| cx.notify()).detach();
            ShareForm {
                bench: bench.downgrade(),
                agent: agent.to_owned(),
                lifetime: 1,
                working: false,
                problem: None,
            }
        });
        let title = if self.share_of(agent).is_some() {
            "Shared read-only"
        } else {
            "Share this chat read-only"
        };
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.w(px(460.)).title(title).child(form.clone())
        });
    }

    fn agent_title(&self, agent: &str) -> String {
        self.view
            .as_ref()
            .and_then(|view| view.workspace.agents.iter().find(|saved| saved.id == agent))
            .map_or_else(|| "this chat".to_owned(), |saved| saved.title.clone())
    }
}

/// What the share dialog holds while it is open.
struct ShareForm {
    bench: WeakEntity<Workbench>,
    agent: String,
    /// The lifetime picked from `LIFETIMES`, by place.
    lifetime: usize,
    working: bool,
    problem: Option<String>,
}

impl ShareForm {
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench.upgrade() else {
            return;
        };
        let (asks, title) = {
            let bench = bench.read(cx);
            (bench.asks.clone(), bench.agent_title(&self.agent))
        };
        let asked = json!({ "agentId": self.agent, "lifetimeMs": LIFETIMES[self.lifetime].1, "title": title });
        let started = crate::core::request(&asks, "shareChat", asked);
        (self.working, self.problem) = (true, None);
        cx.notify();
        cx.spawn_in(window, async move |form, cx| {
            let answer = started.await;
            form.update_in(cx, |form, window, cx| {
                form.working = false;
                match answer.map(|share| share["url"].as_str().map(str::to_owned)) {
                    Ok(Some(url)) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(url));
                        window.push_notification("Copied the share link", cx);
                    }
                    Ok(None) => form.problem = Some("The core answered without a link".to_owned()),
                    Err(error) => form.problem = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn stop(&mut self, share: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench.upgrade() else {
            return;
        };
        let stopped =
            crate::core::request(&bench.read(cx).asks, "stopShare", json!({ "id": share }));
        self.working = true;
        cx.notify();
        cx.spawn_in(window, async move |form, cx| {
            let answer = stopped.await;
            form.update_in(cx, |form, window, cx| {
                form.working = false;
                match answer {
                    Ok(_) => window.close_dialog(cx),
                    Err(error) => form.problem = Some(error),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for ShareForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let (share, refused) = self.bench.upgrade().map_or((None, None), |bench| {
            let bench = bench.read(cx);
            (bench.share_of(&self.agent), bench.refusal_of(&self.agent))
        });
        let para = |text: &'static str| {
            div()
                .text_size(crate::theme::ui_text(12.5))
                .line_height(crate::theme::ui_text(18.))
                .text_color(t.text_secondary)
                .child(text)
        };
        let this = cx.entity();
        let field = match &share {
            Some(share) => {
                let url = share.url.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui::text("Link", 11., FontWeight::MEDIUM).text_color(t.text_tertiary))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .px(px(8.))
                                    .py(px(6.))
                                    .border_1()
                                    .border_color(t.border)
                                    .rounded(theme::RADIUS_2)
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(ui::mono(share.url.clone(), 11., cx)),
                            )
                            .child(
                                Button::new("share-copy", "Copy", Variant::Outline)
                                    .icon(Icon::Copy)
                                    .on_click(move |_, window, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            url.clone(),
                                        ));
                                        window.push_notification("Copied the share link", cx);
                                    }),
                            ),
                    )
                    .child(
                        ui::text(
                            format!(
                                "{} · ends in {}",
                                share.watching(),
                                time_left(share.expires_at)
                            ),
                            11.,
                            FontWeight::NORMAL,
                        )
                        .text_color(t.text_tertiary),
                    )
            }
            None => {
                let options = LIFETIMES
                    .iter()
                    .map(|(label, _)| (Icon::Clock, SharedString::from(*label)))
                    .collect();
                let picks = this.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        ui::text("Link works for", 11., FontWeight::MEDIUM)
                            .text_color(t.text_tertiary),
                    )
                    .child(
                        Segmented::new("share-lifetime", options, self.lifetime).on_select(
                            move |picked, _, cx| {
                                picks.update(cx, |form, cx| {
                                    form.lifetime = *picked;
                                    cx.notify();
                                });
                            },
                        ),
                    )
            }
        };
        let problem = self.problem.clone().or(refused);
        let buttons = match &share {
            Some(share) => {
                let (form, id) = (this.clone(), share.id.clone());
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("share-stop", "Stop sharing", Variant::Outline).on_click(
                            move |_, window, cx| {
                                form.update(cx, |form, cx| form.stop(&id, window, cx));
                            },
                        ),
                    )
                    .child(
                        Button::new("share-done", "Done", Variant::Primary)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
            }
            None => {
                let form = this.clone();
                let label = if self.working {
                    "Creating link…"
                } else {
                    "Create link"
                };
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        Button::new("share-cancel", "Cancel", Variant::Outline)
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("share-create", label, Variant::Primary).on_click(
                            move |_, window, cx| {
                                form.update(cx, |form, cx| {
                                    if !form.working {
                                        form.start(window, cx);
                                    }
                                });
                            },
                        ),
                    )
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(para(
                "Anyone signed in to Sikemux who has the link can read this chat as it happens, from its start. They cannot type in it or answer its requests.",
            ))
            .child(para("It shows everything the agent read and ran, including the contents of files and any secrets in them."))
            .child(field)
            .children(problem.map(|problem| div().text_size(crate::theme::ui_text(12.)).text_color(t.danger).child(problem)))
            .child(buttons)
    }
}

/// How long until `until`, in Unix milliseconds, as the dialog says it.
fn time_left(until: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64);
    let minutes = (until.saturating_sub(now) as f64 / 60_000.).round().max(1.) as u64;
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let hours = (minutes as f64 / 60.).round() as u64;
    match hours {
        1 => "1 hour".to_owned(),
        hours if hours < 48 => format!("{hours} hours"),
        hours => format!("{} days", (hours as f64 / 24.).round() as u64),
    }
}
