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

/// One of the kinds of thing a menu can list, in its heading: the key that asks for it and its
/// name. A click switches the menu to it.
pub struct DockScope {
    pub key: SharedString,
    pub label: SharedString,
    pub on: bool,
    pub choose: Choose,
}

#[derive(IntoElement)]
pub struct DockMenu {
    rows: Vec<DockRow>,
    selected: usize,
    /// What the menu says while it has no rows.
    empty: Option<SharedString>,
    /// The kinds it can list, and what taking a row does, over its rows.
    heading: Option<(Vec<DockScope>, SharedString)>,
}

impl DockMenu {
    pub fn new(rows: Vec<DockRow>, selected: usize) -> Self {
        Self {
            rows,
            selected,
            empty: None,
            heading: None,
        }
    }

    /// A menu with nothing to offer, saying why.
    pub fn empty(text: impl Into<SharedString>) -> Self {
        Self {
            rows: Vec::new(),
            selected: 0,
            empty: Some(text.into()),
            heading: None,
        }
    }

    pub fn heading(mut self, scopes: Vec<DockScope>, aside: impl Into<SharedString>) -> Self {
        self.heading = Some((scopes, aside.into()));
        self
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
                    .text_size(crate::theme::ui_text(9.5))
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
                        .text_size(crate::theme::ui_text(10.5))
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
                        .text_size(crate::theme::ui_text(10.5))
                        .text_color(t.text_tertiary)
                        .children(row.detail),
                )
                .children(row.aside.map(|aside| {
                    div()
                        .flex_none()
                        .font_family(theme::MONO)
                        .text_size(crate::theme::ui_text(9.5))
                        .text_color(t.text_disabled)
                        .child(aside)
                }))
                .into_any_element();
            heading.into_iter().chain([line])
        });
        let heading = self.heading.map(|(scopes, aside)| {
            div()
                .min_h(px(26.))
                .px(px(6.))
                .pb(px(3.))
                .flex()
                .items_center()
                .gap(px(2.))
                .children(scopes.into_iter().enumerate().map(|(index, scope)| {
                    let choose = scope.choose;
                    div()
                        .id(("dock-scope", index))
                        .h(px(22.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .rounded(theme::RADIUS_2)
                        .cursor_pointer()
                        .text_size(crate::theme::ui_text(10.5))
                        .text_color(if scope.on {
                            t.text_primary
                        } else {
                            t.text_tertiary
                        })
                        .when(scope.on, |chip| chip.bg(t.surface_chosen))
                        .hover(move |chip| chip.text_color(t.text_primary))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            choose(window, cx);
                        })
                        .child(
                            div()
                                .font_family(theme::MONO)
                                .text_size(crate::theme::ui_text(9.5))
                                .child(scope.key),
                        )
                        .child(scope.label)
                }))
                .child(div().flex_1())
                .child(
                    div()
                        .pr(px(4.))
                        .font_family(theme::MONO)
                        .text_size(crate::theme::ui_text(9.5))
                        .text_color(t.text_disabled)
                        .child(aside),
                )
        });
        let empty = self.empty.map(|text| {
            div()
                .min_h(px(30.))
                .px(px(10.))
                .flex()
                .items_center()
                .text_size(crate::theme::ui_text(10.5))
                .text_color(t.text_tertiary)
                .child(text)
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
            .children(heading)
            .children(rows)
            .children(empty)
    }
}
