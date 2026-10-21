//! What every settings page is made of: sections under a heading, and rows
//! with a name, a line about them, and the control that changes them.

use gpui_kit::component::switch::Switch;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, App, Div, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
};

use crate::theme::{self, Tokens};
use crate::workbench::Workbench;

/// A section of a page: its heading, a count or state beside it, a line
/// under it, and its rows. One a search led to is lit for a moment.
pub fn section(
    title: &'static str,
    meta: Option<SharedString>,
    sub: Option<SharedString>,
    flash: bool,
    body: impl IntoElement,
    cx: &App,
) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(4.))
        .rounded(theme::RADIUS_3)
        .when(flash, |section| section.bg(t.surface_chosen))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.text_primary)
                        .child(title),
                )
                .children(meta.map(|meta| {
                    div()
                        .text_size(px(11.5))
                        .text_color(t.text_tertiary)
                        .child(meta)
                })),
        )
        .children(sub.map(|sub| {
            div()
                .text_size(px(12.))
                .text_color(t.text_tertiary)
                .child(sub)
        }))
        .child(
            div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(t.border)
                .rounded(theme::RADIUS_3)
                .child(body),
        )
        .into_any_element()
}

/// A row in a section: what it is, a line about it, and its control.
pub fn row(
    label: impl Into<SharedString>,
    about: Option<&'static str>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    row_with(None, label, about.map(SharedString::from), control, cx)
}

/// A row with a mark before its name, and a line about it made as it is drawn.
pub fn row_with(
    mark: Option<AnyElement>,
    label: impl Into<SharedString>,
    about: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> Div {
    let t = Tokens::get(cx);
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(14.))
        .py(px(10.))
        .border_b_1()
        .border_color(t.line_soft)
        .children(mark)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(t.text_primary)
                        .child(label.into()),
                )
                .children(about.map(|about| {
                    div()
                        .text_size(px(11.5))
                        .text_color(t.text_tertiary)
                        .child(about)
                })),
        )
        .child(div().flex_none().child(control))
}

/// A switch that sets the boolean preference `key`.
pub fn switch(id: &'static str, key: &'static str, on: bool, this: &Entity<Workbench>) -> Switch {
    let this = this.clone();
    Switch::new(id)
        .checked(on)
        .on_change(move |on: &bool, _: &mut Window, cx: &mut App| {
            let on = *on;
            this.update(cx, |bench, _| super::set(bench, key, on))
        })
}

/// A line of quiet text in place of a section's rows.
pub fn note(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    let t = Tokens::get(cx);
    div()
        .px(px(14.))
        .py(px(12.))
        .text_size(px(12.))
        .text_color(t.text_tertiary)
        .child(text.into())
        .into_any_element()
}
