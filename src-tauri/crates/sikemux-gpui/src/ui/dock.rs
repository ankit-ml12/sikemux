//! A list docked over the top edge of what it serves, such as the composer's menu of
//! agents, files and commands: rows with a mark, a name and what it is, one of them
//! picked, chosen by click or by the keys its owner binds.

use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, relative, AnyElement, App, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, SharedString, Styled, Window,
};

use crate::theme::{self, Tokens};

type Choose = Rc<dyn Fn(&mut Window, &mut App)>;

/// One row: a mark, a name, what it is, and what goes at its far end.
pub struct DockRow {
    name: SharedString,
    mark: Option<AnyElement>,
    detail: Option<SharedString>,
    aside: Option<SharedString>,
    /// The heading a run of rows starts under.
    group: Option<SharedString>,
    choose: Choose,
}

impl DockRow {
    pub fn new(name: impl Into<SharedString>, choose: Choose) -> Self {
        Self {
            name: name.into(),
            mark: None,
            detail: None,
            aside: None,
            group: None,
            choose,
        }
    }

    pub fn mark(mut self, mark: impl IntoElement) -> Self {
        self.mark = Some(mark.into_any_element());
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn aside(mut self, aside: Option<SharedString>) -> Self {
        self.aside = aside;
        self
    }

    pub fn group(mut self, group: Option<SharedString>) -> Self {
        self.group = group;
        self
    }
}

#[derive(IntoElement)]
pub struct DockMenu {
    rows: Vec<DockRow>,
    selected: usize,
}

impl DockMenu {
    pub fn new(rows: Vec<DockRow>, selected: usize) -> Self {
        Self { rows, selected }
    }
}

impl RenderOnce for DockMenu {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let rows = self.rows.into_iter().enumerate().flat_map(|(index, row)| {
            let heading = row.group.map(|group| {
                div()
                    .px(px(10.))
                    .pt(px(7.))
                    .pb(px(3.))
                    .border_t_1()
                    .border_color(t.line_soft)
                    .text_size(px(9.5))
                    .text_color(t.text_disabled)
                    .child(group)
                    .into_any_element()
            });
            let choose = row.choose;
            let line = div()
                .id(("dock-row", index))
                .min_h(px(30.))
                .px(px(10.))
                .py(px(3.))
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .when(index == self.selected, |line| line.bg(t.surface_chosen))
                .hover(move |line| line.bg(t.surface_hover))
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    choose(window, cx);
                })
                .child(
                    div()
                        .w(px(16.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .children(row.mark),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(theme::MONO)
                        .text_size(px(10.5))
                        .text_color(t.text_primary)
                        .child(row.name),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(10.5))
                        .text_color(t.text_tertiary)
                        .children(row.detail),
                )
                .children(row.aside.map(|aside| {
                    div()
                        .flex_none()
                        .font_family(theme::MONO)
                        .text_size(px(9.5))
                        .text_color(t.text_disabled)
                        .child(aside)
                }))
                .into_any_element();
            heading.into_iter().chain([line])
        });
        div()
            .absolute()
            .left(px(19.))
            .right(px(19.))
            .bottom(relative(1.))
            .mb(px(1.))
            .py(px(4.))
            .flex()
            .flex_col()
            .border_t_1()
            .border_l_1()
            .border_r_1()
            .border_color(t.border)
            .rounded_t(theme::RADIUS_3)
            .bg(t.surface_composer)
            .occlude()
            .children(rows)
    }
}
