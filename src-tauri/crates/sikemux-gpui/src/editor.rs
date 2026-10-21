//! The editor's tab: a file read and saved through the core, edited on Kit's
//! code editor, and opened from anywhere at a line.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::component::input::{
    Editor, EditorState, Input, InputEvent, InputState, Position, Replace,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    actions, div, px, AnyElement, AnyWindowHandle, App, AppContext, Context, Entity, FontWeight,
    InteractiveElement, IntoElement, KeyBinding, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window,
};
use serde_json::{json, Value};
use sikemux_wire::protocol::{Pane, PaneKind};
use tokio::sync::mpsc::UnboundedSender;

use crate::core::{self, Ask};
use crate::keymap::{TextSizeDecrease, TextSizeIncrease, TextSizeReset};
use crate::theme::{self, Tokens};
use crate::ui;
use crate::workbench::Workbench;

mod blame;
mod gutter;
mod insights;
mod lsp;
mod preview;
mod viewer;

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

actions!(
    editor,
    [
        Save,
        GoToLine,
        CloseGoToLine,
        NavigateBack,
        NavigateForward,
        ClosePeek
    ]
);

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
/// How many places back ⌘[ remembers.
const MAX_HISTORY: usize = 128;
/// The most places a peek lists.
const MAX_PEEKED: usize = 300;

pub fn install(cx: &mut App) {
    let save = if cfg!(target_os = "macos") {
        "cmd-s"
    } else {
        "ctrl-s"
    };
    let mac = cfg!(target_os = "macos");
    let keys = |mac_keys: &'static str, other: &'static str| if mac { mac_keys } else { other };
    let text = Some("FileEditor > Input");
    cx.bind_keys([
        KeyBinding::new(save, Save, Some(CONTEXT)),
        KeyBinding::new(keys("cmd-alt-g", "ctrl-alt-g"), GoToLine, Some(CONTEXT)),
        KeyBinding::new("escape", CloseGoToLine, Some("GoToLine > Input")),
        KeyBinding::new("escape", ClosePeek, Some("peek > Input")),
        KeyBinding::new("escape", ClosePeek, Some("peek")),
        KeyBinding::new(keys("cmd-h", "ctrl-h"), Replace, text),
        KeyBinding::new(keys("cmd-[", "ctrl-["), NavigateBack, text),
        KeyBinding::new(keys("cmd-]", "ctrl-]"), NavigateForward, text),
        // The app's own search across the project keeps ⇧⌘F, which Kit's editor takes for replace.
        KeyBinding::new(keys("cmd-shift-f", "ctrl-shift-f"), gpui::NoAction, text),
    ]);
}

/// Every file open in an editor, by its path.
#[derive(Default)]
pub struct Editors {
    open: HashMap<PathBuf, Document>,
    /// Where a file asked to open at a line puts its cursor once it is read.
    reveal: Option<OpenAt>,
    /// The window the editors draw in, which reading and reloading update.
    window: Option<AnyWindowHandle>,
    /// Where the editor was before each jump, and where ⌘[ came back from.
    back: Vec<OpenAt>,
    forward: Vec<OpenAt>,
    insights: insights::Insights,
    /// Files that are not text, shown for what they are.
    viewers: HashMap<PathBuf, viewer::Viewer>,
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
    /// The field asking which line to go to, while it is open.
    goto: Option<(Entity<InputState>, Subscription)>,
    /// The language server the file is open with, once it is.
    served: Option<lsp::Served>,
    /// The places a symbol leads to when there is more than one.
    peek: Option<Peek>,
    /// Whether it shows its text or, for markdown, how it reads, and that rendering.
    mode: preview::Mode,
    rendered: Option<Rc<gpui_transcript::Markdown>>,
    /// What changed since the last commit, marked beside the lines.
    gutter: gutter::Gutter,
    /// Who last changed each line.
    blame: blame::Blame,
    _changes: Subscription,
}

#[derive(Clone)]
enum Notice {
    Failed(String),
    /// The disk moved on under unsaved edits; reloading drops them.
    Conflict(String),
    /// Whether the language servers of the project, by its folder, may run.
    Trust(String),
}

