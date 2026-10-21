//! The desktop UI on GPUI, a client of the core.

mod agents;
mod battery;
mod chat;
pub mod composer;
pub mod core;
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
mod picture;
mod rail;
pub mod syntax;
mod tabs;
pub mod terminal;
pub mod theme;
mod ui;
pub mod workbench;

/// The window's size when it first opens.
pub const WINDOW_SIZE: (f32, f32) = (1340., 880.);
