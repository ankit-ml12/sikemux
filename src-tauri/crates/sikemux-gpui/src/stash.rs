//! The account's Stash: what the person shared into Sikemux from other apps, kept on their
//! account for every device. It has a place in the rail and a pane of its own; an item goes into
//! a chat's draft from the pane, by dragging it onto the chat, from the chip over a composer when
//! items are new, or with `&` in the draft. The chat's core puts it there.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, AnyElement, App, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ObjectFit, ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled,
    StyledImage, Window,
};
use serde_json::{json, Value};

use crate::icons::Icon;
use crate::picture::{Picture, Remote};
use crate::theme::{self, Tokens};
use crate::ui::{self, IconButton, Row};
use crate::workbench::Workbench;
use sikemux_wire::protocol::PaneKind;

/// The narrowest a card gets before the grid drops a column, and the gap between cards.
const MIN_CARD: f32 = 260.;
const GAP: f32 = 14.;
const PREVIEW: f32 = 132.;
/// How wide a card's picture is decoded, enough for the widest a card stretches to.
const PICTURE: f32 = 440.;

/// A stash item being dragged onto a chat.
#[derive(Clone)]
pub struct DraggedStash {
    pub id: String,
}

/// The stash as the core last read it.
pub struct Stash<'a> {
    pub signed_in: bool,
    pub items: Vec<&'a Value>,
    seen_at: Option<&'a str>,
}

impl<'a> Stash<'a> {
    pub fn of(bench: &'a Workbench) -> Self {
        let read = bench.read("stash");
        Self {
            signed_in: read.is_some_and(|read| read["signedIn"].as_bool() == Some(true)),
            items: read
                .and_then(|read| read["items"].as_array())
                .map_or_else(Vec::new, |items| items.iter().collect()),
            seen_at: read.and_then(|read| read["seenAt"].as_str()),
        }
    }

    /// The items stashed since the person last looked, on any device.
    pub fn new_items(&self) -> Vec<&'a Value> {
        self.items
            .iter()
            .copied()
            .filter(
                |item| match (self.seen_at.and_then(epoch_ms), created(item)) {
                    (Some(seen), Some(at)) => at > seen,
                    _ => self.seen_at.is_none(),
                },
            )
            .collect()
    }

    /// When the newest item was stashed, as the server wrote it.
    fn newest(&self) -> Option<&'a str> {
        self.items
            .iter()
            .filter_map(|item| item["createdAt"].as_str())
            .max_by_key(|at| epoch_ms(at))
    }
}

fn epoch_ms(at: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(at)
        .ok()
        .map(|at| at.timestamp_millis())
}

fn created(item: &Value) -> Option<i64> {
    item["createdAt"].as_str().and_then(epoch_ms)
}

/// What an item is called: its page's title, its text's first line, or its file's name.
pub fn title(item: &Value) -> String {
    let text = |key: &str| {
        item[key]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    match item["kind"].as_str() {
        Some("link") => text("url").map(|url| {
            (item["preview"]["title"].as_str().map(str::trim))
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| {
                    url.trim_start_matches("https://")
                        .trim_start_matches("http://")
                })
                .to_owned()
        }),
        Some("text") => text("text").map(|text| text.lines().next().unwrap_or(text).to_owned()),
        _ => text("name").map(str::to_owned),
    }
    .unwrap_or_else(|| "Untitled".to_owned())
}

