//! A terminal pane: the screen the core sends, drawn by gpui-terminal-grid.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use gpui_kit::{
    canvas, div, font, px, AnyElement, App, ClipboardItem, Context, Entity, FocusHandle, FontFallbacks, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, Keystroke, Modifiers, MouseButton as Button, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement, Pixels, Point, Rgba, ScrollWheelEvent, Styled, Window,
};
use gpui_terminal_grid::{Cursor, CursorShape, Grid, Highlight, Layout, Look, Metrics, Span, Underline};
use sikemux_wire::grid::{self as wire, Color, GridPoint, Mods, MouseAction, MouseButton, Row, Screen, TerminalInput};
use sikemux_wire::protocol::SessionId;
use tokio::sync::mpsc;

use crate::core::{Ask, Screenful, TerminalAsk};
use crate::theme::{self, Tokens};
use crate::workbench::Workbench;

const PAGE: u64 = 256;
/// History lines kept around the ones in view.
const HISTORY_KEPT: usize = 4_000;
const FONT_SIZE: f32 = 13.;
const LINE_HEIGHT: f32 = 1.32;

/// Every terminal this window has attached, by session.
#[derive(Default)]
pub struct Terminals {
    open: std::collections::HashMap<SessionId, Terminal>,
    metrics: Option<Metrics>,
}

struct Terminal {
    screen: Screen,
    /// History lines fetched so far, numbered as the core numbers them.
    history: BTreeMap<u64, Arc<Row>>,
    pages_asked: HashSet<u64>,
    /// The line on the top row while scrolled back; none follows the output.
    top: Option<u64>,
    selection: Option<(GridPoint, GridPoint)>,
    /// Where a selection being dragged started.
    anchor: Option<GridPoint>,
    focus: FocusHandle,
    acked: u32,
    asked_size: (u16, u16),
    origin: Point<Pixels>,
    wheel: f32,
}

impl Terminal {
    fn first_line(&self) -> u64 {
        self.screen.header.history_end - u64::from(self.screen.header.history_len)
    }

    fn top_line(&self) -> u64 {
        self.top.unwrap_or(self.screen.header.history_end).max(self.first_line())
    }

    fn row(&self, line: u64) -> Option<&Row> {
        let end = self.screen.header.history_end;
        if line >= end {
            self.screen.rows.get(usize::try_from(line - end).ok()?).map(|row| &**row)
        } else {
            self.history.get(&line).map(|row| &**row)
        }
    }

    fn reset(&mut self) {
        self.screen = Screen::default();
        self.history.clear();
        self.pages_asked.clear();
        self.top = None;
        self.selection = None;
        self.acked = 0;
    }
}

impl Terminals {
    pub fn focus(&self, session: SessionId) -> Option<&FocusHandle> {
        self.open.get(&session).map(|terminal| &terminal.focus)
    }

    pub fn screen(&self, session: SessionId) -> Option<&Screen> {
        self.open.get(&session).map(|terminal| &terminal.screen)
    }

