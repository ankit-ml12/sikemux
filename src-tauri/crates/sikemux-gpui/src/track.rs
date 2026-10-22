//! Things laid side by side while they move between one another: the stage's tabs and the
//! tab rail's pages. A swipe drags them, a landing springs them onto one, and the stage
//! pans one tab over when its tab changes some other way.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::{
    canvas, div, relative, AnimationExt, AnyElement, App, Context, DispatchPhase, ElementId,
    InteractiveElement, IntoElement, ParentElement, ScrollWheelEvent, SharedString, Styled, Task,
    TouchPhase, Window,
};
use sikemux_wire::protocol::PaneKind;

use crate::panes;
use crate::swipe::{self, Ends, Swipe, Swiped};
use crate::workbench::{RailPage, Workbench};

/// One row of things that slide: what a hand or a switch is doing to it now.
pub struct Slide<K> {
    swipe: Swipe,
    motion: Option<Motion<K>>,
    /// What was in front as the slide last knew it, to tell a switch from a redraw.
    front: Option<K>,
    /// The row's width as last drawn, in points.
    width: Rc<Cell<f32>>,
    moves: usize,
    settle: Option<Task<()>>,
}

impl<K> Default for Slide<K> {
    fn default() -> Self {
        Self {
            swipe: Swipe::default(),
            motion: None,
            front: None,
            width: Rc::default(),
            moves: 0,
            settle: None,
        }
    }
}

/// Things travelling along the row: each at its place from where the row comes to rest,
/// the row going from `start` to rest at 0.
struct Motion<K> {
    id: usize,
    shown: Vec<(K, i32)>,
    start: f32,
    /// The landing's spring and its start speed, in places a second; none for a pan.
    spring: Option<f32>,
}

/// What a landing does: what it shows and where from, its speed, and what is in front after.
type Landing<K> = (Vec<(K, i32)>, f32, f32, Option<K>);

/// What a row draws: each thing and its place, where the row is, and the motion moving it
/// with its spring.
type Shown<K> = (Vec<(K, i32)>, f32, Option<(usize, Option<f32>)>);

impl<K: Clone + PartialEq> Slide<K> {
    /// What the row shows: each thing and its place, where the row is, and how it moves.
    fn shown(&self, front: K, beside: impl Fn(i32) -> Option<K>) -> Shown<K> {
        if let Some(motion) = &self.motion {
            return (
                motion.shown.clone(),
                motion.start,
                Some((motion.id, motion.spring)),
            );
        }
        if self.swipe.held() && self.swipe.offset != 0. {
            let way = self.swipe.offset.signum() as i32;
            let mut shown = vec![(front, 0)];
            shown.extend(beside(way).map(|next| (next, way)));
            return (shown, self.swipe.offset, None);
        }
        (vec![(front, 0)], 0., None)
    }

    /// Follows a scroll over the row, where `front` is one of `order`. Answers whether the
    /// row took it, and the landing when the hand lifted.
    fn scroll(
        &mut self,
        event: &ScrollWheelEvent,
        order: &[K],
        front: &K,
    ) -> (bool, Option<Landing<K>>) {
        let slot = order.iter().position(|item| item == front).unwrap_or(0);
        let ends = Ends {
            previous: slot > 0,
            next: slot + 1 < order.len(),
        };
        let beside = |way: i32| {
            slot.checked_add_signed(way as isize)
                .and_then(|at| order.get(at))
                .cloned()
        };
        match self.swipe.scroll(event, self.width.get(), ends) {
            Swiped::Ignored => (false, None),
            Swiped::Moved => {
                self.motion = None;
                (true, None)
            }
            Swiped::Landed { step, from, speed } => {
                if step == 0 {
                    let way = from.signum() as i32;
                    let mut shown = vec![(front.clone(), 0)];
                    shown.extend(beside(way).map(|next| (next, way)));
                    return (true, Some((shown, from, speed, None)));
                }
                let Some(target) = beside(step) else {
                    return (true, None);
                };
                self.front = Some(target.clone());
                let shown = vec![(front.clone(), -step), (target.clone(), 0)];
                (true, Some((shown, from, speed, Some(target))))
            }
        }
    }

    /// Starts things travelling; answers the motion and how long until it has arrived.
    fn travel(
        &mut self,
        shown: Vec<(K, i32)>,
        start: f32,
        spring: Option<f32>,
    ) -> (usize, Duration) {
        self.moves += 1;
        self.motion = Some(Motion {
            id: self.moves,
            shown,
            start,
            spring,
        });
        let lasts = if spring.is_some() {
            swipe::SPRING_LONGEST
        } else {
            swipe::PAN
        };
        (self.moves, lasts)
    }