impl Editors {
    /// Whether the file has edits not yet saved.
    pub fn is_dirty(&self, path: &Path) -> bool {
        self.open.get(path).is_some_and(|document| document.dirty)
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
    if bench.editors.viewers.contains_key(&path) {
        return viewer::pane(bench, this, &path, cx);
    }
    if viewer::not_text(&path) {
        let opening = path.clone();
        cx.defer_in(window, move |bench, _, cx| viewer::load(bench, opening, cx));
        return div().size_full().into_any_element();
    }
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
    let project = lsp::project_of(bench, &path);
    let this = this.clone();
    let saving = path.clone();
    div()
        .size_full()
        .flex()
        .flex_col()
        .key_context(match document.peek {
            Some(_) => "FileEditor peek",
            None => CONTEXT,
        })
        .on_action(
            window.listener_for(&this, move |bench, _: &Save, window, cx| {
                save(bench, &saving, window, cx)
            }),
        )
        .on_action(
            window.listener_for(&this, |bench, _: &NavigateBack, _, cx| {
                step(bench, true, cx)
            }),
        )
        .on_action(
            window.listener_for(&this, |bench, _: &NavigateForward, _, cx| {
                step(bench, false, cx)
            }),
        )
        .on_action(window.listener_for(&this, |bench, _: &ClosePeek, _, cx| close_peeks(bench, cx)))
        .on_action({
            let path = path.clone();
            window.listener_for(&this, move |bench, _: &GoToLine, window, cx| {
                open_goto(bench, &path, window, cx)
            })
        })
        .on_action({
            let path = path.clone();
            window.listener_for(&this, move |bench, _: &CloseGoToLine, window, cx| {
                close_goto(bench, &path, window, cx)
            })
        })
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
        .when_some(document.goto.as_ref(), |pane, (input, _)| {
            pane.child(
                div()
                    .key_context("GoToLine")
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .py(px(6.))
                    .border_b_1()
                    .border_color(t.border)
                    .text_size(px(12.))
                    .child(div().text_color(t.text_tertiary).child("Go to line"))
                    .child(
                        Input::new(input)
                            .appearance(false)
                            .flex_1()
                            .p_0()
                            .text_color(t.text_primary),
                    ),
            )
        })
        .when(
            preview::first_mode(&path) == preview::Mode::Preview,
            |pane| pane.child(preview::toolbar(&this, &path, document.mode, cx)),
        )
        .map(|pane| match (document.mode, document.rendered.clone()) {
            (preview::Mode::Preview, Some(markdown)) => {
                pane.child(div().flex_1().min_h_0().child(preview::Body {
                    path: path.clone(),
                    markdown,
                }))
            }
            _ => pane.child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .font_family(theme::MONO)
                    .text_size(size)
                    .text_color(t.text_primary)
                    .child(Editor::new(&document.state).bordered(false).h_full())
                    .child(gutter::overlay(
                        &this,
                        &path,
                        &document.state,
                        &document.gutter,
                        gpui_kit::component::input::RopeExt::lines_len(document.state.read(cx).text()),
                    ))
                    .child(blame::overlay(
                        &document.state,
                        &document.blame,
                        size,
                        t.text_disabled,
                    )),
            ),
        })
        .when_some(document.peek.as_ref(), |pane, peek| {
            pane.child(peek_panel(&this, peek, cx))
        })
        .child(insights::bar(
            bench,
            &this,
            &path,
            project.as_deref().map(Path::new),
            cx,
        ))
        .into_any_element()
}

/// The places a symbol leads to, by file, with each line's text once read.
struct Peek {
    title: SharedString,
    places: Vec<Place>,
}

struct Place {
    at: OpenAt,
    text: Option<SharedString>,
}

/// Opens `to`, remembering where the editor in front was so ⌘[ comes back.
pub fn go(bench: &mut Workbench, to: OpenAt, cx: &mut Context<Workbench>) {
    if let Some(here) = here(bench, cx) {
        bench.editors.back.push(here);
        if bench.editors.back.len() > MAX_HISTORY {
            bench.editors.back.remove(0);
        }
        bench.editors.forward.clear();
    }
    show(bench, to, cx);
}

