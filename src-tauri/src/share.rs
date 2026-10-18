//! Sharing a chat read-only. The core runs the shares; this gives a page
//! watching someone's shared chat the server's share socket and a sign-in to
//! watch with.

use serde::Serialize;

use crate::error::AppResult;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAccess {
    socket: String,
    /// None when signed out.
    token: Option<String>,
}

#[tauri::command]
pub async fn share_access() -> AppResult<ShareAccess> {
    Ok(ShareAccess {
        socket: sikemux_core::accounts::live::socket_url(
            &sikemux_core::accounts::api_base(),
            "/v1/share",
        ),
        token: crate::account::access_token().await?,
    })
}
