use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $((concat!("icons/", $name, ".svg"), include_bytes!(concat!("../assets/icons/", $name, ".svg")))),*
        ];
    };
}

icons!(
    "agent", "agents", "board", "chevron", "claude", "close", "codex", "command", "editor",
    "file", "focus", "folder", "git", "globe", "inbox", "log-in", "logo", "panel-left",
    "panel-right", "pencil", "plus", "search", "server", "settings", "stack", "terminal",
    "window",
);

/// The icons drawn from icons.mjs's output, compiled in.
pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

pub fn agent(provider: &str) -> &'static str {
    match provider {
        "codex" => "icons/codex.svg",
        "claude" => "icons/claude.svg",
        _ => "icons/agent.svg",
    }
}

/// Marks drawn from a file with their own colours, rather than tinted.
pub fn agent_is_picture(provider: &str) -> bool {
    provider == "codex"
}
