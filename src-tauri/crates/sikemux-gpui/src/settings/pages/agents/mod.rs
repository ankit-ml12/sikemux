//! The agents Sikemux runs: which are installed, the accounts and profiles they
//! start on, the models they may use, their skills, and how sessions behave.

use gpui_kit::{AnyElement, Context, Entity, Window};
use serde_json::Value;

use crate::settings::Page;
use crate::workbench::Workbench;

mod accounts;
mod device;
mod profiles;
mod providers;
mod sessions;
mod setup;
mod skills;

/// What the page keeps between frames, made the first time it shows.
pub struct Fields {
    sessions: sessions::Fields,
    device: device::Fields,
    profiles: profiles::Fields,
    providers: providers::Fields,
    skills: skills::Fields,
}

fn make(bench: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>) {
    crate::accounts::read(bench, cx);
    let fields = Fields {
        sessions: sessions::Fields::new(bench, window, cx),
        device: device::Fields::new(bench, window, cx),
        profiles: profiles::Fields::new(window, cx),
        providers: providers::Fields::new(window, cx),
        skills: skills::Fields::new(window, cx),
    };
    if let Some(settings) = bench.settings.as_mut() {
        settings.pages.agents = Some(fields);
    }
    crate::agent_setup::read(bench, cx);
    sessions::read_names(bench, window, cx);
    device::read_simulator(bench, cx);
    providers::read(bench, cx);
    skills::read(bench, cx);
    bench.parts.redraw(&bench.parts.stage, cx);
}

fn fields_of(bench: &mut Workbench) -> Option<&mut Fields> {
    bench.settings.as_mut()?.pages.agents.as_mut()
}

/// The catalog's entry for the agent `kind`.
fn kind<'a>(bench: &'a Workbench, kind: &str) -> Option<&'a Value> {
    bench
        .read("catalog")
        .and_then(Value::as_array)
        .and_then(|kinds| kinds.iter().find(|entry| entry["type"] == kind))
}

/// What the agent is called, as the catalog names it.
fn label(bench: &Workbench, agent: &str) -> String {
    kind(bench, agent)
        .and_then(|kind| kind["label"].as_str())
        .unwrap_or(agent)
        .to_owned()
}

pub fn render(
    bench: &Workbench,
    pages: &super::State,
    page: Page,
    lit: &dyn Fn(&str) -> bool,
    this: &Entity<Workbench>,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> Vec<AnyElement> {
    let Some(fields) = &pages.agents else {
        cx.defer_in(window, make);
        return Vec::new();
    };
    match page {
        Page::Notifications => vec![sessions::notifications(bench, lit, this, cx)],
        Page::Installed => vec![setup::render(bench, &bench.agent_setup, lit, this, cx)],
        Page::Permissions => vec![sessions::boundary(bench, lit, this, cx)],
        Page::Chats => vec![sessions::render(&fields.sessions, lit, this, cx)],
        Page::Skills => vec![skills::render(&fields.skills, lit, this, cx)],
        Page::Accounts => {
            let mut sections: Vec<AnyElement> = signs_in(bench)
                .into_iter()
                .map(|provider| accounts::render(bench, &provider, lit, this, cx))
                .collect();
            sections.push(profiles::render(bench, &fields.profiles, lit, this, cx));
            sections
        }
        Page::Plans | Page::Providers | Page::LocalModels | Page::Helpers => {
            providers::render(bench, &fields.providers, page, lit, this, window, cx)
        }
        Page::Notch | Page::Voice | Page::Simulator => {
            device::render(bench, &fields.device, page, lit, this, cx)
        }
        _ => Vec::new(),
    }
}

/// The agents the catalog says keep each sign-in apart, so the person can hold several accounts.
fn signs_in(bench: &Workbench) -> Vec<String> {
    bench
        .read("catalog")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|kind| kind["accounts"] == true)
        .filter_map(|kind| kind["type"].as_str().map(str::to_owned))
        .collect()
}

/// The add-account dialog and the install or sign-in sheet, over whichever page is open.
pub fn dialogs(
    bench: &Workbench,
    this: &Entity<Workbench>,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> Vec<AnyElement> {
    let mut open = Vec::new();
    open.extend(accounts::dialog(bench, this, cx));
    open.extend(crate::agent_setup::sheet(bench, this, window, cx));
    open
}
