use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

macro_rules! icons {
    ($($icon:ident => $file:literal),* $(,)?) => {
        /// Every icon the app draws, from icons.mjs's SVGs.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon { $($icon),* }

        impl Icon {
            pub fn path(self) -> &'static str {
                match self { $(Icon::$icon => concat!("icons/", $file, ".svg")),* }
            }
        }

        const FILES: &[(&str, &[u8])] = &[
            $((concat!("icons/", $file, ".svg"), include_bytes!(concat!("../assets/icons/", $file, ".svg")))),*
        ];
    };
}

icons!(
    Agent => "agent",
    Agents => "agents",
    Board => "board",
    Check => "check",
    Chevron => "chevron",
    Claude => "claude",
    Close => "close",
    Codex => "codex",
    Command => "command",
    Editor => "editor",
    Eye => "eye",
    File => "file",
    Focus => "focus",
    Folder => "folder",
    Git => "git",
    Globe => "globe",
    Inbox => "inbox",
    LogIn => "log-in",
    Logo => "logo",
    More => "more",
    PanelLeft => "panel-left",
    PanelRight => "panel-right",
    Pencil => "pencil",
    Plus => "plus",
    Search => "search",
    Server => "server",
    Settings => "settings",
    Share => "share",
    Stack => "stack",
    Terminal => "terminal",
    Window => "window",
);

/// The icons, compiled in.
pub struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

/// An agent's mark, and whether it is a picture with colours of its own rather
/// than a glyph tinted with the agent's brand colour.
pub fn agent(provider: &str) -> (Icon, bool) {
    match provider {
        "codex" => (Icon::Codex, true),
        "claude" => (Icon::Claude, false),
        _ => (Icon::Agent, false),
    }
}
