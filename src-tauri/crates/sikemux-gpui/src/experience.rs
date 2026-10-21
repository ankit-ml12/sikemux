//! The screens over the whole window: the welcome on a first run, and the release
//! notes once a new release runs. The core says when each is due.

use gpui_kit::{div, AnyElement, Context, IntoElement, Window};
use serde_json::Value;
use sikemux_wire::protocol::Preferences;

use crate::workbench::Workbench;
use crate::{welcome, whats_new};

pub enum Experience {
    Welcome(welcome::Welcome),
    WhatsNew(whats_new::WhatsNew),
}

/// Which screens this run has already shown on its own, so neither comes back once closed.
#[derive(Default)]
pub struct Shown {
    welcome: bool,
    news: bool,
}

/// A first run opens the welcome.
pub fn preferences_read(
    bench: &mut Workbench,
    preferences: &Preferences,
    cx: &mut Context<Workbench>,
) {
    if preferences.onboarding_complete || bench.shown.welcome || bench.experience.is_some() {
        return;
    }
    bench.shown.welcome = true;
    open_welcome(bench, cx);
}

/// A release the person has not read the notes of opens them, once the welcome is done.
pub fn whats_new_read(bench: &mut Workbench, value: &Value, cx: &mut Context<Workbench>) {
    if value["unseen"] != true || bench.shown.news || bench.experience.is_some() {
        return;
    }
    bench.shown.news = true;
    open_whats_new(bench, cx);
}

pub fn open_welcome(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    close_shown(bench, cx);
    bench.experience = Some(Experience::Welcome(welcome::Welcome::new(cx)));
    welcome::opened(bench, cx);
    crate::ui::grain::sky_shown(true, cx);
    redraw(bench, cx);
}

pub fn open_whats_new(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    close_shown(bench, cx);
    bench.experience = Some(Experience::WhatsNew(whats_new::WhatsNew::default()));
    whats_new::opened(bench, cx);
    crate::ui::grain::sky_shown(true, cx);
    redraw(bench, cx);
}

/// Closes the screen up, counting the welcome done or the release's notes read.
pub fn close(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    match bench.experience.take() {
        Some(Experience::Welcome(_)) => {
            crate::settings::set(bench, "onboardingComplete", true);
        }
        Some(Experience::WhatsNew(news)) => {
            if let Some(version) = news.running() {
                crate::settings::set(bench, "lastSeenVersion", version);
            }
        }
        None => return,
    }
    crate::ui::grain::sky_shown(false, cx);
    redraw(bench, cx);
}

/// Swaps one screen for the other without counting the first as finished.
fn close_shown(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    if bench.experience.take().is_some() {
        crate::ui::grain::sky_shown(false, cx);
    }
}

fn redraw(bench: &Workbench, cx: &mut Context<Workbench>) {
    bench.parts.redraw(&bench.parts.over, cx);
}

/// What the part over the shell draws.
pub fn draw(bench: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>) -> AnyElement {
    let this = cx.entity();
    match &bench.experience {
        Some(Experience::Welcome(_)) => welcome::render(bench, &this, window, cx),
        Some(Experience::WhatsNew(_)) => whats_new::render(bench, &this, window, cx),
        None => div().into_any_element(),
    }
}
