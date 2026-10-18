//! Each project's board of cards. The core keeps it and tells the window when
//! it changes, and this hands the boards to the page.

use serde_json::Value;
use tauri::{AppHandle, Emitter};

/// The window call the core sends changed boards in.
pub(crate) const CHANGED_METHOD: &str = "agent.board";

const CHANGED_EVENT: &str = "agent_board";

/// Hands the page the boards the core sent.
pub(crate) fn changed(app: &AppHandle, params: &Value) -> Result<Value, String> {
    app.emit_to("main", CHANGED_EVENT, &params["boards"])
        .map_err(|error| error.to_string())?;
    Ok(Value::Null)
}
