//! Updating the app this core runs inside, on the person's channel. The core
//! checks the release feeds, downloads the newest build while checking its
//! signature, and puts it in place of the app. Once no chat would lose work it
//! says the update is ready, the windows start again into it, and the new
//! build takes over this core with its agents still running.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use semver::Version;
use serde::{Deserialize, Serialize};
use sikemux_process::clock::epoch_ms;
use sikemux_process::sync::Locked;
use tokio::sync::Notify;

use super::changed::Topic;
use super::connection::blocking;
use super::{Core, CoreError, CoreResult};
use crate::frozen::UpdateBlocker;
use crate::protocol::UpdateChannel;
use crate::release_feed::{self, platform, Newest};
use crate::releases::{bundled_credits, ReleaseCredits};

const FIRST_CHECK: Duration = Duration::from_secs(4);
const CHECK_EVERY: Duration = Duration::from_secs(30 * 60);
const BLOCKERS_EVERY: Duration = Duration::from_secs(2);
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Where an update stands, as every client shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateState {
    #[default]
    Idle,
    /// Downloaded and in place, waiting for the chats in `blockers` to finish
    /// what they are doing.
    Waiting,
    Downloading,
    Installing,
    /// In place; the windows start again into it.
    Ready,
    Failed,
}

/// The newest release on the person's channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRelease {
    pub version: String,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub credits: Option<ReleaseCredits>,
}

/// The last check, kept because a failed one has nowhere else to show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// Unix milliseconds.
    pub at: u64,
    pub channel: UpdateChannel,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// This build updates itself: a release running inside its app.
    pub updates: bool,
    pub current: String,
    pub checked: Option<UpdateCheck>,
    pub available: Option<UpdateRelease>,
    pub state: UpdateState,
    pub error: Option<String>,
    pub blockers: Vec<UpdateBlocker>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    /// The person chose to stop the chats rather than wait, which the window
    /// that starts again tells the next core.
    pub forced: bool,
    /// What to run in a terminal to install `available`, for an app that its
    /// installer updates rather than itself.
    pub install_command: Option<String>,
}

/// Where to fetch the release `available` names.
#[derive(Clone)]
struct Found {
    url: String,
    signature: String,
}

#[derive(Default)]
struct Held {
    status: UpdateStatus,
    found: Option<Found>,
    /// The download is in place of the app already.
    installed: bool,
}

#[derive(Default)]
pub(crate) struct Updates {
    held: std::sync::Mutex<Held>,
    /// Rings for a check out of turn, such as when the person changes channel.
    wake: Notify,
}

/// The app an update replaces.
#[derive(Clone)]
enum OwnApp {
    /// The macOS app bundle this core was started from.
    Bundle(PathBuf),
    /// The AppImage file the Linux app runs from.
    AppImage(PathBuf),
}

/// The app this core runs inside, when it is a published release that can
/// replace itself. A build made anywhere else, such as a local build, never
/// updates, so a release can never take its place.
fn own_app() -> Option<OwnApp> {
    if !crate::usage::Install::App.published() {
        return None;
    }
    if cfg!(target_os = "macos") {
        let core = std::env::current_exe().ok()?;
        return sikemux_process::paths::app_bundle_of_core(&core)
            .map(|bundle| OwnApp::Bundle(bundle.to_path_buf()));
    }
    // The AppImage runtime names the file it runs from, and the app's core inherits it.
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|file| cfg!(target_os = "linux") && file.is_file())
        .map(OwnApp::AppImage)
}

/// A published Linux app that is a deb or an rpm was put in place by the
/// installer, which updates it.
fn installer_command(channel: UpdateChannel) -> Option<String> {
    if !cfg!(target_os = "linux") || !crate::usage::Install::App.published() || own_app().is_some()
    {
        return None;
    }
    let channel = match channel {
        UpdateChannel::Stable => "",
        UpdateChannel::Nightly => "SIKEMUX_CHANNEL=nightly ",
    };
    Some(format!(
        "curl -fsSL https://sikemux.com/install | {channel}sh -s -- --app"
    ))
}

impl Updates {
    pub(crate) fn status(&self) -> UpdateStatus {
        self.held.locked().status.clone()
    }

    fn edit(&self, core: &Core, change: impl FnOnce(&mut Held)) {
        change(&mut self.held.locked());
        core.changes.changed(Topic::Update, None, None);
    }

    /// Checks again now, as after the person changes channel.
    pub(crate) fn check_soon(&self) {
        self.wake.notify_one();
    }
}

