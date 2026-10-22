//! People's pictures on the code host, which its plugin fetches since the window may not
//! reach the host itself. The latest few hundred are kept; the rest are fetched again.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use gpui_kit::{img, px, AnyElement, App, Context, Image, IntoElement, Styled};
use serde_json::json;

use super::HostArea;
use crate::ui;

const KEPT: usize = 400;

enum Kept {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

#[derive(Default)]
pub struct Pictures {
    kept: RefCell<HashMap<String, Kept>>,
    order: RefCell<VecDeque<String>>,
    /// Addresses drawn without a picture yet, fetched once the frame is drawn.
    wanted: RefCell<Vec<String>>,
}

impl Pictures {
    fn get(&self, url: &str) -> Option<Arc<Image>> {
        match self.kept.borrow().get(url) {
            Some(Kept::Ready(image)) => return Some(image.clone()),
            Some(Kept::Loading | Kept::Failed) => return None,
            None => {}
        }
        self.wanted.borrow_mut().push(url.to_owned());
        None
    }

    fn keep(&self, url: String, kept: Kept) {
        let mut order = self.order.borrow_mut();
        let mut all = self.kept.borrow_mut();
        if all.insert(url.clone(), kept).is_none() {
            order.push_back(url);
        }
        while order.len() > KEPT {
            if let Some(oldest) = order.pop_front() {
                all.remove(&oldest);
            }
        }
    }
}

/// The picture a `data:` address holds.
fn decoded(address: &str) -> Option<Arc<Image>> {
    let (mime, data) = address.strip_prefix("data:")?.split_once(";base64,")?;
    crate::picture::decode(mime, data)
}

impl HostArea {
    /// Someone's picture at `url`, or their initials until it arrives or when they have none.
    pub(super) fn face(
        &self,
        login: Option<&str>,
        url: Option<&str>,
        size: f32,
        cx: &App,
    ) -> AnyElement {
        match url.and_then(|url| self.pictures.get(url)) {
            Some(image) => img(image)
                .size(px(size))
                .flex_none()
                .rounded_full()
                .into_any_element(),
            None => ui::initials(login.unwrap_or("someone"), "", size, cx).into_any_element(),
        }
    }

    /// The signed-in account's picture.
    pub(super) fn avatar(&self) -> Option<&str> {
        let status = self.status.as_ref()?.as_ref().ok()?;
        status.avatar_url.as_deref()
    }

    /// Fetches the pictures the last frame drew without.
    pub(super) fn fetch_pictures(&self, cx: &mut Context<Self>) {
        let wanted: Vec<String> = self.pictures.wanted.borrow_mut().drain(..).collect();
        for url in wanted {
            if self.pictures.kept.borrow().contains_key(&url) {
                continue;
            }
            self.pictures.keep(url.clone(), Kept::Loading);
            let fetched = self.plugin("image", json!({ "url": url }));
            cx.spawn(async move |area, cx| {
                let image = fetched
                    .await
                    .ok()
                    .and_then(|address| decoded(address.as_str()?));
                area.update(cx, |area, cx| {
                    let ready = image.is_some();
                    area.pictures
                        .keep(url, image.map_or(Kept::Failed, Kept::Ready));
                    if ready {
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
    }
}
