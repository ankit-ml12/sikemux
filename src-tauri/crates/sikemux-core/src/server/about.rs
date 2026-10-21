//! What Settings' About page and the first screen ask of the host: a
//! release's notes and who made it, and which of the tools Sikemux works with
//! the host has.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sikemux_process::user_environment::find_executable;

use super::connection::blocking;
use super::{CoreError, CoreResult};
use crate::releases;

#[derive(Deserialize)]
pub(crate) struct NotesParams {
    version: String,
}

pub(crate) async fn release_notes(params: NotesParams) -> CoreResult<releases::ReleaseNotes> {
    releases::release_notes(params.version)
        .await
        .map_err(CoreError::from)
}

#[derive(Deserialize)]
pub(crate) struct AvatarsParams {
    urls: Vec<String>,
}

/// Contributors' pictures as `data:` addresses, by the address GitHub gave.
pub(crate) async fn release_avatars(params: AvatarsParams) -> HashMap<String, String> {
    releases::release_avatars(params.urls).await
}

/// The person's shell, and whether git and the plugins' own command-line
/// tools are on the host.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tools {
    shell: String,
    git: bool,
    aws: bool,
    rnd: bool,
}

pub(crate) async fn tools() -> CoreResult<Tools> {
    blocking(|| {
        Ok(Tools {
            shell: sikemux_pty::user_shell::configured_shell(),
            git: find_executable("git").is_some(),
            aws: find_executable("aws").is_some(),
            rnd: find_executable("rnd").is_some(),
        })
    })
    .await
}
