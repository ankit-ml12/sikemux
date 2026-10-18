//! The agents the app holds, which the core keeps names for. The app tells a
//! core it reconnects to again.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use sikemux_core::client::CoreClient;
use tauri::{AppHandle, Manager, State};

use crate::core_call::call;
use crate::error::AppResult;
use crate::pty::PtyManager;

/// Tells the core every agent the app holds.
const IN_APP: &str = "agentsInApp";

/// The agents the app last said it holds, said again to a core it reconnects to.
#[derive(Default)]
pub struct AgentsInApp(Mutex<Option<Value>>);

#[tauri::command]
pub async fn agent_name_call(
    manager: State<'_, PtyManager>,
    in_app: State<'_, AgentsInApp>,
    method: String,
    params: Value,
) -> AppResult<Value> {
    if method == IN_APP {
        if let Ok(mut last) = in_app.0.lock() {
            *last = Some(params.clone());
        }
    }
    let client = manager.client().await?;
    call(&client, method, params).await
}

/// Gives a core the app just connected to the agents the app holds.
pub(crate) async fn connected(app: &AppHandle, client: &Arc<CoreClient>) {
    let last = app
        .try_state::<AgentsInApp>()
        .and_then(|in_app| in_app.0.lock().ok().and_then(|last| last.clone()));
    if let Some(params) = last {
        if let Err(error) = call(client, IN_APP.to_owned(), params).await {
            eprintln!("Sikemux could not tell its core which agents it holds: {error}");
        }
    }
}
