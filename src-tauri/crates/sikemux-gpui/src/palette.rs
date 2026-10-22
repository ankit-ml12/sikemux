//! The pickers that rise over the stage, one at a time: the command deck, the
//! agent palette, the project picker, New… and the themes. Each fills a list of
//! rows; the person types to narrow it, moves through it and picks one.

use std::ops::Range;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    actions, div, px, uniform_list, AnyElement, App, AppContext, Context, Entity, FocusHandle,
    FontWeight, Hsla, InteractiveElement, IntoElement, KeyBinding, MouseButton, ParentElement,
    ScrollStrategy, SharedString, StatefulInteractiveElement, Styled, Subscription,
    UniformListScrollHandle, Window,
};

use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui;
use crate::workbench::Workbench;

actions!(palette, [Next, Previous, PageDown, PageUp, Choose, Close]);

const ROW_HEIGHT: f32 = 30.;
const SHOWN: usize = 200;
const PAGE: isize = 8;

pub fn install(cx: &mut App) {
    let context = Some("Palette > Input");
    cx.bind_keys([
        KeyBinding::new("down", Next, context),
        KeyBinding::new("tab", Next, context),
        KeyBinding::new("up", Previous, context),
        KeyBinding::new("shift-tab", Previous, context),
        KeyBinding::new("pagedown", PageDown, context),
        KeyBinding::new("pageup", PageUp, context),
        KeyBinding::new("enter", Choose, context),
        KeyBinding::new("escape", Close, context),
    ]);
}

/// What a row shows before its title.
#[derive(Clone)]
pub enum Mark {
    Icon(Icon),
    /// An agent's CLI, in its brand colour.
    Agent(String),
}

#[derive(Clone)]
pub struct Row {
    /// What the palette's chooser is told was picked.
    pub key: SharedString,
    pub title: SharedString,
    /// The small words after the title, such as a command's category.
    pub meta: SharedString,
    /// The keys that do the same, drawn at the end.
    pub keys: Option<SharedString>,
    pub mark: Option<Mark>,
    /// The heading the row sits under while nothing is typed.
    pub group: Option<SharedString>,
    pub disabled: bool,
    /// Everything typing matches against.
    pub search: String,
}

impl Row {
    pub fn new(key: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        let title = title.into();
        Self {
            key: key.into(),
            search: title.to_string(),
            title,
            meta: SharedString::default(),
            keys: None,
            mark: None,
            group: None,
            disabled: false,
        }
    }

    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = meta.into();
        self
    }

    pub fn keys(mut self, keys: Option<SharedString>) -> Self {
        self.keys = keys;
        self
    }

    pub fn mark(mut self, mark: Mark) -> Self {
        self.mark = Some(mark);
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Words typing finds the row by besides its title.
    pub fn also(mut self, words: &str) -> Self {
        self.search.push(' ');
        self.search.push_str(words);
        self
    }
}

type Pick = Rc<dyn Fn(&Row, &mut Workbench, &mut Window, &mut Context<Workbench>)>;
type Leave = Rc<dyn Fn(&mut Workbench, &mut Window, &mut Context<Workbench>)>;
type Typed = Rc<dyn Fn(&str) -> Option<Row>>;

/// One palette as its opener describes it.
pub struct Spec {
    /// Which palette it is, so the shortcut that opened it closes it again.
    pub id: &'static str,
    pub placeholder: SharedString,
    /// What the list says when typing matches nothing.
    pub empty: SharedString,
    pub rows: Vec<Row>,
    pub width: f32,
    pub pick: Pick,
    /// Heard each time the highlight moves, for a palette that shows its row
    /// before it is picked.
    pub browse: Option<Pick>,
    /// Heard when the palette closes with nothing picked.
    pub cancel: Option<Leave>,
    /// The row highlighted while nothing is typed, such as the theme worn now.
    pub start: Option<SharedString>,
    /// A row made of what is typed, offered first when no row has its key.
    pub typed: Option<Typed>,
}

