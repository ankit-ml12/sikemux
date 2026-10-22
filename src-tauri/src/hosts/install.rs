//! "Install Sikemux here" and "Sign in" on an SSH host: the installer, or
//! `sikemux login` on a host that has Sikemux but is off the account, runs
//! over the person's own SSH in a terminal the app shows, so a password ssh or
//! sudo asks for is typed there and never passes through the app. The sign-in
//! code it prints in that terminal is approved by this host for the account
//! (ADR 0018). Only a code read from that session's own output, on the line
//! login prints it on, is approved.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde::Serialize;
use sikemux_core::protocol::{BuildChannel, SessionId, SpawnTarget, TerminalSpawn};
use sikemux_process::sync::Locked;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};
use crate::pty::{core_error, PtyManager};

use super::Hosts;

/// Long enough to type a password, download the core and sign in.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Long enough to connect and start a core.
const START_TIMEOUT: Duration = Duration::from_secs(90);
/// The letters a sign-in code is made of, which the accounts server chooses.
const CODE_LETTERS: &str = "BCDFGHJKLMNPQRSTVWXZ";
/// What `sikemux login` prints in front of the code.
const CODE_LINE: &str = "enter the code";
/// The longest line kept while it is still being written.
const MAX_LINE: usize = 1024;
const DONE_EVENT: &str = "host_installed";

/// The install terminals whose output is read for the code, by session.
struct Watch {
    line: String,
    found: oneshot::Sender<String>,
}

static WATCHING: AtomicBool = AtomicBool::new(false);
static WATCHES: LazyLock<Mutex<HashMap<SessionId, Watch>>> = LazyLock::new(Mutex::default);

/// Reads an install terminal's output for its sign-in code. Every terminal's
/// output passes here, so anything else returns at once.
pub(crate) fn watch_output(id: SessionId, bytes: &[u8]) {
    if !WATCHING.load(Ordering::Relaxed) {
        return;
    }
    let mut watches = WATCHES.locked();
    let Some(watch) = watches.get_mut(&id) else {
        return;
    };
    let mut code = None;
    for piece in String::from_utf8_lossy(bytes).split_inclusive('\n') {
        watch.line.push_str(piece);
        if watch.line.ends_with('\n') {
            code = code.or_else(|| sign_in_code(&watch.line));
            watch.line.clear();
        } else if watch.line.len() > MAX_LINE {
            watch.line.clear();
        }
    }
    if let Some(code) = code {
        if let Some(watch) = watches.remove(&id) {
            let _ = watch.found.send(code);
        }
    }
    WATCHING.store(!watches.is_empty(), Ordering::Relaxed);
}

/// The install terminal ended; whoever waits on its code hears it never came.
pub(crate) fn ended(id: SessionId) {
    if !WATCHING.load(Ordering::Relaxed) {
        return;
    }
    let mut watches = WATCHES.locked();
    watches.remove(&id);
    WATCHING.store(!watches.is_empty(), Ordering::Relaxed);
}

