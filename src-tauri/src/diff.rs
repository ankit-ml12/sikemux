//! The editor's git gutter: which lines of an open file changed since its
//! baseline. It runs in the app, beside the editor, since every visible editor
//! asks on each keystroke.

use sikemux_core::git::diff::{hunks, DiffHunk};
use tauri::async_runtime::spawn_blocking;

use crate::error::{AppError, AppResult};

#[tauri::command]
pub async fn diff_hunks(baseline: String, current: String) -> AppResult<Vec<DiffHunk>> {
    spawn_blocking(move || hunks(&baseline, &current))
        .await
        .map_err(|e| AppError::Other(format!("diff join: {e}")))
}
