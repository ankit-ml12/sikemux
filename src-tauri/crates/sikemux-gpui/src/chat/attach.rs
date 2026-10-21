//! The files a message carries: picked, dropped on the chat or pasted, shown as tiles over
//! the draft, each picture also named in the draft where it belongs, as `[Image #n]`.

use std::path::{Path, PathBuf};

use gpui_kit::base::input::{InlineToken, InlineTokenContext};
use gpui_kit::component::attachment::{Attachment, AttachmentMedia};
use gpui_kit::component::Sizable;
use gpui_kit::{
    div, px, App, Axis, ClipboardEntry, ClipboardItem, Context, Div, ExternalPaths, FontWeight,
    Hsla, InteractiveElement, IntoElement, ParentElement, PathPromptOptions, SharedString, Styled,
    Window,
};
use sikemux_core::acp::MAX_ATTACHMENTS;
use tokio::sync::oneshot;

use super::Composing;
use crate::core::{Ask, ChatAsk};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::ui;
use crate::workbench::Workbench;

const IMAGE_EXTENSIONS: [&str; 13] = [
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif", "tif", "tiff", "heic", "heif",
];

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            IMAGE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

/// macOS deletes the picture a screenshot's thumbnail drags in once the drop is over.
fn is_screenshot_thumbnail(path: &Path) -> bool {
    path.to_string_lossy()
        .contains("/TemporaryItems/NSIRD_screencaptureui_")
}

const IMAGE: &str = "image:";

/// The glyph a tag in the draft wears: a picture's, a named agent's, or a file's or folder's.
fn mark(id: &str, tint: Hsla, cx: &App) -> gpui_kit::AnyElement {
    if id.starts_with(IMAGE) {
        return ui::icon(Icon::Image, 12.)
            .text_color(tint)
            .into_any_element();
    }
    if id.starts_with("agent:") {
        return ui::icon(Icon::Agent, 12.)
            .text_color(tint)
            .into_any_element();
    }
    if id.starts_with("folder:") {
        return ui::icon(Icon::Folder, 12.)
            .text_color(tint)
            .into_any_element();
    }
    let path = Path::new(id.strip_prefix("file:").unwrap_or(id));
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    crate::file_icons::icon(&name, cx).map_or_else(
        || {
            ui::icon(Icon::File, 12.)
                .text_color(tint)
                .into_any_element()
        },
        |icon| gpui_kit::img(icon).size(px(12.)).into_any_element(),
    )
}

fn image_tag(number: usize, path: &Path) -> InlineToken {
    InlineToken::new(
        format!("{IMAGE}{}", path.to_string_lossy()),
        format!("[Image #{number}]"),
    )
    .with_label(format!("Image #{number}"))
}

impl Composing {
    /// Adds the files not already going, up to as many as a message carries, and names each
    /// new picture at the caret.
    fn attach(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut tags = Vec::new();
        for path in paths {
            if path.as_os_str().is_empty() || self.attachments.contains(&path) {
                continue;
            }
            if self.attachments.len() == MAX_ATTACHMENTS {
                break;
            }
            self.attachments.push(path);
            let path = &self.attachments[self.attachments.len() - 1];
            if is_image(path) {
                tags.push(image_tag(self.attachments.len(), path));
            }
        }
        if tags.is_empty() {
            return;
        }
        self.draft.update(cx, |draft, cx| {
            let range = draft.selected_range();
            let value = draft.value();
            let spaced_before = value[..range.start]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace);
            let spaced_after = value[range.end..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace);
            if !spaced_before {
                draft.replace(" ", window, cx);
            }
            for (index, tag) in tags.into_iter().enumerate() {
                if index > 0 {
                    draft.replace(" ", window, cx);
                }
                if let Err(error) = draft.replace_with_token(tag, window, cx) {
                    eprintln!("sikemux: a picture's tag did not go into the draft: {error}");
                }
            }
            if !spaced_after {
                draft.replace(" ", window, cx);
            }
        });
    }

    /// Takes attachment `index` off the message with its tags, and counts the later pictures
    /// down so their tags still name them.
    fn detach(&mut self, index: usize, window: &mut Window, cx: &mut App) {
        if index >= self.attachments.len() {
            return;
        }
        let gone = self
            .attachments
            .remove(index)
            .to_string_lossy()
            .into_owned();
        let attachments = &self.attachments;
        self.draft.update(cx, |draft, cx| {
            let spans = draft.tokens().to_vec();
            for span in spans.iter().rev() {
                let Some(id) = span.token().id().strip_prefix(IMAGE) else {
                    continue;
                };
                if id == gone {
                    let range = span.range();
                    let start = if draft.value()[..range.start].ends_with(' ') {
                        range.start - 1
                    } else {
                        range.start
                    };
                    draft.set_selected_range(start..range.end, cx);
                    draft.replace("", window, cx);
                } else if let Some(at) = attachments
                    .iter()
                    .position(|path| path.to_string_lossy() == id)
                    .filter(|at| *at >= index)
                {
                    if let Err(error) = draft.replace_range_with_token(
                        span.range(),
                        image_tag(at + 1, &attachments[at]),
                        window,
                        cx,
                    ) {
                        eprintln!("sikemux: a picture's tag was not renumbered: {error}");
                    }
                }
            }
        });
    }
}