impl Spec {
    pub fn new(
        id: &'static str,
        placeholder: impl Into<SharedString>,
        rows: Vec<Row>,
        pick: impl Fn(&Row, &mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
    ) -> Self {
        Self {
            id,
            placeholder: placeholder.into(),
            empty: "no matches".into(),
            rows,
            width: 560.,
            pick: Rc::new(pick),
            browse: None,
            cancel: None,
            start: None,
            typed: None,
        }
    }

    pub fn typed(mut self, typed: impl Fn(&str) -> Option<Row> + 'static) -> Self {
        self.typed = Some(Rc::new(typed));
        self
    }

    pub fn start(mut self, key: SharedString) -> Self {
        self.start = Some(key);
        self
    }

    pub fn empty(mut self, empty: impl Into<SharedString>) -> Self {
        self.empty = empty.into();
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn browse(
        mut self,
        browse: impl Fn(&Row, &mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
    ) -> Self {
        self.browse = Some(Rc::new(browse));
        self
    }

    pub fn cancel(
        mut self,
        cancel: impl Fn(&mut Workbench, &mut Window, &mut Context<Workbench>) + 'static,
    ) -> Self {
        self.cancel = Some(Rc::new(cancel));
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Line {
    Heading(usize),
    Row(usize),
}

/// The open palette: what it offers, what the person typed, and the lines
/// that shows.
pub struct Palette {
    spec: Spec,
    input: Entity<InputState>,
    lines: Vec<Line>,
    selected: Option<usize>,
    scroll: UniformListScrollHandle,
    /// What had focus before the palette took it, to hand it back.
    before: Option<FocusHandle>,
    /// The last row is the one made of what was typed.
    typed_shown: bool,
    _typing: Subscription,
}

impl Palette {
    pub fn id(&self) -> &'static str {
        self.spec.id
    }

    fn filter(&mut self, query: &str) {
        if self.typed_shown {
            self.spec.rows.pop();
            self.typed_shown = false;
        }
        let typed = self
            .spec
            .typed
            .as_ref()
            .and_then(|typed| typed(query.trim()))
            .filter(|row| !self.spec.rows.iter().any(|kept| kept.key == row.key));
        let rows = &self.spec.rows;
        self.lines = if query.trim().is_empty() {
            let mut lines = Vec::with_capacity(rows.len() + 4);
            let mut group = None;
            for (place, row) in rows.iter().enumerate() {
                if row.group.is_some() && row.group != group {
                    lines.push(Line::Heading(place));
                    group = row.group.clone();
                }
                lines.push(Line::Row(place));
            }
            lines
        } else {
            crate::fuzzy::rank(query, rows.iter().map(|row| row.search.as_str()), SHOWN)
                .into_iter()
                .map(Line::Row)
                .collect()
        };
        if let Some(row) = typed {
            self.spec.rows.push(row);
            self.lines.insert(0, Line::Row(self.spec.rows.len() - 1));
            self.typed_shown = true;
        }
        let start = self
            .spec
            .start
            .clone()
            .filter(|_| query.trim().is_empty())
            .and_then(|key| self.line_of(&key));
        self.select(start.or_else(|| self.step_from(None, 1)));
    }

    fn line_of(&self, key: &SharedString) -> Option<usize> {
        let rows = &self.spec.rows;
        self.lines
            .iter()
            .position(|line| matches!(line, Line::Row(place) if rows[*place].key == *key))
    }

    /// The key of the row highlighted now.
    pub fn highlighted(&self) -> Option<SharedString> {
        self.selected
            .and_then(|line| self.row(line))
            .map(|row| row.key.clone())
    }

    /// Offers `rows` in place of what it offered, keeping what the person typed
    /// and the row they had reached.
    pub fn refill(&mut self, rows: Vec<Row>, cx: &App) {
        let reached = self.highlighted();
        self.spec.rows = rows;
        self.typed_shown = false;
        let query = self.input.read(cx).value().to_string();
        self.filter(&query);
        if let Some(line) = reached.and_then(|key| self.line_of(&key)) {
            self.select(Some(line));
        }
    }

    fn choosable(&self, line: usize) -> bool {
        matches!(self.lines.get(line), Some(Line::Row(place)) if !self.spec.rows[*place].disabled)
    }

    /// The next row that can be picked `by` lines on from `from`, wrapping.
    fn step_from(&self, from: Option<usize>, by: isize) -> Option<usize> {
        let count = self.lines.len() as isize;
        let start = from.map_or(if by > 0 { -1 } else { count }, |from| from as isize);
        (1..=count)
            .map(|step| (start + by.signum() * step).rem_euclid(count) as usize)
            .find(|&line| self.choosable(line))
    }

    fn step(&mut self, by: isize) {
        let mut at = self.selected;
        for _ in 0..by.unsigned_abs() {
            at = self.step_from(at, by);
        }
        self.select(at);
    }

    fn select(&mut self, line: Option<usize>) {
        self.selected = line;
        if let Some(line) = line {
            self.scroll.scroll_to_item(line, ScrollStrategy::Nearest);
        }
    }

    fn row(&self, line: usize) -> Option<&Row> {
        match self.lines.get(line)? {
            Line::Row(place) => self.spec.rows.get(*place),
            Line::Heading(_) => None,
        }
    }
}

impl Workbench {
    /// Opens `spec`'s palette in place of any other, or closes it when it is
    /// the one open.
    pub fn toggle_palette(&mut self, spec: Spec, window: &mut Window, cx: &mut Context<Self>) {
        let same = self
            .palette
            .as_ref()
            .is_some_and(|open| open.id() == spec.id);
        self.close_palette(window, cx);
        if same {
            return;
        }
        self.close_finder(window, cx);
        let before = window.focused(cx);
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(spec.placeholder.clone()));
        let typing = cx.subscribe_in(
            &input,
            window,
            |bench, input, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    let query = input.read(cx).value().to_string();
                    if let Some(palette) = bench.palette.as_mut() {
                        palette.filter(&query);
                    }
                    bench.browse(window, cx);
                    bench.parts.redraw(&bench.parts.finder, cx);
                }
            },
        );
        input.update(cx, |state, cx| state.focus(window, cx));
        let mut palette = Palette {
            spec,
            input,
            lines: Vec::new(),
            selected: None,
            scroll: UniformListScrollHandle::new(),
            before,
            typed_shown: false,
            _typing: typing,
        };
        palette.filter("");
        self.palette = Some(palette);
        self.parts.redraw(&self.parts.finder, cx);
        cx.notify();
    }

    /// Closes the open palette with nothing picked.
    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(cancel) = self.leave_palette(window, cx).and_then(|spec| spec.cancel) {
            cancel(self, window, cx);
        }
    }