    pub fn apply(&mut self, session: SessionId, screenful: Screenful, cx: &mut App) {
        let Some(terminal) = self.open.get_mut(&session) else {
            return;
        };
        match screenful {
            Screenful::Frame(frame) => {
                if frame.header.epoch != terminal.screen.header.epoch {
                    terminal.history.clear();
                    terminal.pages_asked.clear();
                    terminal.top = None;
                    terminal.selection = None;
                }
                terminal.screen.apply(frame);
            }
            Screenful::Lines(lines) => {
                terminal.pages_asked.remove(&lines.start);
                if lines.epoch == terminal.screen.header.epoch {
                    terminal.history.extend((lines.start..).zip(lines.rows.into_iter().map(Arc::new)));
                }
                if terminal.history.len() > HISTORY_KEPT {
                    let middle = terminal.top_line();
                    let half = HISTORY_KEPT as u64 / 2;
                    terminal.history.retain(|line, _| line.abs_diff(middle) < half);
                }
            }
            Screenful::Copied(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
        }
    }

    /// Attaches again after the connection came back, from a blank screen.
    pub fn reattach(&mut self, asks: &mpsc::UnboundedSender<Ask>) {
        for (session, terminal) in &mut self.open {
            terminal.reset();
            let _ = asks.send(Ask::Terminal(*session, TerminalAsk::Attach));
        }
    }

    /// Lets go of terminals whose sessions are gone.
    pub fn keep(&mut self, live: impl Fn(SessionId) -> bool, asks: &mpsc::UnboundedSender<Ask>) {
        self.open.retain(|session, _| {
            let keep = live(*session);
            if !keep {
                let _ = asks.send(Ask::Terminal(*session, TerminalAsk::Detach));
            }
            keep
        });
    }
}

/// The terminal a pane shows, attaching it the first time it is drawn.
pub fn pane(bench: &Workbench, this: &Entity<Workbench>, session: &str, window: &Window) -> AnyElement {
    let Ok(id) = session.parse::<SessionId>() else {
        return div().into_any_element();
    };
    let this = this.clone();
    let mut pane = div()
        .id(("terminal", id))
        .size_full()
        .p(px(8.))
        .on_key_down(window.listener_for(&this, move |bench, event: &KeyDownEvent, _, cx| {
            if key(bench, id, &event.keystroke, cx) {
                cx.stop_propagation();
                cx.notify();
            }
        }))
        .on_mouse_down(Button::Left, window.listener_for(&this, move |bench, event: &MouseDownEvent, window, cx| {
            mouse_down(bench, id, event, window, cx);
            cx.notify();
        }))
        .on_mouse_move(window.listener_for(&this, move |bench, event: &MouseMoveEvent, _, cx| {
            if mouse_move(bench, id, event) {
                cx.notify();
            }
        }))
        .on_mouse_up(Button::Left, window.listener_for(&this, move |bench, event: &MouseUpEvent, _, cx| {
            mouse_up(bench, id, event);
            cx.notify();
        }))
        .on_scroll_wheel(window.listener_for(&this, move |bench, event: &ScrollWheelEvent, _, cx| {
            if wheel(bench, id, event) {
                cx.notify();
            }
        }));
    if let Some(terminal) = bench.terminals.open.get(&id) {
        pane = pane.track_focus(&terminal.focus);
    }
    pane.child(
        div().size_full().overflow_hidden().child(
            canvas(
                move |bounds, window, cx| this.update(cx, |bench, cx| layout(bench, id, bounds, window, cx)),
                |_, layout, window, cx| layout.paint(window, cx),
            )
            .size_full(),
        ),
    )
    .into_any_element()
}

/// Attaches the terminal the first time, fits it to its bounds, acknowledges
/// the frame about to be drawn, asks for history lines scrolled into view,
/// and shapes the grid.
fn layout(
    bench: &mut Workbench,
    id: SessionId,
    bounds: gpui_kit::Bounds<Pixels>,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) -> Layout {
    let metrics = bench
        .terminals
        .metrics
        .get_or_insert_with(|| {
            let mut mono = font(theme::MONO);
            mono.fallbacks = Some(FontFallbacks::from_fonts(vec![theme::MONO_ICONS.into()]));
            Metrics::new(mono, px(FONT_SIZE), LINE_HEIGHT, window)
        })
        .clone();
    let asks = bench.asks.clone();
    let terminal = bench.terminals.open.entry(id).or_insert_with(|| {
        let _ = asks.send(Ask::Terminal(id, TerminalAsk::Attach));
        cx.notify();
        Terminal {
            screen: Screen::default(),
            history: BTreeMap::new(),
            pages_asked: HashSet::new(),
            top: None,
            selection: None,
            anchor: None,
            focus: cx.focus_handle(),
            acked: 0,
            asked_size: (0, 0),
            origin: Point::default(),
            wheel: 0.,
        }
    });
    terminal.origin = bounds.origin;
    let size = metrics.fit(bounds.size);
    if size != terminal.asked_size {
        terminal.asked_size = size;
        let _ = asks.send(Ask::Terminal(id, TerminalAsk::Resize(size.0, size.1)));
    }
    let header = &terminal.screen.header;
    if header.frame != terminal.acked && header.rows > 0 {
        terminal.acked = header.frame;
        let _ = asks.send(Ask::Terminal(id, TerminalAsk::Ack(header.frame)));
    }
    let top = terminal.top_line();
    let end = top + u64::from(header.rows);
    let mut page = top / PAGE * PAGE;
    while page < end.min(header.history_end) {
        let start = page.max(terminal.first_line());
        let missing = (start..(page + PAGE).min(header.history_end)).any(|line| !terminal.history.contains_key(&line));
        if missing && terminal.pages_asked.insert(start) {
            let count = (page + PAGE).min(header.history_end) - start;
            let _ = asks.send(Ask::Terminal(id, TerminalAsk::Lines { start, count: count as u32 }));
        }
        page += PAGE;
    }
    let tokens = Tokens::get(cx);
    let view = View { terminal, top, tokens, focused: terminal.focus.is_focused(window) };
    Layout::new(&view, bounds.origin, &metrics, window)
}

/// A terminal as the grid draws it: the rows from `top`, coloured by the theme.
struct View<'a> {
    terminal: &'a Terminal,
    top: u64,
    tokens: &'a Tokens,
    focused: bool,
}