impl Workbench {
    /// Hands the next message of `agent`'s chat files the person chose.
    pub fn attach(
        &mut self,
        agent: &str,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (keep, paths): (Vec<PathBuf>, Vec<PathBuf>) = paths
            .into_iter()
            .partition(|path| is_screenshot_thumbnail(path));
        for path in keep {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let name = path.file_name().map_or_else(
                        || "screenshot.png".to_owned(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    self.keep(agent, name, "image/png", bytes, window, cx);
                }
                Err(error) => eprintln!(
                    "sikemux: the dropped screenshot {} does not read: {error}",
                    path.display()
                ),
            }
        }
        let Some(composing) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.composer.as_mut())
        else {
            return;
        };
        composing.attach(paths, window, cx);
        cx.notify();
    }

    /// Asks the core to keep a file that lives nowhere the agent can read, and attaches it
    /// once the core says where it went.
    fn keep(
        &self,
        agent: &str,
        name: String,
        mime: &str,
        bytes: Vec<u8>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (kept, answer) = oneshot::channel();
        let ask = ChatAsk::Keep {
            agent: agent.to_owned(),
            name,
            mime: mime.to_owned(),
            bytes,
            kept,
        };
        if self.asks.send(Ask::Chat(ask)).is_err() {
            return;
        }
        let agent = agent.to_owned();
        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(Ok(path)) => {
                this.update_in(cx, |bench, window, cx| {
                    bench.attach(&agent, vec![path], window, cx)
                })
                .ok();
            }
            Ok(Err(error)) => eprintln!("sikemux: the core did not keep a pasted file: {error}"),
            Err(_) => {}
        })
        .detach();
    }

    pub fn detach(
        &mut self,
        agent: &str,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(composing) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.composer.as_mut())
        {
            composing.detach(index, window, cx);
            cx.notify();
        }
    }

    /// The pictures and files pasted into the draft of `agent`'s chat; whether there were any.
    fn pasted(
        &mut self,
        agent: &str,
        item: &ClipboardItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut taken = false;
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) => {
                    let stamp = chrono::Local::now().format("pasted-%Y%m%d-%H%M%S%3f");
                    let name = format!("{stamp}.{}", image.format.extension());
                    self.keep(
                        agent,
                        name,
                        image.format.mime_type(),
                        image.bytes.clone(),
                        window,
                        cx,
                    );
                    taken = true;
                }
                ClipboardEntry::ExternalPaths(paths) => {
                    self.attach(agent, paths.paths().to_vec(), window, cx);
                    taken = true;
                }
                ClipboardEntry::String(_) => {}
            }
        }
        taken
    }

    /// Opens the system's picker of files, for the next message of `agent`'s chat.
    pub fn add_files(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        let agent = agent.to_owned();
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await {
                this.update_in(cx, |bench, window, cx| {
                    bench.attach(&agent, paths, window, cx)
                })
                .ok();
            }
        })
        .detach();
    }
}

/// What the draft takes in place of its paste, for pictures and files on the clipboard.
pub fn on_paste(
    agent: &str,
    cx: &mut Context<Workbench>,
) -> impl Fn(&ClipboardItem, &mut Window, &mut App) -> bool + use<> {
    let (agent, this) = (agent.to_owned(), cx.weak_entity());
    move |item: &ClipboardItem, window: &mut Window, cx: &mut App| {
        this.update(cx, |bench, cx| bench.pasted(&agent, item, window, cx))
            .unwrap_or(false)
    }
}

