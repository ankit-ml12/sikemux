//! Language servers for the editor's files, run by the core: a file opens
//! with its language's server once the person trusts its project, stays in
//! step as it is edited, and answers where a symbol is defined.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;

use gpui_kit::component::input::{DefinitionProvider, Rope, RopeExt};
use gpui_kit::{App, AppContext, Context, Task, Window};
use lsp_types::{LocationLink, Range, ShowDocumentParams, Uri};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;

use super::{Document, Notice, OpenAt};
use crate::core::{self, Ask};
use crate::workbench::Workbench;

/// How long typing pauses before the server is sent the text.
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(120);
/// What a symbol can lead to, asked in turn until one answers.
const KINDS: [&str; 4] = [
    "definition",
    "declaration",
    "typeDefinition",
    "implementation",
];

/// A file open with a language server, named as the core keeps it.
pub(super) struct Served {
    project: String,
    language: String,
    path: String,
    version: u32,
    /// Sends the text to the server once typing pauses.
    sync: Task<()>,
}

impl Served {
    pub(super) fn document(&self) -> Value {
        json!({ "project": self.project, "language": self.language, "path": self.path })
    }
}

/// The server a file opens with, as the core names it, and the language id
/// the protocol knows the file by.
fn languages(path: &Path, cx: &App) -> Option<(String, String)> {
    let language = crate::file_icons::language(path.file_name()?.to_str()?, cx)?;
    Some(match language.as_str() {
        "jsx" => ("javascript".into(), "javascriptreact".into()),
        "tsx" => ("typescript".into(), "typescriptreact".into()),
        _ => (language.clone(), language),
    })
}

/// The folder of the open project holding `path`.
pub(super) fn project_of(bench: &Workbench, path: &Path) -> Option<String> {
    bench
        .projects()
        .iter()
        .filter(|project| path.starts_with(&project.path))
        .max_by_key(|project| project.path.as_os_str().len())
        .map(|project| project.path.to_string_lossy().into_owned())
}

/// Opens the file with its language's server when the core has one and the
/// person trusts the project, asking them first when they have not said.
pub(super) fn attach(
    bench: &mut Workbench,
    path: PathBuf,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let (Some((language, language_id)), Some(project)) =
        (languages(&path, cx), project_of(bench, &path))
    else {
        return;
    };
    let trust = bench
        .read("preferences")
        .and_then(|preferences| preferences["languageServerTrust"][&project].as_bool());
    let asks = bench.asks.clone();
    cx.spawn_in(window, async move |bench, cx| {
        let served = core::request(&asks, "lsp.languages", json!({}))
            .await
            .unwrap_or_default();
        let served = served
            .as_array()
            .is_some_and(|served| served.iter().any(|name| name.as_str() == Some(&language)));
        match trust {
            _ if !served => return,
            Some(false) => return,
            None => {
                let _ = bench.update(cx, |bench, cx| {
                    if let Some(document) = bench.editors.open.get_mut(&path) {
                        document.notice = Some(Notice::Trust(project));
                        bench.parts.redraw(&bench.parts.stage, cx);
                    }
                });
                return;
            }
            Some(true) => {}
        }
        let started = json!({ "project": project, "language": language });
        if let Err(error) = core::request(&asks, "lsp.start", started).await {
            eprintln!("sikemux: the {language} language server did not start: {error}");
            return;
        }
        let Ok(Some(content)) = bench.update(cx, |bench, cx| {
            bench
                .editors
                .open
                .get(&path)
                .map(|document| document.state.read(cx).value().to_string())
        }) else {
            return;
        };
        let name = path.to_string_lossy().into_owned();
        let opened = json!({
            "project": project,
            "language": language,
            "path": name,
            "content": content,
            "languageId": language_id,
        });
        if let Err(error) = core::request(&asks, "lsp.open", opened).await {
            eprintln!("sikemux: {name} did not open with its language server: {error}");
            return;
        }
        let _ = bench.update(cx, |bench, cx| {
            let Some(document) = bench.editors.open.get_mut(&path) else {
                return;
            };
            let served = Served {
                project,
                language,
                path: name,
                version: 1,
                sync: Task::ready(()),
            };
            let found = Rc::new(RefCell::new(Vec::new()));
            let definitions = Rc::new(Definitions {
                asks: bench.asks.clone(),
                document: served.document(),
                found: found.clone(),
            });
            let (this, current) = (cx.weak_entity(), path.clone());
            document.state.update(cx, |state, _| {
                let lsp = state.lsp_mut();
                lsp.definition_provider = Some(definitions);
                lsp.show_document = Some(Rc::new(
                    move |params: &ShowDocumentParams, window: &mut Window, cx: &mut App| {
                        let found = found.borrow().clone();
                        if found.len() > 1 {
                            let places = found.iter().filter_map(place).collect();
                            let _ = this.update(cx, |bench, cx| {
                                super::peek(bench, &current, "Definitions", places, cx)
                            });
                            return true;
                        }
                        show_document(params, window, cx)
                    },
                ));
            });
            document.served = Some(served);
        });
    })
    .detach();
}

