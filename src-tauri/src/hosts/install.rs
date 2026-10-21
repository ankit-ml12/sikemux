//! "Install Sikemux here" on an SSH host: the installer runs over the person's
//! own SSH in a terminal the app shows, so a password sudo asks for is typed
//! there and never passes through the app. The installer then signs the host
//! in, and the sign-in code it prints in that terminal is approved by this
//! host for the account (ADR 0018). Only a code read from that session's own
//! screen, on the line login prints it on, is approved.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use serde::Serialize;
use sikemux_core::client::CoreClient;
use sikemux_core::protocol::{BuildChannel, SessionId, SpawnTarget, TerminalSpawn};
use sikemux_process::sync::Locked;
use sikemux_pty::launch::PtyDirectCommand;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Notify;

use crate::error::{AppError, AppResult};
use crate::pty::{core_error, PtyManager};

use super::Hosts;

/// Long enough to type a password, download the core and sign in.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// The letters a sign-in code is made of, which the accounts server chooses.
const CODE_LETTERS: &str = "BCDFGHJKLMNPQRSTVWXZ";
/// What `sikemux login` prints in front of the code.
const CODE_LINE: &str = "enter the code";
const DONE_EVENT: &str = "host_installed";

/// The install terminals whose screens are read for the code, by session.
struct Watch {
    changed: Arc<Notify>,
    ended: bool,
}

static WATCHING: AtomicBool = AtomicBool::new(false);
static WATCHES: LazyLock<Mutex<HashMap<SessionId, Watch>>> = LazyLock::new(Mutex::default);

/// A terminal drew something, so an install terminal's screen is read again.
/// Every terminal's frames pass here, so without one it returns at once.
pub(crate) fn screen_changed() {
    if !WATCHING.load(Ordering::Relaxed) {
        return;
    }
    for watch in WATCHES.locked().values() {
        watch.changed.notify_one();
    }
}

/// The install terminal ended; whoever waits on its code hears it never came.
pub(crate) fn ended(id: SessionId) {
    if !WATCHING.load(Ordering::Relaxed) {
        return;
    }
    if let Some(watch) = WATCHES.locked().get_mut(&id) {
        watch.ended = true;
        watch.changed.notify_one();
    }
}

fn unwatch(id: SessionId) {
    let mut watches = WATCHES.locked();
    watches.remove(&id);
    WATCHING.store(!watches.is_empty(), Ordering::Relaxed);
}

/// The sign-in code once the session `id` shows it, or None when it ends first.
async fn code_shown(core: &CoreClient, id: SessionId, changed: &Notify) -> Option<String> {
    loop {
        changed.notified().await;
        let text = core.text(id, None).await.ok()?;
        if let Some(code) = text.lines().find_map(sign_in_code) {
            return Some(code);
        }
        if WATCHES.locked().get(&id).is_none_or(|watch| watch.ended) {
            return None;
        }
    }
}

/// An alias from ~/.ssh/config, never something ssh would read as an option.
fn checked_alias(alias: &str) -> AppResult<&str> {
    let plain = !alias.is_empty()
        && !alias.starts_with('-')
        && alias
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'));
    if plain {
        Ok(alias)
    } else {
        Err(AppError::BadArg(
            "that is not an SSH host from ~/.ssh/config",
        ))
    }
}

/// The sign-in code in a line `sikemux login` printed, if it is that line.
fn sign_in_code(line: &str) -> Option<String> {
    if !line.contains(CODE_LINE) {
        return None;
    }
    line.split(|c: char| !c.is_ascii_uppercase() && c != '-')
        .find(|word| {
            word.len() == 9
                && word.char_indices().all(|(at, c)| {
                    if at == 4 {
                        c == '-'
                    } else {
                        CODE_LETTERS.contains(c)
                    }
                })
        })
        .map(str::to_owned)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Installed {
    alias: String,
    /// Why it did not finish, when it did not.
    problem: Option<String>,
}

/// Opens a terminal on this host that runs the installer on the SSH host
/// `alias` and signs it in to the account, and answers with its session for
/// the page to show. `host_installed` says when it is on the account.
#[tauri::command]
pub async fn host_install(
    app: AppHandle,
    manager: State<'_, PtyManager>,
    alias: String,
    cols: u16,
    rows: u16,
) -> AppResult<SessionId> {
    let alias = checked_alias(&alias)?.to_owned();
    let channel = match BuildChannel::of_release(env!("CARGO_PKG_VERSION")) {
        BuildChannel::Stable => "stable",
        _ => "nightly",
    };
    let remote = format!(
        "curl -fsSL https://sikemux.com/install | NO_COLOR=1 SIKEMUX_CHANNEL={channel} sh -s -- --host --yes"
    );
    let core = manager.client().await?;
    let id = core
        .spawn(
            crate::pty::commands::launch_identity(&app),
            SpawnTarget::Terminal(TerminalSpawn {
                cols,
                rows,
                cwd: None,
                startup: None,
                direct_command: Some(PtyDirectCommand {
                    program: "ssh".into(),
                    args: vec!["-t".into(), "--".into(), alias.clone(), remote],
                    profile: None,
                }),
                context: None,
                env: Default::default(),
                continues: None,
            }),
        )
        .await
        .map_err(core_error)?;
    let changed = Arc::new(Notify::new());
    WATCHES.locked().insert(
        id,
        Watch {
            changed: changed.clone(),
            ended: false,
        },
    );
    WATCHING.store(true, Ordering::Relaxed);
    let core_for_approval = core.clone();
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let finished = async {
            let code = tokio::time::timeout(
                INSTALL_TIMEOUT,
                code_shown(&core_for_approval, id, &changed),
            )
            .await;
            unwatch(id);
            let code = code
                .map_err(|_| AppError::Other("installing took too long".into()))?
                .ok_or_else(|| {
                    AppError::Other("the installer ended without a sign-in code; a host already signed in to your account shows under Hosts as it is".into())
                })?;
            let host = crate::account::approve_host_code(&core_for_approval, &code).await?;
            if let Some(hosts) = app_for_task.try_state::<Hosts>() {
                hosts.reached_by(&host.key, &alias);
            }
            super::follow_links(&app_for_task);
            AppResult::Ok(())
        };
        let problem = finished.await.err().map(|error| error.to_string());
        let _ = app_for_task.emit_to("main", DONE_EVENT, Installed { alias, problem });
    });
    Ok(id)
}
