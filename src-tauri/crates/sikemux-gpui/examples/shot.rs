//! Draws the window offscreen, as the Metal renderer would draw it, and saves it as a PNG at
//! 2x, so the UI can be checked while no one is at the screen:
//!   cargo run --example shot -- out.png [seconds to wait for the core]
//!     [--account "Name <email>"]  draws the rail as if the host were signed in as them
//!     [--scroll pixels]           scrolls the side rail down that far first
//!     [--files]                   shows the tab rail's Files page
//!     [--then step…]              then does each step in turn: "click x,y" in points,
//!                                 "type text", "key cmd-enter", "wait seconds", "shot out.png", "scroll x,y pixels",
//!                                 "drag x,y path…" or "drop x,y path…" to hold files over
//!                                 a place or drop them there, or "copy-image file.png"

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    point, px, size, AppContext, ClipboardEntry, ClipboardItem, ExternalPaths, FileDropEvent,
    HeadlessAppContext, Image, ImageFormat, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, ScrollDelta, ScrollWheelEvent, TouchPhase,
};
use sikemux_gpui::{core, icons, theme, workbench, WINDOW_SIZE};
use sikemux_wire::protocol::AccountInfo;

fn main() -> anyhow::Result<()> {
    let out = std::env::args().nth(1).unwrap_or_else(|| "shot.png".into());
    let wait: f64 = std::env::args()
        .nth(2)
        .and_then(|seconds| seconds.parse().ok())
        .unwrap_or(3.);
    let option = |name: &str| {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1).cloned())
    };
    let account = option("--account");
    let files = std::env::args().any(|arg| arg == "--files");
    let scroll: Option<f32> = option("--scroll").and_then(|pixels| pixels.parse().ok());
    let steps: Vec<String> = std::env::args()
        .skip_while(|arg| arg != "--then")
        .skip(1)
        .collect();
    let bench = Rc::new(RefCell::new(None));
    let kept = bench.clone();
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::bench_text_system(),
        Arc::new(icons::Icons),
        gpui_platform::current_headless_renderer,
    );
    let link = core::start(core::Home::default());
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
    if files {
        if let Some(bench) = bench.borrow().clone() {
            cx.update(|cx| {
                bench.update(cx, |bench, cx| {
                    bench.rail_page = workbench::RailPage::Files;
                    cx.notify();
                })
            });
        }
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs_f64(wait) {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    if let (Some(account), Some(bench)) = (account, bench.borrow().clone()) {
        let (name, email) = account
            .split_once(" <")
            .map_or((account.as_str(), ""), |(name, email)| {
                (name, email.trim_end_matches('>'))
            });
        let shown = AccountInfo {
            name: Some(name.into()),
            email: Some(email.into()),
            picture_hash: None,
        };
        cx.update(|cx| {
            bench.update(cx, |bench, cx| {
                if let Some(view) = bench.view.as_mut() {
                    view.account = Some(Box::new(shown));
                }
                cx.notify();
            })
        });
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
    for step in steps {
        let (verb, rest) = step.split_once(' ').unwrap_or((step.as_str(), ""));
        cx.update_window(window.into(), |_, window, cx| -> anyhow::Result<()> {
            match verb {
                "click" => {
                    let (x, y) = rest
                        .split_once(',')
                        .ok_or_else(|| anyhow::anyhow!("click x,y"))?;
                    let position = point(px(x.trim().parse()?), px(y.trim().parse()?));
                    let modifiers = Modifiers::default();
                    window.dispatch_event(
                        PlatformInput::MouseMove(MouseMoveEvent {
                            position,
                            pressed_button: None,
                            modifiers,
                        }),
                        cx,
                    );
                    window.draw(cx).clear(cx);
                    let (button, click_count) = (MouseButton::Left, 1);
                    window.dispatch_event(
                        PlatformInput::MouseDown(MouseDownEvent {
                            button,
                            position,
                            modifiers,
                            click_count,
                            first_mouse: false,
                        }),
                        cx,
                    );
                    window.dispatch_event(
                        PlatformInput::MouseUp(MouseUpEvent {
                            button,
                            position,
                            modifiers,
                            click_count,
                        }),
                        cx,
                    );
                }
                "type" => {
                    for letter in rest.chars() {
                        let key = if letter == ' ' {
                            "space".to_owned()
                        } else {
                            letter.to_string()
                        };
                        window.dispatch_keystroke(Keystroke::parse(&key)?, cx);
                    }
                }
                "key" => {
                    window.dispatch_keystroke(Keystroke::parse(rest)?, cx);
                }
                "scroll" => {
                    let (at, pixels) = rest
                        .split_once(' ')
                        .ok_or_else(|| anyhow::anyhow!("scroll x,y pixels"))?;
                    let (x, y) = at
                        .split_once(',')
                        .ok_or_else(|| anyhow::anyhow!("scroll x,y pixels"))?;
                    let wheel = ScrollWheelEvent {
                        position: point(px(x.trim().parse()?), px(y.trim().parse()?)),
                        delta: ScrollDelta::Pixels(point(px(0.), px(pixels.trim().parse()?))),
                        modifiers: Modifiers::default(),
                        touch_phase: TouchPhase::Moved,
                    };
                    window.dispatch_event(PlatformInput::ScrollWheel(wheel), cx);
                }
                "drag" | "drop" => {
                    let (at, paths) = rest
                        .split_once(' ')
                        .ok_or_else(|| anyhow::anyhow!("{verb} x,y path…"))?;
                    let (x, y) = at
                        .split_once(',')
                        .ok_or_else(|| anyhow::anyhow!("{verb} x,y path…"))?;
                    let position = point(px(x.trim().parse()?), px(y.trim().parse()?));
                    let paths = ExternalPaths(paths.split(' ').map(PathBuf::from).collect());
                    window.dispatch_event(
                        PlatformInput::FileDrop(FileDropEvent::Entered { position, paths }),
                        cx,
                    );
                    window.draw(cx).clear(cx);
                    window.dispatch_event(
                        PlatformInput::FileDrop(FileDropEvent::Pending { position }),
                        cx,
                    );
                    if verb == "drop" {
                        window.draw(cx).clear(cx);
                        window.dispatch_event(
                            PlatformInput::FileDrop(FileDropEvent::Submit { position }),
                            cx,
                        );
                    }
                }
                "copy-image" => {
                    let image = Image::from_bytes(ImageFormat::Png, std::fs::read(rest)?);
                    cx.write_to_clipboard(ClipboardItem {
                        entries: vec![ClipboardEntry::Image(image)],
                    });
                }
                _ => {}
            }
            Ok(())
        })??;
        let pause = if verb == "wait" {
            rest.parse().unwrap_or(1.)
        } else {
            0.3
        };
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs_f64(pause) {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(50));
        }
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
        if verb == "shot" {
            cx.capture_screenshot(window.into())?.save(rest)?;
        }
    }
    Ok(())
}
