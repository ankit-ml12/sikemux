//! The editor's tab: a file read and saved through the core, edited on Kit's
//! code editor, and opened from anywhere at a line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use gpui_kit::component::input::{Editor, EditorState, InputEvent, Position};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    actions, div, px, AnyElement, AnyWindowHandle, App, AppContext, Context, Entity, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, SharedString, Styled, Subscription,
    Window,
};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use crate::core::{self, Ask};
use crate::keymap::{TextSizeDecrease, TextSizeIncrease, TextSizeReset};
use crate::theme::{self, Tokens};
use crate::ui;
use crate::workbench::Workbench;

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

actions!(editor, [Save]);

const CONTEXT: &str = "FileEditor";
/// How many times a read starts again when the file moves under it.
const READ_TRIES: usize = 3;
/// A save goes in pieces of this many characters, as the core takes them.
const SAVE_PIECE: usize = 512 * 1024;
const FONT_SIZE: f32 = 13.;
const DEFAULT_TEXT_SCALE: f64 = 1.;
const MIN_TEXT_SCALE: f64 = 0.7;
const MAX_TEXT_SCALE: f64 = 2.;
const TEXT_SCALE_STEP: f64 = 0.1;

pub fn install(cx: &mut App) {
    let save = if cfg!(target_os = "macos") {
        "cmd-s"
    } else {
        "ctrl-s"
    };
    cx.bind_keys([KeyBinding::new(save, Save, Some(CONTEXT))]);
}

/// Every file open in an editor, by its path.
#[derive(Default)]
pub struct Editors {
    open: HashMap<PathBuf, Document>,
    /// Where a file asked to open at a line puts its cursor once it is read.
    reveal: Option<OpenAt>,
    /// The window the editors draw in, which reading and reloading update.
    window: Option<AnyWindowHandle>,
}

struct Document {
    state: Entity<EditorState>,
    /// What the file held when it was last read or saved, and that text's version.
    saved: String,
    version: Option<String>,
    loaded: bool,
    dirty: bool,
    /// Why the file could not be read or saved, or that it changed on disk
    /// while it had unsaved edits.
    notice: Option<Notice>,
    _changes: Subscription,
}

#[derive(Clone)]
enum Notice {
    Failed(String),
    /// The disk moved on under unsaved edits; reloading drops them.
    Conflict(String),
}

impl Editors {
    /// Whether the file has edits not yet saved.
    pub fn is_dirty(&self, path: &Path) -> bool {
        self.open.get(path).is_some_and(|document| document.dirty)
    }

    /// Asks for the file to open with its cursor at a line once it is read.
    pub fn reveal(&mut self, at: OpenAt) {
        self.reveal = Some(at);
    }

    /// The files the core keeps watching for changes on disk.
    fn watched(&self) -> Vec<String> {
        self.open
            .keys()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }
}

/// The editor for `path`, opened the first time it is drawn.
pub fn pane(
    bench: &Workbench,
    this: &Entity<Workbench>,
    path: &str,
    window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let path = PathBuf::from(path);
    let t = Tokens::get(cx).clone();
    let Some(document) = bench.editors.open.get(&path) else {
        let opening = path.clone();
        cx.defer_in(window, move |bench, window, cx| {
            open(bench, opening, window, cx)
        });
        return div().size_full().into_any_element();
    };
    if document.loaded {
        if let Some(at) = bench.editors.reveal.clone().filter(|at| at.path == path) {
            cx.defer_in(window, move |bench, window, cx| {
                bench.editors.reveal = None;
                place_cursor(bench, &at, window, cx);
            });
        }
    }
    let size = px((FONT_SIZE as f64 * text_scale(bench)) as f32);
    let this = this.clone();
    let saving = path.clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .key_context(CONTEXT)
        .on_action(
            window.listener_for(&this, move |bench, _: &Save, window, cx| {
                save(bench, &saving, window, cx)
            }),
        )
        .on_action(
            window.listener_for(&this, |bench, _: &TextSizeIncrease, _, _| {
                rescale(bench, Some(TEXT_SCALE_STEP))
            }),
        )
        .on_action(
            window.listener_for(&this, |bench, _: &TextSizeDecrease, _, _| {
                rescale(bench, Some(-TEXT_SCALE_STEP))
            }),
        )
        .on_action(
            window.listener_for(&this, |bench, _: &TextSizeReset, _, _| rescale(bench, None)),
        )
        .when_some(document.notice.clone(), |pane, notice| {
            pane.child(notice_bar(&this, &path, notice, cx))
        })
        .child(
            div()
                .flex_1()
                .min_h_0()
                .font_family(theme::MONO)
                .text_size(size)
                .text_color(t.text_primary)
                .child(Editor::new(&document.state).bordered(false).h_full()),
        )
        .into_any_element()
}

