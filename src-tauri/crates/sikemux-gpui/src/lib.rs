//! The desktop UI on GPUI, a client of the core.

pub mod account_picker;
mod accounts;
mod add_task;
mod agent_menu;
mod agent_palette;
mod agent_setup;
mod agents;
mod battery;
mod boot;
mod chat;
pub mod codehost;
mod commands;
pub mod composer;
pub mod core;
mod desk;
mod drag;
pub mod editor;
mod experience;
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
mod new_tab;
pub mod notifications;
mod palette;
mod panes;
mod picker;
mod picture;
mod ports;
pub mod project_switcher;
mod rail;
pub mod search;
pub mod settings;
mod share;
mod sim_pane;
#[cfg(target_os = "macos")]
mod sim_screen;
#[cfg(not(target_os = "macos"))]
#[path = "sim_screen_elsewhere.rs"]
mod sim_screen;
mod swipe;
mod switcher;
pub mod syntax;
mod tabs;
pub mod terminal;
pub mod theme;
mod theme_palette;
mod track;
mod ui;
pub mod voice;
mod welcome;
mod whats_new;
pub mod workbench;
pub mod worktree_switch;

/// The window's size when it first opens.
pub const WINDOW_SIZE: (f32, f32) = (1340., 880.);
