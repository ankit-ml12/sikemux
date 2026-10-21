//! A link in a message: a file the agent named shows in Finder, and anything else opens
//! where the system opens it.

use std::path::PathBuf;

use gpui_kit::App;
use url::Url;

/// The file a link names, when it names one on this host.
pub fn local_path(href: &str) -> Option<PathBuf> {
    if href.starts_with('/') {
        return Some(PathBuf::from(href));
    }
    let url = Url::parse(href).ok().filter(|url| url.scheme() == "file")?;
    url.to_file_path().ok()
}

pub fn open(href: &str, cx: &mut App) {
    match local_path(href) {
        Some(path) => cx.reveal_path(&path),
        None => cx.open_url(href),
    }
}
