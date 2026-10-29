//! The chat agents the core starts and speaks to: Claude Code over its
//! stream-json control protocol, Codex over its app server, Muse Code over its
//! session protocol, and every other agent over ACP. Each tells the chat the
//! same ACP-shaped session updates, so the app reads them alike.

mod acp;
mod claude;
mod codex;
mod json;
mod mcp_calls;
mod models;
mod muse;
mod parts;
mod patch;
mod pending;
mod process;
mod rpc;
mod servers;
mod session;
mod turns;

use std::sync::Arc;

use serde_json::Value;
use sikemux_fold::update::stamp_replayed;
use tokio::sync::mpsc;

use crate::agent_catalog::{AgentKind, ChatDriver};
use crate::protocol::ChatLaunch;

use super::rebind::Outcome;
use super::{Chat, ChatCommand};

pub(crate) use process::{kill_and_reap, kill_group, AgentRecord, Released};
pub(crate) use session::{Adopted, Begin, Handed, SessionState};

/// How an agent is shown a file the person attached: a markdown link named
/// for the file, the way the agents' own clients write it.
fn file_link(name: Option<&str>, uri: &str) -> String {
    match name.filter(|name| !name.is_empty()) {
        Some(name) => format!("[@{name}]({uri})"),
        None => match uri.strip_prefix("file://") {
            Some(path) => format!("[@{}]({uri})", path.rsplit('/').next().unwrap_or(path)),
            None => uri.to_owned(),
        },
    }
}

fn driver(provider: &str) -> Option<&'static ChatDriver> {
    crate::agent_catalog::agent(provider).and_then(|agent| agent.chat.as_ref())
}

/// Whether a resumed chat of `provider` can show its saved history before its agent starts:
/// only Claude's is a file the core reads itself.
fn shows_saved(provider: &str) -> bool {
    matches!(driver(provider), Some(ChatDriver::Claude))
        || matches!(
            AgentKind::from_name(provider),
            Some(AgentKind::Omp | AgentKind::Opencode)
        )
}

/// The models and efforts `launch`'s agent offers, asked of its CLI without a session.
/// Only Claude and Codex tell them outside one; an ACP agent offers them in `session/new`.
pub(crate) async fn read_options(launch: &ChatLaunch) -> Result<Value, String> {
    let mut command = tokio::process::Command::from(
        sikemux_process::user_environment::agent_command(&launch.program),
    );
    command
        .envs(&launch.env)
        .envs(
            launch
                .account
                .iter()
                .flat_map(|account| account.env.clone()),
        )
        .kill_on_drop(true);
    match driver(&launch.provider) {
        Some(ChatDriver::Claude) => claude::read_options(command).await,
        Some(ChatDriver::Codex) => codex::read_options(command).await,
        _ => Err(format!(
            "{} offers its options only in a session",
            launch.provider
        )),
    }
}

/// Shows the saved history of the session a waiting chat resumes, when the core can read it.
pub(crate) fn show_saved(chat: &Arc<Chat>) {
    let Some(session) = chat.launch.resume_id.clone() else {
        return;
    };
    if !shows_saved(chat.provider()) {
        return;
    }
    if chat
        .saved_shown
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return;
    }
    chat.saved_reading.send_replace(true);
    let chat = chat.clone();
    tokio::task::spawn_blocking(move || {
        let sink = session::Sink::showing(chat.clone());
        let shown = match driver(chat.provider()) {
            Some(ChatDriver::Claude) => claude::replay_saved(&chat.launch, &session, &sink),
            _ => acp::replay_saved(&chat.launch, &session, &sink),
        };
        if let Err(error) = shown {
            chat.saved_shown
                .store(false, std::sync::atomic::Ordering::Release);
            eprintln!(
                "sikemux core: could not show {}'s saved chat: {error}",
                chat.agent_id()
            );
        }
        chat.saved_reading.send_replace(false);
    });
}

pub(super) async fn run(
    chat: Arc<Chat>,
    launch: ChatLaunch,
    commands: &mut mpsc::UnboundedReceiver<ChatCommand>,
    begin: Begin,
) -> Result<Outcome, String> {
    let driver =
        driver(chat.provider()).ok_or_else(|| format!("{} has no chat", chat.provider()))?;
    match driver {
        ChatDriver::Claude => session::run::<claude::Claude>(chat, launch, commands, begin).await,
        ChatDriver::Codex => session::run::<codex::Codex>(chat, launch, commands, begin).await,
        ChatDriver::Muse => session::run::<muse::Muse>(chat, launch, commands, begin).await,
        ChatDriver::Acp { .. } => session::run::<acp::Acp>(chat, launch, commands, begin).await,
    }
}
