//! Opens a chat with sikemux-core's fake agent on a throwaway core, in a tab,
//! and sends it prompts, so the app's chat can be tried without a real agent:
//!   sikemux-core --socket /tmp/sx/core.sock --data-dir /tmp/sx/data &
//!   <core's folder>/examples/remote /tmp/sx/core.sock <sikemux-fake-acp-agent> work "stream 400"
//! A debug core lets only the app, and programs it knows by where they live,
//! act as the person, so copy this example to `examples/remote` beside the core.
//! `SIKEMUX_CORE_SOCKET=/tmp/sx/core.sock cargo run --example shot` then shows the chat;
//! copied to `sikemux` beside the core, the shot can also answer it with `--then` clicks.
//! The fake agent answers `work` with a thought and three tool calls, `ask`
//! with a permission request, `stream N` with N words, and anything else by
//! echoing it, markdown and all. `FAKE_PROVIDER=claude` with
//! `sikemux-fake-claude` in its place also answers `question` with three
//! questions for the person, one of them secret, and `delegate` with a
//! subagent that reads a file. `FAKE_MODE=workspace-write`
//! has the agent ask before it acts, and `FAKE_MENTION=<agent id>` names that
//! agent with `@` in every prompt, for the core to describe. `FAKE_PROJECT=<folder>`
//! opens the chat in that folder's project rather than the temporary folder's.
//! `FAKE_NAME=<name>` names the agent as the person would, and `FAKE_CLOSE=1` closes
//! its tab once the prompts are sent, so it is listed among the closed agents.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use sikemux_client::client::CoreClient;
use sikemux_wire::protocol::ChatContext;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run())
}

async fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let socket = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("which core's socket?"))?,
    );
    let agent = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("where is sikemux-fake-acp-agent?"))?;
    let (client, _events) = CoreClient::connect(&socket).await?;
    client.register_window().await?;
    let provider = std::env::var("FAKE_PROVIDER").unwrap_or("opencode".into());
    let profile = json!({ "id": "fake", "name": "Fake", "provider": provider, "accent": "#a277ff", "executablePath": agent });
    client
        .call("setAgentAccounts", json!({ "profiles": [profile] }))
        .await?;
    let folder = std::env::var_os("FAKE_PROJECT").map_or_else(std::env::temp_dir, PathBuf::from);
    let name = folder
        .file_name()
        .map_or("tmp".into(), |name| name.to_string_lossy().into_owned());
    let project = json!({ "id": name, "name": name, "path": folder });
    client.call("addProject", project).await?;
    let (chat, _) = client
        .start_chat(format!("{provider}:fake"), name.clone(), None)
        .await?;
    let tab = client
        .call(
            "openTab",
            json!({ "project": name, "kind": "agent", "target": chat }),
        )
        .await?;
    if let Ok(named) = std::env::var("FAKE_NAME") {
        client
            .call("renameAgent", json!({ "agentId": chat, "name": named }))
            .await?;
    }
    println!("chat {chat}");
    if let Ok(mode) = std::env::var("FAKE_MODE") {
        std::thread::sleep(Duration::from_secs(2));
        client.acp_set_permission_mode(chat.clone(), mode).await?;
    }
    let mentions: Vec<ChatContext> = std::env::var("FAKE_MENTION")
        .into_iter()
        .map(|agent| ChatContext {
            uri: format!("sikemux://agent/{agent}"),
            title: "@mate".into(),
            text: String::new(),
        })
        .collect();
    for prompt in args {
        client
            .acp_prompt(chat.clone(), None, prompt, Vec::new(), mentions.clone())
            .await?;
        std::thread::sleep(Duration::from_secs(2));
    }
    if std::env::var_os("FAKE_CLOSE").is_some() {
        client
            .call("closeTab", json!({ "project": name, "tab": tab }))
            .await?;
    }
    Ok(())
}
