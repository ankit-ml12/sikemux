//! A picture shown properly: over the whole window, as large as the window lets it
//! be, with a way to keep a copy in Downloads.

use std::path::PathBuf;

use gpui_kit::component::WindowExt;
use gpui_kit::{
    div, img, Context, ObjectFit, ParentElement, SharedString, Styled, StyledImage, Window,
};
use serde_json::json;

use crate::icons::Icon;
use crate::ui::{Button, Variant};
use crate::workbench::Workbench;

impl Workbench {
    /// Shows the picture at `path` over the window until Esc, a click outside it, or ✕.
    pub fn show_picture(&self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let name: SharedString = path
            .file_name()
            .map_or_else(|| path.to_string_lossy(), |name| name.to_string_lossy())
            .into_owned()
            .into();
        let this = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, _| {
            let room = window.viewport_size();
            let save = {
                let (this, path) = (this.clone(), path.clone());
                Button::new("picture-save", "Save to Downloads", Variant::Outline)
                    .icon(Icon::Download)
                    .on_click(move |_, window, cx| {
                        this.update(cx, |bench, cx| bench.keep_picture(path.clone(), window, cx))
                            .ok();
                    })
            };
            dialog
                .w(room.width * 0.9)
                .max_w(room.width * 0.9)
                .margin_top(room.height * 0.04)
                .title(name.clone())
                .child(
                    div().flex().justify_center().child(
                        img(path.clone())
                            .max_w_full()
                            .max_h(room.height * 0.78)
                            .object_fit(ObjectFit::Contain),
                    ),
                )
                .footer(div().flex().justify_end().child(save))
        });
    }

    /// Copies the picture into the Downloads folder of the host the core runs on.
    fn keep_picture(&self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(downloads) =
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Downloads"))
        else {
            return;
        };
        let asked = json!({ "from": path, "dir": downloads });
        let kept = crate::core::request(&self.asks, "files.copyInto", asked);
        cx.spawn_in(window, async move |_, cx| {
            let said = match kept.await {
                Ok(saved) => {
                    let name = saved
                        .as_str()
                        .and_then(|saved| saved.rsplit('/').next())
                        .unwrap_or("the picture")
                        .to_owned();
                    format!("Saved {name} to Downloads")
                }
                Err(error) => format!("Could not save the picture: {error}"),
            };
            cx.update(|window, cx| window.push_notification(said, cx))
                .ok();
        })
        .detach();
    }
}
