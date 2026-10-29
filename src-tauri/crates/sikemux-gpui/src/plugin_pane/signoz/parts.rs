//! Small parts the pane's pages share.

use super::*;

pub(super) fn chip(text: impl Into<SharedString>, ink: Hsla, cx: &App) -> AnyElement {
    let _ = cx;
    div()
        .flex_none()
        .px(px(6.))
        .py(px(1.))
        .rounded(RADIUS_1)
        .bg(ink.opacity(0.16))
        .text_size(crate::theme::ui_text(10.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .whitespace_nowrap()
        .child(text.into())
        .into_any_element()
}

pub(super) fn quiet(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    ui::meta(text, cx).py(px(8.)).into_any_element()
}
