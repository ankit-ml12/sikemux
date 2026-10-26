//! Each settings page, and what the pages keep between frames.

use gpui_kit::{AnyElement, Context, Entity, Window};
use serde_json::Value;

pub use appearance::install;

use super::{Page, Settings};
use crate::workbench::Workbench;

mod about;
mod actions;
mod activity;
mod agents;
mod appearance;
mod cli;
mod cloud;
mod devices;
mod general;
pub(crate) use general::sent as session_sent;
mod keybindings;
mod plugins;
pub mod theme_editor;

/// What the pages keep while settings are open: what the core last answered
/// for them, and the fields being typed in.
#[derive(Default)]
pub struct State {
    pub agents: Option<agents::Fields>,
    pub devices: Option<devices::Fields>,
    pub actions: Option<actions::Fields>,
    pub activity: Option<activity::Fields>,
    pub appearance: Option<appearance::Fields>,
    pub cloud: Option<cloud::Fields>,
    pub general: Option<general::Fields>,
    pub keybindings: Option<keybindings::Fields>,
    /// The command a search found, which the shortcuts page opens filtered to.
    pub shortcut: Option<&'static str>,
    /// The command line's install, as the core last said.
    pub cli: Option<Value>,
    pub cli_busy: bool,
}

/// The sections of a page in the order it shows them, as searches name them.
pub fn sections(page: Page) -> Vec<&'static str> {
    super::index::sections(page)
}

/// The sections of the page settings show.
pub fn render(
    bench: &Workbench,
    settings: &Settings,
    flash: Option<&str>,
    this: &Entity<Workbench>,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> Vec<AnyElement> {
    let lit = |section: &str| flash == Some(section);
    let pages = &settings.pages;
    let page = settings.page;
    let mut sections = match page {
        Page::Account | Page::Devices | Page::Hosts => {
            devices::render(bench, pages, page, &lit, this, window, cx)
        }
        Page::General => general::render(bench, pages, &lit, this, window, cx),
        Page::Keyboard => keybindings::render(bench, pages, &lit, this, window, cx),
        Page::Theme | Page::Interface | Page::Buddies | Page::Window => {
            appearance::render(bench, pages, page, &lit, this, window, cx)
        }
        Page::Notifications
        | Page::Installed
        | Page::Permissions
        | Page::Chats
        | Page::Skills
        | Page::Helpers
        | Page::Accounts
        | Page::Plans
        | Page::Providers
        | Page::LocalModels
        | Page::Notch
        | Page::Voice
        | Page::Simulator => agents::render(bench, pages, page, &lit, this, window, cx),
        Page::Integrations => plugins::render(bench, &lit, this, cx),
        Page::SignOn => cloud::render(bench, pages, &lit, this, window, cx),
        Page::Actions => actions::render(bench, pages, &lit, this, window, cx),
        Page::Cli => cli::render(bench, pages, &lit, this, window, cx),
        Page::Usage => activity::render(bench, pages, &lit, this, window, cx),
        Page::About => about::render(bench, &lit, this, cx),
    };
    sections.extend(agents::dialogs(bench, this, window, cx));
    sections
}

/// Lists the account's other hosts again while the Devices page shows them,
/// since a change to remote access may have linked or dropped one.
pub fn remote_moved(bench: &Workbench, cx: &mut Context<Workbench>) {
    let shown = bench
        .settings
        .as_ref()
        .is_some_and(|settings| settings.pages.devices.is_some());
    if shown {
        devices::read_links(bench, cx);
    }
}
