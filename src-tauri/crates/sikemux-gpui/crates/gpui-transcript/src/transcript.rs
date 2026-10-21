use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    div, list, px, AnyElement, App, ClipboardItem, Context, FocusHandle, FollowMode,
    InteractiveElement, IntoElement, KeyDownEvent, ListAlignment, ListState, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Point, Render, SharedString, Styled, Window,
};

use crate::draw::{Pen, Style};
use crate::markdown::Markdown;
use crate::select::{spot_at, Place, Selecting, Shown, Span, Spot};

/// A row of a transcript, known by an id that stays the same for as long as the row lives.
pub trait Row: 'static {
    fn id(&self) -> &str;

    /// The markdown the row draws, in the order it draws it, for copying a selection.
    fn docs(&self) -> Vec<&Markdown> {
        Vec::new()
    }
}

/// One change to a transcript's rows, as a host maps its own feed onto them.
pub enum Change<R> {
    /// The row replaces the one with its id, or goes at the end.
    Put(R),
    Update(SharedString, Box<dyn FnOnce(&mut R)>),
    /// The row and every row after it go.
    DropFrom(SharedString),
    /// Only these rows stay, from the first of them on.
    Keep(Vec<SharedString>),
    /// An earlier page, oldest first, goes above the rows held; any already held are skipped.
    Prepend(Vec<R>),
    Reset(Vec<R>),
}

type DrawRow<R> = Rc<dyn Fn(&[R], usize, &Pen, &mut Window, &mut App) -> AnyElement>;
type Ask = Rc<dyn Fn(&mut Window, &mut App)>;

/// Rows within this many of the first one count as the top, where an earlier page is asked for.
const NEAR_TOP: usize = 8;

/// A hidden transcript draws nothing, so it holds changes until it is shown,
/// but no more than this many before it applies them anyway.
const MOST_QUEUED: usize = 1024;

/// A chat's rows drawn from the bottom up, only those in view. Changes pushed
/// in one frame are applied together when it draws; rows added above never
/// move what the reader sees, and the end stays in view while they are at it.
/// The reader selects text across rows by dragging or shift-clicking, and ⌘C copies it.
pub struct Transcript<R: Row> {
    rows: Vec<R>,
    list: ListState,
    queue: Vec<Change<R>>,
    style: Rc<Style>,
    draw_row: DrawRow<R>,
    on_reach_top: Option<Ask>,
    asked_earlier: bool,
    focus: FocusHandle,
    ends: Option<(Spot, Spot)>,
    dragging: bool,
    shown: Rc<RefCell<Vec<Shown>>>,
}

