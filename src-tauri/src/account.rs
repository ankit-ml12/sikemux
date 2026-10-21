//! The app's calls to the accounts server as this host, signed with the
//! core's key: the devices and hosts on the account, and joining them. The
//! core signs this host in and out itself.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Method, Response};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sikemux_core::accounts::protocol::{
    Challenge, Device, DeviceList, DeviceRegistration, DeviceRole, JoinTicket,
};
use sikemux_core::client::CoreClient;
use sikemux_core::protocol::{DeviceInfo, RemoteStatus};
use tauri::State;

use sikemux_host::account::{api, http};

use crate::error::{AppError, AppResult};
use crate::pty::{core_error, PtyManager};

/// The core's key and its signature, as the accounts server checks them.
#[derive(Serialize, Deserialize)]
pub(crate) struct HostSignature {
    pub key: String,
    pub signature: String,
}

/// Calls the accounts API as this host, signed with the core's key, which
/// the server takes until the host is taken off the account.
async fn host_request(
    core: &CoreClient,
    method: Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> AppResult<Response> {
    let body = match body {
        Some(body) => serde_json::to_vec(body)?,
        None => Vec::new(),
    };
    let time = sikemux_process::clock::epoch_secs();
    let signed = core
        .call(
            "signHostRequest",
            serde_json::json!({
                "method": method.as_str(),
                "path": path,
                "time": time,
                "bodySha256": hex::encode(Sha256::digest(&body)),
            }),
        )
        .await
        .map_err(core_error)?;
    let HostSignature { key, signature } = serde_json::from_value(signed)?;
    let mut request = http().request(method, api(path)).header(
        AUTHORIZATION,
        format!("SikemuxHost {key}.{time}.{signature}"),
    );
    if !body.is_empty() {
        request = request.header(CONTENT_TYPE, "application/json").body(body);
    }
    let response = request.send().await?;
    if !response.status().is_success() {
        return Err(api_failure(response).await);
    }
    Ok(response)
}

async fn as_host<T: DeserializeOwned>(
    core: &CoreClient,
    method: Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> AppResult<T> {
    Ok(host_request(core, method, path, body).await?.json().await?)
}

/// Approves the sign-in code another host showed, for "Install Sikemux here",
/// and answers with that host.
pub(crate) async fn approve_host_code(
    core: &CoreClient,
    code: &str,
) -> AppResult<sikemux_core::accounts::host::ApprovedHost> {
    as_host(
        core,
        Method::POST,
        &format!("/v1/host-sign-ins/{code}/approve-from-host"),
        None,
    )
    .await
}

/// Renames a device or host on the account, which every device and host
/// shows from then on.
#[tauri::command]
pub async fn account_rename_device(
    manager: State<'_, PtyManager>,
    key: String,
    name: String,
) -> AppResult<()> {
    let core = manager.client().await?;
    host_request(
        &core,
        Method::POST,
        &format!("/v1/devices/{key}/name"),
        Some(&serde_json::json!({ "name": name })),
    )
    .await?;
    Ok(())
}

/// Takes a device or host off the account, which revokes it on every host at
/// once.
#[tauri::command]
pub async fn account_remove_device(manager: State<'_, PtyManager>, key: String) -> AppResult<()> {
    let core = manager.client().await?;
    host_request(&core, Method::DELETE, &format!("/v1/devices/{key}"), None).await?;
    Ok(())
}

/// The core's status, while an account owns this host.
async fn owned(core: &CoreClient) -> AppResult<Option<RemoteStatus>> {
    let remote = core.remote_status().await.map_err(core_error)?;
    Ok((remote.owner.is_some() && !remote.core_id.is_empty()).then_some(remote))
}

/// The phones on the account this host has not paired, newest first, so the
/// person can let them in right after signing in.
#[tauri::command]
pub async fn account_phones(manager: State<'_, PtyManager>) -> AppResult<Vec<Device>> {
    let core = manager.client().await?;
    let Some(remote) = owned(&core).await? else {
        return Ok(Vec::new());
    };
    let list: DeviceList = as_host(&core, Method::GET, "/v1/devices?role=client", None).await?;
    Ok(unpaired_phones(list.devices, &remote.devices))
}

/// Adds this app to the account as a device of its own, added by this host,
/// and asks the account for a ticket that lets it join `host`.
pub(crate) async fn app_join_ticket(
    core: &CoreClient,
    device: &sikemux_core::device::Device,
    host: &str,
) -> AppResult<JoinTicket> {
    let remote = owned(core)
        .await?
        .ok_or_else(|| AppError::Other("sign in to reach your other hosts".into()))?;
    let user_id = remote.owner.unwrap_or_default();
    let hosts: DeviceList = as_host(core, Method::GET, "/v1/devices?role=host", None).await?;
    let own = hosts
        .devices
        .iter()
        .find(|known| known.key == remote.core_id)
        .map_or("this host", |known| known.name.as_str());
    let challenge: Challenge = as_host(core, Method::POST, "/v1/devices/challenge", None).await?;
    let signature = device
        .sign_registration(&challenge.nonce, &user_id)
        .map_err(|reason| AppError::Other(reason.into()))?;
    let registration = DeviceRegistration {
        key: device.id(),
        role: DeviceRole::Client,
        name: format!("Sikemux app on {own}").chars().take(64).collect(),
        platform: sikemux_core::accounts::protocol::Platform::Macos,
        channel: None,
        nonce: challenge.nonce,
        signature,
    };
    let _: Device = as_host(
        core,
        Method::POST,
        "/v1/devices",
        Some(&serde_json::to_value(&registration)?),
    )
    .await?;
    as_host(
        core,
        Method::POST,
        &format!("/v1/devices/{}/join", device.id()),
        Some(&serde_json::json!({ "host": host })),
    )
    .await
}

fn unpaired_phones(devices: Vec<Device>, paired: &[DeviceInfo]) -> Vec<Device> {
    devices
        .into_iter()
        .filter(|device| device.role == DeviceRole::Client)
        .filter(|device| !paired.iter().any(|known| known.id == device.key))
        .collect()
}

async fn api_failure(response: Response) -> AppError {
    AppError::Other(sikemux_core::accounts::refusal(response).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sikemux_core::accounts::protocol::Platform;

    #[test]
    fn only_phones_this_host_has_not_paired_are_offered() {
        let device = |key: &str, role: DeviceRole| Device {
            key: key.into(),
            role,
            name: key.into(),
            platform: Platform::Ios,
            channel: None,
            created_at: "2026-10-08T00:00:00Z".into(),
            last_seen_at: None,
        };
        let paired = DeviceInfo {
            id: "paired".into(),
            name: "Paired".into(),
            platform: "ios".into(),
            access: sikemux_core::protocol::DeviceAccess::Full,
            paired_at: 0,
            last_seen: None,
        };
        let offered = unpaired_phones(
            vec![
                device("new", DeviceRole::Client),
                device("paired", DeviceRole::Client),
                device("host", DeviceRole::Host),
            ],
            &[paired],
        );
        let keys: Vec<_> = offered.iter().map(|device| device.key.as_str()).collect();
        assert_eq!(keys, ["new"]);
    }
}