/// Checks a little after the core starts and every half hour after, while the
/// core runs in an app that can update.
pub(crate) async fn serve(core: Arc<Core>) {
    let by_installer = installer_command(core.preferences.get().update_channel);
    core.updates.edit(&core, |held| {
        held.status.updates = own_app().is_some();
        held.status.current = core.build.version.clone();
        held.status.install_command = by_installer.clone();
    });
    if !core.updates.status().updates && by_installer.is_none() {
        return;
    }
    tokio::time::sleep(FIRST_CHECK).await;
    loop {
        let _ = check(&core).await;
        let _ = tokio::time::timeout(CHECK_EVERY, core.updates.wake.notified()).await;
    }
}

/// Looks for a release newer than this one on the person's channel. One feed
/// that does not answer must not hide a release the other one offers.
pub(crate) async fn check(core: &Core) -> CoreResult<UpdateStatus> {
    let status = core.updates.status();
    let channel = core.preferences.get().update_channel;
    let by_installer = installer_command(channel);
    if !status.updates && by_installer.is_none() {
        return Err("this build does not update itself".into());
    }
    if matches!(
        status.state,
        UpdateState::Downloading
            | UpdateState::Installing
            | UpdateState::Waiting
            | UpdateState::Ready
    ) {
        return Ok(status);
    }
    let current =
        Version::parse(&status.current).map_err(|error| CoreError::from(error.to_string()))?;
    let (newest, error) = match release_feed::newest(channel).await {
        Ok(newest) => (Some(newest), None),
        Err(error) => (None, Some(error)),
    };
    let release = newest.filter(|newest| newest.version > current).and_then(
        |Newest { feed, raw, .. }| {
            let found = feed.platforms.get(&platform()).map(|asset| Found {
                url: asset.url.clone(),
                signature: asset.signature.clone(),
            });
            if status.updates && found.is_none() {
                return None;
            }
            let credits = bundled_credits(&raw);
            Some((
                UpdateRelease {
                    version: feed.version,
                    notes: feed.notes,
                    date: feed.pub_date,
                    credits,
                },
                found,
            ))
        },
    );
    core.updates.edit(core, |held| {
        held.status.checked = Some(UpdateCheck {
            at: epoch_ms(),
            channel,
            error: error.clone(),
        });
        if error.is_some() {
            return;
        }
        held.status.available = release.as_ref().map(|(release, _)| release.clone());
        held.status.install_command = by_installer;
        held.found = release.and_then(|(_, found)| found);
        held.status.error = None;
        held.status.state = UpdateState::Idle;
    });
    match error {
        Some(error) => Err(CoreError::from(format!("update check: {error}"))),
        None => Ok(core.updates.status()),
    }
}

#[derive(Deserialize)]
pub(crate) struct InstallParams {
    /// Stop the chats an update would wait on rather than wait.
    #[serde(default)]
    force: bool,
}

/// Downloads and puts in place the release `check` found, in the background;
/// clients follow it in the status.
pub(crate) fn install(core: &Arc<Core>, params: InstallParams) -> CoreResult<UpdateStatus> {
    let status = core.updates.status();
    if matches!(
        status.state,
        UpdateState::Downloading | UpdateState::Installing | UpdateState::Ready
    ) {
        return Ok(status);
    }
    let app = own_app().ok_or_else(|| CoreError::from("this build does not update itself"))?;
    let (found, installed) = {
        let held = core.updates.held.locked();
        (held.found.clone(), held.installed)
    };
    let found = found.ok_or_else(|| CoreError::from("there is no update to install"))?;
    core.updates.edit(core, |held| {
        held.status.forced = params.force;
        held.status.error = None;
        held.status.downloaded_bytes = 0;
        held.status.total_bytes = None;
        held.status.state = if installed {
            UpdateState::Waiting
        } else {
            UpdateState::Downloading
        };
    });
    let status = core.updates.status();
    let core = core.clone();
    tokio::spawn(async move {
        if let Err(error) = put_in_place(&core, &app, &found, installed).await {
            core.updates.edit(&core, |held| {
                held.status.state = UpdateState::Failed;
                held.status.error = Some(error);
            });
            return;
        }
        wait_for_chats(&core).await;
    });
    Ok(status)
}

/// Stops waiting on chats; the update stays in place for later.
pub(crate) fn cancel(core: &Core) -> UpdateStatus {
    core.updates.edit(core, |held| {
        if held.status.state == UpdateState::Waiting {
            held.status.state = UpdateState::Idle;
            held.status.blockers.clear();
        }
    });
    core.updates.status()
}

