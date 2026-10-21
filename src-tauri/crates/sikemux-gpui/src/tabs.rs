use gpui_kit::{div, px, App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, WindowControlArea};

use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, IconButton, Tab};
use crate::workbench::Workbench;

/// The bar over the stage: the project's tabs, then the clock.
pub fn top_bar(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let agents = bench.project.as_deref().map(|project| bench.agents_of(project)).unwrap_or_default();
    let tabs: Vec<_> = agents
        .into_iter()
        .map(|agent| {
            let id = agent.id.clone();
            Tab::new(SharedString::from(format!("tab-{}", agent.id)), agent.title.clone())
                .mark(ui::mark(&agent.provider, 19.))
                .note(agent.name.as_ref().map(|name| SharedString::from(format!("@{name}"))))
                .tail(ui::status_mark(agent.status, cx))
                .active(bench.agent.as_deref() == Some(agent.id.as_str()))
                .on_click(cx.listener(move |bench, _, _, cx| {
                    bench.agent = Some(id.clone());
                    cx.notify();
                }))
                .into_any_element()
        })
        .collect();
    div()
        .id("top-bar")
        .flex()
        .flex_none()
        .items_center()
        .h(px(34.))
        .window_control_area(WindowControlArea::Drag)
        .child(div().w(px(80.)).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .pl(px(20.))
                .pr(px(12.))
                .flex()
                .items_center()
                .overflow_hidden()
                .child(div().h_full().min_w_0().overflow_hidden().px(px(4.)).flex().items_center().gap(px(4.)).children(tabs))
                .child(IconButton::new("new-tab", Icon::Plus, "New tab").glyph(13.)),
        )
        .child(div().flex().items_center().gap(px(8.)).pr(px(16.)).child(clock(cx)).child(IconButton::new("focus", Icon::Focus, "Focus").width(30.)))
}

fn clock(cx: &App) -> impl IntoElement {
    let t = Tokens::get(cx);
    let now = chrono::Local::now();
    div()
        .flex()
        .items_baseline()
        .font_features(crate::theme::tabular())
        .child(ui::text(now.format("%-I:%M").to_string(), 12., FontWeight::SEMIBOLD).text_color(t.text_secondary))
        .child(ui::text(now.format("%p").to_string(), 10., FontWeight::SEMIBOLD).ml(px(4.)).text_color(t.text_tertiary))
}
