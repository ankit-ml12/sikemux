//! The attach menu over a chat's draft: files from this host, and the account's Stash, what
//! the person shared into Sikemux from other apps. Picking a stash item has the chat's core put
//! it in the draft, as it puts one shared straight to the chat.

use std::rc::Rc;

use gpui_kit::base::Popover;
use gpui_kit::component::WindowExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, Anchor, AnyElement, App, Context, Entity, FontWeight, InteractiveElement,
    IntoElement, ObjectFit, ParentElement, Render, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, WeakEntity, Window,
};
use serde_json::{json, Value};

use crate::composer::Act;
use crate::icons::Icon;
use crate::picture::{Picture, Remote};
use crate::theme::{self, Tokens};
use crate::ui::{self, IconButton};
use crate::workbench::Workbench;

type Dismiss = Rc<dyn Fn(&mut Window, &mut App)>;

const TILE: f32 = 36.;
const ROW: &str = "stash-row";

/// Wraps the composer's attach button in the menu it opens for `agent`'s chat; `on_files`
/// opens the system's picker of files.
pub fn around(
    trigger: IconButton,
    agent: &str,
    on_files: Act,
    bench: &Entity<Workbench>,
) -> Popover {
    let (bench, agent) = (bench.downgrade(), agent.to_owned());
    let key = SharedString::from(format!("attach-menu-{agent}"));
    Popover::new(key.clone())
        .anchor(Anchor::BottomLeft)
        .offset(px(12.))
        .trigger(trigger)
        .content(move |_, window, cx| {
            let popover = cx.entity();
            let close: Dismiss = Rc::new(move |window, cx| {
                popover.update(cx, |popover, cx| popover.dismiss(window, cx));
            });
            let (bench, agent, on_files) = (bench.clone(), agent.clone(), on_files.clone());
            window.use_keyed_state(key.clone(), cx, move |_, _| Menu {
                bench,
                agent,
                on_files,
                close,
                attaching: None,
                scroll: ScrollHandle::new(),
            })
        })
}

pub struct Menu {
    bench: WeakEntity<Workbench>,
    agent: String,
    on_files: Act,
    close: Dismiss,
    /// The item the chat's core is putting in the draft.
    attaching: Option<String>,
    scroll: ScrollHandle,
}

