use std::fs;
use std::path::PathBuf;

use tauri::async_runtime::spawn_blocking;

use crate::error::{AppError, AppResult};

fn config_path() -> AppResult<PathBuf> {
    let home = std::env::var("HOME").map_err(|_| AppError::Fs("HOME is not set".into()))?;
    Ok(PathBuf::from(home).join(".ssh/config"))
}

/// Ensure the SSH editor has a real file to open, without ever modifying an
/// existing config. Returning the canonical app path keeps the frontend's
/// regular file editor independent of platform-specific home discovery.
#[tauri::command]
pub async fn ssh_config_ensure() -> AppResult<String> {
    spawn_blocking(ensure_ssh_config)
        .await
        .map_err(|error| AppError::Fs(format!("ssh_config_ensure join: {error}")))?
}

fn ensure_ssh_config() -> AppResult<String> {
    let path = config_path()?;
    let ssh_dir = path
        .parent()
        .ok_or_else(|| AppError::Fs("invalid SSH config path".into()))?;
    if !ssh_dir.exists() {
        fs::create_dir_all(ssh_dir)?;
        #[cfg(unix)]
        fs::set_permissions(ssh_dir, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    }
    if !path.exists() {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path)?;
    }
    Ok(path.to_string_lossy().into_owned())
}
