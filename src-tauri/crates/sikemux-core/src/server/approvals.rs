//! Plugin tools the person approves each call to. The tool's check says what
//! the person is asked, the question goes straight to them, and what they pick
//! decides whether the tool's method runs. The call's arguments stay in memory
//! only, since they may hold a password.

use std::fmt;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::cli::protocol::{HarnessRequest, CORE_REPLY_TIMEOUT};
use crate::protocol::WindowCall;

use super::plugins::{self, Gated};
use super::questions::{self, Choice};
use super::Core;

/// What a choice on the card does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Does {
    /// Runs the tool's method as the agent called it.
    Approve,
    /// Opens the plugin's pane on the call, to change it before it runs.
    Edit,
    Decline,
}

#[derive(Deserialize)]
struct Offered {
    label: String,
    #[serde(default)]
    detail: Option<String>,
    does: Does,
}

/// What the person is asked, as the tool's check answers it.
#[derive(Deserialize)]
struct Proposal {
    header: String,
    text: String,
    choices: Vec<Offered>,
    /// Where the plugin's pane opens to change the call; needed by an `edit` choice.
    #[serde(default)]
    edit: Option<String>,
}

/// A call waiting on the person.
pub(crate) struct Approval {
    gated: Gated,
    arguments: Value,
    does: Vec<Does>,
    edit: Option<String>,
}

impl fmt::Debug for Approval {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Approval")
            .field("plugin", &self.gated.plugin)
            .field("method", &self.gated.method)
            .finish_non_exhaustive()
    }
}

/// Checks the call and puts it to the person, answering the agent that it waits on them.
pub(crate) async fn propose(
    core: &Arc<Core>,
    request: &HarnessRequest,
    gated: Gated,
    arguments: Value,
) -> Result<Value, String> {
    let agent_id = request
        .agent_id
        .clone()
        .ok_or("this tool needs the calling agent's id")?;
    let checked = plugins::call(&gated.plugin, &gated.check, arguments.clone())
        .await
        .map_err(|failure| plugins::explain(&failure))?;
    let proposal: Proposal = serde_json::from_value(checked).map_err(|error| {
        format!(
            "{} answered a check Sikemux cannot read: {error}",
            gated.plugin
        )
    })?;
    let does: Vec<Does> = proposal
        .choices
        .iter()
        .map(|offered| offered.does)
        .collect();
    if does.contains(&Does::Edit) && proposal.edit.is_none() {
        return Err(format!(
            "{} offered an edit with nowhere to make it",
            gated.plugin
        ));
    }
    let choices = proposal
        .choices
        .into_iter()
        .map(|offered| Choice {
            label: offered.label,
            detail: offered.detail,
        })
        .collect();
    let approval = Approval {
        gated,
        arguments,
        does,
        edit: proposal.edit,
    };
    questions::put_to_person(
        core,
        &agent_id,
        &request.project,
        proposal.header,
        proposal.text,
        choices,
        approval,
    )?;
    Ok(json!(
        "Sent to the person to approve. Stop here and end your turn without guessing: what they decide comes to you as a message."
    ))
}

/// Does what the person picked, answering what the agent is told. A typed answer turns the
/// call down and passes their words on.
pub(crate) async fn settle(
    core: &Arc<Core>,
    approval: &Approval,
    choice: Option<usize>,
    answer: &str,
) -> Result<String, String> {
    let Gated { plugin, method, .. } = &approval.gated;
    match choice.and_then(|choice| approval.does.get(choice)) {
        Some(Does::Approve) => {
            let done = plugins::call(plugin, method, approval.arguments.clone())
                .await
                .map_err(|failure| failure.message)?;
            plugins::tell_agents_if_theirs(core, plugin);
            super::plugin_views::wrote(core, plugin, method);
            Ok(match done {
                Value::String(said) => format!("The person approved it. {said}"),
                _ => "The person approved it, and it is done.".to_owned(),
            })
        }
        Some(Does::Edit) => {
            let open = WindowCall::OpenPlugin {
                plugin: plugin.clone(),
                at: approval.edit.clone().unwrap_or_default(),
            };
            let call = core
                .window
                .call(open, "change it in its form", false, None)?;
            let late = "Sikemux's window did not open the form; try again.".to_owned();
            super::harness::answer_of(core, call, CORE_REPLY_TIMEOUT, late).await?;
            Ok("The person opened it in Sikemux to change it first; nothing is done until they save it there.".to_owned())
        }
        Some(Does::Decline) => Ok("The person turned it down; nothing was done.".to_owned()),
        None => Ok(format!(
            "The person turned it down; nothing was done. They said: {answer}"
        )),
    }
}
