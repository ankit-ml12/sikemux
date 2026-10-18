//! How the core starts an agent: the launch it builds from what the person
//! chose, with Sikemux's tools and the agent's environment, for terminals and
//! chats alike. Every chat, however it starts, passes through here or
//! [`own_tools`].

use std::path::PathBuf;
use std::sync::Arc;

use crate::cli::protocol::TOOLS_OFFER_ENV;
use crate::launch::tools::Tools;
use crate::launch::AgentSpec;
use crate::protocol::{
    ChatAccount, ChatLaunch, Event, RequestId, Response, TerminalSpawn, WAKE_WAIT,
};

use super::chat;
use super::connection::{blocking, ClientConn};
use super::workspace::{launcher_of, ChatChoice, ResumeChoice};
use super::{Core, CoreError, CoreResult};

/// Where this core's agents reach their tools; none for a core without a
/// tool endpoint.
pub(crate) fn tools(core: &Core) -> Option<Tools> {
    let config = &core.listening.get()?.config;
    Some(Tools {
        executable: std::env::current_exe().ok()?,
        endpoint: config.cli_endpoint.clone()?,
        data_dir: config.data_dir.clone(),
    })
}

/// Gives a terminal agent its tools, its account and the hosted model
/// providers' keys. A launch that cannot be told about the tools still runs.
pub(crate) async fn terminal_agent(core: &Arc<Core>, spawn: &mut TerminalSpawn) {
    let (Some(command), Some(context)) = (spawn.direct_command.as_mut(), spawn.context.as_ref())
    else {
        return;
    };
    let Some(agent_type) = context.agent_type.clone() else {
        return;
    };
    if let (Some(agent_id), Some(tools)) = (context.agent_id.as_deref(), tools(core)) {
        match tools
            .terminal(agent_id, &agent_type, &command.program)
            .await
        {
            Ok(given) => spawn.env.extend(given.apply(&mut command.args)),
            Err(error) => {
                eprintln!("sikemux core: Sikemux tools are unavailable to this agent: {error}")
            }
        }
    }
    let profile = command.profile.as_ref();
    let spec = AgentSpec {
        provider: agent_type,
        executable_path: None,
        config_path: profile.and_then(|profile| profile.config_path.clone()),
        environment_keys: profile
            .map(|profile| profile.environment_keys.clone())
            .unwrap_or_default(),
    };
    let program = command.program.clone();
    let owner = core.clone();
    let launch = blocking(move || {
        let providers = owner.workspaces.provider_keys(&spec.provider);
        Ok(spec.launch(program.into(), providers))
    })
    .await;
    if let Ok(launch) = launch {
        spawn.env.extend(launch.env);
    }
}

/// Gives a chat about to start the tools that are its own: those the launch
/// offers in its environment, and only for its agent.
pub(crate) fn own_tools(core: &Core, launch: &mut ChatLaunch) {
    launch
        .env
        .insert("SIKEMUX_AGENT_ID".into(), launch.agent_id.clone());
    let Some(tools) = tools(core) else {
        return;
    };
    let offer = launch.env.get(TOOLS_OFFER_ENV).map(String::as_str);
    match tools.chat_server(&launch.agent_id, offer) {
        Ok(server) => launch.mcp_servers = vec![server],
        Err(error) => {
            eprintln!("sikemux core: Sikemux tools are unavailable to this chat: {error}")
        }
    }
}

/// Gives a Hermes or Grok chat its own home with the skills library, unless
/// it has one. Answers whether the launch changed.
pub(crate) async fn own_home(core: &Core, launch: &mut ChatLaunch) -> bool {
    let has_home = ["HERMES_HOME", "GROK_HOME"]
        .iter()
        .any(|home| launch.env.contains_key(*home));
    if has_home || !matches!(launch.provider.as_str(), "hermes" | "grok") {
        return false;
    }
    let Some(tools) = tools(core) else {
        return false;
    };
    tools
        .chat_home(
            &launch.agent_id,
            &launch.provider,
            &launch.program,
            &mut launch.env,
        )
        .await;
    true
}

/// A chat to start: on which launcher, where, and how.
pub(crate) struct Launch {
    pub agent_id: String,
    pub launcher: String,
    pub cwd: PathBuf,
    pub permission_mode: Option<String>,
    pub model: Option<String>,
    /// The hosted model provider it runs on in place of its maker's account.
    pub model_provider: Option<String>,
    pub effort: Option<String>,
    /// The provider's session to load instead of opening a new one.
    pub resume_id: Option<String>,
    pub account: Option<ChatAccount>,
    pub fallbacks: Vec<ChatAccount>,
}