fn show(bench: &mut Workbench, at: OpenAt, cx: &mut Context<Workbench>) {
    let path = at.path.clone();
    bench.editors.reveal = Some(at);
    bench.open_file(&path);
    bench.parts.redraw(&bench.parts.stage, cx);
}

/// Where the cursor is in the editor in front.
fn here(bench: &Workbench, cx: &App) -> Option<OpenAt> {
    let path = bench.front_file()?;
    let position = bench
        .editors
        .open
        .get(&path)?
        .state
        .read(cx)
        .cursor_position();
    Some(OpenAt {
        path,
        line: position.line,
        column: Some(position.character),
    })
}

/// Goes back to where the editor was before its last jump, or forward again.
fn step(bench: &mut Workbench, back: bool, cx: &mut Context<Workbench>) {
    let taken = match back {
        true => bench.editors.back.pop(),
        false => bench.editors.forward.pop(),
    };
    let Some(to) = taken else {
        return;
    };
    if let Some(here) = here(bench, cx) {
        match back {
            true => bench.editors.forward.push(here),
            false => bench.editors.back.push(here),
        }
    }
    show(bench, to, cx);
}

/// Lists the places a symbol leads to under the editor of `path`, and reads
/// each one's line.
fn peek(
    bench: &mut Workbench,
    path: &Path,
    title: &str,
    places: Vec<OpenAt>,
    cx: &mut Context<Workbench>,
) {
    let Some(document) = bench.editors.open.get_mut(path) else {
        return;
    };
    let places: Vec<Place> = places
        .into_iter()
        .take(MAX_PEEKED)
        .map(|at| Place { at, text: None })
        .collect();
    let mut files: Vec<PathBuf> = places.iter().map(|place| place.at.path.clone()).collect();
    files.dedup();
    document.peek = Some(Peek {
        title: title.to_owned().into(),
        places,
    });
    bench.parts.redraw(&bench.parts.stage, cx);
    let (asks, peeked) = (bench.asks.clone(), path.to_owned());
    cx.spawn(async move |bench, cx| {
        for file in files {
            let Ok((text, _)) = read_file(asks.clone(), file.to_string_lossy().into_owned()).await
            else {
                continue;
            };
            let lines: Vec<&str> = text.lines().collect();
            let _ = bench.update(cx, |bench, cx| {
                let Some(peek) = bench
                    .editors
                    .open
                    .get_mut(&peeked)
                    .and_then(|document| document.peek.as_mut())
                else {
                    return;
                };
                for place in peek.places.iter_mut().filter(|place| place.at.path == file) {
                    let line = lines
                        .get(place.at.line as usize)
                        .copied()
                        .unwrap_or_default();
                    place.text = Some(line.trim().to_owned().into());
                }
                bench.parts.redraw(&bench.parts.stage, cx);
            });
        }
    })
    .detach();
}

fn close_peeks(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    for document in bench.editors.open.values_mut() {
        document.peek = None;
    }
    bench.parts.redraw(&bench.parts.stage, cx);
}

