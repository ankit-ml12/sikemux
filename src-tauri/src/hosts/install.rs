//! "Install Sikemux here" on an SSH host: the installer runs over the person's
//! own SSH, then `sikemux login` there prints a sign-in code, which this host
//! approves for the account (ADR 0018). Only a code read from that session's
//! own output is approved.

use std::process::Stdio;
use std::time::Duration;

use sikemux_core::protocol::BuildChannel;
use tauri::{AppHandle, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::pty::PtyManager;

use super::Hosts;

const INSTALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(3 * 60);
/// The letters a sign-in code is made of, which the accounts server chooses.
const CODE_LETTERS: &str = "BCDFGHJKLMNPQRSTVWXZ";
/// What `sikemux login` prints in front of the code.
const CODE_LINE: &str = "enter the code";
/// Where the installer puts `sikemux`, as root and as anyone else.
const REMOTE_PATH: &str = "PATH=\"$HOME/.local/bin:/usr/local/bin:$PATH\"";

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

fn ssh(alias: &str, remote: &str) -> Command {
    let mut command = Command::from(sikemux_process::user_environment::command("ssh"));
    command
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=20",
            "--",
            alias,
            remote,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
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

fn failed(step: &str, output: &[u8]) -> AppError {
    let said = String::from_utf8_lossy(output);
    let last = said
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    AppError::Other(format!("{step} failed: {}", last.trim()))
}

async fn install(alias: &str) -> AppResult<()> {
    let channel = BuildChannel::of_release(env!("CARGO_PKG_VERSION"));
    let channel = if channel == BuildChannel::Stable {
        "stable"
    } else {
        "nightly"
    };
    let remote = format!(
        "curl -fsSL https://sikemux.com/install | SIKEMUX_CHANNEL={channel} sh -s -- --host --yes"
    );
    let run = tokio::time::timeout(INSTALL_TIMEOUT, ssh(alias, &remote).output())
        .await
        .map_err(|_| AppError::Other("installing took too long".into()))??;
    if run.status.success() {
        Ok(())
    } else {
        Err(failed(
            "Installing",
            if run.stderr.is_empty() {
                &run.stdout
            } else {
                &run.stderr
            },
        ))
    }
}

/// Signs the host in, approving the code its own `sikemux login` printed,
/// and answers with its key on the account, or none when it was signed in
/// already.
async fn sign_in(manager: &PtyManager, alias: &str) -> AppResult<Option<String>> {
    let mut login = ssh(alias, &format!("{REMOTE_PATH} NO_COLOR=1 sikemux login"))
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = login
        .stdout
        .take()
        .ok_or_else(|| AppError::Other("could not read the host's sign-in".into()))?;
    let mut lines = BufReader::new(stdout).lines();
    let mut key = None;
    let signed = tokio::time::timeout(LOGIN_TIMEOUT, async {
        while let Some(line) = lines.next_line().await? {
            if key.is_some() {
                continue;
            }
            if let Some(code) = sign_in_code(&line) {
                let core = manager.client().await?;
                key = Some(crate::account::approve_host_code(&core, &code).await?.key);
            }
        }
        login.wait().await.map_err(AppError::from)
    })
    .await
    .map_err(|_| AppError::Other("signing in took too long".into()))??;
    if !signed.success() {
        return Err(AppError::Other("the host could not sign in".into()));
    }
    Ok(key)
}

/// Installs the core on the SSH host `alias` and signs it in to the account,
/// which links it with every other host and shows it in the app.
#[tauri::command]
pub async fn host_install(
    app: AppHandle,
    hosts: State<'_, Hosts>,
    manager: State<'_, PtyManager>,
    alias: String,
) -> AppResult<()> {
    let alias = checked_alias(&alias)?;
    install(alias).await?;
    if let Some(key) = sign_in(&manager, alias).await? {
        hosts.reached_by(&key, alias);
    }
    super::follow_links(&app);
    Ok(())
}