    /// Notes what is in front now, answering what was before when it changed.
    fn moved_to(&mut self, now: Option<K>) -> Option<(K, K)> {
        let was = std::mem::replace(&mut self.front, now.clone());
        match (was, now) {
            (Some(was), Some(now)) if was != now && !self.swipe.held() => Some((was, now)),
            _ => None,
        }
    }
}

impl Workbench {
    /// Follows a scroll over the stage; true when the stage took it as a swipe between tabs.
    fn swipe_stage(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) -> bool {
        let Some(strip) = self.strip() else {
            return false;
        };
        let order: Vec<String> = self.tabs(&strip).iter().map(|tab| tab.id.clone()).collect();
        let Some(front) = self.tab() else {
            return false;
        };
        if event.touch_phase == TouchPhase::Started && !pages_sideways(front) {
            self.track.swipe = Swipe::default();
            return false;
        }
        let front = front.id.clone();
        let (taken, landed) = self.track.scroll(event, &order, &front);
        if let Some((shown, from, speed, target)) = landed {
            if let Some(target) = target {
                self.activate_tab(&strip, &target);
            }
            let moved = self.track.travel(shown, from, Some(speed));
            self.settle(moved, |bench| &mut bench.track, cx);
        }
        if taken {
            self.parts.redraw(&self.parts.stage, cx);
        }
        taken
    }

    /// Follows a scroll over the tab rail; true when it took it as a swipe between pages.
    fn swipe_rail(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) -> bool {
        let front = self.rail_page;
        let (taken, landed) = self.rail_slide.scroll(event, &RailPage::ALL, &front);
        if let Some((shown, from, speed, target)) = landed {
            if let Some(target) = target {
                self.rail_page = target;
            }
            let moved = self.rail_slide.travel(shown, from, Some(speed));
            self.settle(moved, |bench| &mut bench.rail_slide, cx);
        }
        if taken {
            self.parts.redraw(&self.parts.tabs, cx);
        }
        taken
    }

    /// Shows another of the tab rail's pages, springing over to it.
    pub fn show_rail_page(&mut self, page: RailPage, cx: &mut Context<Self>) {
        if self.rail_page != page {
            self.keep_rails(cx);
        }
        self.rail_page = page;
        let Some((was, now)) = self.rail_slide.moved_to(Some(page)) else {
            return cx.notify();
        };
        let at = |page: RailPage| {
            RailPage::ALL
                .iter()
                .position(|kept| *kept == page)
                .unwrap_or(0)
        };
        let way = if at(now) > at(was) { 1 } else { -1 };
        let moved = self
            .rail_slide
            .travel(vec![(was, -way), (now, 0)], -way as f32, Some(0.));
        self.settle(moved, |bench| &mut bench.rail_slide, cx);
        cx.notify();
    }

    /// Pans one tab over when the tab in front changed some other way than a swipe.
    pub(crate) fn front_moved(&mut self, cx: &mut Context<Self>) {
        let Some(strip) = self.strip() else {
            self.track.front = None;
            return;
        };
        let order: Vec<String> = self.tabs(&strip).iter().map(|tab| tab.id.clone()).collect();
        let now = self.tab().map(|tab| tab.id.clone());
        let Some((was, now)) = self.track.moved_to(now) else {
            return;
        };
        let at = |id: &str| order.iter().position(|tab| tab == id);
        let (Some(from), Some(to)) = (at(&was), at(&now)) else {
            return;
        };
        let way = if to > from { 1 } else { -1 };
        let moved = self
            .track
            .travel(vec![(was, -way), (now, 0)], -way as f32, None);
        self.settle(moved, |bench| &mut bench.track, cx);
        self.parts.redraw(&self.parts.stage, cx);
    }

    /// Stops drawing a motion's extra things once it has arrived.
    fn settle<K: 'static>(
        &mut self,
        (id, lasts): (usize, Duration),
        slide: fn(&mut Workbench) -> &mut Slide<K>,
        cx: &mut Context<Self>,
    ) {
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(lasts).await;
            let _ = this.update(cx, |bench, cx| {
                let row = slide(bench);
                if row.motion.as_ref().is_some_and(|motion| motion.id == id) {
                    row.motion = None;
                    cx.notify();
                }
            });
        });
        slide(self).settle = Some(task);
    }
}

