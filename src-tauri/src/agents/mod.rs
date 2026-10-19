pub(crate) mod config;
pub(crate) mod executable;

use serde::{Deserialize, Serialize};
use sikemux_core::agent_accounts::status::AgentStatus;

pub use sikemux_core::agent_catalog::AgentKind;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    #[serde(rename = "type")]
    kind: &'static str,
    label: &'static str,
    command: String,
    available: bool,
    error: Option<String>,
    warning: Option<String>,
    profile_id: Option<String>,
    config_path: Option<String>,
    #[serde(rename = "defaultModel")]
    default_model: Option<String>,
    #[serde(rename = "defaultEffort")]
    default_effort: Option<String>,
    status: AgentStatus,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProfileRequest {
    #[serde(rename = "type")]
    kind: AgentKind,
    profile_id: Option<String>,
    executable_path: Option<String>,
    config_path: Option<String>,
}
