//! Which of two nested scrolling things a scroll gesture moves: the one it began over. Something
//! that scrolls inside a scrolling list, such as a diagram in a chat, takes a gesture that starts
//! over it, and lets one that started elsewhere carry on moving the list even when it slides
//! under the pointer.

use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui_kit::{ScrollEvent, TouchPhase};

/// A pause between scroll events this long starts a new gesture, for wheels that send no phases.
const GESTURE_GAP: Duration = Duration::from_millis(180);

thread_local! {
    static TAKEN: Cell<bool> = const { Cell::new(false) };
}

/// Forgets what took the last gesture, as a trackpad gesture starts. Called by what hears
/// scrolls before the things under the pointer do, such as the swipe between tabs.
pub fn gesture_starts() {
    TAKEN.set(false);
}

/// Whether something inside took the gesture under way, so what lies around it, such as the
/// swipe between tabs, leaves the gesture to it.
pub fn taken_inside() -> bool {
    TAKEN.get()
}

#[derive(Default)]
pub struct ScrollLatch {
    last: Option<Instant>,
    mine: bool,
}

impl ScrollLatch {
    /// Whether the element takes `event`. It must hear every scroll in the window, with `over`
    /// saying whether the pointer is on it, so it knows where each gesture began.
    pub fn takes(&mut self, event: &ScrollEvent, over: bool) -> bool {
        let now = Instant::now();
        let starts = event.touch_phase == TouchPhase::Started
            || self
                .last
                .is_none_or(|last| now.duration_since(last) > GESTURE_GAP);
        self.last = Some(now);
        if starts {
            self.mine = over;
            if over {
                TAKEN.set(true);
            }
        }
        self.mine
    }
}