/// Where a link goes or what the item is, then how long ago it was stashed.
pub fn place(item: &Value) -> String {
    let what = match item["kind"].as_str() {
        Some("link") => (item["preview"]["site"].as_str())
            .filter(|site| !site.is_empty())
            .map(str::to_owned)
            .or_else(|| site_of(item["url"].as_str()?)),
        Some("text") => Some("Text".to_owned()),
        _ => item["size"].as_u64().map(crate::codehost::bytes),
    };
    let ago = item["createdAt"].as_str().map(crate::codehost::ago);
    [what, ago]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn site_of(url: &str) -> Option<String> {
    url::Url::parse(url).ok()?.host_str().map(str::to_owned)
}

/// Whether the item's title, note, link or text holds every word of `query`, ignoring case.
pub fn matches(item: &Value, query: &str) -> bool {
    let said = [
        Some(title(item)),
        item["note"].as_str().map(str::to_owned),
        item["url"].as_str().map(str::to_owned),
        item["text"].as_str().map(str::to_owned),
        item["preview"]["site"].as_str().map(str::to_owned),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase();
    query
        .to_lowercase()
        .split(|c: char| c.is_whitespace() || c == '&')
        .filter(|word| !word.is_empty())
        .all(|word| said.contains(word))
}

/// The glyph of an item with no picture to show: its file's, or its kind's.
pub fn glyph(item: &Value, size: f32, cx: &App) -> AnyElement {
    let ink = Tokens::get(cx).text_tertiary;
    if item["kind"].as_str() == Some("file") {
        if let Some(name) = item["name"].as_str() {
            return crate::file_icons::glyph(name, size, cx);
        }
    }
    let icon = match item["kind"].as_str() {
        Some("link") => Icon::Globe,
        Some("text") => Icon::Text,
        Some("image") => Icon::Image,
        Some("video") => Icon::Camera,
        _ => Icon::File,
    };
    ui::icon(icon, size).text_color(ink).into_any_element()
}

/// The item's picture, or its page's, when it has one, which the core fetches.
fn picture(bench: &Workbench, item: &Value) -> Option<Picture> {
    let asks = bench.asks.clone();
    match item["kind"].as_str()? {
        "image" => Some(Picture::Remote(Remote::stashed(
            item["id"].as_str()?.to_owned(),
            asks,
        ))),
        "link" => Some(Picture::Remote(Remote::new(
            item["preview"]["imageUrl"].as_str()?.to_owned(),
            asks,
        ))),
        _ => None,
    }
}

/// The item small, as a mark beside its words: its picture, else its glyph.
pub fn mark(bench: &Workbench, item: &Value, size: f32, cx: &App) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let tile = div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_1)
        .overflow_hidden()
        .bg(t.surface_raised);
    match picture(bench, item) {
        Some(shown) => {
            let fallback = glyph(item, size * 0.6, cx);
            let fallback = std::cell::RefCell::new(Some(fallback));
            tile.child(
                img(shown.source_at(px(size)))
                    .size(px(size))
                    .object_fit(ObjectFit::Cover)
                    .with_fallback(move || {
                        fallback
                            .borrow_mut()
                            .take()
                            .unwrap_or_else(|| div().into_any_element())
                    }),
            )
            .into_any_element()
        }
        None => tile.child(glyph(item, size * 0.66, cx)).into_any_element(),
    }
}

impl Workbench {
    /// Puts the stash items `ids` in the draft of `agent`'s chat, through the chat's core. What
    /// is typed is kept first, so the core adds them after it.
    pub fn add_from_stash(&mut self, agent: &str, ids: Vec<String>, cx: &mut Context<Self>) {
        self.keep_draft(agent, cx);
        let target = self.chat_target(agent);
        for id in ids {
            let asked = target.request("attachStash", json!({ "id": id, "agentId": agent }));
            cx.spawn(async move |bench, cx| {
                if let Err(error) = asked.await {
                    bench
                        .update(cx, |bench, cx| {
                            let words = format!("Couldn't add that from your Stash: {error}");
                            bench.notify(crate::toast::error(words), cx)
                        })
                        .ok();
                }
            })
            .detach();
        }
    }

    /// Takes stash item `id` off the account, on every device.
    pub fn remove_from_stash(&self, id: &str, cx: &mut Context<Self>) {
        let asked = crate::core::request(&self.asks, "removeStash", json!({ "id": id }));
        cx.spawn(async move |bench, cx| {
            if let Err(error) = asked.await {
                bench
                    .update(cx, |bench, cx| {
                        let words = format!("Couldn't remove that from your Stash: {error}");
                        bench.notify(crate::toast::error(words), cx)
                    })
                    .ok();
            }
        })
        .detach();
    }

