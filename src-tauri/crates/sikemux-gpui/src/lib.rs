//! The desktop UI on GPUI, a client of the core.

pub mod account_picker;
mod accounts;
mod agent_menu;
mod agents;
mod battery;
mod chat;
pub mod composer;
pub mod core;
mod desk;
mod drag;
pub mod editor;
mod faces;
pub mod file_icons;
mod files;
mod finder;
mod fuzzy;
pub mod git;
mod home;
pub mod icons;
mod keymap;
pub mod model_picker;
mod panes;
mod picture;
mod ports;
pub mod project_switcher;
mod rail;
pub mod settings;
mod share;
mod sim_pane;
#[cfg(target_os = "macos")]
mod sim_screen;
#[cfg(not(target_os = "macos"))]
#[path = "sim_screen_elsewhere.rs"]
mod sim_screen;
pub mod syntax;
mod tabs;
pub mod terminal;
pub mod theme;
mod ui;
pub mod workbench;
pub mod worktree_switch;

/// The window's size when it first opens.
pub const WINDOW_SIZE: (f32, f32) = (1340., 880.);
