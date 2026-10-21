//! Files that are not text, shown for what they are: a picture to zoom, and
//! anything else as the first of its bytes. A bar over each names the file
//! and opens or reveals it.

use std::path::{Path, PathBuf};

use base64::Engine;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, uniform_list, AnyElement, Context, Entity, InteractiveElement, IntoElement,
    ObjectFit, ParentElement, SharedString, StatefulInteractiveElement, Styled, StyledImage,
};
use serde_json::json;

use crate::core;
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui;
use crate::workbench::Workbench;

/// How much of a file the byte view shows.
const SHOWN_BYTES: u64 = 64 * 1024;
const BYTES_PER_ROW: usize = 16;

/// Extensions that are never text, so the file goes straight to a viewer.
const NOT_TEXT: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif", "tif", "tiff", "heic",
    "heif", "psd", "pdf", "ttf", "otf", "woff", "woff2", "mp4", "m4v", "mov", "webm", "mkv", "avi",
    "mp3", "m4a", "aac", "wav", "flac", "aif", "aiff", "ogg", "oga", "opus", "doc", "docx", "xls",
    "xlsx", "ppt", "pptx", "rtf", "odt", "ods", "odp", "pages", "numbers", "epub", "zip", "gz",
    "tgz", "bz2", "xz", "7z", "rar", "tar", "jar", "dmg", "iso", "exe", "dll", "so", "dylib", "o",
    "a", "class", "pyc", "wasm", "sqlite", "sqlite3",
];

/// Whether a file is shown in a viewer without being read as text first.
pub(super) fn not_text(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| NOT_TEXT.contains(&extension.to_ascii_lowercase().as_str()))
}

/// A file shown for what it is, once the core says what that is.
#[derive(Default)]
pub(super) struct Viewer {
    about: Option<About>,
    zoom: Zoom,
    bytes: Option<Vec<u8>>,
    failed: Option<SharedString>,
}

struct About {
    mime: SharedString,
    size: u64,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Zoom {
    #[default]
    Fit,
    /// One pixel of the picture to one of the screen, scrolled when larger.
    Actual,
}

#[derive(PartialEq)]
enum Kind {
    Image,
    /// What the window cannot draw yet: documents, sound and video.
    Elsewhere,
    Bytes,
}

fn kind_of(mime: &str) -> Kind {
    let documents = mime == "application/pdf"
        || mime.starts_with("application/vnd.openxmlformats-")
        || mime.starts_with("application/vnd.oasis.opendocument.")
        || matches!(
            mime,
            "application/rtf"
                | "application/msword"
                | "application/vnd.ms-excel"
                | "application/vnd.ms-powerpoint"
                | "application/vnd.apple.pages"
                | "application/vnd.apple.numbers"
                | "application/vnd.apple.keynote"
                | "application/epub+zip"
                | "image/vnd.adobe.photoshop"
        );
    if documents || mime.starts_with("audio/") || mime.starts_with("video/") {
        return Kind::Elsewhere;
    }
    if mime.starts_with("image/") {
        return Kind::Image;
    }
    Kind::Bytes
}

/// Asks the core what the file is, and reads the bytes a byte view shows.
pub(super) fn load(bench: &mut Workbench, path: PathBuf, cx: &mut Context<Workbench>) {
    bench.editors.viewers.entry(path.clone()).or_default();
    let asks = bench.asks.clone();
    cx.spawn(async move |bench, cx| {
        let name = path.to_string_lossy().into_owned();
        let about = core::request(&asks, "files.preview", json!({ "path": name })).await;
        let about = match about {
            Ok(about) => About {
                mime: about["mime"]
                    .as_str()
                    .unwrap_or("application/octet-stream")
                    .to_owned()
                    .into(),
                size: about["size"].as_u64().unwrap_or(0),
            },
            Err(error) => {
                let _ = bench.update(cx, |bench, cx| {
                    if let Some(viewer) = bench.editors.viewers.get_mut(&path) {
                        viewer.failed = Some(error.into());
                    }
                    bench.parts.redraw(&bench.parts.stage, cx);
                });
                return;
            }
        };
        let bytes = match kind_of(&about.mime) {
            Kind::Bytes => {
                let read = json!({ "path": name, "at": 0, "length": SHOWN_BYTES });
                core::request(&asks, "files.bytes", read)
                    .await
                    .ok()
                    .and_then(|read| {
                        base64::engine::general_purpose::STANDARD
                            .decode(read["base64"].as_str()?)
                            .ok()
                    })
            }
            _ => None,
        };
        let _ = bench.update(cx, |bench, cx| {
            if let Some(viewer) = bench.editors.viewers.get_mut(&path) {
                viewer.about = Some(about);
                viewer.bytes = bytes;
                viewer.failed = None;
            }
            bench.parts.redraw(&bench.parts.stage, cx);
        });
    })
    .detach();
}

/// The viewer of the file at `path`.
pub(super) fn pane(
    bench: &Workbench,
    this: &Entity<Workbench>,
    path: &Path,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let Some(viewer) = bench.editors.viewers.get(path) else {
        return div().size_full().into_any_element();
    };
    let kind = viewer.about.as_ref().map(|about| kind_of(&about.mime));
    let body: AnyElement = match (&viewer.failed, kind) {
        (Some(error), _) => message(error.clone(), &t),
        (None, None) => message("Reading…".into(), &t),
        (None, Some(Kind::Image)) => {
            let picture = img(path.to_owned());
            let picture = match viewer.zoom {
                Zoom::Fit => picture.size_full().object_fit(ObjectFit::Contain),
                Zoom::Actual => picture.object_fit(ObjectFit::None),
            };
            div()
                .id("viewer-image")
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_scroll()
                .child(picture)
                .into_any_element()
        }
        (None, Some(Kind::Elsewhere)) => message(
            "This kind of file opens in its own app from here for now.".into(),
            &t,
        ),
        (None, Some(Kind::Bytes)) => bytes(viewer.bytes.clone().unwrap_or_default(), &t),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .child(bar(this, path, viewer, cx))
        .child(div().flex_1().min_h_0().child(body))
        .into_any_element()
}

fn message(text: SharedString, t: &Tokens) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(13.))
        .text_color(t.text_tertiary)
        .child(text)
        .into_any_element()
}