    /// Opens a link in the browser, copies text, and opens a file in the app the system picks
    /// once this host has kept it.
    pub fn open_stash_item(&self, item: &Value, cx: &mut Context<Self>) {
        match item["kind"].as_str() {
            Some("link") => {
                if let Some(url) = item["url"].as_str() {
                    cx.open_url(url);
                }
            }
            Some("text") => {
                let text = item["text"].as_str().unwrap_or_default().to_owned();
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
                self.notify(crate::toast::success("Copied"), cx);
            }
            _ => {
                let Some(id) = item["id"].as_str() else {
                    return;
                };
                let asked = crate::core::request(&self.asks, "stashFile", json!({ "id": id }));
                cx.spawn(async move |bench, cx| match asked.await {
                    Ok(kept) => {
                        if let Some(path) = kept["path"].as_str() {
                            let path = std::path::PathBuf::from(path);
                            cx.update(|cx| cx.open_with_system(&path));
                        }
                    }
                    Err(error) => {
                        bench
                            .update(cx, |bench, cx| {
                                let words = format!("Couldn't open that: {error}");
                                bench.notify(crate::toast::error(words), cx)
                            })
                            .ok();
                    }
                })
                .detach();
            }
        }
    }

    /// The person has seen everything in the stash, which every device then knows.
    pub fn see_stash(&self) {
        if let Some(at) = Stash::of(self).newest() {
            let _ = crate::core::request(&self.asks, "seeStash", json!({ "at": at }));
        }
    }

    /// Opens the Stash's pane in the strip in front, and marks the stash seen.
    pub fn open_stash(&self) {
        if let Some(strip) = self.strip() {
            let params = json!({ "kind": PaneKind::Stash, "target": "stash" });
            self.call_in_front("openTab", strip.with(params));
        }
        self.see_stash();
    }

    /// Whether the Stash's pane is the one in front.
    pub fn stash_in_front(&self) -> bool {
        self.tab()
            .and_then(|tab| crate::tabs::first_view(&tab.pane))
            .is_some_and(|(kind, _)| kind == PaneKind::Stash)
    }
}

