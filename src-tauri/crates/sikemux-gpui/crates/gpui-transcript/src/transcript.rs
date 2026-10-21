use std::rc::Rc;

use gpui::{
    list, px, AnyElement, App, Context, FollowMode, IntoElement, ListAlignment, ListState, Render,
    SharedString, Styled, Window,
};

use crate::rows::{Edit, Row, Rows};

/// One change to a transcript's rows, as a host maps its own feed onto them.
pub enum Change<R> {
    /// The row replaces the one with its id, or goes at the end.
    Put(R),
    Update(SharedString, Box<dyn FnOnce(&mut R)>),
    /// The row and every row after it go.
    DropFrom(SharedString),
    /// Only these rows stay, from the first of them on.
    Keep(Vec<SharedString>),
    /// An earlier page, oldest first, goes above the rows held.
    Prepend(Vec<R>),
    Reset(Vec<R>),
}

type DrawRow<R> = Rc<dyn Fn(&Rows<R>, usize, &mut Window, &mut App) -> AnyElement>;
type Ask = Rc<dyn Fn(&mut Window, &mut App)>;

/// Rows within this many of the first one count as the top, where an earlier page is asked for.
const NEAR_TOP: usize = 8;

/// A hidden transcript draws nothing, so it holds changes until it is shown,
/// but no more than this many before it applies them anyway.
const MOST_QUEUED: usize = 1024;

/// A chat's rows drawn from the bottom up, only those in view. Changes pushed
/// in one frame are applied together when it draws; rows added above never
/// move what the reader sees, and the end stays in view while they are at it.
pub struct Transcript<R: Row> {
    rows: Rows<R>,
    list: ListState,
    queue: Vec<Change<R>>,
    draw_row: DrawRow<R>,
    on_reach_top: Option<Ask>,
    asked_earlier: bool,
}

impl<R: Row> Transcript<R> {
    /// `draw_row` draws the row at an index; it gets every row, so a row can look at its neighbours.
    pub fn new(
        rows: Vec<R>,
        draw_row: impl Fn(&Rows<R>, usize, &mut Window, &mut App) -> AnyElement + 'static,
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
            rows: Rows::new(rows),
            list,
            queue: Vec::new(),
            draw_row: Rc::new(draw_row),
            on_reach_top: None,
            asked_earlier: false,
        }
    }

    /// Called once each time the reader scrolls near the first row held, to fetch an earlier page.
    pub fn on_reach_top(mut self, ask: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_reach_top = Some(Rc::new(ask));
        self
    }

    /// The rows as of the last frame drawn; changes pushed since are not in them yet.
    pub fn rows(&self) -> &Rows<R> {
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

    fn apply(&mut self) {
        for change in std::mem::take(&mut self.queue) {
            let edit = match change {
                Change::Put(row) => self.rows.put(row),
                Change::Update(id, change) => self.rows.update(&id, change),
                Change::DropFrom(id) => self.rows.drop_from(&id),
                Change::Keep(ids) => self.rows.keep(ids.iter().map(|id| id.as_ref())),
                Change::Prepend(rows) => {
                    self.asked_earlier = false;
                    self.rows.prepend(rows)
                }
                Change::Reset(rows) => {
                    self.asked_earlier = false;
                    self.rows.reset(rows)
                }
            };
            match edit {
                Edit::Nothing => {}
                Edit::Changed(ix) => self.list.remeasure_items(ix..ix + 1),
                Edit::Spliced(range, count) => self.list.splice(range, count),
                Edit::Removed(ranges) => ranges
                    .into_iter()
                    .for_each(|range| self.list.splice(range, 0)),
                Edit::Reset => {
                    self.list.reset(self.rows.len());
                    self.list.set_follow_mode(FollowMode::Tail);
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
}

impl<R: Row> Render for Transcript<R> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply();
        let this = cx.entity();
        list(self.list.clone(), move |ix, window, cx| {
            this.update(cx, |this, cx| {
                let draw_row = this.draw_row.clone();
                draw_row(&this.rows, ix, window, cx)
            })
        })
        .size_full()
    }
}
