use std::rc::Rc;

use gpui_kit::base::{AlertDialog, Button as BaseButton};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnimationExt, AnyElement, App, Div, Entity, FocusHandle, FontWeight, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window,
};

use super::text;
use crate::motion;
use crate::theme::{self, Tokens, RADIUS_2, RADIUS_4};

pub(crate) type Answer = Rc<dyn Fn(&mut Window, &mut App)>;
type Typed = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// The sheet every dialog is drawn on: a title, what it says, then its buttons on a
/// sunken foot. Escape calls `no`, Enter `yes`.
pub(crate) fn sheet(
    title: SharedString,
    body: AnyElement,
    foot: Div,
    yes: Answer,
    no: Answer,
    cx: &mut App,
) -> AlertDialog {
    sheet_of(px(440.), title, body, foot, yes, no, cx)
}

/// The keyboard a dialog holds while it shows, and where it goes back to once answered.
struct Held {
    focus: FocusHandle,
    before: Option<FocusHandle>,
}

impl Held {
    /// `answer`, then the keyboard back where it was before the dialog took it.
    fn back(&self, answer: Answer) -> Answer {
        let before = self.before.clone();
        Rc::new(move |window, cx| {
            answer(window, cx);
            if let Some(before) = &before {
                window.focus(before, cx);
            }
        })
    }
}

/// Takes the keyboard into a dialog the first time it draws, so Enter and Escape answer it
/// and not what is behind it.
fn hold_focus(window: &mut Window, cx: &mut App) -> Held {
    let held = window.use_keyed_state("dialog-focus", cx, |window, cx| {
        let before = window.focused(cx);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        (focus, before)
    });
    let (focus, before) = held.read(cx).clone();
    Held { focus, before }
}

pub(crate) fn sheet_of(
    width: Pixels,
    title: SharedString,
    body: AnyElement,
    foot: Div,
    yes: Answer,
    no: Answer,
    cx: &mut App,
) -> AlertDialog {
    let t = Tokens::get(cx).clone();
    AlertDialog::new(cx)
        .open(true)
        .close_on_escape(true)
        .backdrop(
            div()
                .size_full()
                .bg(Hsla {
                    h: 0.,
                    s: 0.,
                    l: 0.,
                    a: 0.55,
                })
                .with_animation(
                    "dialog-scrim",
                    motion::entrance(motion::SCRIM_IN),
                    |scrim, shown| scrim.opacity(shown),
                ),
        )
        .on_cancel(move |_, window, cx| {
            no(window, cx);
            true
        })
        .on_ok(move |_, window, cx| {
            yes(window, cx);
            true
        })
        .popup(
            div()
                .id("dialog")
                .w(width)
                .flex()
                .flex_col()
                .overflow_hidden()
                .bg(t.surface_overlay)
                .border_1()
                .border_color(t.border)
                .rounded(RADIUS_4)
                .shadow(t.shadow_float())
                .child(
                    text(title, 14., FontWeight::SEMIBOLD)
                        .px(px(20.))
                        .pt(px(16.))
                        .text_color(t.text_primary),
                )
                .child(body)
                .child(
                    foot.flex()
                        .items_center()
                        .justify_end()
                        .gap(px(8.))
                        .mt(px(6.))
                        .pt(px(12.))
                        .pr(px(12.))
                        .pb(px(12.))
                        .pl(px(16.))
                        .border_t_1()
                        .border_color(t.border)
                        .bg(t.surface_sunken),
                )
                .with_animation(
                    "dialog-in",
                    motion::entrance(motion::DIALOG_IN),
                    |dialog, risen| dialog.opacity(risen).relative().top(px(8. * (1. - risen))),
                ),
        )
}

/// A dialog's button: the plain one, or the filled one that answers yes.
pub(crate) fn button(
    id: &'static str,
    label: SharedString,
    fill: Option<Hsla>,
    answer: Answer,
    cx: &App,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .h(px(30.))
        .px(px(12.))
        .rounded(RADIUS_2)
        .border_1()
        .text_size(crate::theme::ui_text(13.))
        .font_weight(FontWeight::MEDIUM)
        .on_click(move |_, window, cx| answer(window, cx))
        .map(|button| match fill {
            Some(fill) => button
                .border_color(gpui_kit::transparent_black())
                .bg(fill)
                .text_color(t.on_accent)
                .font_weight(FontWeight::SEMIBOLD)
                .hover(|style| style.opacity(0.92)),
            None => button
                .border_color(t.border)
                .bg(t.surface_raised)
                .text_color(t.text_secondary)
                .hover(move |style| {
                    style
                        .text_color(t.text_primary)
                        .border_color(t.border_strong)
                }),
        })
        .child(label)
}