/// The Stash's place in the rail, with how many items wait in it.
pub fn rail_row(bench: &Workbench, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let stash = Stash::of(bench);
    let count = (stash.signed_in && !stash.items.is_empty()).then(|| {
        div()
            .h(px(16.))
            .min_w(px(16.))
            .px(px(5.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(t.surface_chosen)
            .text_size(theme::ui_text(10.5))
            .font_weight(FontWeight::MEDIUM)
            .font_features(theme::tabular())
            .text_color(t.text_secondary)
            .child(stash.items.len().to_string())
    });
    Row::new("rail-stash", "Stash")
        .mark(16., ui::icon(Icon::Stack, 13.).text_color(t.text_secondary))
        .quiet()
        .chosen(bench.settings.is_none() && bench.stash_in_front())
        .status(count)
        .on_click(cx.listener(|bench, _, _, _| bench.open_stash()))
        .into_any_element()
}

/// Over a chat's composer while items came into the stash since the person last looked:
/// how many, Add to put them all in this chat's draft, and × to leave them.
pub fn chip(bench: &Workbench, agent: &str, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let stash = Stash::of(bench);
    let offering = bench
        .chats
        .get(agent)
        .and_then(|chat| chat.composer.as_ref())
        .is_some_and(|composing| composing.menu.is_some());
    if !stash.signed_in || offering {
        return None;
    }
    let fresh: Vec<String> = stash
        .new_items()
        .iter()
        .filter(|item| item["takenAt"].is_null())
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect();
    if fresh.is_empty() {
        return None;
    }
    let t = Tokens::get(cx).clone();
    let words = match fresh.len() {
        1 => "1 new in Stash".to_owned(),
        count => format!("{count} new in Stash"),
    };
    let add = {
        let agent = agent.to_owned();
        cx.listener(move |bench, _, _, cx| {
            bench.add_from_stash(&agent, fresh.clone(), cx);
            bench.see_stash();
        })
    };
    let dot = || div().text_color(t.text_disabled).child("·");
    Some(
        div()
            .w_full()
            .px(px(16.))
            .flex()
            .justify_center()
            .child(
                div().w_full().max_w(px(820.)).pb(px(8.)).flex().child(
                    div()
                        .h(px(26.))
                        .pl(px(9.))
                        .pr(px(3.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .rounded_full()
                        .border_1()
                        .border_color(t.line)
                        .bg(t.surface_raised)
                        .text_size(theme::ui_text(12.))
                        .text_color(t.text_secondary)
                        .child(ui::icon(Icon::Stack, 12.).text_color(t.text_tertiary))
                        .child(
                            div()
                                .id("stash-chip-open")
                                .cursor_pointer()
                                .hover(move |words| words.text_color(t.text_primary))
                                .on_click(cx.listener(|bench, _, _, _| bench.open_stash()))
                                .child(words),
                        )
                        .child(dot())
                        .child(
                            div()
                                .id("stash-chip-add")
                                .cursor_pointer()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.accent)
                                .on_click(add)
                                .child("Add"),
                        )
                        .child(
                            IconButton::new("stash-chip-leave", Icon::Close, "Not now")
                                .small()
                                .on_click(cx.listener(|bench, _, _, _| bench.see_stash())),
                        ),
                ),
            )
            .into_any_element(),
    )
}

/// The Stash's pane: every item as a card with its preview, to add to a chat, open or remove,
/// or drag onto a chat.
pub fn pane(bench: &Workbench, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let stash = Stash::of(bench);
    let count = stash.items.len();
    let head = div()
        .flex_none()
        .px(px(24.))
        .pt(px(20.))
        .pb(px(14.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::icon(Icon::Stack, 15.).text_color(t.text_secondary))
                .child(ui::text("Stash", 15., FontWeight::SEMIBOLD).text_color(t.text_primary))
                .when(stash.signed_in && count > 0, |head| {
                    head.child(
                        ui::text(count.to_string(), 12., FontWeight::MEDIUM)
                            .text_color(t.text_tertiary),
                    )
                }),
        )
        .child(
            ui::text(
                "What you share to Sikemux from your phone. Add it to a chat, or drag it onto one.",
                12.,
                FontWeight::NORMAL,
            )
            .text_color(t.text_tertiary),
        );
    let body = if !stash.signed_in || stash.items.is_empty() {
        empty(stash.signed_in, &t).into_any_element()
    } else {
        let columns = cx.try_global::<Columns>().map_or(2, |columns| columns.0);
        let cards: Vec<(AnyElement, f32)> = stash
            .items
            .iter()
            .enumerate()
            .map(|(at, item)| (card(bench, at, item, cx), tall(item)))
            .collect();
        div()
            .id("stash-cards")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(24.))
            .pb(px(24.))
            .child(grid(cards, columns))
            .into_any_element()
    };
    ui::pane(cx)
        .child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(head)
                .child(div().h(px(1.)).mx(px(24.)).mb(px(16.)).bg(t.line))
                .child(body),
        )
        .into_any_element()
}

/// How many columns the stash's grid had room for when it was last drawn.
#[derive(Clone, Copy, PartialEq)]
struct Columns(u16);

impl gpui_kit::Global for Columns {}

/// Roughly how tall a card draws, for placing it in the shortest column: a compact link's is a
/// row, any other's has a picture.
fn tall(item: &Value) -> f32 {
    let compact = item["kind"].as_str() == Some("link") && item["preview"]["imageUrl"].is_null();
    let noted = item["note"]
        .as_str()
        .is_some_and(|note| !note.trim().is_empty());
    match (compact, noted) {
        (true, true) => 128.,
        (true, false) => 106.,
        (false, _) if item["kind"].as_str() == Some("text") => 352.,
        (false, _) => 372.,
    }
}

/// The cards in as many columns as fit, each column as wide as the others and each card going
/// under the shortest column so far, so short cards stack rather than leave gaps.
fn grid(cards: Vec<(AnyElement, f32)>, columns: u16) -> impl IntoElement {
    let mut stacks: Vec<(f32, Vec<AnyElement>)> =
        (0..columns.max(1)).map(|_| (0., Vec::new())).collect();
    for (card, tall) in cards {
        if let Some(shortest) = stacks.iter_mut().min_by(|a, b| a.0.total_cmp(&b.0)) {
            shortest.0 += tall + GAP;
            shortest.1.push(card);
        }
    }
    div()
        .w_full()
        .on_children_prepainted(|bounds, window, cx| {
            let Some(width) = bounds.first().map(|bounds| f32::from(bounds.size.width)) else {
                return;
            };
            let fit = Columns(((width + GAP) / (MIN_CARD + GAP)).floor().max(1.) as u16);
            if cx.try_global::<Columns>() != Some(&fit) {
                cx.set_global(fit);
                window.refresh();
            }
        })
        .child(
            div()
                .w_full()
                .flex()
                .items_start()
                .gap(px(GAP))
                .children(stacks.into_iter().map(|(_, cards)| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(GAP))
                        .children(cards)
                })),
        )
}

