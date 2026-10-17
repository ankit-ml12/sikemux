// The Vercel account signed in here. Its access token lives in the Keychain; the
// file beside it only says whose it is and which team calls use when they name none.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{VercelError, VercelResult};

/// A development build keeps an entry of its own, so it never spends or replaces the
/// installed app's sign-in.
#[cfg(not(test))]
const KEY_SERVICE: &str = if cfg!(debug_assertions) {
    "sikemux-vercel-token-dev"
} else {
    "sikemux-vercel-token"
};
/// Tests keep to an entry of their own, so they never replace or delete a real token.
#[cfg(test)]
const KEY_SERVICE: &str = "sikemux-vercel-token-test";

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// Vercel's user id.
    pub id: String,
    pub username: String,
    pub name: String,
    pub email: String,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VercelConfig {
    #[serde(default)]
    pub account: Option<Account>,
    /// The team calls use when they name none, by id; none means the account's own projects.
    #[serde(default)]
    pub team: Option<String>,
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

pub fn load(data_dir: &Path) -> VercelConfig {
    std::fs::read(config_path(data_dir))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn io_error(error: std::io::Error) -> VercelError {
    VercelError::Transport(format!("saving settings: {error}"))
}

pub fn save(data_dir: &Path, config: &VercelConfig) -> VercelResult<()> {
    std::fs::create_dir_all(data_dir).map_err(io_error)?;
    let path = config_path(data_dir);
    let staged = path.with_extension("json.tmp");
    std::fs::write(&staged, serde_json::to_vec_pretty(config)?).map_err(io_error)?;
    std::fs::rename(&staged, &path).map_err(io_error)
}

fn keychain_error(error: sikemux_keychain::KeychainError) -> VercelError {
    match error {
        sikemux_keychain::KeychainError::Invalid(message) => VercelError::BadArg(message),
        sikemux_keychain::KeychainError::Failed(message) => VercelError::Keychain(message),
    }
}

pub fn keychain_read(account: &Account) -> VercelResult<Option<String>> {
    sikemux_keychain::read(KEY_SERVICE, &account.id).map_err(keychain_error)
}

pub fn keychain_write(account: &Account, token: &str) -> VercelResult<()> {
    sikemux_keychain::write(KEY_SERVICE, &account.id, token).map_err(keychain_error)
}

pub fn keychain_delete(account: &Account) -> VercelResult<()> {
    sikemux_keychain::delete(KEY_SERVICE, &account.id).map_err(keychain_error)
}

/// Runs work that touches the Keychain on a thread meant for blocking.
pub async fn blocking<T: Send + 'static>(
    work: Box<dyn FnOnce() -> VercelResult<T> + Send>,
) -> VercelResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| VercelError::Keychain(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_saved_reads_as_signed_out_and_a_saved_account_reads_back() {
        let dir =
            std::env::temp_dir().join(format!("sikemux-vercel-config-{}", std::process::id()));
        assert_eq!(load(&dir), VercelConfig::default());
        let saved = VercelConfig {
            account: Some(Account {
                id: "u1".into(),
                username: "ankit".into(),
                name: "Ankit".into(),
                email: "a@example.com".into(),
            }),
            team: Some("team_1".into()),
        };
        save(&dir, &saved).expect("saves");
        assert_eq!(load(&dir), saved);
        std::fs::remove_dir_all(dir).ok();
    }
}
