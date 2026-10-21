use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, rgba, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, WindowControlArea,
};

use crate::theme;
use crate::ui::{agent_mark, icon, slot, status_mark};
use crate::workbench::Workbench;

/// The bar over the stage: the project's tabs, then the clock.
pub fn top_bar(bench: &Workbench, cx: &mut Context<Workbench>) -> impl IntoElement {
    let agents = bench
        .project
        .as_deref()
        .map(|project| bench.agents_of(project))
        .unwrap_or_default();
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
                .child(
                    div()
                        .h_full()
                        .min_w_0()
                        .overflow_hidden()
                        .px(px(4.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .children(agents.into_iter().map(|agent| {
                            let active = bench.agent.as_deref() == Some(agent.id.as_str());
                            let id = agent.id.clone();
                            div()
                                .id(SharedString::from(format!("tab-{}", agent.id)))
                                .h(px(26.))
                                .min_w(px(120.))
                                .pl(px(8.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .rounded(theme::RADIUS_2)
                                .bg(if active { rgba(0x232328ff) } else { theme::surface() })
                                .when(!active, |tab| {
                                    tab.border_1().border_color(rgba(0xe7e5ef0f))
                                })
                                .text_color(if active { theme::primary() } else { theme::tertiary() })
                                .hover(|tab| tab.bg(rgba(0x1c1c20ff)))
                                .on_click(cx.listener(move |bench, _, _, cx| {
                                    bench.agent = Some(id.clone());
                                    cx.notify();
                                }))
                                .child(slot(22., agent_mark(&agent.provider, 19.)))
                                .child(
                                    div()
                                        .max_w(px(160.))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(agent.title.clone()),
                                )
                                .children(agent.name.clone().map(|name| {
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme::tertiary())
                                        .child(format!("@{name}"))
                                }))
                                .child(div().size(px(20.)).mr(px(8.)).flex_none().children(status_mark(agent.status)))
                        }))
                        .child(
                            div()
                                .h_full()
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .child(icon("icons/plus.svg", 13., theme::tertiary())),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .pr(px(16.))
                .child(clock())
                .child(slot(30., icon("icons/focus.svg", 15., theme::tertiary()))),
        )
}

fn clock() -> impl IntoElement {
    let now = chrono::Local::now();
    div()
        .flex()
        .items_baseline()
        .font_features(theme::tabular())
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::secondary())
                .child(now.format("%-I:%M").to_string()),
        )
        .child(
            div()
                .ml(px(4.))
                .text_size(px(10.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::tertiary())
                .child(now.format("%p").to_string()),
        )
}
