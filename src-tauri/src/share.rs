//! Sharing a chat read-only. The core runs the shares; this gives a page
//! watching someone's shared chat the server's share socket, and signs its
//! hello there with the core's key so it watches as this host's account.

use serde::Serialize;
use tauri::State;

use crate::account::HostSignature;
use crate::error::AppResult;
use crate::pty::{core_error, PtyManager};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccess {
    socket: String,
}

#[tauri::command]
pub async fn share_access() -> AppResult<ShareAccess> {
    Ok(ShareAccess {
        socket: sikemux_core::accounts::live::socket_url(
            &sikemux_core::accounts::api_base(),
            "/v1/share",
        ),
    })
}

/// The core's key and its signature over the viewer's text for `nonce`, the
/// challenge the share socket sent.
#[tauri::command]
pub async fn share_viewer_hello(
    manager: State<'_, PtyManager>,
    nonce: String,
) -> AppResult<HostSignature> {
    let signed = manager
        .client()
        .await?
        .call("signShareViewer", serde_json::json!({ "nonce": nonce }))
        .await
        .map_err(core_error)?;
    Ok(serde_json::from_value(signed)?)
}