/// A question asked before something goes ahead: what it is, what it does, and the word
/// that goes ahead, filled in the danger colour when it cannot be undone. Escape or the
/// other button answers no, Enter yes.
#[derive(IntoElement)]
pub struct Confirm {
    title: SharedString,
    body: SharedString,
    yes: SharedString,
    destructive: bool,
    on_yes: Answer,
    on_no: Answer,
}

impl Confirm {
    pub fn new(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        yes: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            yes: yes.into(),
            destructive: false,
            on_yes: Rc::new(|_, _| {}),
            on_no: Rc::new(|_, _| {}),
        }
    }

    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    pub fn on_yes(mut self, answer: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_yes = Rc::new(answer);
        self
    }

    pub fn on_no(mut self, answer: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_no = Rc::new(answer);
        self
    }
}

impl RenderOnce for Confirm {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let fill = if self.destructive { t.danger } else { t.accent };
        let held = hold_focus(window, cx);
        let (on_yes, on_no) = (held.back(self.on_yes), held.back(self.on_no));
        let foot = div()
            .child(button(
                "confirm-no",
                "cancel".into(),
                None,
                on_no.clone(),
                cx,
            ))
            .child(button(
                "confirm-yes",
                self.yes,
                Some(fill),
                on_yes.clone(),
                cx,
            ));
        sheet(
            self.title,
            div()
                .px(px(16.))
                .py(px(14.))
                .text_size(crate::theme::ui_text(12.))
                .line_height(crate::theme::ui_text(18.))
                .text_color(t.text_secondary)
                .child(self.body)
                .into_any_element(),
            foot,
            on_yes,
            on_no,
            cx,
        )
        .focus_handle(held.focus)
    }
}

/// A question answered with a line of text, such as a new branch's name. Enter gives
/// what was typed, Escape gives up.
#[derive(IntoElement)]
pub struct Prompt {
    title: SharedString,
    field: Entity<InputState>,
    on_yes: Typed,
    on_no: Answer,
}

impl Prompt {
    pub fn new(title: impl Into<SharedString>, field: &Entity<InputState>) -> Self {
        Self {
            title: title.into(),
            field: field.clone(),
            on_yes: Rc::new(|_, _, _| {}),
            on_no: Rc::new(|_, _| {}),
        }
    }

    pub fn on_yes(mut self, answer: impl Fn(String, &mut Window, &mut App) + 'static) -> Self {
        self.on_yes = Rc::new(answer);
        self
    }

    pub fn on_no(mut self, answer: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_no = Rc::new(answer);
        self
    }
}