    fn leave_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Spec> {
        let palette = self.palette.take()?;
        palette
            .before
            .as_ref()
            .unwrap_or(&self.focus)
            .focus(window, cx);
        self.parts.redraw(&self.parts.finder, cx);
        cx.notify();
        Some(palette.spec)
    }

    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = self.palette.as_ref() else {
            return;
        };
        let (Some(browse), Some(row)) = (
            palette.spec.browse.clone(),
            palette.selected.and_then(|line| palette.row(line)).cloned(),
        ) else {
            return;
        };
        browse(&row, self, window, cx);
    }

    fn step_palette(&mut self, by: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = self.palette.as_mut() {
            palette.step(by);
        }
        self.browse(window, cx);
        self.parts.redraw(&self.parts.finder, cx);
    }

    fn pick(&mut self, line: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self
            .palette
            .as_ref()
            .and_then(|palette| {
                line.filter(|&line| palette.choosable(line))
                    .and_then(|line| palette.row(line))
            })
            .cloned()
        else {
            return;
        };
        if let Some(spec) = self.leave_palette(window, cx) {
            let pick = spec.pick;
            cx.defer_in(window, move |bench, window, cx| {
                pick(&row, bench, window, cx)
            });
        }
    }
}

pub fn overlay(
    bench: &Workbench,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> Option<AnyElement> {
    let palette = bench.palette.as_ref()?;
    let t = Tokens::get(cx).clone();
    let count = palette.lines.len();
    let close = cx.listener(|bench: &mut Workbench, _, window, cx| bench.close_palette(window, cx));
    Some(
        div()
            .id("palette")
            .absolute()
            .inset_0()
            .flex()
            .justify_center()
            .items_start()
            .pt(window.viewport_size().height * 0.13)
            .bg(Hsla {
                a: 0.68,
                ..t.surface_sunken
            })
            .occlude()
            .on_mouse_down(MouseButton::Left, close)
            .child(
                ui::float(cx)
                    .key_context("Palette")
                    .w(px(palette.spec.width))
                    .max_w(gpui_kit::relative(0.86))
                    .max_h(gpui_kit::relative(0.62))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(
                        cx.listener(|bench, _: &Next, window, cx| {
                            bench.step_palette(1, window, cx)
                        }),
                    )
                    .on_action(cx.listener(|bench, _: &Previous, window, cx| {
                        bench.step_palette(-1, window, cx)
                    }))
                    .on_action(cx.listener(|bench, _: &PageDown, window, cx| {
                        bench.step_palette(PAGE, window, cx)
                    }))
                    .on_action(cx.listener(|bench, _: &PageUp, window, cx| {
                        bench.step_palette(-PAGE, window, cx)
                    }))
                    .on_action(cx.listener(|bench, _: &Choose, window, cx| {
                        let selected = bench.palette.as_ref().and_then(|palette| palette.selected);
                        bench.pick(selected, window, cx);
                    }))
                    .on_action(
                        cx.listener(|bench, _: &Close, window, cx| bench.close_palette(window, cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(16.))
                            .py(px(14.))
                            .border_b_1()
                            .border_color(t.border)
                            .child(ui::icon(Icon::Search, 15.).text_color(t.text_tertiary))
                            .child(
                                Input::new(&palette.input)
                                    .appearance(false)
                                    .flex_1()
                                    .p_0()
                                    .text_size(px(14.))
                                    .text_color(t.text_primary),
                            ),
                    )
                    .map(|panel| {
                        if count == 0 {
                            return panel.child(
                                div()
                                    .p(px(28.))
                                    .text_center()
                                    .text_color(t.text_tertiary)
                                    .child(palette.spec.empty.clone()),
                            );
                        }
                        panel.child(
                            uniform_list(
                                "palette-lines",
                                count,
                                cx.processor(
                                    |bench: &mut Workbench, range: Range<usize>, _window, cx| {
                                        range
                                            .map(|line| line_of(bench, line, cx))
                                            .collect::<Vec<_>>()
                                    },
                                ),
                            )
                            .track_scroll(&palette.scroll)
                            .h(px(ROW_HEIGHT * count as f32 + 8.))
                            .min_h_0()
                            .flex_shrink(1.)
                            .py(px(4.))
                            .px(px(6.)),
                        )
                    }),
            )
            .into_any_element(),
    )
}

fn line_of(bench: &Workbench, line: usize, cx: &mut Context<Workbench>) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let Some(palette) = bench.palette.as_ref() else {
        return div().into_any_element();
    };
    let row = match palette.lines.get(line) {
        Some(Line::Heading(place)) => {
            let group = palette.spec.rows[*place].group.clone().unwrap_or_default();
            return div()
                .h(px(ROW_HEIGHT))
                .px(px(8.))
                .flex()
                .items_end()
                .pb(px(4.))
                .child(ui::caps(&group, cx))
                .into_any_element();
        }
        Some(Line::Row(place)) => &palette.spec.rows[*place],
        None => return div().into_any_element(),
    };
    let selected = palette.selected == Some(line);
    let ink = if row.disabled {
        t.text_tertiary
    } else if selected {
        t.text_primary
    } else {
        t.text_secondary
    };
    div()
        .id(("palette-line", line))
        .w_full()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(ROW_HEIGHT))
        .px(px(8.))
        .rounded(theme::RADIUS_2)
        .text_size(px(13.))
        .text_color(ink)
        .when(selected, |element| element.bg(t.surface_chosen))
        .when(!row.disabled, |element| {
            element
                .on_mouse_move(cx.listener(move |bench, _, window, cx| {
                    let Some(palette) = bench
                        .palette
                        .as_mut()
                        .filter(|palette| palette.selected != Some(line))
                    else {
                        return;
                    };
                    palette.selected = Some(line);
                    bench.browse(window, cx);
                    bench.parts.redraw(&bench.parts.finder, cx);
                }))
                .on_click(
                    cx.listener(move |bench, _, window, cx| bench.pick(Some(line), window, cx)),
                )
        })
        .when_some(row.mark.clone(), |element, mark| {
            element.child(
                div()
                    .w(px(18.))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .child(match mark {
                        Mark::Icon(glyph) => ui::picture(glyph, 14., t.text_tertiary),
                        Mark::Agent(provider) => ui::mark(&provider, 14.),
                    }),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .child(row.title.clone()),
        )
        .when(!row.meta.is_empty(), |element| {
            element.child(ui::meta(row.meta.clone(), cx).flex_none())
        })
        .when_some(row.keys.clone(), |element, keys| {
            element.child(ui::kbd(keys, cx).flex_none().pl(px(6.)))
        })
        .into_any_element()
}
