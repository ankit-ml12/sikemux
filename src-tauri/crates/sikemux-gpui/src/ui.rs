use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, svg, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, Rgba,
    SharedString, Styled,
};
use sikemux_wire::protocol::AgentStatus;

use crate::{icons, theme};

pub fn icon(path: &'static str, size: f32, color: Rgba) -> impl IntoElement {
    svg().path(path).size(px(size)).flex_none().text_color(color)
}

pub fn agent_mark(provider: &str, size: f32) -> impl IntoElement {
    let path = icons::agent(provider);
    if icons::agent_is_picture(provider) {
        img(path).size(px(size)).flex_none().into_any_element()
    } else {
        icon(path, size, theme::brand(provider)).into_any_element()
    }
}

/// A box of `slot` that centres what it holds, as the app's mark slots do.
pub fn slot(slot: f32, child: impl IntoElement) -> Div {
    div()
        .size(px(slot))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(child)
}

/// The rails' row: a mark slot, then what it says, on the hover and chosen tints.
pub fn row(height: f32, chosen: bool) -> Div {
    div()
        .h(px(height))
        .pl(px(3.))
        .pr(px(8.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(theme::RADIUS_2)
        .when(chosen, |row| row.bg(theme::chosen()))
        .when(!chosen, |row| row.hover(|row| row.bg(theme::hover())))
}

/// A row's title, cut short with an ellipsis when it does not fit.
pub fn title(value: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::primary())
        .child(value.into())
}

pub fn text(value: impl Into<SharedString>, size: f32, weight: f32, color: Rgba) -> Div {
    div()
        .text_size(px(size))
        .font_weight(FontWeight(weight))
        .text_color(color)
        .whitespace_nowrap()
        .child(value.into())
}

pub fn kbd(keys: &'static str) -> Div {
    div()
        .font_family(theme::KBD)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .line_height(px(11.))
        .text_color(theme::tertiary())
        .child(keys)
}

/// GPUI has no letter-spacing, so the spaced capitals of a group's header are
/// laid out a letter at a time.
pub fn tracked(label: &str, size: f32, spacing: f32, color: Rgba) -> Div {
    div()
        .flex()
        .flex_none()
        .text_size(px(size))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .children(label.chars().map(move |letter| {
            div()
                .mr(px(size * spacing))
                .child(SharedString::from(letter.to_string()))
        }))
}

/// A group's header: its spaced capitals, a rule and what it can do.
pub fn group_header(label: &str, actions: Option<Div>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pt(px(4.))
        .pr(px(6.))
        .pb(px(6.))
        .pl(px(8.))
        .line_height(px(11. * 1.45))
        .child(tracked(&label.to_uppercase(), 11., 0.2, theme::tertiary()))
        .child(div().flex_1().h(px(1.)).bg(theme::border()))
        .children(actions)
}

pub fn status_mark(status: AgentStatus) -> Option<Div> {
    let dot = |color: Rgba| Some(slot(16., div().size(px(7.)).rounded_full().bg(color)));
    match status {
        AgentStatus::Working => Some(
            slot(
                16.,
                div()
                    .w(px(10.5))
                    .flex()
                    .flex_wrap()
                    .gap(px(1.5))
                    .children((0..9).map(|_| div().size(px(2.5)).rounded(px(0.5)).bg(theme::live()))),
            ),
        ),
        AgentStatus::Blocked => dot(theme::warn()),
        AgentStatus::Done => dot(theme::accent()),
        _ => None,
    }
}