impl View<'_> {
    fn rgb(&self, color: Color) -> Hsla {
        let [r, g, b] = match color {
            Color::Rgb(rgb) => rgb,
            Color::Named(index) => self.named(index),
            Color::Indexed(index) => self.named(u16::from(index)),
        };
        Rgba { r: f32::from(r) / 255., g: f32::from(g) / 255., b: f32::from(b) / 255., a: 1. }.into()
    }

    fn named(&self, index: u16) -> [u8; 3] {
        let palette = &self.terminal.screen.header.palette;
        palette
            .iter()
            .find(|(named, _)| *named == index)
            .map_or_else(|| self.tokens.terminal.color(usize::from(index)), |(_, rgb)| *rgb)
    }

    fn look(&self, style: &wire::Style) -> Look {
        let flags = style.flags;
        let default_bg = style.bg == Color::Named(wire::NAMED_BACKGROUND);
        let (mut fg, mut bg) = (self.rgb(style.fg), (!default_bg).then(|| self.rgb(style.bg)));
        if flags & wire::STYLE_INVERSE != 0 {
            (fg, bg) = (bg.unwrap_or_else(|| self.rgb(style.bg)), Some(fg));
        }
        if flags & wire::STYLE_DIM != 0 {
            fg.a *= 0.6;
        }
        if flags & wire::STYLE_HIDDEN != 0 {
            fg.a = 0.;
        }
        let underline = [
            (wire::STYLE_UNDERLINE, Underline::Single),
            (wire::STYLE_DOUBLE_UNDERLINE, Underline::Double),
            (wire::STYLE_CURLY_UNDERLINE, Underline::Curly),
            (wire::STYLE_DOTTED_UNDERLINE, Underline::Dotted),
            (wire::STYLE_DASHED_UNDERLINE, Underline::Dashed),
        ]
        .into_iter()
        .find_map(|(flag, underline)| (flags & flag != 0).then_some(underline));
        Look {
            fg,
            bg,
            bold: flags & wire::STYLE_BOLD != 0,
            italic: flags & wire::STYLE_ITALIC != 0,
            underline,
            underline_color: style.underline_color.map(|color| self.rgb(color)),
            strike: flags & wire::STYLE_STRIKE != 0,
        }
    }
}

impl Grid for View<'_> {
    fn size(&self) -> (u16, u16) {
        (self.terminal.screen.header.cols, self.terminal.screen.header.rows)
    }

    fn row(&self, row: u16, each: &mut dyn FnMut(Span<'_>)) {
        let Some(line) = self.terminal.row(self.top + u64::from(row)) else {
            return;
        };
        for span in &line.spans {
            each(Span { column: span.col, text: &span.text, wide: span.wide, cluster: span.cluster, look: self.look(&span.style) });
        }
    }

    fn cursor(&self) -> Option<Cursor> {
        let header = &self.terminal.screen.header;
        let line = header.history_end + u64::from(header.cursor.row);
        let row = u16::try_from(line.checked_sub(self.top)?).ok().filter(|row| *row < header.rows)?;
        let shape = match header.cursor.shape & !wire::CURSOR_BLINKS {
            wire::CURSOR_HIDDEN => return None,
            _ if !self.focused => CursorShape::Hollow,
            wire::CURSOR_UNDERLINE => CursorShape::Underline,
            wire::CURSOR_BEAM => CursorShape::Beam,
            wire::CURSOR_HOLLOW => CursorShape::Hollow,
            _ => CursorShape::Block,
        };
        let color = self.rgb(Color::Named(258));
        Some(Cursor { row, column: header.cursor.col, shape, color, text: self.tokens.terminal_cursor_text })
    }

    fn highlights(&self, each: &mut dyn FnMut(Highlight)) {
        let Some((start, end)) = self.terminal.selection else {
            return;
        };
        let cols = self.terminal.screen.header.cols;
        for row in 0..self.terminal.screen.header.rows {
            let line = self.top + u64::from(row);
            if line < start.line || line > end.line {
                continue;
            }
            let from = if line == start.line { start.col } else { 0 };
            let to = if line == end.line { end.col + 1 } else { cols };
            each(Highlight { row, columns: from..to, color: self.tokens.terminal_selection });
        }
    }
}

