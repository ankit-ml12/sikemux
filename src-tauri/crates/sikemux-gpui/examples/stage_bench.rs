//! Times the whole window while two terminals sit beside a chat: idle with a terminal's
//! cursor blinking, one blink on its own, the stage drawn again with nothing in it changed,
//! and a terminal flooding. The core must have a
//! chat in its tab in front, as `fake_chat`'s `remote` leaves it:
//!   SIKEMUX_CORE_SOCKET=/tmp/gb/core.sock HOME=/tmp/gb/home cargo run --release --example stage_bench
//! `STAGE_BENCH_LOOP=N` only draws the stage again N times, for a profiler to watch, and
//! `STAGE_BENCH_FLOOD=S` only floods a terminal with an endless `yes` for S seconds.
//! It runs offscreen on a simulated clock at 120 Hz and prints, for each phase, the main
//! thread's CPU for each frame: the window's work and, when something changed, its draw.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{point, px, size, AnyWindowHandle, AppContext, Entity, HeadlessAppContext, Keystroke};
use serde_json::json;
use sikemux_gpui::hosts::Place;
use sikemux_gpui::workbench::Workbench;
use sikemux_gpui::{core, icons, terminal, theme, WINDOW_SIZE};
use sikemux_wire::protocol::{Pane, PaneKind, SessionId};

const FRAME: Duration = Duration::from_micros(8_333);
const FLOOD_LINE: &str = "yes 'the quick brown fox jumps over the lazy dog'";
const FLOOD: &str = "yes 'the quick brown fox jumps over the lazy dog' | head -n 400000";

struct Bench {
    cx: HeadlessAppContext,
    window: AnyWindowHandle,
    bench: Entity<Workbench>,
    frames: Vec<f64>,
}

impl Bench {
    fn frame(&mut self) {
        self.after(|_| {});
    }

    /// One frame that first does `change`, timed with it. Headless mode draws a window when
    /// its changes are flushed, and only when something in it changed, as the app does.
    fn after(&mut self, change: impl FnOnce(&mut Self)) {
        let cpu = thread_cpu_ms();
        change(self);
        self.cx.advance_clock(FRAME);
        self.cx.run_until_parked();
        self.frames.push(thread_cpu_ms() - cpu);
    }

