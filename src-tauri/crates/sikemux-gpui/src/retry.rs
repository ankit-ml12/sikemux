use std::time::Duration;

use gpui_kit::{Context, Task};

const FIRST: Duration = Duration::from_secs(1);
const LONGEST: Duration = Duration::from_secs(60);

/// Asks a refused call again after a wait that doubles each time, from a second up to a
/// minute, so a call the core keeps refusing is never asked in a tight loop.
#[derive(Default)]
pub struct Retry {
    tries: u32,
    waiting: Option<Task<()>>,
}

impl Retry {
    /// The call was answered, so the next refusal waits the shortest time again.
    pub fn answered(&mut self) {
        self.tries = 0;
    }

    /// Runs `again` once the wait is over, unless a retry is already waiting.
    pub fn refused<T: 'static>(
        &mut self,
        cx: &mut Context<T>,
        retry: fn(&mut T) -> &mut Retry,
        again: impl FnOnce(&mut T, &mut Context<T>) + 'static,
    ) {
        if self.waiting.is_some() {
            return;
        }
        let wait = FIRST.saturating_mul(1 << self.tries.min(6)).min(LONGEST);
        self.tries += 1;
        self.waiting = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| {
                retry(this).waiting = None;
                again(this, cx);
            });
        }));
    }
}
