//! Each settings page, and what the pages keep between frames.

use gpui_kit::{AnyElement, Context, Entity, Window};
use serde_json::Value;

use super::{Page, Settings};
use crate::workbench::Workbench;

mod about;
mod appearance;
mod cli;
mod cloud;
mod general;
mod keybindings;
mod plugins;

/// What the pages keep while settings are open: what the core last answered
/// for them, and the fields being typed in.
#[derive(Default)]
pub struct State {
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
pub fn sections(page: Page) -> &'static [&'static str] {
    match page {
        Page::General => general::SECTIONS,
        Page::Appearance => appearance::SECTIONS,
        Page::Keybindings => keybindings::SECTIONS,
        Page::Activity => &["Overview", "Calendar", "By model", "By agent", "By project"],
        Page::About => about::SECTIONS,
        Page::Agents => &[
            "Agents on this machine",
            "Launch boundary",
            "Claude accounts",
            "Codex accounts",
            "Provider profiles",
            "Coding plans",
            "Model providers",
            "Local models",
            "Helpers",
            "Skills",
            "Sessions",
            "Voice",
            "Notch",
            "iOS Simulator",
        ],
        Page::Actions => &["Your actions", "New action"],
        Page::Cli => cli::SECTIONS,
        Page::Devices => &["Your account", "Remote access", "Phones"],
        Page::Cloud => cloud::SECTIONS,
        Page::Plugins => plugins::SECTIONS,
    }
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
    match settings.page {
        Page::About => about::render(bench, &lit, this, cx),
        Page::Appearance => appearance::render(bench, &settings.pages, &lit, this, window, cx),
        Page::Cli => cli::render(bench, &settings.pages, &lit, this, window, cx),
        Page::Cloud => cloud::render(bench, &settings.pages, &lit, this, window, cx),
        Page::Plugins => plugins::render(bench, &lit, this, cx),
        Page::General => general::render(bench, &settings.pages, &lit, this, window, cx),
        Page::Keybindings => keybindings::render(bench, &settings.pages, &lit, this, window, cx),
        _ => Vec::new(),
    }
}
