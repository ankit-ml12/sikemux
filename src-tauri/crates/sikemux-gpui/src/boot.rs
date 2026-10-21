//! What the window shows while the core is not there: the mark while it opens,
//! and a word on what is happening when that takes a while or the connection
//! drops. The window reconnects by itself, so neither asks anything.

use std::time::Duration;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement, Styled,
};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui;
use crate::workbench::Workbench;

/// How long the mark shows alone before the window says what it waits for.
pub const PATIENCE: Duration = Duration::from_millis(900);

impl Workbench {
    /// Says what the window waits for once opening has taken longer than [`PATIENCE`].
    pub fn wait_patiently(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PATIENCE).await;
            let _ = this.update(cx, |bench, cx| {
                bench.slow_start = true;
                cx.notify();
            });
        })
        .detach();
    }
}

fn mark_with(
    words: Option<String>,
    detail: Option<String>,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(14.))
        .child(ui::icon(Icon::Logo, 44.).text_color(t.accent))
        .when_some(words, |column, words| {
            column.child(
                div()
                    .text_size(px(13.))
                    .text_color(t.text_secondary)
                    .child(words),
            )
        })
        .when_some(detail, |column, detail| {
            column.child(
                ui::meta(detail, cx)
                    .max_w(px(420.))
                    .text_center()
                    .whitespace_normal(),
            )
        })
}

/// Covers the window until the core's first view arrives.
pub fn opening(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    if bench.view.is_some() {
        return None;
    }
    let t = Tokens::get(cx).clone();
    let words = bench.slow_start.then(|| {
        if bench.version.is_empty() {
            "Waiting for Sikemux to start".to_owned()
        } else {
            "Opening your workspace".to_owned()
        }
    });
    let detail = bench.slow_start.then(|| bench.lost.clone()).flatten();
    Some(
        div()
            .id("opening")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(t.surface_sunken)
            .occlude()
            .child(mark_with(words, detail, cx))
            .into_any_element(),
    )
}

/// Covers the workspace while the connection to the core is down.
pub fn reconnecting(bench: &Workbench, cx: &mut Context<Workbench>) -> Option<AnyElement> {
    let reason = bench.lost.clone().filter(|_| bench.view.is_some())?;
    let t = Tokens::get(cx).clone();
    Some(
        div()
            .id("reconnecting")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(Hsla {
                a: 0.82,
                ..t.surface_sunken
            })
            .occlude()
            .child(mark_with(
                Some("Lost Sikemux, reconnecting".to_owned()),
                Some(reason),
                cx,
            ))
            .into_any_element(),
    )
}