/// Whether a swipe over this tab pages between tabs; an editor or a page scrolls sideways
/// itself, so its gestures stay its own.
fn pages_sideways(tab: &sikemux_wire::protocol::Tab) -> bool {
    !matches!(
        crate::tabs::first_view(&tab.pane),
        Some((
            PaneKind::Files | PaneKind::Desk | PaneKind::Browser | PaneKind::Plugin,
            _
        ))
    )
}

/// The stage's tabs: the one in front, with the one beside it while they move.
pub fn stage(
    bench: &Workbench,
    front: &sikemux_wire::protocol::Tab,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let order: Vec<String> = bench
        .strip()
        .map(|strip| {
            bench
                .tabs(&strip)
                .iter()
                .map(|tab| tab.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let slot = order.iter().position(|tab| *tab == front.id).unwrap_or(0);
    let (shown, start, motion) = bench.track.shown(front.id.clone(), |way| {
        slot.checked_add_signed(way as isize)
            .and_then(|at| order.get(at))
            .cloned()
    });
    let mut cards = Vec::new();
    for (id, place) in shown {
        let strip = bench.strip();
        let tab = strip
            .as_ref()
            .and_then(|strip| bench.tabs(strip).iter().find(|tab| tab.id == id))
            .cloned();
        if let Some(tab) = tab {
            cards.push((panes::tab(bench, &tab, window, cx), place));
        }
    }
    let this = cx.entity().downgrade();
    div()
        .id(SharedString::from("stage-track"))
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_hidden()
        .child(listener(&bench.track.width, move |event, cx| {
            this.update(cx, |bench, cx| bench.swipe_stage(event, cx))
                .unwrap_or(false)
        }))
        .child(lane("stage-track", cards, start, motion))
        .into_any_element()
}

/// The tab rail's pages: the one shown, with the one beside it while they move.
pub fn rail(
    bench: &Workbench,
    width: f32,
    page: impl Fn(RailPage, &mut Context<Workbench>) -> AnyElement,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let slot = RailPage::ALL
        .iter()
        .position(|kept| *kept == bench.rail_page)
        .unwrap_or(0);
    let (shown, start, motion) = bench.rail_slide.shown(bench.rail_page, |way| {
        slot.checked_add_signed(way as isize)
            .and_then(|at| RailPage::ALL.get(at))
            .copied()
    });
    let cards = shown
        .into_iter()
        .map(|(shown, place)| (page(shown, cx), place))
        .collect();
    let this = cx.entity().downgrade();
    div()
        .id(SharedString::from("rail-track"))
        .relative()
        .flex_none()
        .w(gpui_kit::px(width))
        .h_full()
        .overflow_hidden()
        .child(listener(&bench.rail_slide.width, move |event, cx| {
            this.update(cx, |bench, cx| bench.swipe_rail(event, cx))
                .unwrap_or(false)
        }))
        .child(lane("rail-track", cards, start, motion))
        .into_any_element()
}

/// A row of things, each a row's width, sitting `start` places along and travelling to
/// rest at 0 while it moves.
fn lane(
    id: &'static str,
    mut cards: Vec<(AnyElement, i32)>,
    start: f32,
    motion: Option<(usize, Option<f32>)>,
) -> AnyElement {
    cards.sort_by_key(|(_, place)| *place);
    let first = cards.first().map_or(0, |(_, place)| *place);
    let last = cards.last().map_or(0, |(_, place)| *place);
    let count = (last - first + 1) as f32;
    let lane = div()
        .absolute()
        .top_0()
        .bottom_0()
        .flex()
        .w(relative(count))
        .left(relative(first as f32 - start))
        .children(
            cards
                .into_iter()
                .map(|(card, _)| div().h_full().w(relative(1. / count)).flex().child(card)),
        );
    let Some((moved, spring)) = motion else {
        return lane.into_any_element();
    };
    let animation = match spring {
        Some(speed) => swipe::spring(start, speed),
        None => swipe::pan(),
    };
    lane.with_animation(
        ElementId::NamedInteger(id.into(), moved as u64),
        animation,
        move |lane, travelled| lane.left(relative(first as f32 - start * (1. - travelled))),
    )
    .into_any_element()
}

/// Measures its row, and hears scrolls over it before what is under the pointer does, so a
/// swipe does not also scroll what it starts on.
fn listener(
    width: &Rc<Cell<f32>>,
    swiped: impl Fn(&ScrollWheelEvent, &mut App) -> bool + 'static,
) -> impl IntoElement {
    let (width, swiped) = (width.clone(), Rc::new(swiped));
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            width.set(f32::from(bounds.size.width));
            let swiped = swiped.clone();
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture
                    && bounds.contains(&event.position)
                    && swiped(event, cx)
                {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .size_full()
}
