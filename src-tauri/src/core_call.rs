//! The page's requests to the core. The core decides who may make each, so
//! the app passes every one on unchanged.

use serde_json::Value;
use sikemux_core::protocol::{Request, Response};
use tauri::State;

use crate::error::AppResult;
use crate::pty::{core_error, PtyManager};

#[tauri::command]
pub async fn core_call(
    manager: State<'_, PtyManager>,
    method: String,
    params: Value,
) -> AppResult<Value> {
    let client = manager.client().await?;
    client.call(&method, params).await.map_err(core_error)
}

/// One of the core's requests as is, for the page to reach a request no app
/// command has its own work for.
#[tauri::command]
pub async fn core_request(
    manager: State<'_, PtyManager>,
    request: Request,
) -> AppResult<Response> {
    let client = manager.client().await?;
    client.request(request).await.map_err(core_error)
}