/// The panel under an editor listing the places a symbol leads to, by file.
fn peek_panel(
    this: &Entity<Workbench>,
    peek: &Peek,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let mut rows: Vec<AnyElement> = Vec::new();
    let mut file: Option<&Path> = None;
    for (index, place) in peek.places.iter().enumerate() {
        if file != Some(place.at.path.as_path()) {
            file = Some(place.at.path.as_path());
            let name = place
                .at
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let folder = place
                .at
                .path
                .parent()
                .map(|folder| folder.to_string_lossy().into_owned());
            rows.push(
                div()
                    .flex()
                    .gap(px(8.))
                    .px(px(12.))
                    .pt(px(6.))
                    .child(div().text_color(t.text_primary).children(name))
                    .child(div().text_color(t.text_tertiary).children(folder))
                    .into_any_element(),
            );
        }
        let (this, at) = (this.clone(), place.at.clone());
        rows.push(
            div()
                .id(("peek", index))
                .flex()
                .gap(px(12.))
                .h(px(24.))
                .items_center()
                .px(px(12.))
                .cursor_pointer()
                .hover(move |style| style.bg(t.surface_hover))
                .on_click(move |_, _, cx| {
                    let at = at.clone();
                    this.update(cx, |bench, cx| {
                        close_peeks(bench, cx);
                        go(bench, at, cx);
                    })
                })
                .child(
                    div()
                        .w(px(40.))
                        .text_right()
                        .text_color(t.text_tertiary)
                        .child((place.at.line + 1).to_string()),
                )
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(t.text_secondary)
                        .children(place.text.clone()),
                )
                .into_any_element(),
        );
    }
    div()
        .flex()
        .flex_col()
        .max_h(px(320.))
        .border_t_1()
        .border_color(t.border)
        .font_family(theme::MONO)
        .text_size(px(12.))
        .child(
            div()
                .flex()
                .items_center()
                .h(px(30.))
                .px(px(12.))
                .gap(px(8.))
                .border_b_1()
                .border_color(t.border)
                .child(
                    div()
                        .text_color(t.text_secondary)
                        .child(peek.title.to_uppercase()),
                )
                .child(
                    div()
                        .text_color(t.text_tertiary)
                        .child(peek.places.len().to_string()),
                )
                .child(div().flex_1())
                .child(
                    ui::IconButton::new("peek-close", crate::icons::Icon::Close, "Close — esc")
                        .small()
                        .on_click({
                            let this = this.clone();
                            move |_, _, cx| this.update(cx, close_peeks)
                        }),
                ),
        )
        .child(
            div()
                .id("peek-places")
                .flex()
                .flex_col()
                .overflow_y_scroll()
                .pb(px(6.))
                .children(rows),
        )
}

/// Opens the field asking which line to go to, as `line` or `line:column`
/// counted from 1.
fn open_goto(bench: &mut Workbench, path: &Path, window: &mut Window, cx: &mut Context<Workbench>) {
    if !bench.editors.open.contains_key(path) {
        return;
    }
    let input = cx.new(|cx| InputState::new(window, cx).placeholder("line[:column]"));
    let asked_for = path.to_owned();
    let entered = cx.subscribe_in(
        &input,
        window,
        move |bench, input, event: &InputEvent, window, cx| {
            if !matches!(event, InputEvent::PressEnter { .. }) {
                return;
            }
            let asked = input.read(cx).value();
            let mut parts = asked
                .trim()
                .splitn(2, ':')
                .map(|part| part.trim().parse::<u32>().ok());
            let line = parts.next().flatten();
            let column = parts.next().flatten();
            close_goto(bench, &asked_for, window, cx);
            if let Some(line) = line {
                let at = OpenAt {
                    path: asked_for.clone(),
                    line: line.saturating_sub(1),
                    column: column.map(|column| column.saturating_sub(1)),
                };
                place_cursor(bench, &at, window, cx);
            }
        },
    );
    input.update(cx, |input, cx| input.focus(window, cx));
    if let Some(document) = bench.editors.open.get_mut(path) {
        document.goto = Some((input, entered));
    }
    bench.parts.redraw(&bench.parts.stage, cx);
}

/// Closes the go-to-line field and gives the editor back the keyboard.
fn close_goto(
    bench: &mut Workbench,
    path: &Path,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let Some(document) = bench.editors.open.get_mut(path) else {
        return;
    };
    if document.goto.take().is_some() {
        document
            .state
            .update(cx, |state, cx| state.focus(window, cx));
        bench.parts.redraw(&bench.parts.stage, cx);
    }
}