impl Menu {
    fn attach(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asked) = self.bench.upgrade().map(|bench| {
            let params = json!({ "id": id, "agentId": self.agent });
            bench
                .read(cx)
                .chat_target(&self.agent)
                .request("attachStash", params)
        }) else {
            return;
        };
        self.attaching = Some(id);
        cx.notify();
        cx.spawn_in(window, async move |menu, cx| {
            let answer = asked.await;
            menu.update_in(cx, |menu, window, cx| {
                menu.attaching = None;
                match answer {
                    Ok(_) => (menu.close)(window, cx),
                    Err(error) => {
                        window.push_notification(format!("Couldn't attach that: {error}"), cx);
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn remove(&self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(asks) = self
            .bench
            .upgrade()
            .map(|bench| bench.read(cx).asks.clone())
        else {
            return;
        };
        let removed = crate::core::request(&asks, "removeStash", json!({ "id": id }));
        cx.spawn_in(window, async move |_, cx| {
            if let Err(error) = removed.await {
                cx.update(|window, cx| {
                    window.push_notification(format!("Couldn't remove that: {error}"), cx)
                })
                .ok();
            }
        })
        .detach();
    }

    fn files_row(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let (on_files, close) = (self.on_files.clone(), self.close.clone());
        div()
            .id("attach-files")
            .mx(px(ui::PANEL_INSET))
            .mt(px(ui::PANEL_INSET))
            .px(px(8.))
            .h(px(32.))
            .flex()
            .items_center()
            .gap(px(9.))
            .rounded(theme::RADIUS_2)
            .cursor_pointer()
            .text_color(t.text_primary)
            .hover(|row| row.bg(Tokens::get(cx).surface_hover))
            .on_click(move |_, window, cx| {
                close(window, cx);
                on_files(window, cx);
            })
            .child(ui::mark_box(
                16.,
                ui::icon(Icon::File, 14.).text_color(t.text_secondary),
            ))
            .child(ui::text("Choose files…", 12.5, FontWeight::NORMAL).flex_1())
    }

    fn row(&self, at: usize, item: &Value, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        let id = item["id"].as_str().unwrap_or_default().to_owned();
        let attaching = self.attaching.as_deref() == Some(id.as_str());
        let busy = self.attaching.is_some();
        let this = cx.entity();
        let (title, meta) = words(item);
        let remove = {
            let (this, id) = (this.clone(), id.clone());
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                this.update(cx, |menu, cx| menu.remove(id.clone(), window, cx));
            }
        };
        div()
            .id(("stash-item", at))
            .group(ROW)
            .px(px(6.))
            .py(px(5.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(theme::RADIUS_2)
            .when(!busy, |row| {
                let hover = t.surface_hover;
                row.cursor_pointer()
                    .hover(move |row| row.bg(hover))
                    .on_click(move |_, window, cx| {
                        let id = id.clone();
                        this.update(cx, |menu, cx| menu.attach(id, window, cx));
                    })
            })
            .when(attaching, |row| row.bg(t.surface_chosen))
            .when(busy && !attaching, |row| row.opacity(0.5))
            .child(self.tile(item, t, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .child(
                        ui::text(title, 12.5, FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .truncate(),
                    )
                    .child(
                        ui::text(meta, 10.5, FontWeight::NORMAL)
                            .text_color(t.text_tertiary)
                            .truncate(),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .invisible()
                    .group_hover(ROW, |slot| slot.visible())
                    .when(!busy, |slot| {
                        slot.child(ui::close_button(
                            ("stash-remove", at),
                            "Remove from Stash",
                            remove,
                            cx,
                        ))
                    }),
            )
            .into_any_element()
    }

    /// What the item is, at a glance: its picture, its page's picture, or its kind's glyph.
    fn tile(&self, item: &Value, t: &Tokens, cx: &App) -> AnyElement {
        let asks = self
            .bench
            .upgrade()
            .map(|bench| bench.read(cx).asks.clone());
        let picture = match (item["kind"].as_str(), asks) {
            (Some("image"), Some(asks)) => item["id"]
                .as_str()
                .map(|id| Picture::Remote(Remote::stashed(id.to_owned(), asks))),
            (Some("link"), Some(asks)) => item["preview"]["imageUrl"]
                .as_str()
                .map(|url| Picture::Remote(Remote::new(url.to_owned(), asks))),
            _ => None,
        };
        let glyph = glyph(item, t, cx);
        let tile = div()
            .size(px(TILE))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(theme::RADIUS_2)
            .overflow_hidden()
            .bg(t.surface_raised)
            .border_1()
            .border_color(t.line);
        match picture {
            Some(picture) => tile
                .child(
                    img(picture.source_at(px(TILE)))
                        .size(px(TILE))
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(move || glyph()),
                )
                .into_any_element(),
            None => tile.child(glyph()).into_any_element(),
        }
    }

    fn empty(&self, signed_in: bool, t: &Tokens) -> impl IntoElement {
        let (head, words) = match signed_in {
            true => (
                "Nothing in your Stash",
                "Share a link, a picture or a file to Sikemux from any app on your phone. It waits here until you attach it.",
            ),
            false => (
                "Your Stash is on your account",
                "Sign in to Sikemux to keep what you share from your phone here.",
            ),
        };
        div()
            .px(px(20.))
            .pt(px(18.))
            .pb(px(20.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .size(px(32.))
                    .mb(px(4.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(theme::RADIUS_2)
                    .bg(t.surface_raised)
                    .border_1()
                    .border_color(t.line)
                    .child(ui::icon(Icon::Stash, 15.).text_color(t.text_tertiary)),
            )
            .child(ui::text(head, 12.5, FontWeight::MEDIUM).text_color(t.text_primary))
            .child(
                div()
                    .max_w(px(260.))
                    .text_size(theme::ui_text(11.5))
                    .line_height(theme::ui_text(16.))
                    .text_color(t.text_tertiary)
                    .text_center()
                    .child(words),
            )
    }
}

/// The glyph for an item with no picture to show: its file's, or its kind's.
fn glyph(item: &Value, t: &Tokens, cx: &App) -> impl Fn() -> AnyElement + 'static {
    let ink = t.text_tertiary;
    let named = match item["kind"].as_str() {
        Some("file") => item["name"]
            .as_str()
            .map(|name| crate::file_icons::glyph(name, 16., cx)),
        _ => None,
    };
    let icon = match item["kind"].as_str() {
        Some("link") => Icon::Globe,
        Some("text") => Icon::Text,
        Some("image") => Icon::Image,
        Some("video") => Icon::Camera,
        _ => Icon::File,
    };
    let named = std::cell::RefCell::new(named);
    move || {
        named
            .borrow_mut()
            .take()
            .unwrap_or_else(|| ui::icon(icon, 15.).text_color(ink).into_any_element())
    }
}

/// The item's title and the line under it: where it came from or what it says, and when.
fn words(item: &Value) -> (String, String) {
    let text = |key: &str| {
        item[key]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    let url = text("url");
    let host = url.and_then(|url| url::Url::parse(url).ok()?.host_str().map(str::to_owned));
    let title = match item["kind"].as_str() {
        Some("link") => (item["preview"]["title"].as_str().map(str::trim))
            .filter(|title| !title.is_empty())
            .map(str::to_owned)
            .or_else(|| url.map(str::to_owned)),
        Some("text") => text("text").map(|text| text.lines().next().unwrap_or(text).to_owned()),
        _ => text("name").map(str::to_owned),
    }
    .unwrap_or_else(|| "Untitled".to_owned());
    let source = match item["kind"].as_str() {
        Some("link") => (item["preview"]["site"].as_str())
            .filter(|site| !site.is_empty())
            .map(str::to_owned)
            .or(host),
        Some("text") => Some("Text".to_owned()),
        _ => item["size"].as_u64().map(crate::codehost::bytes),
    };
    let ago = item["createdAt"].as_str().map(crate::codehost::ago);
    let meta = [text("note").map(|note| format!("“{note}”")), source, ago]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (title, meta)
}

impl Render for Menu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let read = self
            .bench
            .upgrade()
            .and_then(|bench| bench.read(cx).read("stash").cloned())
            .unwrap_or(Value::Null);
        let signed_in = read["signedIn"].as_bool().unwrap_or(false);
        let items: Vec<Value> = read["items"].as_array().cloned().unwrap_or_default();
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(at, item)| self.row(at, item, &t, cx))
            .collect();
        let count = (!items.is_empty()).then(|| items.len().to_string());
        div()
            .w(px(340.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(t.surface_overlay)
            .border_1()
            .border_color(t.border)
            .rounded(theme::RADIUS_3)
            .shadow(t.shadow_float())
            .child(self.files_row(&t, cx))
            .child(
                div()
                    .h(px(1.))
                    .mx(px(ui::PANEL_INSET + 3.))
                    .my(px(6.))
                    .bg(t.line),
            )
            .child(
                div()
                    .px(px(ui::PANEL_INSET + 8.))
                    .pb(px(4.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        ui::text("STASH", 10., FontWeight::MEDIUM)
                            .letter_spacing(0.08)
                            .text_color(t.text_tertiary),
                    )
                    .children(count.map(|count| {
                        ui::text(count, 10., FontWeight::MEDIUM).text_color(t.text_disabled)
                    })),
            )
            .map(|menu| match rows.is_empty() {
                true => menu.child(self.empty(signed_in, &t)),
                false => menu.child(
                    div()
                        .id("stash-items")
                        .px(px(ui::PANEL_INSET))
                        .pb(px(ui::PANEL_INSET))
                        .max_h(px(360.))
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .children(rows),
                ),
            })
    }
}
