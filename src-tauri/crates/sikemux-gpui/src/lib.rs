//! The desktop UI on GPUI, a client of the core.

mod agents;
mod battery;
mod chat;
pub mod composer;
mod faces;
mod file_icons;
mod files;
mod finder;
mod fuzzy;
pub mod core;
mod home;
pub mod icons;
mod rail;
mod tabs;
pub mod terminal;
pub mod theme;
mod ui;
pub mod workbench;

/// The window's size when it first opens.
pub const WINDOW_SIZE: (f32, f32) = (1340., 880.);
