//! Draws the window offscreen, as the Metal renderer would draw it, and saves it as a PNG at
//! 2x, so the UI can be checked while no one is at the screen:
//!   cargo run --example shot -- out.png [seconds to wait for the core]
//!     [--account "Name <email>"]  draws the rail as if the host were signed in as them
//!     [--scroll pixels]           scrolls the side rail down that far first
//!     [--files]                   shows the tab rail's Files page
//!     [--size 900x700]            draws the window at that size in points
//!     [--then step…]              then does each step in turn: "click x,y", "dclick x,y", "rclick x,y" or "move x,y" in points,
//!                                 "type text", "key cmd-enter", "wait seconds", "shot out.png", "scroll x,y pixels",
//!                                 "swipe x,y pixels" or "swipe x,y pixels hold" for a sideways trackpad swipe,
//!                                 "drag x,y path…" or "drop x,y path…" to hold files over
//!                                 a place or drop them there, "copy-image file.png", or "share" to open the
//!                                 share dialog of the chat in front; "press x,y", "hold x,y" and
//!                                 "release x,y" drag the pointer with its button down, and
//!                                 "clipboard" prints the text copied; "state" prints one line of what
//!                                 the window shows (Settings, any modal, the front view, the focus, the tabs,
//!                                 the chosen host, a trust question, the front chat's tool details held and
//!                                 open); "open-tool" opens the front chat's first tool call; "host this",
//!                                 "host host:<key>" or "host ssh:<alias>" chooses the host in front, as a
//!                                 click on its row does

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

/// How often a wait lets the app run, moving its clock on as far, so its timers fire as they would.
const STEP: Duration = Duration::from_millis(50);

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
    let window_size = option("--size")
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse().ok()?, height.parse().ok()?))
        })
        .unwrap_or(WINDOW_SIZE);
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
                    size: size(px(window_size.0), px(window_size.1)),
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
    // Offscreen, the window never comes to the front, which is when the app takes the core's
    // calls, so it takes them from the start.
    if let Some(bench) = bench.borrow().clone() {
        cx.update(|cx| {
            let _ = bench.read(cx).asks.send(core::Ask::RegisterWindow);
        });
    }
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
                "move" => {
                    let (x, y) = rest
                        .split_once(',')
                        .ok_or_else(|| anyhow::anyhow!("move x,y"))?;
                    let position = point(px(x.trim().parse()?), px(y.trim().parse()?));
                    window.dispatch_event(
                        PlatformInput::MouseMove(MouseMoveEvent {
                            position,
                            pressed_button: None,
                            modifiers: Modifiers::default(),
                        }),
                        cx,
                    );
                    window.draw(cx).clear(cx);
                }
                "click" | "dclick" | "rclick" => {
                    let (x, y) = rest
                        .split_once(',')
                        .ok_or_else(|| anyhow::anyhow!("{verb} x,y"))?;
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
                    let button = if verb == "rclick" {
                        MouseButton::Right
                    } else {
                        MouseButton::Left
                    };
                    let click_count = if verb == "dclick" { 2 } else { 1 };
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
                "press" | "hold" | "release" => {
                    let (x, y) = rest
                        .split_once(',')
                        .ok_or_else(|| anyhow::anyhow!("{verb} x,y"))?;
                    let position = point(px(x.trim().parse()?), px(y.trim().parse()?));
                    let (button, modifiers) = (MouseButton::Left, Modifiers::default());
                    let event = match verb {
                        "press" => PlatformInput::MouseDown(MouseDownEvent {
                            button,
                            position,
                            modifiers,
                            click_count: 1,
                            first_mouse: false,
                        }),
                        "hold" => PlatformInput::MouseMove(MouseMoveEvent {
                            position,
                            pressed_button: Some(button),
                            modifiers,
                        }),
                        _ => PlatformInput::MouseUp(MouseUpEvent {
                            button,
                            position,
                            modifiers,
                            click_count: 1,
                        }),
                    };
                    window.dispatch_event(event, cx);
                    window.draw(cx).clear(cx);
                }
                "clipboard" => {
                    let copied = cx.read_from_clipboard().and_then(|item| item.text());
                    println!("clipboard: {copied:?}");
                }
                "state" => {
                    if let Some(bench) = bench.borrow().clone() {
                        let (held, open) = bench.read(cx).tool_details(cx).unwrap_or_default();
                        let (strip, composer) = cx
                            .try_global::<sikemux_gpui::composer::Drawn>()
                            .map_or((0., 0.), |drawn| (drawn.strip, drawn.composer));
                        println!(
                            "{} held={held} open={open} strip={strip:.0} composer={composer:.0}",
                            state(bench.read(cx), &window.context_stack())
                        );
                    }
                }
                "open-tool" => {
                    if let Some(bench) = bench.borrow().clone() {
                        bench.update(cx, |bench, cx| bench.open_first_tool(cx));
                    }
                }
                "host" => {
                    let choice = match rest.split_once(':') {
                        Some(("host", key)) => sikemux_gpui::hosts::HostChoice::Account(key.into()),
                        Some(("ssh", alias)) => sikemux_gpui::hosts::HostChoice::Ssh(alias.into()),
                        _ => sikemux_gpui::hosts::HostChoice::This,
                    };
                    if let Some(bench) = bench.borrow().clone() {
                        bench.update(cx, |bench, cx| bench.choose_host(choice, cx));
                    }
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
                "swipe" => {
                    let mut words = rest.split(' ');
                    let (x, y) = words
                        .next()
                        .and_then(|at| at.split_once(','))
                        .ok_or_else(|| anyhow::anyhow!("swipe x,y pixels"))?;
                    let pixels: f32 = words
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("swipe x,y pixels"))?
                        .parse()?;
                    let hold = words.next() == Some("hold");
                    let position = point(px(x.trim().parse()?), px(y.trim().parse()?));
                    let wheel = |dx: f32, touch_phase| ScrollWheelEvent {
                        position,
                        delta: ScrollDelta::Pixels(point(px(dx), px(0.))),
                        modifiers: Modifiers::default(),
                        touch_phase,
                    };
                    window.dispatch_event(
                        PlatformInput::ScrollWheel(wheel(0., TouchPhase::Started)),
                        cx,
                    );
                    for _ in 0..10 {
                        window.dispatch_event(
                            PlatformInput::ScrollWheel(wheel(pixels / 10., TouchPhase::Moved)),
                            cx,
                        );
                        std::thread::sleep(Duration::from_millis(8));
                    }
                    if !hold {
                        window.dispatch_event(
                            PlatformInput::ScrollWheel(wheel(0., TouchPhase::Ended)),
                            cx,
                        );
                    }
                    window.draw(cx).clear(cx);
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
                "share" => {
                    if let Some(bench) = bench.borrow().clone() {
                        bench.update(cx, |bench, cx| {
                            if let Some(agent) = bench.agent() {
                                bench.open_share(&agent.id, window, cx);
                            }
                        });
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
            std::thread::sleep(STEP);
            cx.advance_clock(STEP);
        }
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))?;
        if verb == "shot" {
            cx.capture_screenshot(window.into())?.save(rest)?;
        }
    }
    Ok(())
}

