//! Which host on the account an SSH host is, asked once over the person's own
//! SSH when a terminal on it connects, so the host shows as one row under
//! Hosts without installing Sikemux there again (ADR 0018). The question never
//! prompts: with no key the SSH config already offers, or no Sikemux there,
//! nothing changes.

use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult};
use crate::pty::{core_error, PtyManager};

use super::install::{checked_alias, sikemux_over_ssh};
use super::Hosts;

/// Long enough to connect and ask; a host that takes longer is left alone.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    running: bool,
    /// Missing from releases before the probe, and while the core is stopped.
    core_id: Option<String>,
    account: Option<String>,
    /// The host's own name, which it signed in to the account under.
    name: Option<String>,
}

/// What the SSH host's Sikemux is, beside this host's account.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Found {
    Same,
    SignedOut,
    Other,
    Stopped,
    /// A release too old to say which host it is.
    Outdated,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probed {
    /// The host's key, which the account knows it by, when it says.
    key: Option<String>,
    found: Found,
}

/// Asks the SSH host `alias` whether it runs Sikemux, and which host it is,
/// and makes it one row with that host when the account has it. A host still
/// under the name it signed in with takes the alias, the person's name for it.
#[tauri::command]
pub async fn host_probe(
    app: AppHandle,
    manager: State<'_, PtyManager>,
    alias: String,
) -> AppResult<Option<Probed>> {
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
        &sikemux_over_ssh("status --json"),
    ])
    .stdin(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true);
    let output = tokio::time::timeout(PROBE_TIMEOUT, ssh.output())
        .await
        .map_err(|_| AppError::Other(format!("{alias} did not answer in time")))??;
    // Any status that reads means Sikemux is there, running or not.
    let Ok(status) = serde_json::from_slice::<Status>(&output.stdout) else {
        return Ok(None);
    };
    if !status.running {
        return Ok(Some(Probed {
            key: status.core_id,
            found: Found::Stopped,
        }));
    }
    let Status {
        core_id: Some(key),
        account,
        name,
        ..
    } = status
    else {
        return Ok(Some(Probed {
            key: None,
            found: Found::Outdated,
        }));
    };
    let core = manager.client().await?;
    let own = core.remote_status().await.map_err(core_error)?.owner;
    let found = match account {
        None => Found::SignedOut,
        Some(account) if Some(&account) == own.as_ref() => Found::Same,
        Some(_) => Found::Other,
    };
    if let Some(hosts) = app.try_state::<Hosts>() {
        if let Ok(host) = hosts.by_id(&key) {
            hosts.reached_by(&key, &alias);
            if name.is_some_and(|name| name == host.name() && name != alias) {
                crate::account::rename_device(&core, &key, &alias).await?;
            }
        }
    }
    Ok(Some(Probed {
        key: Some(key),
        found,
    }))
}