    /// Frames until `done` holds, at most `limit` of them, with a pause for the core between.
    fn until(&mut self, limit: usize, done: impl Fn(&Workbench) -> bool) -> bool {
        for _ in 0..limit {
            self.frame();
            if self.bench.read_with(&self.cx, |bench, _| done(bench)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn report(&mut self, phase: &str) {
        let mut frames = std::mem::take(&mut self.frames);
        frames.sort_by(f64::total_cmp);
        let at = |q: f64| {
            frames
                .get(((frames.len() as f64 - 1.) * q).round() as usize)
                .copied()
                .unwrap_or(0.)
        };
        let total: f64 = frames.iter().sum();
        let simulated = frames.len() as f64 * FRAME.as_secs_f64();
        println!(
            "{phase}: {} frames · frame CPU p50/p90/p99/max {:.2}/{:.2}/{:.2}/{:.2} ms · {:.1}% of one core at 120 Hz",
            frames.len(),
            at(0.5),
            at(0.9),
            at(0.99),
            at(1.),
            total / 1e3 / simulated.max(1e-9) * 100.,
        );
    }
}

/// The terminals a tab shows beside its other views.
fn terminals_in(pane: &Pane, found: &mut Vec<SessionId>) {
    match pane {
        Pane::View {
            kind: PaneKind::Terminal,
            target,
            ..
        } => found.extend(target.parse::<SessionId>().ok()),
        Pane::View { .. } => {}
        Pane::Split { panes, .. } => panes.iter().for_each(|pane| terminals_in(pane, found)),
    }
}

/// The chat's view in a tab, by id.
fn agent_view(pane: &Pane) -> Option<String> {
    match pane {
        Pane::View {
            id,
            kind: PaneKind::Agent,
            ..
        } => Some(id.clone()),
        Pane::View { .. } => None,
        Pane::Split { panes, .. } => panes.iter().find_map(agent_view),
    }
}

fn main() -> anyhow::Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_platform::bench_text_system(),
        Arc::new(icons::Icons),
        gpui_platform::current_headless_renderer,
    );
    let link = core::start(core::Home::default());
    let held = Rc::new(Cell::new(None));
    let kept = held.clone();
    let window = cx.update(|cx| {
        gpui_kit::init(cx);
        theme::install(cx);
        cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                    origin: point(px(0.), px(0.)),
                    size: size(px(WINDOW_SIZE.0), px(WINDOW_SIZE.1)),
                })),
                focus: false,
                show: false,
                ..Default::default()
            },
            |window, cx| {
                let bench = cx.new(|cx| Workbench::new(link, window, cx));
                kept.set(Some(bench.clone()));
                cx.new(|cx| gpui_kit::base::Root::new(bench, window, cx))
            },
        )
    })?;
    let bench = held.take().expect("the workbench");
    let mut run = Bench {
        cx,
        window: window.into(),
        bench,
        frames: Vec::new(),
    };
    run.bench.update(&mut run.cx, |bench, _| {
        bench.call(
            "setPreference",
            json!({ "key": "onboardingComplete", "value": true }),
        )
    });
    if !run.until(600, |bench| bench.agent().is_some()) {
        anyhow::bail!("the core's tab in front holds no chat");
    }
    let shown = |bench: &Workbench| {
        let mut found = Vec::new();
        if let Some(tab) = bench.tab() {
            terminals_in(&tab.pane, &mut found);
        }
        found
            .into_iter()
            .map(|session| bench.terminals.key(&Place::This, session))
            .collect::<Vec<_>>()
    };
    let (asks, project, tab, view) = run
        .bench
        .read_with(&run.cx, |bench, _| {
            let tab = bench.tab()?;
            let view = agent_view(&tab.pane)?;
            Some((
                bench.asks.clone(),
                bench.project()?.id.clone(),
                tab.id.clone(),
                view,
            ))
        })
        .ok_or_else(|| anyhow::anyhow!("the tab in front shows no chat"))?;
    let runtime = tokio::runtime::Runtime::new()?;
    let already = run.bench.read_with(&run.cx, |bench, _| shown(bench).len());
    for count in already + 1..=2 {
        let opened = runtime
            .block_on(core::request(
                &asks,
                "newTerminal",
                json!({ "project": project }),
            ))
            .map_err(anyhow::Error::msg)?;
        let params = json!({
            "project": project,
            "tab": tab,
            "pane": view,
            "side": "right",
            "fromTab": opened,
        });
        runtime
            .block_on(core::request(&asks, "splitPane", params))
            .map_err(anyhow::Error::msg)?;
        if !run.until(600, |bench| {
            let found = shown(bench);
            found.len() == count && found.iter().all(|id| bench.terminals.screen(*id).is_some())
        }) {
            anyhow::bail!("terminal {count} did not open beside the chat");
        }
    }
    let id = run.bench.read_with(&run.cx, |bench, _| shown(bench)[0]);
    run.cx
        .update_window(run.window, |_, window, cx| {
            let focus = run.bench.read(cx).terminals.focus(id).cloned();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
        .ok();
    for _ in 0..120 {
        run.frame();
        std::thread::sleep(Duration::from_millis(5));
    }
    run.frames.clear();

    run.bench.update(&mut run.cx, |bench, cx| {
        bench.experience = None;
        cx.notify();
    });
    for _ in 0..30 {
        run.frame();
    }
    run.frames.clear();
    if let Ok(path) = std::env::var("STAGE_BENCH_SHOT") {
        run.cx.capture_screenshot(run.window)?.save(path)?;
    }
    if let Some(seconds) = std::env::var("STAGE_BENCH_FLOOD")
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
    {
        let flood = match std::env::var("STAGE_BENCH_LINES") {
            Ok(lines) => format!("{FLOOD_LINE} | head -n {lines}"),
            Err(_) => FLOOD_LINE.to_owned(),
        };
        run.bench.update(&mut run.cx, |bench, _| {
            terminal::type_text(bench, id, flood);
        });
        run.cx
            .update_window(run.window, |_, window, cx| {
                if let Ok(enter) = Keystroke::parse("enter") {
                    window.dispatch_keystroke(enter, cx);
                }
            })
            .ok();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(seconds) {
            run.frame();
            if let Some(slow) = run.frames.last().filter(|frame| **frame > 50.) {
                println!(
                    "frame {} at {:.2} s took {slow:.1} ms",
                    run.frames.len(),
                    started.elapsed().as_secs_f64()
                );
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        run.report(&format!(
            "flooding the terminal for {seconds} s of wall time"
        ));
        return Ok(());
    }
    if let Some(times) = std::env::var("STAGE_BENCH_LOOP")
        .ok()
        .and_then(|times| times.parse::<usize>().ok())
    {
        for _ in 0..times {
            run.after(|run| {
                run.bench.update(&mut run.cx, |bench, cx| {
                    bench.parts.redraw(&bench.parts.stage, cx)
                })
            });
        }
        run.report(&format!(
            "the stage drawn again {times} times, for a profiler"
        ));
        return Ok(());
    }
    for _ in 0..600 {
        run.frame();
    }
    run.report("idle for 5 s, the terminal focused");

    for _ in 0..20 {
        run.after(|run| {
            run.bench
                .update(&mut run.cx, |bench, cx| bench.blink_now(id, cx))
        });
        for _ in 0..3 {
            run.frame();
        }
    }
    let blinks: Vec<f64> = run.frames.iter().step_by(4).copied().collect();
    run.frames = blinks;
    run.report("one cursor blink, the frame it is drawn in");

    for _ in 0..20 {
        run.after(|run| {
            run.bench.update(&mut run.cx, |bench, cx| {
                bench.parts.redraw(&bench.parts.stage, cx)
            })
        });
        for _ in 0..3 {
            run.frame();
        }
    }
    let stages: Vec<f64> = run.frames.iter().step_by(4).copied().collect();
    run.frames = stages;
    run.report("the stage drawn again, nothing in it changed");

    for _ in 0..20 {
        run.after(|run| {
            run.cx
                .update_window(run.window, |_, window, _| window.refresh())
                .ok();
        });
        for _ in 0..3 {
            run.frame();
        }
    }
    let whole: Vec<f64> = run.frames.iter().step_by(4).copied().collect();
    run.frames = whole;
    run.report("the whole window drawn again from scratch");

    let started = Instant::now();
    run.bench.update(&mut run.cx, |bench, _| {
        terminal::type_text(bench, id, FLOOD.into());
    });
    run.cx
        .update_window(run.window, |_, window, cx| {
            if let Ok(enter) = Keystroke::parse("enter") {
                window.dispatch_keystroke(enter, cx);
            }
        })
        .ok();
    for _ in 0..1200 {
        run.frame();
        std::thread::sleep(Duration::from_millis(1));
    }
    if let Ok(path) = std::env::var("STAGE_BENCH_SHOT") {
        run.cx
            .capture_screenshot(run.window)?
            .save(path.replace(".png", "-flood.png"))?;
    }
    run.report(&format!(
        "flooding the terminal for 10 s simulated ({:.1} s wall)",
        started.elapsed().as_secs_f64()
    ));
    Ok(())
}

fn thread_cpu_ms() -> f64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime only writes the struct it is handed.
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
    time.tv_sec as f64 * 1e3 + time.tv_nsec as f64 / 1e6
}