fn send(bench: &mut Workbench, id: SessionId, input: TerminalInput) {
    if let Some(terminal) = bench.terminals.open.get_mut(&id) {
        terminal.top = None;
    }
    let _ = bench.asks.send(Ask::Terminal(id, TerminalAsk::Input(input)));
}

fn mods(modifiers: &Modifiers) -> Mods {
    Mods { shift: modifiers.shift, alt: modifiers.alt, ctrl: modifiers.control, meta: modifiers.platform }
}

/// A key GPUI names as the `KeyboardEvent.key` the core's encoder takes.
fn named_key(key: &str) -> Option<String> {
    Some(match key {
        "enter" => "Enter".into(),
        "backspace" => "Backspace".into(),
        "tab" => "Tab".into(),
        "escape" => "Escape".into(),
        "up" => "ArrowUp".into(),
        "down" => "ArrowDown".into(),
        "left" => "ArrowLeft".into(),
        "right" => "ArrowRight".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        "insert" => "Insert".into(),
        "delete" => "Delete".into(),
        _ if key.len() > 1 && key.starts_with('f') && key[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)) => {
            key.to_uppercase()
        }
        _ => return None,
    })
}

/// Handles a key, answering whether the terminal took it.
fn key(bench: &mut Workbench, id: SessionId, keystroke: &Keystroke, cx: &mut Context<Workbench>) -> bool {
    let Some(terminal) = bench.terminals.open.get_mut(&id) else {
        return false;
    };
    let modifiers = &keystroke.modifiers;
    let key = keystroke.key.as_str();
    let header = &terminal.screen.header;
    if modifiers.platform {
        match key {
            "c" if terminal.selection.is_some() => {
                let _ = bench.asks.send(Ask::Terminal(id, TerminalAsk::Copy(terminal.selection)));
            }
            "a" => {
                let start = GridPoint { line: terminal.first_line(), col: 0 };
                let last = header.history_end + u64::from(header.rows.saturating_sub(1));
                terminal.selection = Some((start, GridPoint { line: last, col: header.cols.saturating_sub(1) }));
            }
            "v" => match cx.read_from_clipboard().and_then(|item| item.text()) {
                Some(text) => send(bench, id, TerminalInput::Paste { text }),
                None => return false,
            },
            "left" | "right" | "backspace" => {
                let input = TerminalInput::Key { key: named_key(key).unwrap_or_default(), mods: mods(modifiers) };
                send(bench, id, input);
            }
            _ => return false,
        }
        return true;
    }
    if modifiers.shift && matches!(key, "pageup" | "pagedown") && header.modes & wire::MODE_ALT_SCREEN == 0 {
        let rows = i64::from(header.rows);
        scroll(terminal, if key == "pageup" { -rows } else { rows });
        return true;
    }
    let input = if let Some(name) = named_key(key) {
        TerminalInput::Key { key: name, mods: mods(modifiers) }
    } else if modifiers.control || modifiers.alt {
        let typed = if key == "space" { " " } else { key };
        if typed.chars().count() != 1 {
            return false;
        }
        let typed = if modifiers.shift { typed.to_uppercase() } else { typed.to_owned() };
        TerminalInput::Key { key: typed, mods: mods(modifiers) }
    } else {
        match &keystroke.key_char {
            Some(text) if !text.is_empty() => TerminalInput::Text { text: text.clone() },
            _ => return false,
        }
    };
    send(bench, id, input);
    true
}

/// Moves the view by `lines`, negative toward older output.
fn scroll(terminal: &mut Terminal, lines: i64) {
    let end = terminal.screen.header.history_end;
    let top = terminal.top_line().saturating_add_signed(lines).clamp(terminal.first_line(), end);
    terminal.top = (top < end).then_some(top);
}

/// The cell under a window position, as a line number and column.
fn point_at(terminal: &Terminal, metrics: &Metrics, position: Point<Pixels>) -> (GridPoint, u16) {
    let header = &terminal.screen.header;
    let (col, row) = metrics.cell_at(position - terminal.origin, (header.cols, header.rows));
    (GridPoint { line: terminal.top_line() + u64::from(row), col }, row)
}

