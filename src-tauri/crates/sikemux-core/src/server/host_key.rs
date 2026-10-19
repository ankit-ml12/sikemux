//! The host's key, and the digest of the devices, links and account it
//! trusts. They live in the person's secret store, out of reach of the agents
//! that can edit `<socket>.remote.json`, so a file this host did not write is
//! caught when the core starts.

use std::path::{Path, PathBuf};

use iroh::SecretKey;

use super::{CoreError, CoreResult};

const SERVICE: &str = if cfg!(debug_assertions) {
    "sikemux-host-dev"
} else {
    "sikemux-host"
};

#[derive(Clone)]
pub(super) struct HostKey {
    pub secret: SecretKey,
    pub trusted: String,
}

pub(super) enum Keeper {
    /// The secret store's entry, named for the socket so each core has its own.
    Store { account: String },
    /// A private file beside the socket, for a core run by tests.
    File(PathBuf),
}

impl Keeper {
    pub(super) fn new(socket: &Path, secret_store: bool) -> CoreResult<Self> {
        if !secret_store {
            let mut path = socket.as_os_str().to_owned();
            path.push(".host");
            return Ok(Self::File(path.into()));
        }
        let account = socket
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| CoreError::from("the core's socket has no name"))?;
        Ok(Self::Store {
            account: account.to_owned(),
        })
    }

    pub(super) fn load(&self) -> CoreResult<Option<HostKey>> {
        let text = match self {
            Self::Store { account } => {
                sikemux_keychain::read(SERVICE, account).map_err(|error| error.to_string())?
            }
            Self::File(path) => match std::fs::read_to_string(path) {
                Ok(text) => Some(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            },
        };
        Ok(text.as_deref().and_then(decode))
    }

    pub(super) fn save(&self, key: &HostKey) -> CoreResult<()> {
        let text = format!("{}.{}", hex::encode(key.secret.to_bytes()), key.trusted);
        match self {
            Self::Store { account } => sikemux_keychain::write(SERVICE, account, &text)
                .map_err(|error| CoreError::from(error.to_string())),
            Self::File(path) => {
                sikemux_process::files::write_atomic(path, text.as_bytes(), true)?;
                Ok(())
            }
        }
    }
}

fn decode(text: &str) -> Option<HostKey> {
    let (secret, trusted) = text.trim().split_once('.')?;
    let bytes: [u8; 32] = hex::decode(secret).ok()?.try_into().ok()?;
    Some(HostKey {
        secret: SecretKey::from_bytes(&bytes),
        trusted: trusted.to_owned(),
    })
}
