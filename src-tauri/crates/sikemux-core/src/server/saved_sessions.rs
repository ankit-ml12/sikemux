//! The conversations the agent CLIs saved on this host: listing them, and
//! renaming, deleting or moving one to another project the way each
//! provider's own commands do.

use serde::Deserialize;
use serde_json::Value;

use crate::agent_catalog::AgentKind;
use crate::sessions::{self, recent::RecentRequest};

use super::connection::blocking;
use super::{CoreError, CoreResult};

pub(crate) const METHODS: [&str; 5] = [
    "savedSessions",
    "recentSessions",
    "renameSession",
    "deleteSession",
    "relocateSession",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Listing {
    agent: AgentKind,
    cwd: String,
    #[serde(default)]
    config_path: Option<String>,
}

#[derive(Deserialize)]
struct Recent {
    request: RecentRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    agent: AgentKind,
    cwd: String,
    session_id: String,
    #[serde(default)]
    executable_path: Option<String>,
    #[serde(default)]
    config_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Renamed {
    #[serde(flatten)]
    saved: Saved,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Moved {
    agent: AgentKind,
    from_cwd: String,
    to_cwd: String,
    session_id: String,
    #[serde(default)]
    config_path: Option<String>,
}

fn parse<T: for<'de> Deserialize<'de>>(params: Value) -> CoreResult<T> {
    serde_json::from_value(params)
        .map_err(|error| CoreError::from(format!("the call's params do not fit: {error}")))
}

pub(crate) async fn call(method: &str, params: Value) -> CoreResult<Value> {
    Ok(match method {
        "savedSessions" => {
            let listing: Listing = parse(params)?;
            let found = blocking(move || {
                Ok(sessions::agent_sessions(
                    listing.agent,
                    &listing.cwd,
                    listing.config_path.as_deref(),
                ))
            })
            .await?;
            serde_json::to_value(found)?
        }
        "recentSessions" => {
            let recent: Recent = parse(params)?;
            let page = blocking(move || Ok(sessions::recent::recent_page(recent.request)?)).await?;
            serde_json::to_value(page)?
        }
        "renameSession" => {
            let renamed: Renamed = parse(params)?;
            let saved = renamed.saved;
            sessions::rename::rename(
                saved.agent,
                saved.cwd,
                saved.session_id,
                renamed.title,
                saved.executable_path,
                saved.config_path,
            )
            .await?;
            Value::Null
        }
        "deleteSession" => {
            let saved: Saved = parse(params)?;
            sessions::delete::delete(
                saved.agent,
                saved.cwd,
                saved.session_id,
                saved.executable_path,
                saved.config_path,
            )
            .await?;
            Value::Null
        }
        "relocateSession" => {
            let moved: Moved = parse(params)?;
            sessions::relocate::relocate(
                moved.agent,
                moved.from_cwd,
                moved.to_cwd,
                moved.session_id,
                moved.config_path,
            )
            .await?;
            Value::Null
        }
        _ => return Err(format!("this host does not know {method}").into()),
    })
}
