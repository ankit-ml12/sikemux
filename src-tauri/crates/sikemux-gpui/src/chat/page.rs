//! A page an agent showed with `page_show`, drawn in its reply at the height the
//! page reports. The core keeps the page; its view keeps no storage and never
//! navigates, and a link the person clicks in it opens on the agent's desk.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::{
    div, px, AnyElement, App, Global, IntoElement, ParentElement, SharedString, Styled, Window,
};
use gpui_native_view::{native_view, NativeSurface};
use serde_json::json;
use sikemux_wire::chat::PageRef;

use super::Looks;
use crate::theme::{self, Tokens};

/// A page's height before it reports its own, when the agent gave none.
const DEFAULT_HEIGHT: f32 = 320.;
const MAX_HEIGHT: f32 = 4000.;

/// Every page the window has drawn, by id, so a chat drawn again keeps each one's view and height.
#[derive(Default)]
struct Pages {
    shown: HashMap<String, Shown>,
}

impl Global for Pages {}

struct Shown {
    height: Rc<Cell<f32>>,
    surface: Option<NativeSurface>,
}

pub(super) fn page(page: &PageRef, looks: &Looks, window: &mut Window, cx: &mut App) -> AnyElement {
    if !cx.has_global::<Pages>() {
        cx.set_global(Pages::default());
    }
    let known = cx
        .global::<Pages>()
        .shown
        .get(&page.id)
        .map(|shown| (shown.height.get(), shown.surface.clone()));
    let (height, surface) = match known {
        Some(known) => known,
        None => {
            let height = page.height.map_or(DEFAULT_HEIGHT, |height| height as f32);
            cx.global_mut::<Pages>().shown.insert(
                page.id.clone(),
                Shown {
                    height: Rc::new(Cell::new(height)),
                    surface: None,
                },
            );
            fetch(page, looks, window, cx);
            (height, None)
        }
    };
    div()
        .relative()
        .w_full()
        .h(px(height.clamp(1., MAX_HEIGHT)))
        .children(crate::ui::spot(|| format!("chat-page-{}", page.id)))
        .children(surface.map(|surface| {
            native_view(SharedString::from(format!("page-{}", page.id)), &surface).size_full()
        }))
        .into_any_element()
}

/// Asks the core for the page, then shows it in a view of its own.
fn fetch(page: &PageRef, looks: &Looks, window: &mut Window, cx: &mut App) {
    let Some(target) = looks
        .bench
        .upgrade()
        .map(|bench| bench.read(cx).chat_target(&looks.agent))
    else {
        return;
    };
    let answer = target.request("page", json!({ "id": page.id }));
    let theme = theme::page_rule(Tokens::get(cx), opaque_ground(Tokens::get(cx)));
    let (id, reader) = (page.id.clone(), looks.reader.clone());
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let (bench, agent) = (looks.bench.clone(), looks.agent.clone());
    window
        .spawn(cx, async move |cx| {
            let Ok(answer) = answer.await else {
                return;
            };
            let Some(html) = answer["html"].as_str() else {
                return;
            };
            let html = sikemux_core::pages::themed(html, &theme, None);
            let (events, mut heard) = tokio::sync::mpsc::unbounded_channel();
            let _ = cx.update(|window, cx| {
                let surface = shown_view(&html, events, window, cx);
                if let Some(shown) = cx.global_mut::<Pages>().shown.get_mut(&id) {
                    shown.surface = surface;
                }
                reader.update(cx, |_, cx| cx.notify());
            });
            while let Some(event) = heard.recv().await {
                #[cfg_attr(
                    not(any(target_os = "macos", target_os = "linux")),
                    allow(unused_variables)
                )]
                let applied = cx.update(|_, cx| match event {
                    #[cfg(any(target_os = "macos", target_os = "linux"))]
                    sikemux_browser::page_view::PageEvent::Height(height) => {
                        if let Some(shown) = cx.global::<Pages>().shown.get(&id) {
                            shown.height.set((height as f32).clamp(1., MAX_HEIGHT));
                        }
                        reader.update(cx, |_, cx| cx.notify());
                    }
                    #[cfg(any(target_os = "macos", target_os = "linux"))]
                    sikemux_browser::page_view::PageEvent::Link(url) => {
                        let _ = bench.update(cx, |bench, _| bench.open_on_desk(&agent, &url));
                    }
                    #[cfg(any(target_os = "macos", target_os = "linux"))]
                    sikemux_browser::page_view::PageEvent::Wheel(delta) => {
                        let _ = bench.update(cx, |bench, cx| {
                            if let Some(chat) = bench.chats.get(&agent) {
                                chat.transcript.update(cx, |transcript, cx| {
                                    transcript.list().scroll_by(px(delta as f32));
                                    cx.notify();
                                });
                            }
                        });
                    }
                });
                if applied.is_err() {
                    break;
                }
            }
        })
        .detach();
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
type Events = tokio::sync::mpsc::UnboundedSender<sikemux_browser::page_view::PageEvent>;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
type Events = tokio::sync::mpsc::UnboundedSender<std::convert::Infallible>;