/// One line of what the window shows, for a check to match without looking: its keys stay in
/// this order, and new ones go at the end.
fn state(bench: &workbench::Workbench, stack: &[gpui::KeyContext]) -> String {
    let names = |context: &gpui::KeyContext| -> Vec<String> {
        context
            .primary()
            .into_iter()
            .chain(context.secondary())
            .filter(|entry| entry.value.is_none())
            .map(|entry| entry.key.to_string())
            .collect()
    };
    let modal = stack
        .iter()
        .rev()
        .find(|context| context.contains("Modal"))
        .and_then(|context| names(context).into_iter().find(|name| name != "Modal"));
    let focus = stack
        .last()
        .and_then(|context| names(context).into_iter().next());
    let tab = bench.focused_kind().map(|kind| format!("{kind:?}"));
    let tabs = bench.strip().map_or(0, |strip| bench.tabs(&strip).len());
    let settings = if bench.settings.is_some() {
        "open"
    } else {
        "closed"
    };
    let shown = |name: Option<String>| name.map_or_else(|| "none".to_owned(), |name| kebab(&name));
    let host = bench.chosen_host().key();
    let asking = ["trust", "host-trust"].iter().any(|key| {
        bench
            .read(key)
            .and_then(|asks| asks.as_array())
            .is_some_and(|asks| !asks.is_empty())
    });
    let trust = if asking { "asking" } else { "none" };
    format!(
        "state settings={settings} modal={} tab={} focus={} tabs={tabs} host={host} trust={trust}",
        shown(modal),
        shown(tab),
        shown(focus),
    )
}

/// `ProjectSwitcher` as `project-switcher`.
fn kebab(name: &str) -> String {
    let mut kebab = String::new();
    for (at, letter) in name.chars().enumerate() {
        if letter.is_uppercase() && at > 0 {
            kebab.push('-');
        }
        kebab.extend(letter.to_lowercase());
    }
    kebab
}
