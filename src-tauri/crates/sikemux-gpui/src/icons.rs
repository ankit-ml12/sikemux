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

            /// Whether it has colours of its own, rather than drawing in the text's.
            pub fn is_picture(self) -> bool {
                match self { $(Icon::$icon => const { !tints(include_bytes!(concat!("../assets/icons/", $file, ".svg"))) }),* }
            }
        }

        const FILES: &[(&str, &[u8])] = &[
            $((concat!("icons/", $file, ".svg"), include_bytes!(concat!("../assets/icons/", $file, ".svg")))),*
        ];
    };
}

icons!(
    Actions => "actions",
    Agent => "agent",
    Agents => "agents",
    ArrowUp => "arrow-up",
    Aws => "aws",
    BatteryBolt => "battery-bolt",
    BatteryBoltGap => "battery-bolt-gap",
    Board => "board",
    Bruno => "bruno",
    Check => "check",
    Chevron => "chevron",
    Claude => "claude",
    Close => "close",
    Codex => "codex",
    Command => "command",
    Database => "database",
    Editor => "editor",
    Eye => "eye",
    File => "file",
    Focus => "focus",
    Folder => "folder",
    Git => "git",
    Github => "github",
    Globe => "globe",
    Inbox => "inbox",
    Issues => "issues",
    Jira => "jira",
    Laptop => "laptop",
    Mic => "mic",
    Linear => "linear",
    LogIn => "log-in",
    LogOut => "log-out",
    MarkCancelled => "mark-cancelled",
    MarkFailure => "mark-failure",
    MarkSkipped => "mark-skipped",
    MarkSuccess => "mark-success",
    Logo => "logo",
    More => "more",
    PanelLeft => "panel-left",
    PanelRight => "panel-right",
    Pencil => "pencil",
    Plug => "plug",
    Plus => "plus",
    PullRequest => "pull-request",
    Refresh => "refresh",
    Releases => "releases",
    Run => "run",
    Rundeck => "rundeck",
    Search => "search",
    Server => "server",
    Settings => "settings",
    Share => "share",
    Shield => "shield",
    ShieldBolt => "shield-bolt",
    Signoz => "signoz",
    Slack => "slack",
    Stack => "stack",
    Stop => "stop",
    Terminal => "terminal",
    Vercel => "vercel",
    Window => "window",
);

/// Whether an SVG draws in `currentColor`.
const fn tints(svg: &[u8]) -> bool {
    let needle = b"currentColor";
    let mut start = 0;
    while start + needle.len() <= svg.len() {
        let mut matched = 0;
        while matched < needle.len() && svg[start + matched] == needle[matched] {
            matched += 1;
        }
        if matched == needle.len() {
            return true;
        }
        start += 1;
    }
    false
}

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

/// An agent's mark.
pub fn agent(provider: &str) -> Icon {
    match provider {
        "codex" => Icon::Codex,
        "claude" => Icon::Claude,
        _ => Icon::Agent,
    }
}
