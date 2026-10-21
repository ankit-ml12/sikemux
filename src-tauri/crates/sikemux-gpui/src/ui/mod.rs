//! The shared component set. Screens are built from these; one a screen needs
//! that is missing belongs here, once.

mod controls;
mod floats;
mod lists;
mod pane;

pub use controls::{Button, Field, IconButton, Segmented, Toggle, Variant};
pub use floats::{tooltip, Menu, MenuItem};
pub use lists::{Group, Row, Tab, ROW};
pub use pane::{pane, PaneHeader};

use gpui_kit::{
    div, img, px, svg, AnyElement, App, Div, FontFallbacks, FontWeight, Hsla, IntoElement,
    ParentElement, SharedString, Styled, Svg,
};

use sikemux_wire::protocol::AgentStatus;

use crate::icons::{self, Icon};
use crate::theme::{self, Tokens};

pub fn icon(icon: Icon, size: f32) -> Svg {
    svg().path(icon.path()).size(px(size)).flex_none()
}

/// A mark at `size`: a picture as it is drawn, or a glyph in `tint`.
pub fn picture(glyph: Icon, size: f32, tint: Hsla) -> AnyElement {
    if glyph.is_picture() {
        img(glyph.path()).size(px(size)).flex_none().into_any_element()
    } else {
        icon(glyph, size).text_color(tint).into_any_element()
    }
}

/// An agent's mark at `size`, in its brand colour.
pub fn mark(provider: &str, size: f32) -> AnyElement {
    picture(icons::agent(provider), size, theme::brand(provider))
}

/// A square that centres what it holds, so marks of any drawing line up.
pub fn mark_box(size: f32, child: impl IntoElement) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(child)
}

/// One line of UI text that never wraps.
pub fn text(value: impl Into<SharedString>, size: f32, weight: FontWeight) -> Div {
    div()
        .text_size(px(size))
        .font_weight(weight)
        .whitespace_nowrap()
        .child(value.into())
}

/// What a row says about itself after its title: 11px in the tertiary ink.
pub fn meta(value: impl Into<SharedString>, cx: &App) -> Div {
    text(value, 11., FontWeight::NORMAL).text_color(Tokens::get(cx).text_tertiary)
}

/// What the machine says: the mono face, with tabular figures and its icon glyphs.
pub fn mono(value: impl Into<SharedString>, size: f32, cx: &App) -> Div {
    let mut mono = text(value, size, FontWeight::NORMAL)
        .font_family(theme::MONO)
        .font_features(theme::tabular())
        .text_color(Tokens::get(cx).text_tertiary);
    mono.text_style().font_fallbacks = Some(FontFallbacks::from_fonts(vec![theme::MONO_ICONS.into()]));
    mono
}

/// A group's spaced capitals.
pub fn caps(value: &str, cx: &App) -> Div {
    text(value.to_uppercase(), 11., FontWeight::MEDIUM)
        .letter_spacing(0.2)
        .line_height(px(16.))
        .text_color(Tokens::get(cx).text_tertiary)
}

/// A shortcut as the system draws key names.
pub fn kbd(keys: &'static str, cx: &App) -> Div {
    text(keys, 11., FontWeight::MEDIUM)
        .font_family(theme::KBD)
        .line_height(px(11.))
        .text_color(Tokens::get(cx).text_tertiary)
}

/// A rounded frame over the shell, as the rails and panes are.
pub fn panel(cx: &App) -> Div {
    let t = Tokens::get(cx);
    div()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(t.surface)
        .border_1()
        .border_color(t.border)
        .rounded(theme::RADIUS_3)
}

/// What an agent is doing, in a 16px box: a grid while it works, a
/// dot while it waits on the person or has finished unseen, nothing when idle.
pub fn status_mark(status: AgentStatus, cx: &App) -> Option<Div> {
    let t = Tokens::get(cx);
    let dot = |color| Some(mark_box(16., div().size(px(7.)).rounded_full().bg(color)));
    match status {
        AgentStatus::Working => Some(mark_box(
            16.,
            div()
                .w(px(10.5))
                .flex()
                .flex_wrap()
                .gap(px(1.5))
                .children((0..9).map(|_| div().size(px(2.5)).rounded(px(0.5)).bg(t.live))),
        )),
        AgentStatus::Blocked => dot(t.warn),
        AgentStatus::Done => dot(t.accent),
        _ => None,
    }
}

/// How something reachable stands, as its 7px dot says.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Pending,
    /// A hollow ring: there, but not connected or not signed in.
    Idle,
    Failed,
}

pub fn dot(tone: Tone, cx: &App) -> Div {
    let t = Tokens::get(cx);
    let dot = div().size(px(7.)).rounded_full();
    match tone {
        Tone::Ok => dot.bg(t.live),
        Tone::Pending => dot.bg(t.warn),
        Tone::Idle => {
            let mut ring = dot.border_color(t.text_tertiary);
            let width = Some(px(1.5).into());
            ring.style().border_widths = gpui_kit::EdgesRefinement { top: width, right: width, bottom: width, left: width };
            ring
        }
        Tone::Failed => dot.bg(t.danger),
    }
}