/// Sends the server the file's text once typing pauses.
pub(super) fn changed(
    document: &mut Document,
    asks: &UnboundedSender<Ask>,
    cx: &mut Context<Workbench>,
) {
    let Some(served) = document.served.as_mut() else {
        return;
    };
    served.version += 1;
    let mut params = served.document();
    params["version"] = served.version.into();
    let (asks, state) = (asks.clone(), document.state.clone());
    served.sync = cx.spawn(async move |_, cx| {
        cx.background_executor().timer(CHANGE_DEBOUNCE).await;
        let content = state.read_with(cx, |state, _| state.value().to_string());
        params["content"] = content.into();
        let _ = core::request(&asks, "lsp.change", params).await;
    });
}

/// Tells the server the file was saved with `content`.
pub(super) fn saved(document: &Document, content: &str, asks: &UnboundedSender<Ask>) {
    if let Some(served) = &document.served {
        let mut params = served.document();
        params["content"] = content.into();
        let _ = asks.send(Ask::Call {
            method: "lsp.save",
            params,
        });
    }
}

/// Lets the server go of a file no editor holds any more.
pub(super) fn closed(document: &Document, asks: &UnboundedSender<Ask>) {
    if let Some(served) = &document.served {
        let _ = asks.send(Ask::Call {
            method: "lsp.close",
            params: served.document(),
        });
    }
}

/// Remembers whether the project's language servers may run, then opens the
/// file with its server when they may.
pub(super) fn trust(
    bench: &mut Workbench,
    path: PathBuf,
    project: String,
    allowed: bool,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let mut trusted = bench
        .read("preferences")
        .and_then(|preferences| preferences["languageServerTrust"].as_object().cloned())
        .unwrap_or_default();
    trusted.insert(project, allowed.into());
    let _ = bench.asks.send(Ask::Call {
        method: "setPreference",
        params: json!({ "key": "languageServerTrust", "value": trusted }),
    });
    if let Some(document) = bench.editors.open.get_mut(&path) {
        document.notice = None;
    }
    if allowed {
        attach(bench, path, window, cx);
    }
}

/// Opens a definition through editor.openAt, so ⌘[ comes back from it.
fn show_document(params: &ShowDocumentParams, window: &mut Window, cx: &mut App) -> bool {
    let Some(path) = file_path(&params.uri) else {
        return false;
    };
    let start = params
        .selection
        .map(|range| range.start)
        .unwrap_or_default();
    let at = OpenAt {
        path,
        line: start.line,
        column: Some(start.character),
    };
    window.dispatch_action(Box::new(at), cx);
    true
}

fn place(link: &LocationLink) -> Option<OpenAt> {
    let start = link.target_selection_range.start;
    Some(OpenAt {
        path: file_path(&link.target_uri)?,
        line: start.line,
        column: Some(start.character),
    })
}

/// The file a `file://` URI names, its escapes undone.
fn file_path(uri: &Uri) -> Option<PathBuf> {
    if uri.scheme().map(|scheme| scheme.as_str()) != Some("file") {
        return None;
    }
    let escaped = uri.path().as_str().as_bytes();
    let mut bytes = Vec::with_capacity(escaped.len());
    let mut at = 0;
    while at < escaped.len() {
        let hex = escaped
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (escaped[at], hex) {
            (b'%', Some(byte)) => {
                bytes.push(byte);
                at += 3;
            }
            (byte, _) => {
                bytes.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(bytes).ok().map(PathBuf::from)
}

/// Where a symbol leads, as the core's language server says. The places
/// last found are kept, since the editor jumps to the first of them alone.
struct Definitions {
    asks: UnboundedSender<Ask>,
    document: Value,
    found: Rc<RefCell<Vec<LocationLink>>>,
}

impl DefinitionProvider for Definitions {
    fn definitions(
        &self,
        text: &Rope,
        offset: usize,
        _: &mut Window,
        cx: &mut App,
    ) -> Task<anyhow::Result<Vec<LocationLink>>> {
        let position = text.offset_to_position(offset);
        let (asks, document) = (self.asks.clone(), self.document.clone());
        let asked = cx.background_spawn(async move {
            for kind in KINDS {
                let mut params = document.clone();
                params["line"] = position.line.into();
                params["character"] = position.character.into();
                params["kind"] = kind.into();
                let found = core::request(&asks, "lsp.locations", params)
                    .await
                    .map_err(anyhow::Error::msg)?;
                let links: Vec<LocationLink> = found
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(link)
                    .collect();
                if !links.is_empty() {
                    return Ok(links);
                }
            }
            anyhow::Ok(Vec::new())
        });
        let found = self.found.clone();
        cx.spawn(async move |_| {
            let links = asked.await?;
            found.replace(links.clone());
            Ok(links)
        })
    }
}

fn link(location: &Value) -> Option<LocationLink> {
    let target_uri = Uri::from_str(location["uri"].as_str()?).ok()?;
    let range: Range = serde_json::from_value(location["range"].clone()).ok()?;
    Some(LocationLink {
        origin_selection_range: None,
        target_uri,
        target_range: range,
        target_selection_range: range,
    })
}