impl<R: Row> Transcript<R> {
    /// `draw_row` draws the row at an index with the pen it is handed; it gets
    /// every row, so a row can look at its neighbours.
    pub fn new(
        rows: Vec<R>,
        style: Rc<Style>,
        draw_row: impl Fn(&[R], usize, &Pen, &mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        let list = ListState::new(rows.len(), ListAlignment::Bottom, px(400.));
        list.set_follow_mode(FollowMode::Tail);
        let this = cx.weak_entity();
        list.set_scroll_handler(move |event, window, cx| {
            let near_top = event.visible_range.start < NEAR_TOP;
            this.update(cx, |this, cx| this.scrolled(near_top, window, cx))
                .ok();
        });
        Transcript {
            rows,
            list,
            queue: Vec::new(),
            style,
            draw_row: Rc::new(draw_row),
            on_reach_top: None,
            asked_earlier: false,
            focus: cx.focus_handle(),
            ends: None,
            dragging: false,
            shown: Rc::default(),
        }
    }

    /// Called once each time the reader scrolls near the first row held, to fetch an earlier page.
    pub fn on_reach_top(mut self, ask: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reach_top = Some(Rc::new(ask));
        self
    }

    /// The rows as of the last frame drawn; changes pushed since are not in them yet.
    pub fn rows(&self) -> &[R] {
        &self.rows
    }

    pub fn list(&self) -> &ListState {
        &self.list
    }

    pub fn push(&mut self, change: Change<R>, cx: &mut Context<Self>) {
        self.extend([change], cx);
    }

    pub fn extend(&mut self, changes: impl IntoIterator<Item = Change<R>>, cx: &mut Context<Self>) {
        self.queue.extend(changes);
        if self.queue.len() >= MOST_QUEUED {
            self.apply();
        }
        cx.notify();
    }

    /// Most changes reach the newest rows, so the search starts from the end.
    fn position(&self, id: &str) -> Option<usize> {
        self.rows.iter().rposition(|row| row.id() == id)
    }

    fn apply(&mut self) {
        for change in std::mem::take(&mut self.queue) {
            let end = self.rows.len();
            match change {
                Change::Put(row) => match self.position(row.id()) {
                    Some(ix) => {
                        self.rows[ix] = row;
                        self.list.remeasure_items(ix..ix + 1);
                    }
                    None => {
                        self.rows.push(row);
                        self.list.splice(end..end, 1);
                    }
                },
                Change::Update(id, change) => {
                    if let Some(ix) = self.position(&id) {
                        change(&mut self.rows[ix]);
                        self.list.remeasure_items(ix..ix + 1);
                    }
                }
                Change::DropFrom(id) => {
                    if let Some(ix) = self.position(&id) {
                        self.rows.truncate(ix);
                        self.list.splice(ix..end, 0);
                    }
                }
                Change::Keep(ids) => {
                    let kept: HashSet<&str> = ids.iter().map(|id| id.as_ref()).collect();
                    let first = self.rows.iter().position(|row| kept.contains(row.id()));
                    for ix in (first.unwrap_or(0)..end).rev() {
                        if !kept.contains(self.rows[ix].id()) {
                            self.rows.remove(ix);
                            self.list.splice(ix..ix + 1, 0);
                        }
                    }
                }
                Change::Prepend(rows) => {
                    let held: HashSet<&str> = self.rows.iter().map(Row::id).collect();
                    let older: Vec<R> = rows
                        .into_iter()
                        .filter(|row| !held.contains(row.id()))
                        .collect();
                    self.list.splice(0..0, older.len());
                    self.rows.splice(0..0, older);
                    self.asked_earlier = false;
                }
                Change::Reset(rows) => {
                    self.rows = rows;
                    self.list.reset(self.rows.len());
                    self.list.set_follow_mode(FollowMode::Tail);
                    self.asked_earlier = false;
                }
            }
        }
    }

    fn scrolled(&mut self, near_top: bool, window: &mut Window, cx: &mut App) {
        if !near_top {
            self.asked_earlier = false;
            return;
        }
        if self.asked_earlier || self.rows.is_empty() {
            return;
        }
        self.asked_earlier = true;
        if let Some(ask) = self.on_reach_top.clone() {
            ask(window, cx);
        }
    }

    fn place(&self, spot: &Spot) -> Option<Place> {
        let (row, doc, block, text) = &spot.key;
        Some((self.position(row)?, *doc, *block, *text, spot.at))
    }

    /// The selection's ends in reading order, while both rows are still held.
    fn span(&self) -> Option<Span> {
        let (anchor, head) = self.ends.as_ref()?;
        let (anchor, head) = (self.place(anchor)?, self.place(head)?);
        Some(Span(anchor.min(head), anchor.max(head)))
    }

    fn press(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        let Some(spot) = spot_at(&self.shown.borrow(), event.position) else {
            return;
        };
        self.ends = match self.ends.take() {
            Some((anchor, _)) if event.modifiers.shift => Some((anchor, spot)),
            _ => Some((spot.clone(), spot)),
        };
        self.dragging = true;
        cx.notify();
    }

    /// Moves the selection's free end, scrolling when the pointer is past the top or bottom.
    fn drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.dragging || !event.dragging() {
            return;
        }
        let view = self.list.viewport_bounds();
        let (y, top, bottom) = (event.position.y, view.top(), view.bottom());
        if y < top || y > bottom {
            self.list
                .scroll_by(if y < top { y - top } else { y - bottom });
        }
        let inside = Point::new(event.position.x, y.clamp(top, bottom - px(1.)));
        let spot = spot_at(&self.shown.borrow(), inside);
        if let (Some(spot), Some((_, head))) = (spot, self.ends.as_mut()) {
            *head = spot;
        }
        cx.notify();
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if !keystroke.modifiers.platform || keystroke.key != "c" {
            return;
        }
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            cx.stop_propagation();
        }
    }

    /// The selected text, read from the rows themselves, so rows out of view copy too.
    fn selected_text(&self) -> Option<String> {
        let span = self.span()?;
        let mut blocks = Vec::new();
        for ix in span.0 .0..=span.1 .0 {
            for (doc, markdown) in self.rows[ix].docs().into_iter().enumerate() {
                for block in 0..markdown.blocks().len() {
                    let texts = markdown.texts(block);
                    let picked: Vec<&str> = (1..)
                        .zip(&texts)
                        .filter_map(|(n, text)| {
                            text.get(span.range((ix, doc, block, n), text.len())?)
                        })
                        .collect();
                    if !picked.is_empty() {
                        blocks.push(picked.join("\n"));
                    }
                }
            }
        }
        (!blocks.is_empty()).then(|| blocks.join("\n\n"))
    }
}

impl<R: Row> Render for Transcript<R> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply();
        self.shown.borrow_mut().clear();
        let span = self.span();
        let this = cx.entity();
        let rows = list(self.list.clone(), move |ix, window, cx| {
            this.update(cx, |this, cx| {
                let selecting = this.rows.get(ix).map(|row| Selecting {
                    row: row.id().to_owned().into(),
                    ix,
                    span,
                    shown: this.shown.clone(),
                });
                let pen = Pen::selecting(this.style.clone(), selecting);
                let draw_row = this.draw_row.clone();
                draw_row(&this.rows, ix, &pen, window, cx)
            })
        });
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event, window, cx| this.press(event, window, cx)),
            )
            .on_mouse_move(cx.listener(|this, event, _, cx| this.drag(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_key_down(cx.listener(|this, event, _, cx| this.key(event, cx)))
            .child(rows.size_full())
    }
}