/// The first bytes of a file, sixteen to a row: where each row starts, the
/// bytes, and the characters they would print as.
fn bytes(bytes: Vec<u8>, t: &Tokens) -> AnyElement {
    let rows = bytes.len().div_ceil(BYTES_PER_ROW);
    let (ink, dim) = (t.text_secondary, t.text_tertiary);
    uniform_list("viewer-bytes", rows, move |range, _, _| {
        range
            .map(|row| {
                let start = row * BYTES_PER_ROW;
                let chunk = &bytes[start..(start + BYTES_PER_ROW).min(bytes.len())];
                let hex: Vec<String> = chunk.iter().map(|byte| format!("{byte:02x}")).collect();
                let printed: String = chunk
                    .iter()
                    .map(|&byte| {
                        if byte.is_ascii_graphic() || byte == b' ' {
                            byte as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                div()
                    .flex()
                    .gap(px(16.))
                    .px(px(16.))
                    .h(px(20.))
                    .items_center()
                    .child(
                        div()
                            .w(px(72.))
                            .text_color(dim)
                            .child(format!("{start:08x}")),
                    )
                    .child(div().w(px(392.)).text_color(ink).child(hex.join(" ")))
                    .child(div().text_color(dim).child(printed))
            })
            .collect()
    })
    .size_full()
    .font_family(theme::MONO)
    .text_size(px(12.))
    .into_any_element()
}

/// The bar over a viewer: the file's name, size and type, the picture's
/// zoom, and buttons that read it again, open it in its own app, or show it
/// in Finder.
fn bar(
    this: &Entity<Workbench>,
    path: &Path,
    viewer: &Viewer,
    cx: &mut Context<Workbench>,
) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let about = viewer.about.as_ref();
    let zoom = |id: &'static str, label: &'static str, to: Zoom| {
        let (this, path) = (this.clone(), path.to_owned());
        div()
            .id(id)
            .px(px(6.))
            .cursor_pointer()
            .text_color(t.text_secondary)
            .hover(move |style| style.text_color(t.text_primary))
            .on_click(move |_, _, cx| {
                this.update(cx, |bench, cx| {
                    if let Some(viewer) = bench.editors.viewers.get_mut(&path) {
                        viewer.zoom = to;
                    }
                    bench.parts.redraw(&bench.parts.stage, cx);
                })
            })
            .child(label)
    };
    let image = about.is_some_and(|about| kind_of(&about.mime) == Kind::Image);
    let (reload, open, reveal) = (path.to_owned(), path.to_owned(), path.to_owned());
    let this_reload = this.clone();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(32.))
        .px(px(12.))
        .border_b_1()
        .border_color(t.border)
        .text_size(px(12.))
        .child(ui::icon(Icon::File, 14.).text_color(t.text_tertiary))
        .child(div().text_color(t.text_primary).child(name))
        .children(about.map(|about| div().text_color(t.text_tertiary).child(size(about.size))))
        .children(about.map(|about| div().text_color(t.text_tertiary).child(about.mime.clone())))
        .child(div().flex_1())
        .when(image, |bar| {
            bar.child(zoom("viewer-fit", "Fit", Zoom::Fit)).child(zoom(
                "viewer-actual",
                "1:1",
                Zoom::Actual,
            ))
        })
        .child(
            ui::IconButton::new("viewer-reload", Icon::Refresh, "Read it again")
                .small()
                .on_click(move |_, _, cx| {
                    let path = reload.clone();
                    this_reload.update(cx, |bench, cx| load(bench, path, cx))
                }),
        )
        .child(
            ui::IconButton::new("viewer-open", Icon::Share, "Open in its own app")
                .small()
                .on_click(move |_, _, cx| cx.open_with_system(&open)),
        )
        .child(
            ui::IconButton::new("viewer-reveal", Icon::Folder, "Show in Finder")
                .small()
                .on_click(move |_, _, cx| cx.reveal_path(&reveal)),
        )
}

/// A size in bytes as people read it.
fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }
    match unit {
        0 => format!("{bytes} B"),
        _ => format!("{value:.1} {}", UNITS[unit]),
    }
}
