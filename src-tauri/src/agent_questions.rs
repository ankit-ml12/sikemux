//! Questions agents put to the person. The core keeps them and tells the
//! window when they change, and this hands them to the page.

use serde_json::Value;
use tauri::{AppHandle, Emitter};

/// The window call the core sends the open questions in.
pub(crate) const CHANGED_METHOD: &str = "agent.questions";

const CHANGED_EVENT: &str = "agent_questions";

/// Hands the page the open questions the core sent.
pub(crate) fn changed(app: &AppHandle, params: &Value) -> Result<Value, String> {
    app.emit_to("main", CHANGED_EVENT, &params["questions"])
        .map_err(|error| error.to_string())?;
    Ok(Value::Null)
}