/// Starts the chat `chosen` describes, with `by` hearing it from its first
/// event. Every chat a person starts or wakes, from any client, starts here.
pub(crate) async fn start(
    core: &Arc<Core>,
    by: Option<&Arc<ClientConn>>,
    chosen: Launch,
) -> CoreResult<Arc<chat::Chat>> {
    let launcher = core.workspaces.launcher(&chosen.launcher)?;
    let program = launcher.agent.program().await?;
    let launcher_id = launcher.id.clone();
    let owner = core.clone();
    let (cwd, model_provider, model) = (chosen.cwd, chosen.model_provider, chosen.model);
    let mut launch = blocking(move || {
        owner
            .workspaces
            .chat_launch(&launcher, program, &cwd, model_provider.as_deref(), model)
            .map_err(CoreError::from)
    })
    .await?;
    launch.agent_id = chosen.agent_id;
    launch.resume_id = chosen.resume_id;
    if let Some(mode) = chosen.permission_mode {
        launch.permission_mode = mode;
    }
    launch.effort = chosen.effort;
    launch.account = chosen.account;
    launch.fallbacks = chosen.fallbacks;
    let started = chat::begin(core, launch, by, Some(launcher_id))?;
    core.broadcast_local(&Event::ChatBegun {
        chat: started.info(),
    });
    Ok(started)
}

/// Starts an agent of the workspace as its record says, on its saved
/// conversation and the account it last ran on.
pub(crate) async fn start_agent(
    core: &Arc<Core>,
    by: Option<&Arc<ClientConn>>,
    agent_id: &str,
) -> CoreResult<Arc<chat::Chat>> {
    let kept = core
        .workspaces
        .kept_launch(agent_id)
        .ok_or_else(|| CoreError::from("that agent is no longer on this host"))?;
    let agent = kept.agent;
    let chosen = Launch {
        agent_id: agent.id.clone(),
        launcher: launcher_of(&agent),
        cwd: agent.cwd,
        permission_mode: Some(agent.permission_mode),
        model: agent.model,
        model_provider: agent.model_provider,
        effort: agent.effort,
        resume_id: agent.resume_id,
        account: kept.account,
        fallbacks: kept.fallbacks,
    };
    start(core, by, chosen).await
}

fn running(core: &Core, agent_id: &str) -> bool {
    core.chats.get(agent_id).is_some()
}

/// Wakes an agent's chat and answers once it is ready, so a client taking it
/// up next finds it there.
pub(crate) async fn wake_chat(
    core: &Arc<Core>,
    by: Option<&Arc<ClientConn>>,
    agent_id: String,
) -> CoreResult<Response> {
    let deadline = tokio::time::Instant::now() + WAKE_WAIT;
    if !running(core, &agent_id) {
        start_agent(core, by, &agent_id).await?;
    }
    tokio::time::timeout_at(deadline, chat::until_ready(core, &agent_id))
        .await
        .map_err(|_| CoreError::from("the chat did not wake in time"))??;
    Ok(Response::Done)
}

/// Starts a new chat in one of the projects and answers `client` once its
/// session is ready.
pub(crate) fn start_chat(
    core: &Arc<Core>,
    client: &Arc<ClientConn>,
    request_id: RequestId,
    choice: ChatChoice,
) {
    let (core, client) = (core.clone(), client.clone());
    tokio::spawn(async move {
        let result = start_new(&core, &client, choice).await;
        client.respond(request_id, result);
    });
}

async fn start_new(
    core: &Arc<Core>,
    client: &Arc<ClientConn>,
    choice: ChatChoice,
) -> CoreResult<Response> {
    let agent_id = format!("agent-{}", uuid::Uuid::new_v4().simple());
    let chosen = Launch {
        agent_id: agent_id.clone(),
        launcher: choice.launcher,
        cwd: core.workspaces.project_path(&choice.project)?,
        permission_mode: choice.permission_mode,
        model: choice.model,
        model_provider: None,
        effort: choice.effort,
        resume_id: choice.resume_id,
        account: None,
        fallbacks: Vec::new(),
    };
    let started = start(core, Some(client), chosen).await?;
    let start = chat::until_started(core, started).await?;
    Ok(Response::ChatBegun { agent_id, start })
}

/// Takes up a recent chat again, answering like [`start_chat`].
pub(crate) fn resume_chat(
    core: &Arc<Core>,
    client: &Arc<ClientConn>,
    request_id: RequestId,
    choice: ResumeChoice,
) {
    match core.workspaces.resume_choice(choice, &core.chats.list()) {
        Ok(choice) => start_chat(core, client, request_id, choice),
        Err(error) => client.respond(request_id, Err(error)),
    }
}