fn empty(signed_in: bool, t: &Tokens) -> Div {
    let (head, words) = match signed_in {
        true => (
            "Nothing in your Stash",
            "Share a link, a picture or a file to Sikemux from any app on your phone. It waits here until you add it to a chat.",
        ),
        false => (
            "Your Stash is on your account",
            "Sign in to Sikemux to keep what you share from your phone here.",
        ),
    };
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .pb(px(64.))
        .child(
            div()
                .size(px(40.))
                .mb(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::RADIUS_3)
                .bg(t.surface_raised)
                .border_1()
                .border_color(t.line)
                .child(ui::icon(Icon::Stack, 18.).text_color(t.text_tertiary)),
        )
        .child(ui::text(head, 14., FontWeight::MEDIUM).text_color(t.text_primary))
        .child(
            div()
                .max_w(px(340.))
                .text_size(theme::ui_text(12.5))
                .line_height(theme::ui_text(18.))
                .text_color(t.text_tertiary)
                .text_center()
                .child(words),
        )
}

/// One item: its preview over its title, where it is from and its note, then what to do with it.
fn card(bench: &Workbench, at: usize, item: &Value, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let id = item["id"].as_str().unwrap_or_default().to_owned();
    let note = item["note"]
        .as_str()
        .map(str::trim)
        .filter(|note| !note.is_empty())
        .map(|note| format!("“{note}”"));
    let dragged = DraggedStash { id: id.clone() };
    let label: SharedString = title(item).into();
    let ghost = label.clone();
    let compact = item["kind"].as_str() == Some("link") && item["preview"]["imageUrl"].is_null();
    let words = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .when(item["kind"].as_str() != Some("text"), |words| {
            words.child(
                ui::text(label, 12.5, FontWeight::MEDIUM)
                    .text_color(t.text_primary)
                    .truncate(),
            )
        })
        .child(
            ui::text(place(item), 10.5, FontWeight::NORMAL)
                .text_color(t.text_tertiary)
                .truncate(),
        );
    let body = match compact {
        true => div()
            .px(px(12.))
            .pt(px(12.))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(favicon(bench, item, &t, cx))
            .child(words.children(note.map(|note| {
                ui::text(note, 11.5, FontWeight::NORMAL)
                    .pt(px(3.))
                    .text_color(t.text_secondary)
                    .truncate()
            }))),
        false => div().px(px(12.)).pt(px(10.)).child(
            words.child(
                ui::text(note.unwrap_or_default(), 11.5, FontWeight::NORMAL)
                    .h(px(19.))
                    .pt(px(3.))
                    .text_color(t.text_secondary)
                    .truncate(),
            ),
        ),
    };
    div()
        .id(("stash-card", at))
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .rounded(theme::RADIUS_3)
        .overflow_hidden()
        .border_1()
        .border_color(t.line)
        .bg(t.surface_raised)
        .hover(move |card| card.border_color(t.border_strong))
        .on_drag(dragged, move |_: &DraggedStash, _: Point<Pixels>, _, cx| {
            crate::drag::ghost(ghost.clone(), cx)
        })
        .when(!compact, |card| card.child(preview(bench, item, &t, cx)))
        .child(body)
        .child(actions(bench, at, item, &id, cx))
        .into_any_element()
}

