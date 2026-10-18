//! The login Keychain, through Security.framework. An item saved here trusts
//! the code that saved it by its designated requirement, which the app and
//! the core share and every update keeps, so no other program can read it
//! without the person's say. Any program the person runs can still replace
//! or delete an item.

use std::sync::{Mutex, PoisonError};

use security_framework::base::Error;
use security_framework::os::macos::keychain::SecKeychain;
use security_framework::passwords::{self, PasswordOptions};

use crate::{KeychainError, KeychainResult};

const NOT_FOUND: i32 = -25300;
/// What the Keychain answers, instead of asking the person, for an item that
/// does not trust this code.
const NOT_TRUSTED: [i32; 2] = [-25308, -25293];

fn failed(error: Error) -> KeychainError {
    KeychainError::Failed(format!("the keychain would not do it: {error}"))
}

/// Runs `work` with the Keychain refusing rather than asking the person,
/// which is process-wide, so one call at a time.
fn quietly<T>(work: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    static TURN: Mutex<()> = Mutex::new(());
    let _turn = TURN.lock().unwrap_or_else(PoisonError::into_inner);
    let _quiet = SecKeychain::disable_user_interaction()?;
    work()
}

fn text(bytes: &[u8]) -> Option<String> {
    let secret = String::from_utf8_lossy(bytes).trim().to_owned();
    (!secret.is_empty()).then_some(secret)
}

pub(crate) fn read(service: &str, account: &str) -> KeychainResult<Option<String>> {
    let read = quietly(|| {
        passwords::generic_password(PasswordOptions::new_generic_password(service, account))
    });
    match read {
        Ok(bytes) => Ok(text(&bytes)),
        Err(error) if error.code() == NOT_FOUND => Ok(None),
        Err(error) if NOT_TRUSTED.contains(&error.code()) => legacy::adopt(service, account),
        Err(error) => Err(failed(error)),
    }
}

/// Replacing an item would keep whatever it trusted before, so it is made
/// again.
pub(crate) fn write(service: &str, account: &str, secret: &str) -> KeychainResult<()> {
    delete(service, account)?;
    quietly(|| passwords::set_generic_password(service, account, secret.as_bytes()))
        .map_err(failed)
}

pub(crate) fn delete(service: &str, account: &str) -> KeychainResult<()> {
    match quietly(|| passwords::delete_generic_password(service, account)) {
        Err(error) if error.code() != NOT_FOUND => Err(failed(error)),
        _ => Ok(()),
    }
}

/// Builds before 0.5.3 saved through the `security` tool, so their items
/// trust that tool, and with it any program that runs it. Each is made again
/// as Sikemux's own the first time it is read.
mod legacy {
    use std::time::Duration;

    use crate::{KeychainError, KeychainResult};

    const TIMEOUT: Duration = Duration::from_secs(10);
    const OUTPUT_LIMIT: usize = 64 * 1024;
    /// What `security` exits with when there is no such entry.
    const NOT_FOUND: i32 = 44;

    pub(super) fn adopt(service: &str, account: &str) -> KeychainResult<Option<String>> {
        let output = sikemux_process::run(
            sikemux_process::user_environment::command("/usr/bin/security").args([
                "find-generic-password",
                "-s",
                service,
                "-a",
                account,
                "-w",
            ]),
            None,
            TIMEOUT,
            OUTPUT_LIMIT,
            None,
        )
        .map_err(|error| KeychainError::Failed(error.to_string()))?;
        if output.status.code() == Some(NOT_FOUND) {
            return Ok(None);
        }
        if !output.status.success() {
            return Err(KeychainError::Failed(format!(
                "could not read the saved token: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let Some(secret) = super::text(&output.stdout) else {
            return Ok(None);
        };
        super::write(service, account, &secret)?;
        Ok(Some(secret))
    }
}
