//! Draws the window offscreen, as the Metal renderer would draw it, and saves it as a PNG at
//! 2x, so the UI can be checked while no one is at the screen:
//!   cargo run --example shot -- out.png [seconds to wait for the core]
//!     [--account "Name <email>"]  draws the rail as if the host were signed in as them
//!     [--scroll pixels]           scrolls the side rail down that far first

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{point, px, size, AppContext, HeadlessAppContext, Modifiers, PlatformInput, ScrollDelta, ScrollWheelEvent, TouchPhase};
use sikemux_wire::protocol::AccountInfo;
use sikemux_gpui::{core, icons, theme, workbench, WINDOW_SIZE};

fn main() -> anyhow::Result<()> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "shot.png".into());
    let wait: f64 = std::env::args().nth(2).and_then(|seconds| seconds.parse().ok()).unwrap_or(3.);
    let option = |name: &str| {
        let args: Vec<String> = std::env::args().collect();
        args.iter().position(|arg| arg == name).and_then(|at| args.get(at + 1).cloned())
    };
    let account = option("--account");
    let scroll: Option<f32> = option("--scroll").and_then(|pixels| pixels.parse().ok());
    let bench = Rc::new(RefCell::new(None));
    let kept = bench.clone();
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::bench_text_system(),
        Arc::new(icons::Icons),
        gpui_platform::current_headless_renderer,
    );
    let link = core::start(core::Home::new());
    let window = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::install(cx);
        let link = link;
        cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                    origin: gpui::point(px(0.), px(0.)),
                    size: size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| workbench::Workbench::new(link, window, cx));
                *kept.borrow_mut() = Some(view.clone());
                cx.new(|cx| gpui_kit::base::Root::new(view, window, cx))
            },
        )
    })?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs_f64(wait) {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    if let (Some(account), Some(bench)) = (account, bench.borrow().clone()) {
        let (name, email) = account.split_once(" <").map_or((account.as_str(), ""), |(name, email)| (name, email.trim_end_matches('>')));
        let shown = AccountInfo { name: Some(name.into()), email: Some(email.into()), picture_hash: None };
        cx.update(|cx| bench.update(cx, |bench, cx| {
            if let Some(view) = bench.view.as_mut() {
                view.account = Some(Box::new(shown));
            }
            cx.notify();
        }));
    }
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    if let Some(pixels) = scroll {
        cx.update_window(window.into(), |_, window, cx| {
            let wheel = ScrollWheelEvent {
                position: point(px(120.), px(400.)),
                delta: ScrollDelta::Pixels(point(px(0.), px(-pixels))),
                modifiers: Modifiers::default(),
                touch_phase: TouchPhase::Moved,
            };
            window.dispatch_event(PlatformInput::ScrollWheel(wheel), cx);
        })?;
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    }
    cx.capture_screenshot(window.into())?.save(&out)?;
    Ok(())
}