/// A page on Linux is a window of its own, which has nothing behind it to show through, so it
/// draws the window's ground itself.
fn opaque_ground(t: &Tokens) -> Option<gpui_kit::Hsla> {
    cfg!(target_os = "linux").then(|| theme::page_ground(t))
}

#[cfg(target_os = "macos")]
fn shown_view(html: &str, events: Events, window: &Window, _: &App) -> Option<NativeSurface> {
    use sikemux_browser::page_view::PageView;
    let mtm = objc2::MainThreadMarker::new()?;
    let page = PageView::new(mtm, html, move |event| {
        let _ = events.send(event);
    });
    let view = gpui_native_view::OwnedView::new(page.view(), window)?;
    Some(NativeSurface::new(Shows { _page: page, view }))
}

#[cfg(target_os = "linux")]
fn shown_view(html: &str, events: Events, window: &Window, cx: &App) -> Option<NativeSurface> {
    let parent = gpui_native_view::window_x11(window)?;
    let ground = theme::page_ground_rgb(Tokens::get(cx));
    let page = sikemux_browser::page_view::PageView::new(parent, html, ground, move |event| {
        let _ = events.send(event);
    });
    match page {
        Ok(page) => Some(NativeSurface::new(Shows(page))),
        Err(error) => {
            eprintln!("sikemux: an agent's page could not be shown: {error}");
            None
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn shown_view(_: &str, _: Events, _: &Window, _: &App) -> Option<NativeSurface> {
    None
}

/// The page's view placed in its row, kept with the page that fills it.
#[cfg(target_os = "macos")]
struct Shows {
    _page: sikemux_browser::page_view::PageView,
    view: gpui_native_view::OwnedView,
}

#[cfg(target_os = "macos")]
impl gpui_native_view::Surface for Shows {
    fn place(&self, frame: Option<gpui_native_view::Frame>) {
        self.view.place(frame);
    }

    fn responder(&self) -> Option<gpui_native_view::Retained<gpui_native_view::NSView>> {
        self.view.responder()
    }
}

/// The page's own window, placed in its row and cut to the part of the row that shows.
#[cfg(target_os = "linux")]
struct Shows(sikemux_browser::page_view::PageView);

#[cfg(target_os = "linux")]
impl gpui_native_view::Surface for Shows {
    fn place(&self, frame: Option<gpui_native_view::Frame>) {
        let rect = |bounds: gpui_kit::Bounds<gpui_kit::Pixels>| {
            (
                f64::from(bounds.left()),
                f64::from(bounds.top()),
                f64::from(bounds.size.width),
                f64::from(bounds.size.height),
            )
        };
        self.0
            .place(frame.map(|frame| (rect(frame.bounds), rect(frame.visible))));
    }
}