pub fn on_drop(
    agent: &str,
    cx: &mut Context<Workbench>,
) -> impl Fn(&ExternalPaths, &mut Window, &mut App) + use<> {
    let (agent, this) = (agent.to_owned(), cx.weak_entity());
    move |dropped: &ExternalPaths, window: &mut Window, cx: &mut App| {
        this.update(cx, |bench, cx| {
            bench.attach(&agent, dropped.paths().to_vec(), window, cx)
        })
        .ok();
    }
}

/// What shows over a chat while files are dragged over it.
pub fn drop_hint(cx: &App) -> Div {
    let t = Tokens::get(cx).clone();
    div()
        .absolute()
        .inset(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_dashed()
        .border_color(t.border_selected)
        .rounded(theme::RADIUS_4)
        .bg(Hsla {
            a: 0.86,
            ..t.surface_sunken
        })
        .invisible()
        .drag_over::<ExternalPaths>(|style, _, _, _| style.visible())
        .child(
            ui::text(
                "Drop files or folders into this session",
                13.,
                FontWeight::MEDIUM,
            )
            .text_color(t.text_primary),
        )
}

/// A tag in the draft, set on the line like a word of it.
pub fn chip(token: &InlineTokenContext, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let t = Tokens::get(cx).clone();
    let selected = token.is_selected();
    div().h(token.line_height()).flex().items_center().child(
        div()
            .h(px(20.))
            .max_w(token.available_width().min(px(260.)))
            .mx(px(1.))
            .pl(px(4.))
            .pr(px(6.))
            .flex()
            .items_center()
            .gap(px(4.))
            .border_1()
            .border_color(if selected {
                Hsla {
                    a: 0.3,
                    ..t.text_primary
                }
            } else {
                t.line
            })
            .rounded(theme::RADIUS_1)
            .bg(if selected {
                t.surface_chosen
            } else {
                Hsla {
                    a: 0.06,
                    ..t.text_primary
                }
            })
            .text_size(px(12.))
            .text_color(t.text_primary)
            .child(mark(token.token().id(), t.text_tertiary, cx))
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(token.token().label().clone()),
            ),
    )
}

/// The files going with the message, as tiles: a picture's preview or a file's icon, its
/// number, and a way to take it off.
pub fn tiles(
    attachments: &[PathBuf],
    remove: impl Fn(usize, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> Option<impl IntoElement> {
    if attachments.is_empty() {
        return None;
    }
    let t = Tokens::get(cx).clone();
    let tiles = attachments.iter().enumerate().map(|(index, path)| {
        let name: SharedString = path
            .file_name()
            .map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy())
            .into_owned()
            .into();
        let media = if is_image(path) {
            AttachmentMedia::new().src(path.clone())
        } else {
            let icon = crate::file_icons::icon(&name, cx)
                .map(|icon| gpui_kit::img(icon).size(px(28.)).into_any_element());
            AttachmentMedia::new().child(icon.unwrap_or_else(|| {
                ui::icon(Icon::File, 26.)
                    .text_color(t.text_tertiary)
                    .into_any_element()
            }))
        };
        let number = div().size_full().relative().child(
            div()
                .absolute()
                .left(px(3.))
                .bottom(px(3.))
                .px(px(4.))
                .py(px(2.))
                .rounded(theme::RADIUS_1)
                .bg(Hsla {
                    a: 0.72,
                    ..t.surface_sunken
                })
                .child(ui::mono(format!("#{}", index + 1), 9.5, cx).text_color(t.text_primary)),
        );
        let remove = remove.clone();
        Attachment::new()
            .id(SharedString::from(format!("attachment-{index}")))
            .axis(Axis::Vertical)
            .with_size(gpui_kit::component::Size::Size(px(16.6)))
            .tooltip(path.to_string_lossy().into_owned())
            .media(
                media
                    .bg(Hsla {
                        a: 0.48,
                        ..t.surface_sunken
                    })
                    .overlay(number),
            )
            .on_remove(move |_, window, cx| remove(index, window, cx))
    });
    Some(
        div()
            .flex()
            .flex_wrap()
            .gap(px(7.))
            .px(px(6.))
            .pt(px(6.))
            .children(tiles),
    )
}