fn reports_mouse(terminal: &Terminal, modifiers: &Modifiers) -> bool {
    terminal.screen.header.modes & wire::MODE_MOUSE_ANY != 0 && !modifiers.shift && !modifiers.alt
}

fn mouse_down(bench: &mut Workbench, id: SessionId, event: &MouseDownEvent, window: &mut Window, cx: &mut App) {
    let Some(metrics) = bench.terminals.metrics.clone() else {
        return;
    };
    let Some(terminal) = bench.terminals.open.get_mut(&id) else {
        return;
    };
    window.focus(&terminal.focus, cx);
    let (point, row) = point_at(terminal, &metrics, event.position);
    if event.modifiers.platform {
        let link = terminal.row(point.line).and_then(|line| {
            let span = line.spans.iter().find(|span| (span.col..span.col + span.width).contains(&point.col))?;
            span.style.link.clone()
        });
        if let Some(link) = link {
            cx.open_url(&link);
        }
        return;
    }
    if reports_mouse(terminal, &event.modifiers) {
        let input = TerminalInput::Mouse { action: MouseAction::Press, button: MouseButton::Left, col: point.col, row, mods: mods(&event.modifiers) };
        let _ = bench.asks.send(Ask::Terminal(id, TerminalAsk::Input(input)));
        return;
    }
    terminal.selection = None;
    terminal.anchor = Some(point);
}

fn mouse_move(bench: &mut Workbench, id: SessionId, event: &MouseMoveEvent) -> bool {
    let Some(metrics) = bench.terminals.metrics.clone() else {
        return false;
    };
    let Some(terminal) = bench.terminals.open.get_mut(&id) else {
        return false;
    };
    let (point, row) = point_at(terminal, &metrics, event.position);
    let modes = terminal.screen.header.modes;
    if reports_mouse(terminal, &event.modifiers) {
        let pressed = event.pressed_button == Some(Button::Left);
        if modes & wire::MODE_MOUSE_MOTION != 0 || (pressed && modes & wire::MODE_MOUSE_DRAG != 0) {
            let button = if pressed { MouseButton::Left } else { MouseButton::None };
            let input = TerminalInput::Mouse { action: MouseAction::Move, button, col: point.col, row, mods: mods(&event.modifiers) };
            let _ = bench.asks.send(Ask::Terminal(id, TerminalAsk::Input(input)));
        }
        return false;
    }
    let Some(anchor) = terminal.anchor.filter(|_| event.pressed_button == Some(Button::Left)) else {
        return false;
    };
    terminal.selection = Some(if point < anchor { (point, anchor) } else { (anchor, point) });
    true
}

fn mouse_up(bench: &mut Workbench, id: SessionId, event: &MouseUpEvent) {
    let Some(metrics) = bench.terminals.metrics.clone() else {
        return;
    };
    let Some(terminal) = bench.terminals.open.get_mut(&id) else {
        return;
    };
    terminal.anchor = None;
    if reports_mouse(terminal, &event.modifiers) {
        let (point, row) = point_at(terminal, &metrics, event.position);
        let input = TerminalInput::Mouse { action: MouseAction::Release, button: MouseButton::Left, col: point.col, row, mods: mods(&event.modifiers) };
        let _ = bench.asks.send(Ask::Terminal(id, TerminalAsk::Input(input)));
    }
}

fn wheel(bench: &mut Workbench, id: SessionId, event: &ScrollWheelEvent) -> bool {
    let Some(metrics) = bench.terminals.metrics.clone() else {
        return false;
    };
    let Some(terminal) = bench.terminals.open.get_mut(&id) else {
        return false;
    };
    terminal.wheel += event.delta.pixel_delta(metrics.cell.height).y / metrics.cell.height;
    let lines = terminal.wheel.trunc();
    terminal.wheel -= lines;
    if lines == 0. {
        return false;
    }
    let modes = terminal.screen.header.modes;
    let alternate = modes & wire::MODE_ALT_SCREEN != 0 && modes & wire::MODE_ALTERNATE_SCROLL != 0;
    if reports_mouse(terminal, &event.modifiers) || alternate {
        let (point, row) = point_at(terminal, &metrics, event.position);
        let input = TerminalInput::Wheel { col: point.col, row, lines: -(lines as i32), mods: mods(&event.modifiers) };
        let _ = bench.asks.send(Ask::Terminal(id, TerminalAsk::Input(input)));
        return false;
    }
    scroll(terminal, -(lines as i64));
    true
}

