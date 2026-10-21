//! The editor's tab: a file opened from anywhere at a line.

use std::path::PathBuf;

/// Opens a file in the editor with the cursor at a line, counted from 0, and
/// a column, or the line's first character that is not blank when there is
/// none. Every diff's "open in editor" sends this.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(name = "editor.openAt", no_json)]
pub struct OpenAt {
    pub path: PathBuf,
    pub line: u32,
    pub column: Option<u32>,
}