/// An alias from ~/.ssh/config, never something ssh would read as an option.
pub(super) fn checked_alias(alias: &str) -> AppResult<&str> {
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

/// `sikemux` with `args`, run on an SSH host through its login shell's PATH
/// and where the installer puts it when that PATH does not reach it.
pub(super) fn sikemux_over_ssh(args: &str) -> String {
    format!(r#"exec "$SHELL" -lc 'PATH="$PATH:$HOME/.local/bin" sikemux {args}'"#)
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
    sign_in_over_ssh(app, manager, alias, installer(), Code::Needed, cols, rows).await
}

/// The installer for this app's channel, as an SSH host runs it.
fn installer() -> String {
    let channel = match BuildChannel::of_release(env!("CARGO_PKG_VERSION")) {
        BuildChannel::Stable => "stable",
        _ => "nightly",
    };
    format!(
        "curl -fsSL https://sikemux.com/install | SIKEMUX_CHANNEL={channel} sh -s -- --host --yes"
    )
}

/// Opens a terminal on this host that updates Sikemux on the SSH host
/// `alias`, which runs an older release, with the installer, then restarts
/// its core, and answers with its session for the page to show.
/// `host_installed` says when it is done; a host signed out signs in there.
#[tauri::command]
pub async fn host_update(
    app: AppHandle,
    manager: State<'_, PtyManager>,
    alias: String,
    cols: u16,
    rows: u16,
) -> AppResult<SessionId> {
    let remote = format!("{} && {}", installer(), sikemux_over_ssh("restart"));
    sign_in_over_ssh(app, manager, alias, remote, Code::IfPrinted, cols, rows).await
}

/// Starts the stopped core on the SSH host `alias`, over the person's own SSH
/// and never prompting, as the probe asks.
#[tauri::command]
pub async fn host_start(alias: String) -> AppResult<()> {
    let alias = checked_alias(&alias)?.to_owned();
    let mut ssh = tokio::process::Command::from(sikemux_process::user_environment::command("ssh"));
    ssh.args([
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "ControlMaster=no",
        "-o",
        "ConnectTimeout=10",
        "--",
        &alias,
    ])
    .arg(sikemux_over_ssh("start"))
    .stdin(std::process::Stdio::null())
    .kill_on_drop(true);
    let output = tokio::time::timeout(START_TIMEOUT, ssh.output())
        .await
        .map_err(|_| AppError::Other(format!("{alias} did not start its core in time")))??;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(AppError::Other(if said.is_empty() {
        format!("{alias} could not start its core")
    } else {
        said
    }))
}

/// Whether the run must end in a sign-in code, or may end without one, as an
/// update of a host already signed in does.
#[derive(Clone, Copy)]
enum Code {
    Needed,
    IfPrinted,
}

/// Opens a terminal on this host that runs `sikemux login` on the SSH host
/// `alias`, which already runs Sikemux, and answers with its session for the
/// page to show. `host_installed` says when it is on the account.
#[tauri::command]
pub async fn host_sign_in(
    app: AppHandle,
    manager: State<'_, PtyManager>,
    alias: String,
    cols: u16,
    rows: u16,
) -> AppResult<SessionId> {
    sign_in_over_ssh(
        app,
        manager,
        alias,
        sikemux_over_ssh("login"),
        Code::Needed,
        cols,
        rows,
    )
    .await
}

fn single_quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Runs `remote` on the SSH host `alias` in a terminal the page shows, and
/// approves for the account the sign-in code it prints there.
async fn sign_in_over_ssh(
    app: AppHandle,
    manager: State<'_, PtyManager>,
    alias: String,
    remote: String,
    expected: Code,
    cols: u16,
    rows: u16,
) -> AppResult<SessionId> {
    let alias = checked_alias(&alias)?.to_owned();
    let core = manager.client().await?;
    let id = core
        .spawn(
            crate::pty::commands::launch_identity(&app),
            SpawnTarget::Terminal(TerminalSpawn {
                cols,
                rows,
                cwd: None,
                startup: Some(format!("ssh -t -- {alias} {}", single_quoted(&remote))),
                direct_command: None,
                context: None,
                env: Default::default(),
                continues: None,
            }),
        )
        .await
        .map_err(core_error)?;
    let (found, code) = oneshot::channel();
    WATCHES.locked().insert(
        id,
        Watch {
            line: String::new(),
            found,
        },
    );
    WATCHING.store(true, Ordering::Relaxed);
    let core_for_approval = core.clone();
    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let finished = async {
            let code = match tokio::time::timeout(INSTALL_TIMEOUT, code)
                .await
                .map_err(|_| AppError::Other("installing took too long".into()))?
            {
                Ok(code) => code,
                Err(_) if matches!(expected, Code::IfPrinted) => return AppResult::Ok(()),
                Err(_) => return Err(AppError::Other("it ended without a sign-in code; a host already signed in to your account shows under Hosts as it is".into())),
            };
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
