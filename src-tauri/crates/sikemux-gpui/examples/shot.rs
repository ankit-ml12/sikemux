//! Draws the window offscreen, as the Metal renderer would draw it, and saves it as a PNG at
//! 2x, so the UI can be checked while no one is at the screen:
//!   cargo run --example shot -- out.png [seconds to wait for the core]

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{px, size, AppContext, HeadlessAppContext};
use sikemux_gpui::{core, icons, theme, workbench, WINDOW_SIZE};

fn main() -> anyhow::Result<()> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "shot.png".into());
    let wait: f64 = std::env::args().nth(2).and_then(|seconds| seconds.parse().ok()).unwrap_or(3.);
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
                cx.new(|cx| gpui_kit::base::Root::new(view, window, cx))
            },
        )
    })?;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs_f64(wait) {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
    cx.capture_screenshot(window.into())?.save(&out)?;
    Ok(())
}
