//! The app's shortcuts. Each of today's command ids is bound to the GPUI action of that name,
//! with the keys the person chose in the core's preferences, else the defaults both apps read.
//! An id no part of the app has an action for yet stays unbound.

use std::collections::BTreeMap;

use gpui_kit::component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};
use gpui_kit::{
    actions, App, Global, KeyBinding, Menu, MenuItem, OsAction, SharedString, SystemMenuType,
};
use serde::Deserialize;

static TABLE: &str = include_str!("../../sikemux-core/src/assets/keybindings.json");

actions!(
    app,
    [
        Hide,
        HideOthers,
        ShowAll,
        Quit,
        Minimize,
        Zoom,
        ToggleFullScreen
    ]
);

/// Defines unit actions named by today's command ids.
macro_rules! commands {
    ($($(#[$doc:meta])* $name:ident = $id:literal,)*) => {$(
        $(#[$doc])*
        #[derive(Clone, Default, PartialEq, gpui::Action)]
        #[action(name = $id)]
        pub struct $name;
    )*};
}

commands! {
    PreviousTab = "window.previous",
    NextTab = "window.next",
    /// The previous tab showing the same kind of pane as the one in front.
    PreviousOfKind = "tab.previous",
    NextOfKind = "tab.next",
    GoToTab1 = "tab.goto1",
    GoToTab2 = "tab.goto2",
    GoToTab3 = "tab.goto3",
    GoToTab4 = "tab.goto4",
    GoToTab5 = "tab.goto5",
    GoToTab6 = "tab.goto6",
    GoToTab7 = "tab.goto7",
    GoToTab8 = "tab.goto8",
    /// The last tab, wherever it is.
    GoToLastTab = "tab.goto9",
    CloseTab = "pane.close",
    /// Hides both rails.
    FocusMode = "view.focusMode",
    /// Starts a chat with the agent the rail is set to.
    NewChat = "agent.new",
}

/// Stops every agent and terminal the core runs, then quits.
#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "app.quitAndStopEverything")]
pub struct QuitAndStopEverything;

/// One command, as the shared table names and binds it by default.
#[derive(Deserialize)]
pub struct Shortcut {
    pub id: String,
    mac: Option<String>,
    other: Option<String>,
}

/// The table, and the keys each command is bound to now in the table's own spelling,
/// such as `Meta+Shift+KeyP`.
pub struct Keymap {
    pub shortcuts: Vec<Shortcut>,
    bound: BTreeMap<String, String>,
}

impl Global for Keymap {}

pub fn install(cx: &mut App) {
    let shortcuts = serde_json::from_str(TABLE).expect("the shortcut table reads");
    cx.set_global(Keymap {
        shortcuts,
        bound: BTreeMap::new(),
    });
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("alt-cmd-q", QuitAndStopEverything, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
    ]);
    bind(&BTreeMap::new(), cx);
    cx.set_menus(menus());
}

/// Binds every command to the person's keys, else its default. It runs again whenever the
/// preferences change; keys a command no longer uses stop doing anything.
pub fn bind(overrides: &BTreeMap<String, Option<String>>, cx: &mut App) {
    let keymap = cx.global::<Keymap>();
    let mut bound = BTreeMap::new();
    let mut bindings = Vec::new();
    for shortcut in &keymap.shortcuts {
        let keys = match overrides.get(&shortcut.id) {
            Some(chosen) => chosen.clone(),
            None if cfg!(target_os = "macos") => shortcut.mac.clone(),
            None => shortcut.other.clone(),
        };
        let Some((keys, gpui_keys)) = keys.and_then(|keys| Some((keys.clone(), keystroke(&keys)?)))
        else {
            continue;
        };
        let Ok(action) = cx.build_action(&shortcut.id, None) else {
            continue;
        };
        if let Ok(binding) = KeyBinding::load(
            &gpui_keys,
            action,
            None,
            false,
            None,
            &gpui::DummyKeyboardMapper,
        ) {
            bindings.push(binding);
            bound.insert(shortcut.id.clone(), keys);
        }
    }
    let released: Vec<KeyBinding> = keymap
        .bound
        .values()
        .filter(|keys| !bound.values().any(|now| now == *keys))
        .filter_map(|keys| keystroke(keys))
        .map(|keys| KeyBinding::new(&keys, gpui::NoAction, None))
        .collect();
    cx.bind_keys(released);
    cx.bind_keys(bindings);
    cx.global_mut::<Keymap>().bound = bound;
}

/// The keys `id` is bound to as the app shows them, such as ⌘⇧P, or none when it is unbound.
pub fn hint(id: &str, cx: &App) -> Option<SharedString> {
    let keys = cx.try_global::<Keymap>()?.bound.get(id)?;
    let mut parts: Vec<&str> = keys.split('+').collect();
    let code = parts.pop()?;
    let mac = cfg!(target_os = "macos");
    let modifiers = parts.into_iter().map(|part| match (part, mac) {
        ("Meta", true) => "⌘",
        ("Ctrl", true) => "⌃",
        ("Alt", true) => "⌥",
        ("Shift", true) => "⇧",
        (part, _) => part,
    });
    let label = match code {
        "Backquote" => "`".to_owned(),
        "Backslash" => "\\".to_owned(),
        "BracketLeft" => "[".to_owned(),
        "BracketRight" => "]".to_owned(),
        "Comma" => ",".to_owned(),
        "Enter" => "↵".to_owned(),
        "Equal" => "=".to_owned(),
        "Escape" => "Esc".to_owned(),
        "Minus" => "-".to_owned(),
        "Period" => ".".to_owned(),
        "Quote" => "'".to_owned(),
        "Semicolon" => ";".to_owned(),
        "Slash" => "/".to_owned(),
        code => code
            .strip_prefix("Key")
            .or_else(|| code.strip_prefix("Digit"))
            .or_else(|| code.strip_prefix("Arrow"))
            .unwrap_or(code)
            .to_owned(),
    };
    let text: Vec<String> = modifiers.map(str::to_owned).chain([label]).collect();
    Some(text.join(if mac { "" } else { "+" }).into())
}

/// A binding in the table's spelling, `Meta+Shift+KeyP`, as GPUI writes it, `cmd-shift-p`.
fn keystroke(keys: &str) -> Option<String> {
    let parts: Option<Vec<String>> = keys
        .split('+')
        .map(|part| {
            Some(match part {
                "Meta" => "cmd".to_owned(),
                "Ctrl" => "ctrl".to_owned(),
                "Alt" => "alt".to_owned(),
                "Shift" => "shift".to_owned(),
                code => key(code)?,
            })
        })
        .collect();
    Some(parts?.join("-"))
}

fn key(code: &str) -> Option<String> {
    if let Some(letter) = code.strip_prefix("Key") {
        return Some(letter.to_lowercase());
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return Some(digit.to_owned());
    }
    if let Some(number) = code.strip_prefix('F').filter(|n| n.parse::<u8>().is_ok()) {
        return Some(format!("f{number}"));
    }
    Some(
        match code {
            "Backquote" => "`",
            "Backslash" => "\\",
            "BracketLeft" => "[",
            "BracketRight" => "]",
            "Comma" => ",",
            "Equal" => "=",
            "Minus" => "-",
            "Period" => ".",
            "Quote" => "'",
            "Semicolon" => ";",
            "Slash" => "/",
            "Enter" | "NumpadEnter" => "enter",
            "Escape" => "escape",
            "Tab" => "tab",
            "Space" => "space",
            "Backspace" => "backspace",
            "Delete" => "delete",
            "ArrowLeft" => "left",
            "ArrowRight" => "right",
            "ArrowUp" => "up",
            "ArrowDown" => "down",
            "Home" => "home",
            "End" => "end",
            "PageUp" => "pageup",
            "PageDown" => "pagedown",
            _ => return None,
        }
        .to_owned(),
    )
}

/// The menu bar, as today's app has it.
fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Sikemux".into(),
            items: vec![
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Sikemux", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit Sikemux", Quit),
                MenuItem::action("Quit and Stop Everything", QuitAndStopEverything),
            ],
            disabled: false,
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            ],
            disabled: false,
        },
        Menu {
            name: "View".into(),
            items: vec![MenuItem::action("Enter Full Screen", ToggleFullScreen)],
            disabled: false,
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
            ],
            disabled: false,
        },
    ]
}