/// The bar over an editor saying it could not read or save its file, or
/// that the file changed on disk under unsaved edits.
fn notice_bar(
    this: &Entity<Workbench>,
    path: &Path,
    notice: Notice,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let (text, reload): (SharedString, bool) = match notice {
        Notice::Failed(error) => (error.into(), false),
        Notice::Conflict(error) => (
            format!("{name} has an external change. Your editor buffer was preserved. {error}")
                .into(),
            true,
        ),
    };
    let (this, path) = (this.clone(), path.to_owned());
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(12.))
        .py(px(6.))
        .border_b_1()
        .border_color(t.border)
        .bg(t.surface_sunken)
        .text_size(px(12.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(t.warn)
                .font_weight(FontWeight::MEDIUM)
                .child(text),
        )
        .when(reload, |bar| {
            bar.child(
                ui::Button::new("editor-reload", "Reload disk", ui::Variant::Outline).on_click(
                    move |_, window, cx| {
                        let path = path.clone();
                        this.update(cx, |bench, cx| {
                            if let Some(document) = bench.editors.open.get_mut(&path) {
                                document.notice = None;
                                document.dirty = false;
                                document.version = None;
                            }
                            read(bench, path, window, cx);
                        })
                    },
                ),
            )
        })
}

/// Makes the editor for `path` and reads the file into it.
fn open(bench: &mut Workbench, path: PathBuf, window: &mut Window, cx: &mut Context<Workbench>) {
    if bench.editors.open.contains_key(&path) {
        return;
    }
    bench.editors.window = Some(window.window_handle());
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let language = crate::file_icons::language(&name, cx).unwrap_or_default();
    let factory = crate::syntax::highlighters(cx);
    let state = cx.new(|cx| {
        let mut state = EditorState::new(window, cx)
            .language(language)
            .soft_wrap(false)
            .line_number(true);
        state.set_highlighter_factory(factory, cx);
        state
    });
    let watched = path.clone();
    let changes = cx.subscribe(&state, move |bench, state, event: &InputEvent, cx| {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        let Some(document) = bench.editors.open.get_mut(&watched) else {
            return;
        };
        let text = state.read(cx).value();
        let dirty = text.len() != document.saved.len() || text.as_ref() != document.saved;
        if dirty != document.dirty {
            document.dirty = dirty;
            cx.notify();
        }
    });
    bench.editors.open.insert(
        path.clone(),
        Document {
            state,
            saved: String::new(),
            version: None,
            loaded: false,
            dirty: false,
            notice: None,
            _changes: changes,
        },
    );
    show_files(&bench.editors, &bench.asks);
    read(bench, path, window, cx);
}

/// Tells the core which files to watch: every one open in an editor.
fn show_files(editors: &Editors, asks: &UnboundedSender<Ask>) {
    let _ = asks.send(Ask::Call {
        method: "showFiles",
        params: json!({ "paths": editors.watched() }),
    });
}

/// Reads the whole file in the pieces the core gives, starting again when it
/// changes under the read.
async fn read_file(asks: UnboundedSender<Ask>, path: String) -> Result<(String, String), String> {
    for _ in 0..READ_TRIES {
        let mut text = String::new();
        let mut at = 0u64;
        let mut stamp: Option<String> = None;
        let moved = loop {
            let piece =
                core::request(&asks, "files.read", json!({ "path": path, "at": at })).await?;
            let piece_stamp = piece["stamp"].as_str().map(str::to_owned);
            if stamp.is_some() && piece_stamp != stamp {
                break true;
            }
            stamp = piece_stamp;
            text.push_str(piece["text"].as_str().unwrap_or_default());
            at = piece["next"].as_u64().unwrap_or(at);
            if let Some(version) = piece["version"].as_str() {
                return Ok((text, version.to_owned()));
            }
            if piece["text"].as_str().is_none_or(str::is_empty) {
                break false;
            }
        };
        if !moved {
            break;
        }
    }
    Err(format!("{path} kept changing while it was read"))
}

/// Reads the file into its editor, replacing what it shows.
fn read(bench: &mut Workbench, path: PathBuf, window: &mut Window, cx: &mut Context<Workbench>) {
    let asks = bench.asks.clone();
    let name = path.to_string_lossy().into_owned();
    cx.spawn_in(window, async move |bench, cx| {
        let read = read_file(asks, name).await;
        let _ = bench.update_in(cx, |bench, window, cx| {
            let Some(document) = bench.editors.open.get_mut(&path) else {
                return;
            };
            match read {
                Ok((_, version)) if document.version.as_ref() == Some(&version) => {}
                Ok(_) if document.dirty => {
                    document.notice = Some(Notice::Conflict(
                        "The disk version changed while this editor had unsaved work.".into(),
                    ));
                }
                Ok((text, version)) => {
                    document
                        .state
                        .update(cx, |state, cx| state.set_value(text.clone(), window, cx));
                    document.saved = text;
                    document.version = Some(version);
                    document.loaded = true;
                    document.dirty = false;
                    document.notice = None;
                }
                Err(error) => document.notice = Some(Notice::Failed(error)),
            }
            bench.parts.redraw(&bench.parts.stage, cx);
        });
    })
    .detach();
}