/// The bar over an editor saying it could not read or save its file, or
/// that the file changed on disk under unsaved edits.
/// Something a notice bar's button does to the open file.
type Run = dyn Fn(&mut Workbench, &mut Window, &mut Context<Workbench>);

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
    let text: SharedString = match &notice {
        Notice::Failed(error) => error.clone().into(),
        Notice::Conflict(error) => {
            format!("{name} has an external change. Your editor buffer was preserved. {error}")
                .into()
        }
        Notice::Trust(project) => {
            let folder = Path::new(project).file_name().map_or_else(
                || project.clone(),
                |name| name.to_string_lossy().into_owned(),
            );
            format!(
                "Start language servers for {folder}? They can run code from this project, such as its build scripts and installed tools. Only allow this for projects you trust."
            )
            .into()
        }
    };
    let button =
        |id: &'static str,
         label: &'static str,
         variant,
         run: Rc<Run>| {
            let this = this.clone();
            ui::Button::new(id, label, variant).on_click(move |_, window, cx| {
                let run = run.clone();
                this.update(cx, |bench, cx| run(bench, window, cx))
            })
        };
    let reload = {
        let path = path.to_owned();
        button(
            "editor-reload",
            "Reload disk",
            ui::Variant::Outline,
            Rc::new(move |bench, window, cx| {
                if let Some(document) = bench.editors.open.get_mut(&path) {
                    document.notice = None;
                    document.dirty = false;
                    document.version = None;
                }
                read(bench, path.clone(), window, cx);
            }),
        )
    };
    let answers = match &notice {
        Notice::Trust(project) => {
            let answer = |allowed: bool| {
                let (path, project) = (path.to_owned(), project.clone());
                Rc::new(
                    move |bench: &mut Workbench,
                          window: &mut Window,
                          cx: &mut Context<Workbench>| {
                        lsp::trust(bench, path.clone(), project.clone(), allowed, window, cx)
                    },
                )
                    as Rc<dyn Fn(&mut Workbench, &mut Window, &mut Context<Workbench>)>
            };
            vec![
                button("editor-trust", "Allow", ui::Variant::Primary, answer(true)),
                button(
                    "editor-distrust",
                    "Don't allow",
                    ui::Variant::Outline,
                    answer(false),
                ),
            ]
        }
        Notice::Conflict(_) => vec![reload],
        Notice::Failed(_) => Vec::new(),
    };
    let tone = match notice {
        Notice::Trust(_) => t.text_primary,
        _ => t.warn,
    };
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
                .text_color(tone)
                .font_weight(FontWeight::MEDIUM)
                .child(text),
        )
        .children(answers)
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
        let asks = bench.asks.clone();
        let Some(document) = bench.editors.open.get_mut(&watched) else {
            return;
        };
        let text = state.read(cx).value();
        let dirty = text.len() != document.saved.len() || text.as_ref() != document.saved;
        lsp::changed(document, &asks, cx);
        if dirty != document.dirty {
            document.dirty = dirty;
            cx.notify();
        }
        if dirty {
            keep_preview(bench, &watched);
        }
        gutter::changed(&watched, bench, cx);
        blame::typed(bench, &watched, cx);
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
            goto: None,
            served: None,
            peek: None,
            mode: preview::first_mode(&path),
            rendered: None,
            gutter: gutter::Gutter::default(),
            blame: blame::Blame::default(),
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
            let asks = bench.asks.clone();
            let Some(document) = bench.editors.open.get_mut(&path) else {
                return;
            };
            let first = !document.loaded;
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
                    if document.mode == preview::Mode::Preview {
                        document.rendered = Some(Rc::new(preview::render(&document.saved)));
                    }
                    lsp::changed(document, &asks, cx);
                }
                Err(error) if error.starts_with("not-text:") => {
                    bench.editors.open.remove(&path);
                    viewer::load(bench, path.clone(), cx);
                    show_files(&bench.editors, &bench.asks);
                    return;
                }
                Err(error) => document.notice = Some(Notice::Failed(error)),
            }
            if first && document.loaded {
                lsp::attach(bench, path.clone(), window, cx);
                gutter::read_base(bench, &path, cx);
                blame::read(bench, &path, cx);
            } else {
                gutter::changed(&path, bench, cx);
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
            let landed = saved.is_ok();
            match saved {
                Ok(version) => {
                    lsp::saved(document, &text, &bench.asks);
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
            if landed {
                blame::read(bench, &path, cx);
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

/// Keeps the tab showing `path` when it was opened only to look at, as an
/// edit there means the person is working in it.
fn keep_preview(bench: &Workbench, path: &Path) {
    let Some(strip) = bench.strip() else {
        return;
    };
    let target = path.to_string_lossy();
    let previewed = bench.tabs(&strip).iter().find(|tab| {
        tab.preview
            && matches!(
                &tab.pane,
                Pane::View { kind: PaneKind::Files, target: shown, .. } if *shown == target
            )
    });
    if let Some(tab) = previewed {
        bench.keep_tab(&strip, &tab.id);
    }
}

/// Lets go of every file with no unsaved edits that no tab shows any more.
pub fn keep(bench: &mut Workbench) {
    let mut shown = Vec::new();
    if let Some(view) = &bench.view {
        let strips = view
            .layout
            .projects
            .values()
            .chain(view.layout.hosts.values());
        for tab in strips.flat_map(|strip| &strip.tabs) {
            files_in(&tab.pane, &mut shown);
        }
    }
    bench.editors.viewers.retain(|path, _| shown.contains(path));
    let gone: Vec<PathBuf> = bench
        .editors
        .open
        .iter()
        .filter(|(path, document)| !document.dirty && !shown.contains(*path))
        .map(|(path, _)| path.clone())
        .collect();
    if gone.is_empty() {
        return;
    }
    for path in gone {
        if let Some(document) = bench.editors.open.remove(&path) {
            lsp::closed(&document, &bench.asks);
        }
    }
    show_files(&bench.editors, &bench.asks);
}

fn files_in(pane: &Pane, found: &mut Vec<PathBuf>) {
    match pane {
        Pane::View {
            kind: PaneKind::Files,
            target,
            ..
        } => found.push(PathBuf::from(target)),
        Pane::View { .. } => {}
        Pane::Split { panes, .. } => panes.iter().for_each(|pane| files_in(pane, found)),
    }
}

/// Reads again what open files were at their last commit, when the
/// repository moved: `paths` from its root, `.git` for its head, or none for anything.
pub fn git_changed(bench: &mut Workbench, paths: Option<&[String]>, cx: &mut Context<Workbench>) {
    let open: Vec<PathBuf> = bench
        .editors
        .open
        .keys()
        .filter(|path| {
            paths.is_none_or(|paths| {
                paths
                    .iter()
                    .any(|changed| changed == ".git" || path.ends_with(changed))
            })
        })
        .cloned()
        .collect();
    for path in open {
        gutter::read_base(bench, &path, cx);
        blame::read(bench, &path, cx);
    }
}

/// Reads again the problems of files whose language server said they changed.
pub fn diagnostics_changed(bench: &mut Workbench, paths: &[String], cx: &mut Context<Workbench>) {
    insights::diagnostics_changed(bench, paths, cx);
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
    let Some(document) = bench.editors.open.get_mut(&at.path) else {
        return;
    };
    document.mode = preview::Mode::Source;
    document.state.update(cx, |state, cx| {
        let column = at.column.unwrap_or_else(|| {
            let text = state.value();
            let line = text.lines().nth(at.line as usize).unwrap_or_default();
            (line.len() - line.trim_start().len()) as u32
        });
        state.set_cursor_position(Position::new(at.line, column), window, cx);
    });
}

/// Shows the file's text, or how its markdown reads, rendered from what the
/// editor holds now.
fn show_mode(
    bench: &mut Workbench,
    path: &Path,
    mode: preview::Mode,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let Some(document) = bench.editors.open.get_mut(path) else {
        return;
    };
    document.mode = mode;
    match mode {
        preview::Mode::Preview => {
            let text = document.state.read(cx).value();
            document.rendered = Some(Rc::new(preview::render(&text)));
            document.goto = None;
        }
        preview::Mode::Source => document
            .state
            .update(cx, |state, cx| state.focus(window, cx)),
    }
    bench.parts.redraw(&bench.parts.stage, cx);
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
