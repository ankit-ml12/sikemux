//! What an agent's gestures leave in a chat: emoji on the message it answered,
//! and GIFs under its reply.

use gpui_kit::{
    div, img, px, AnyElement, App, FontWeight, IntoElement, ObjectFit, ParentElement, Styled,
    StyledImage,
};
use sikemux_wire::chat::ChatMessage;

use super::Drawn;
use crate::picture::{Picture, Remote};
use crate::theme::{self, Tokens};
use crate::ui;

/// The longest side a GIF is drawn at, in points.
const GIF_SIDE: f32 = 240.;
/// How far the emoji hang over the bottom of the bubble.
const OVERHANG: f32 = 8.;

/// The emoji the agent reacted with, hanging off the bottom of the message it answered.
pub fn reactions(message: &ChatMessage, looks: &Drawn) -> Option<AnyElement> {
    if message.reactions.is_empty() {
        return None;
    }
    Some(
        div()
            .mt(px(-OVERHANG))
            .mr(px(10.))
            .flex()
            .gap(px(4.))
            .children(message.reactions.iter().map(|emoji| {
                div()
                    .px(px(6.))
                    .py(px(1.))
                    .rounded_full()
                    .border_1()
                    .border_color(looks.line)
                    .bg(looks.bubble)
                    .child(ui::text(emoji.clone(), 13., FontWeight::NORMAL))
            }))
            .into_any_element(),
    )
}

/// The GIFs the agent sent in its turn, under its reply, each credited as KLIPY asks.
pub fn gifs(message: &ChatMessage, looks: &Drawn, cx: &App) -> Option<AnyElement> {
    if message.gifs.is_empty() {
        return None;
    }
    let asks = looks.bench.upgrade()?.read(cx).asks.clone();
    let shown = message.gifs.iter().map(|gif| {
        let (width, height) = fit(gif.width, gif.height);
        let picture = Picture::Remote(Remote::new(gif.url.clone(), asks.clone()));
        div()
            .w(px(width))
            .h(px(height))
            .rounded(theme::RADIUS_3)
            .overflow_hidden()
            .bg(looks.bubble)
            .child(
                img(picture.source())
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
    });
    Some(
        div()
            .pt(px(4.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().flex().flex_wrap().gap(px(8.)).children(shown))
            .child(
                ui::text("Powered by KLIPY", 10., FontWeight::NORMAL)
                    .text_color(Tokens::get(cx).text_tertiary),
            )
            .into_any_element(),
    )
}

/// A GIF's size at no more than `GIF_SIDE` along its longer side, keeping its shape.
fn fit(width: u32, height: u32) -> (f32, f32) {
    let (width, height) = (width.max(1) as f32, height.max(1) as f32);
    let scale = (GIF_SIDE / width.max(height)).min(1.);
    (width * scale, height * scale)
}