/// Saves the editor's text over the file, refused by the core when the file
/// moved on since it was read. A save of an unchanged file reads it again.
fn save(bench: &mut Workbench, path: &Path, window: &mut Window, cx: &mut Context<Workbench>) {
    let Some(document) = bench.editors.open.get(path) else {
        return;
    };
    if !document.dirty {
        if let Some(document) = bench.editors.open.get_mut(path) {
            document.version = None;
        }
        return read(bench, path.to_owned(), window, cx);
    }
    let text = document.state.read(cx).value().to_string();
    let version = document.version.clone();
    let asks = bench.asks.clone();
    let path = path.to_owned();
    cx.spawn_in(window, async move |bench, cx| {
        let saved = write_file(&asks, &path.to_string_lossy(), &text, version).await;
        let _ = bench.update_in(cx, |bench, _, cx| {
            let Some(document) = bench.editors.open.get_mut(&path) else {
                return;
            };
            match saved {
                Ok(version) => {
                    document.dirty = document.state.read(cx).value().as_ref() != text;
                    document.saved = text;
                    document.version = Some(version);
                    document.notice = None;
                }
                Err(error) if error.starts_with("file-conflict:") => {
                    let error = error.trim_start_matches("file-conflict:").trim().to_owned();
                    document.notice = Some(Notice::Conflict(error));
                }
                Err(error) => document.notice = Some(Notice::Failed(error)),
            }
            bench.parts.redraw(&bench.parts.stage, cx);
        });
    })
    .detach();
}

/// Writes `text` in the pieces the core takes, never splitting a character,
/// answering the saved file's version.
async fn write_file(
    asks: &UnboundedSender<Ask>,
    path: &str,
    text: &str,
    version: Option<String>,
) -> Result<String, String> {
    let mut upload: Option<String> = None;
    let mut rest = text;
    loop {
        let mut end = rest.len().min(SAVE_PIECE);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let (piece, after) = rest.split_at(end);
        let more = !after.is_empty();
        let written = core::request(
            asks,
            "files.write",
            json!({ "path": path, "text": piece, "version": version, "upload": upload, "more": more }),
        )
        .await?;
        if !more {
            return written["version"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{path} was saved without a version"));
        }
        upload = written["upload"].as_str().map(str::to_owned);
        rest = after;
    }
}

/// Files that changed on disk: clean editors take the new text, and one with
/// unsaved edits says the disk moved on when its version did.
pub fn changed(bench: &mut Workbench, paths: &[String], cx: &mut Context<Workbench>) {
    let Some(window) = bench.editors.window else {
        return;
    };
    let changed: Vec<PathBuf> = paths
        .iter()
        .map(PathBuf::from)
        .filter(|path| bench.editors.open.contains_key(path))
        .collect();
    if changed.is_empty() {
        return;
    }
    cx.spawn(async move |this, cx| {
        let _ = window.update(cx, |_, window, cx| {
            let _ = this.update(cx, |bench, cx| {
                for path in changed {
                    read(bench, path, window, cx);
                }
            });
        });
    })
    .detach();
}

/// Puts the cursor where `at` says and brings its line into view.
fn place_cursor(
    bench: &mut Workbench,
    at: &OpenAt,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let Some(document) = bench.editors.open.get(&at.path) else {
        return;
    };
    document.state.update(cx, |state, cx| {
        let column = at.column.unwrap_or_else(|| {
            let text = state.value();
            let line = text.lines().nth(at.line as usize).unwrap_or_default();
            (line.len() - line.trim_start().len()) as u32
        });
        state.set_cursor_position(Position::new(at.line, column), window, cx);
    });
}

/// The editors' text scale the person chose.
fn text_scale(bench: &Workbench) -> f64 {
    bench
        .read("preferences")
        .and_then(|preferences| preferences["editorTextScale"].as_f64())
        .unwrap_or(DEFAULT_TEXT_SCALE)
}

/// Makes the editors' text bigger or smaller by `step`, or back to its size without one.
fn rescale(bench: &Workbench, step: Option<f64>) {
    let scale = step.map_or(DEFAULT_TEXT_SCALE, |step| {
        ((text_scale(bench) + step) * 10.).round() / 10.
    });
    let _ = bench.asks.send(Ask::Call {
        method: "setPreference",
        params: json!({ "key": "editorTextScale", "value": Value::from(scale.clamp(MIN_TEXT_SCALE, MAX_TEXT_SCALE)) }),
    });
}