/// A link's site in a small tile: its icon when the site has one, else a globe.
fn favicon(bench: &Workbench, item: &Value, t: &Tokens, cx: &App) -> AnyElement {
    let tile = div()
        .size(px(32.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_2)
        .overflow_hidden()
        .bg(t.surface_sunken)
        .border_1()
        .border_color(t.line);
    let icon = item["url"].as_str().and_then(|url| {
        let url = url::Url::parse(url).ok()?;
        Some(format!(
            "{}://{}/favicon.ico",
            url.scheme(),
            url.host_str()?
        ))
    });
    match icon {
        Some(icon) => {
            let fallback = std::cell::RefCell::new(Some(glyph(item, 15., cx)));
            let shown = Picture::Remote(Remote::new(icon, bench.asks.clone()));
            tile.child(
                img(shown.source_at(px(18.)))
                    .size(px(18.))
                    .object_fit(ObjectFit::Contain)
                    .with_fallback(move || {
                        fallback
                            .borrow_mut()
                            .take()
                            .unwrap_or_else(|| div().into_any_element())
                    }),
            )
            .into_any_element()
        }
        None => tile.child(glyph(item, 15., cx)).into_any_element(),
    }
}

/// The top of a card: a photo or a page's picture filling it, a text's opening lines, or a
/// file's glyph.
fn preview(bench: &Workbench, item: &Value, t: &Tokens, cx: &App) -> AnyElement {
    let frame = div()
        .h(px(PREVIEW))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(t.surface_sunken)
        .border_b_1()
        .border_color(t.line);
    let plain = |cx: &App| {
        let site = match item["kind"].as_str() {
            Some("link") => item["url"].as_str().and_then(site_of),
            Some("file" | "video" | "image") => item["name"]
                .as_str()
                .and_then(|name| name.rsplit_once('.'))
                .map(|(_, extension)| extension.to_uppercase()),
            _ => None,
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .child(glyph(item, 26., cx))
            .children(site.map(|site| {
                ui::text(site, 10.5, FontWeight::MEDIUM)
                    .letter_spacing(0.04)
                    .text_color(Tokens::get(cx).text_tertiary)
            }))
            .into_any_element()
    };
    if item["kind"].as_str() == Some("text") {
        let text = item["text"].as_str().unwrap_or_default().trim().to_owned();
        return frame
            .items_start()
            .justify_start()
            .p(px(12.))
            .child(
                div()
                    .h_full()
                    .overflow_hidden()
                    .text_size(theme::ui_text(12.))
                    .line_height(theme::ui_text(17.))
                    .text_color(t.text_secondary)
                    .child(text),
            )
            .into_any_element();
    }
    match picture(bench, item) {
        Some(shown) => {
            let fallback = std::cell::RefCell::new(Some(plain(cx)));
            frame
                .child(
                    img(shown.source_at(px(PICTURE)))
                        .w_full()
                        .h(px(PREVIEW))
                        .object_fit(ObjectFit::Cover)
                        .with_fallback(move || {
                            fallback
                                .borrow_mut()
                                .take()
                                .unwrap_or_else(|| div().into_any_element())
                        }),
                )
                .into_any_element()
        }
        None => frame.child(plain(cx)).into_any_element(),
    }
}

/// Add to chat, into the chat in front or one picked from the open chats; Open; Remove.
fn actions(
    bench: &Workbench,
    at: usize,
    item: &Value,
    id: &str,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let agents: Vec<(String, String)> = bench.shown().map_or_else(Vec::new, |view| {
        view.workspace
            .agents
            .iter()
            .map(|agent| {
                let name = agent.name.clone().unwrap_or_else(|| agent.title.clone());
                (agent.id.clone(), name)
            })
            .collect()
    });
    let focused = bench
        .focused_agent()
        .and_then(|focused| agents.iter().find(|(agent, _)| agent == focused))
        .cloned();
    let this = cx.entity().downgrade();
    let chooser = move |agents: Vec<(String, String)>, id: String| {
        let this = this.clone();
        move |mut menu: gpui_kit::component::menu::PopupMenu,
              _: &mut Window,
              _: &mut Context<gpui_kit::component::menu::PopupMenu>| {
            for (agent, name) in &agents {
                let (agent, id, this) = (agent.clone(), id.clone(), this.clone());
                menu = menu.item(
                    gpui_kit::component::menu::PopupMenuItem::new(name.clone()).on_click(
                        move |_, _, cx| {
                            this.update(cx, |bench, cx| {
                                bench.add_from_stash(&agent, vec![id.clone()], cx)
                            })
                            .ok();
                        },
                    ),
                );
            }
            menu
        }
    };
    let button =
        || ui::Button::new(("stash-add", at), "Add to chat", ui::Variant::Outline).icon(Icon::Plus);
    let add = match focused {
        Some((agent, name)) => {
            let others: Vec<(String, String)> = agents
                .iter()
                .filter(|(other, _)| *other != agent)
                .cloned()
                .collect();
            let picker = (!others.is_empty()).then(|| {
                ui::menu_button(
                    ("stash-pick", at),
                    gpui_kit::Anchor::TopLeft,
                    IconButton::new(
                        ("stash-pick-face", at),
                        Icon::Chevron,
                        "Add to another chat",
                    )
                    .small()
                    .turned(std::f32::consts::FRAC_PI_2),
                    chooser(others, id.to_owned()),
                )
            });
            let id = id.to_owned();
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .child(
                    button()
                        .tooltip(format!("Add to {name}"))
                        .on_click(cx.listener(move |bench, _, _, cx| {
                            bench.add_from_stash(&agent, vec![id.clone()], cx)
                        })),
                )
                .children(picker)
                .into_any_element()
        }
        None if !agents.is_empty() => ui::menu_button(
            ("stash-choose", at),
            gpui_kit::Anchor::TopLeft,
            button(),
            chooser(agents, id.to_owned()),
        )
        .into_any_element(),
        None => button()
            .disabled(true)
            .tooltip("Open a chat to add this to it")
            .into_any_element(),
    };
    let (opened, removed) = (item.clone(), id.to_owned());
    let open_label = match item["kind"].as_str() {
        Some("link") => "Open in browser",
        Some("text") => "Copy",
        _ => "Open",
    };
    let open_icon = match item["kind"].as_str() {
        Some("text") => Icon::Copy,
        Some("link") => Icon::Globe,
        _ => Icon::Eye,
    };
    div()
        .px(px(8.))
        .pt(px(10.))
        .pb(px(8.))
        .flex()
        .items_center()
        .gap(px(2.))
        .child(add)
        .child(div().flex_1())
        .child(
            IconButton::new(("stash-open", at), open_icon, open_label)
                .small()
                .on_click(cx.listener(move |bench, _, _, cx| bench.open_stash_item(&opened, cx))),
        )
        .child(
            IconButton::new(("stash-remove", at), Icon::Trash, "Remove from Stash")
                .small()
                .on_click(
                    cx.listener(move |bench, _, _, cx| bench.remove_from_stash(&removed, cx)),
                ),
        )
}

/// Takes stash items dropped on `agent`'s chat into its draft.
pub fn takes_drops(pane: Div, agent: &str, cx: &mut Context<Workbench>) -> Div {
    let (agent, this) = (agent.to_owned(), cx.weak_entity());
    pane.on_drop(move |dropped: &DraggedStash, _: &mut Window, cx| {
        this.update(cx, |bench, cx| {
            bench.add_from_stash(&agent, vec![dropped.id.clone()], cx)
        })
        .ok();
    })
}