async fn put_in_place(
    core: &Core,
    app: &OwnApp,
    found: &Found,
    installed: bool,
) -> Result<(), String> {
    if installed {
        return Ok(());
    }
    let key = release_feed::release_key()?;
    let archive = match app {
        OwnApp::Bundle(_) => {
            std::env::temp_dir().join(format!("sikemux-update-{}.tar.gz", std::process::id()))
        }
        // Beside the AppImage, so putting it in place is one rename on the same disk.
        OwnApp::AppImage(file) => beside(file, "update")?,
    };
    let mut last = Instant::now();
    let fetched = release_feed::fetch_verified(
        &found.url,
        &found.signature,
        &key,
        &archive,
        None,
        |done, total| {
            if last.elapsed() >= PROGRESS_EVERY || total == Some(done) {
                last = Instant::now();
                core.updates.edit(core, |held| {
                    held.status.downloaded_bytes = done;
                    held.status.total_bytes = total;
                });
            }
        },
    )
    .await;
    if let Err(error) = fetched {
        let _ = std::fs::remove_file(&archive);
        return Err(error);
    }
    core.updates
        .edit(core, |held| held.status.state = UpdateState::Installing);
    let (app, unpack) = (app.clone(), archive.clone());
    let swapped = blocking(move || {
        match &app {
            OwnApp::Bundle(bundle) => swap(bundle, &unpack),
            OwnApp::AppImage(file) => replace_appimage(file, &unpack),
        }
        .map_err(CoreError::from)
    })
    .await;
    let _ = std::fs::remove_file(&archive);
    swapped.map_err(|error| error.to_string())?;
    core.updates.edit(core, |held| held.installed = true);
    Ok(())
}

/// Says the update is ready once no chat would lose work to the restart, or at
/// once when the person chose to stop them.
async fn wait_for_chats(core: &Core) {
    loop {
        let status = core.updates.status();
        if !matches!(status.state, UpdateState::Installing | UpdateState::Waiting) {
            return;
        }
        let blockers: Vec<UpdateBlocker> = if status.forced {
            Vec::new()
        } else {
            core.chats
                .blockers()
                .into_iter()
                .map(|blocker| UpdateBlocker {
                    agent_id: blocker.agent_id,
                    provider: blocker.provider,
                    title: blocker.title,
                    reason: blocker.reason,
                })
                .collect()
        };
        let ready = blockers.is_empty();
        core.updates.edit(core, |held| {
            held.status.blockers = blockers;
            held.status.state = if ready {
                UpdateState::Ready
            } else {
                UpdateState::Waiting
            };
        });
        if ready {
            return;
        }
        tokio::time::sleep(BLOCKERS_EVERY).await;
    }
}

/// `.<name>.<suffix>` in the folder `file` is in.
fn beside(file: &Path, suffix: &str) -> Result<PathBuf, String> {
    let name = file.file_name().ok_or("the app has no name")?;
    Ok(file.with_file_name(format!(".{}.{suffix}", name.to_string_lossy())))
}

/// Puts the AppImage at `download` in place of `file`. The running app keeps
/// the copy it mounted until it starts again from the new one.
fn replace_appimage(file: &Path, download: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let cannot =
        |error: std::io::Error| format!("Sikemux cannot write to {}: {error}", file.display());
    std::fs::set_permissions(download, std::fs::Permissions::from_mode(0o755)).map_err(cannot)?;
    std::fs::rename(download, file).map_err(cannot)
}

/// Puts the app in `archive` in place of `bundle`, keeping the old one until
/// the new one is there, so a failure leaves the app as it was.
pub(crate) fn swap(bundle: &Path, archive: &Path) -> Result<(), String> {
    let parent = bundle.parent().ok_or("the app is not in a folder")?;
    let name = bundle
        .file_name()
        .ok_or("the app has no name")?
        .to_string_lossy()
        .into_owned();
    let cannot =
        |error: std::io::Error| format!("Sikemux cannot write to {}: {error}", parent.display());
    let staging = parent.join(format!(".{name}.update"));
    let old = parent.join(format!(".{name}.old"));
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir_all(&old);
    std::fs::create_dir(&staging).map_err(cannot)?;
    let unpacked = sikemux_process::user_environment::command("/usr/bin/tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&staging)
        .status()
        .map_err(|error| error.to_string())?;
    let new = std::fs::read_dir(&staging)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|extension| extension == "app"));
    let Some(new) = new.filter(|_| unpacked.success()) else {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("the download holds no app".into());
    };
    std::fs::rename(bundle, &old).map_err(cannot)?;
    if let Err(error) = std::fs::rename(&new, bundle) {
        let _ = std::fs::rename(&old, bundle);
        let _ = std::fs::remove_dir_all(&staging);
        return Err(cannot(error));
    }
    let _ = std::fs::remove_dir_all(&old);
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}
