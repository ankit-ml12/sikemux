//! The page's calls to the core's methods. The core decides who may make
//! each, so the app passes every one on unchanged.

use serde_json::Value;
use sikemux_core::client::{CoreClient, Reply};
use sikemux_core::protocol::{Request, Response};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::pty::{core_error, PtyManager};

#[tauri::command]
pub async fn core_call(
    manager: State<'_, PtyManager>,
    method: String,
    params: Value,
) -> AppResult<Value> {
    let client = manager.client().await?;
    call(&client, method, params).await
}

pub(crate) async fn call(client: &CoreClient, method: String, params: Value) -> AppResult<Value> {
    let reply = client
        .submit(Request::Call { method, params }, |reply| reply)
        .map_err(core_error)?
        .await
        .map_err(core_error)?
        .map_err(core_error)?;
    match reply {
        Reply::Response(Response::Called { result }) => Ok(result),
        _ => Err(AppError::Other(
            "the core answered a call with something else".into(),
        )),
    }
}
