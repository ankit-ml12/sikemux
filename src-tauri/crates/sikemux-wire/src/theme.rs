//! A colour theme: the app's colours, the code editor's, syntax highlighting
//! and the terminal's, each a CSS colour. The core resolves the person's
//! choice to one of these; clients draw with it.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub chrome: ThemeChrome,
    pub editor: ThemeEditor,
    pub highlight: ThemeHighlight,
    pub terminal: ThemeTerminal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeChrome {
    pub bg: String,
    pub bg_dim: String,
    pub bg_raised: String,
    pub ink: String,
    pub ink_dim: String,
    pub ink_muted: String,
    pub acc: String,
    pub acc_line: String,
    pub acc_dim: String,
    pub line: String,
    pub hl: String,
    pub danger: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeEditor {
    pub fg: String,
    pub bg: String,
    pub caret: String,
    pub selection: String,
    pub active_line: String,
    pub gutter: String,
    pub gutter_active: String,
    pub indent: String,
    pub indent_active: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeHighlight {
    pub keyword: String,
    pub string: String,
    pub comment: String,
    pub number: String,
    pub function: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub variable: String,
    pub property: String,
    pub tag: String,
    pub operator: String,
    pub link: String,
    pub invalid: String,
    pub meta: String,
    pub heading: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeTerminal {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
    pub cursor_accent: String,
    pub selection_background: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

/// A theme as a picker lists it: its name, where it comes from and its colours at a glance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub source: ThemeSource,
    pub bg: String,
    pub line: String,
    pub acc: String,
    /// The terminal's red, green, yellow, blue, magenta and cyan.
    pub swatches: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeSource {
    /// One the person made, which they can edit and delete.
    Custom,
    Sikemux,
    Ghostty,
}

/// The picked theme with the colours every app works out from it, nudged
/// until they read on the theme's grounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub theme: Theme,
    /// Text that recedes: labels, hints, timestamps.
    pub ink_muted: String,
    pub danger: String,
    /// Running, connected, passing.
    pub live: String,
    /// Waiting on the person, or at risk.
    pub warn: String,
    /// Commands and the names of things that run.
    pub cmd: String,
    pub git_modified: String,
    pub git_untracked: String,
    pub git_added: String,
    pub git_deleted: String,
    pub git_renamed: String,
}
