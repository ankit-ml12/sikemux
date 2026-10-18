//! How the core starts an agent: the launch it builds from what the person
//! chose, with Sikemux's tools and the agent's environment, for terminals and
//! chats alike.

use crate::launch::tools::Tools;
use crate::launch::AgentSpec;
use crate::protocol::TerminalSpawn;

use super::connection::blocking;
use super::Core;

/// Where this core's agents reach their tools; none for a core without a
/// tool endpoint or a data folder.
pub(crate) fn tools(core: &Core) -> Option<Tools> {
    let config = &core.listening.get()?.config;
    Some(Tools {
        executable: std::env::current_exe().ok()?,
        endpoint: config.cli_endpoint.clone()?,
        data_dir: config.data_dir.clone()?,
    })
}

/// Gives a terminal agent its tools, its account and the hosted model
/// providers' keys. A launch that cannot be told about the tools still runs.
pub(crate) async fn terminal_agent(core: &std::sync::Arc<Core>, spawn: &mut TerminalSpawn) {
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