impl RenderOnce for Prompt {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let (field, on_yes) = (self.field.clone(), self.on_yes);
        let yes: Answer = Rc::new(move |window, cx| {
            let typed = field.read(cx).value().to_string();
            on_yes(typed, window, cx)
        });
        let foot = div()
            .child(button(
                "prompt-no",
                "cancel".into(),
                None,
                self.on_no.clone(),
                cx,
            ))
            .child(button(
                "prompt-yes",
                "ok".into(),
                Some(t.accent),
                yes.clone(),
                cx,
            ));
        sheet(
            self.title,
            div()
                .px(px(16.))
                .py(px(14.))
                .child(
                    div()
                        .h(px(32.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .border_1()
                        .border_color(t.border_strong)
                        .rounded(RADIUS_2)
                        .bg(t.surface_sunken)
                        .font_family(theme::MONO)
                        .text_size(crate::theme::ui_text(12.))
                        .child(Input::new(&self.field).appearance(false)),
                )
                .into_any_element(),
            foot,
            yes,
            self.on_no,
            cx,
        )
    }
}

/// One of a keyed menu's choices: the key that picks it, what it does, and a hint.
pub struct Choice {
    key: Option<char>,
    label: SharedString,
    hint: Option<SharedString>,
    destructive: bool,
    disabled: bool,
    pick: Answer,
}

impl Choice {
    pub fn new(
        key: impl Into<Option<char>>,
        label: impl Into<SharedString>,
        pick: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            hint: None,
            destructive: false,
            disabled: false,
            pick: Rc::new(pick),
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A menu of choices each picked by a key, as the Git pane's menus are. Escape gives up.
#[derive(IntoElement)]
pub struct Choices {
    title: SharedString,
    choices: Vec<Choice>,
    on_no: Answer,
}

impl Choices {
    pub fn new(title: impl Into<SharedString>, choices: Vec<Choice>) -> Self {
        Self {
            title: title.into(),
            choices,
            on_no: Rc::new(|_, _| {}),
        }
    }

    pub fn on_no(mut self, answer: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_no = Rc::new(answer);
        self
    }
}

impl RenderOnce for Choices {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Tokens::get(cx).clone();
        let held = hold_focus(window, cx);
        let keyed: Vec<(char, Answer)> = self
            .choices
            .iter()
            .filter(|choice| !choice.disabled)
            .filter_map(|choice| Some((choice.key?, held.back(choice.pick.clone()))))
            .collect();
        let rows = self.choices.into_iter().enumerate().map(|(index, choice)| {
            let ink = if choice.disabled {
                t.text_tertiary
            } else if choice.destructive {
                t.danger
            } else {
                t.text_secondary
            };
            let (wash, lit) = if choice.destructive {
                (t.danger.opacity(0.12), t.danger)
            } else {
                (t.surface_hover, t.text_primary)
            };
            let pick = held.back(choice.pick.clone());
            BaseButton::new(("choice", index))
                .accessibility_label(choice.label.clone())
                .disabled(choice.disabled)
                .w_full()
                .px(px(16.))
                .py(px(6.))
                .flex()
                .items_center()
                .justify_start()
                .gap(px(10.))
                .font_family(theme::MONO)
                .text_size(crate::theme::ui_text(12.))
                .text_color(ink)
                .when(choice.disabled, |row| row.opacity(0.6))
                .when(!choice.disabled, |row| {
                    row.hover(move |style| style.bg(wash).text_color(lit))
                })
                .on_click(move |_, window, cx| pick(window, cx))
                .children(choice.key.map(|key| {
                    div()
                        .w(px(18.))
                        .flex_none()
                        .text_center()
                        .border_1()
                        .border_color(if choice.destructive {
                            t.danger.opacity(0.3)
                        } else {
                            t.border
                        })
                        .rounded(px(4.))
                        .bg(t.surface)
                        .text_size(crate::theme::ui_text(10.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if choice.destructive {
                            t.danger
                        } else {
                            t.text_primary
                        })
                        .child(key.to_string())
                }))
                .child(div().flex_1().min_w_0().child(choice.label))
                .children(choice.hint.map(|hint| {
                    div()
                        .flex_none()
                        .text_size(crate::theme::ui_text(10.5))
                        .text_color(t.text_tertiary)
                        .child(hint)
                }))
        });
        let no = held.back(self.on_no.clone());
        sheet(
            self.title,
            div()
                .py(px(8.))
                .flex()
                .flex_col()
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let typed = event.keystroke.key_char.as_deref().unwrap_or_default();
                    let mut chars = typed.chars();
                    if let (Some(typed), None) = (chars.next(), chars.next()) {
                        if let Some((_, pick)) = keyed.iter().find(|(key, _)| *key == typed) {
                            cx.stop_propagation();
                            pick(window, cx);
                        }
                    }
                })
                .children(rows)
                .into_any_element(),
            div()
                .text_size(crate::theme::ui_text(11.))
                .text_color(t.text_tertiary)
                .child("esc  cancel"),
            no.clone(),
            no,
            cx,
        )
        .focus_handle(held.focus)
    }
}

/// The keys a place answers to, in sections, as a wide sheet Escape closes.
#[derive(IntoElement)]
pub struct Cheatsheet {
    title: SharedString,
    sections: &'static [Section],
    on_close: Answer,
}

/// A cheatsheet's section: its name, then each key with what it does.
pub type Section = (&'static str, &'static [(&'static str, &'static str)]);

impl Cheatsheet {
    pub fn new(title: impl Into<SharedString>, sections: &'static [Section]) -> Self {
        Self {
            title: title.into(),
            sections,
            on_close: Rc::new(|_, _| {}),
        }
    }

    pub fn on_close(mut self, close: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Rc::new(close);
        self
    }
}

impl RenderOnce for Cheatsheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let held = hold_focus(window, cx);
        let t = Tokens::get(cx).clone();
        let count = self.sections.len();
        let sections = self
            .sections
            .iter()
            .enumerate()
            .map(|(index, (title, rows))| {
                div()
                    .px(px(16.))
                    .pt(px(8.))
                    .pb(px(12.))
                    .when(index + 1 < count, |section| {
                        section.border_b_1().border_color(t.border)
                    })
                    .child(
                        div()
                            .mb(px(6.))
                            .text_size(crate::theme::ui_text(10.))
                            .text_color(t.text_tertiary)
                            .child(title.to_uppercase()),
                    )
                    .children(rows.iter().map(|(keys, label)| {
                        div()
                            .flex()
                            .gap(px(12.))
                            .py(px(2.))
                            .text_size(crate::theme::ui_text(11.5))
                            .child(
                                div()
                                    .w(px(180.))
                                    .flex_none()
                                    .text_color(t.accent)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(*keys),
                            )
                            .child(div().text_color(t.text_secondary).child(*label))
                    }))
            });
        let close = held.back(self.on_close.clone());
        sheet_of(
            px(560.),
            self.title,
            div()
                .id("cheatsheet")
                .py(px(4.))
                .max_h(px(560.))
                .overflow_y_scroll()
                .font_family(theme::MONO)
                .children(sections)
                .into_any_element(),
            div()
                .text_size(crate::theme::ui_text(11.))
                .text_color(t.text_tertiary)
                .child("esc  close"),
            close.clone(),
            close,
            cx,
        )
        .focus_handle(held.focus)
    }
}
