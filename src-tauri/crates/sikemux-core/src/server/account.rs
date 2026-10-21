//! The Sikemux account this host is signed in to, as the app keeps it: who
//! signed in, in the keychain, and their name, email and picture, in the data
//! folder. The core sends clients what they show of it, and reads it again
//! whenever the app changes that folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use notify::{RecursiveMode, Watcher};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sikemux_process::sync::Locked;

use crate::protocol::AccountInfo;

use super::Core;

const KEY_SERVICE: &str = "sikemux-account";
const FOLDER: &str = "account";
const PROFILE: &str = "profile.json";
const PICTURE_PREFIX: &str = "picture-";
const MAX_PICTURE_BYTES: u64 = 1024 * 1024;

/// Dev builds keep their own account entry, as they run their own core.
fn key_account() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "default"
    }
}

#[derive(Deserialize)]
struct Saved {
    user_id: String,
    email: Option<String>,
}

#[derive(Deserialize)]
struct Profile {
    user_id: String,
    name: Option<String>,
    email: Option<String>,
    picture: Option<String>,
}

/// What names the picture the app cached for `url`.
fn picture_hash(url: &str) -> String {
    hex::encode(Sha256::digest(url.as_bytes()).get(..16).unwrap_or_default())
}

/// Who is signed in, as clients show them; none while nobody is.
fn read(data_dir: &Path) -> Option<AccountInfo> {
    let text = sikemux_keychain::read(KEY_SERVICE, key_account()).ok()??;
    let saved: Saved = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(text.trim()).ok()?).ok()?;
    let folder = data_dir.join(FOLDER);
    let profile = fs::read(folder.join(PROFILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Profile>(&bytes).ok())
        .filter(|profile| profile.user_id == saved.user_id);
    let picture_hash = profile
        .as_ref()
        .and_then(|profile| profile.picture.as_deref())
        .map(picture_hash)
        .filter(|hash| folder.join(format!("{PICTURE_PREFIX}{hash}")).is_file());
    let (name, email) = match profile {
        Some(profile) => (profile.name, profile.email.or(saved.email)),
        None => (None, saved.email),
    };
    Some(AccountInfo {
        name,
        email,
        picture_hash,
    })
}

/// The account's picture as the app cached it, base64-encoded, while
/// `hash` still names it.
pub(crate) fn picture(core: &Core, hash: &str) -> Option<String> {
    if !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let path = core
        .workspaces
        .data_dir()?
        .join(FOLDER)
        .join(format!("{PICTURE_PREFIX}{hash}"));
    if fs::metadata(&path).ok()?.len() > MAX_PICTURE_BYTES {
        return None;
    }
    Some(STANDARD.encode(fs::read(path).ok()?))
}

fn refresh(core: &Core, data_dir: &Path) {
    let now = read(data_dir);
    let changed = {
        let mut account = core.account.locked();
        let changed = *account != now;
        *account = now;
        changed
    };
    if changed {
        core.view_changed();
    }
}

/// Reads the account now, and again whenever the app changes its folder.
pub(crate) fn watch(core: &Arc<Core>, data_dir: PathBuf) {
    let folder = data_dir.join(FOLDER);
    if fs::create_dir_all(&folder).is_err() {
        return;
    }
    let weak: Weak<Core> = Arc::downgrade(core);
    let watched = data_dir.clone();
    std::thread::spawn(move || {
        if let Some(core) = weak.upgrade() {
            refresh(&core, &watched);
        }
    });
    let weak: Weak<Core> = Arc::downgrade(core);
    core.account_watch.get_or_init(|| {
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let (true, Some(core)) = (event.is_ok(), weak.upgrade()) {
                    refresh(&core, &data_dir);
                }
            })
            .ok()?;
        watcher.watch(&folder, RecursiveMode::NonRecursive).ok()?;
        Some(watcher)
    });
}
