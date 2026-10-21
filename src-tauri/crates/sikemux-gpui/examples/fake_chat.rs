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
//! has the agent ask before it acts.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use sikemux_client::client::CoreClient;

fn main() -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run())
}

async fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let socket = PathBuf::from(args.next().ok_or_else(|| anyhow::anyhow!("which core's socket?"))?);
    let agent = args.next().ok_or_else(|| anyhow::anyhow!("where is sikemux-fake-acp-agent?"))?;
    let (client, _events) = CoreClient::connect(&socket).await?;
    client.register_window().await?;
    let provider = std::env::var("FAKE_PROVIDER").unwrap_or("opencode".into());
    let profile = json!({ "id": "fake", "name": "Fake", "provider": provider, "accent": "#a277ff", "executablePath": agent });
    client.call("setAgentAccounts", json!({ "profiles": [profile] })).await?;
    let project = json!({ "id": "tmp", "name": "tmp", "path": std::env::temp_dir() });
    client.call("addProject", project).await?;
    let (chat, _) = client.start_chat(format!("{provider}:fake"), "tmp".into(), None).await?;
    client.call("openTab", json!({ "project": "tmp", "kind": "agent", "target": chat })).await?;
    println!("chat {chat}");
    if let Ok(mode) = std::env::var("FAKE_MODE") {
        std::thread::sleep(Duration::from_secs(2));
        client.acp_set_permission_mode(chat.clone(), mode).await?;
    }
    for prompt in args {
        client.acp_prompt(chat.clone(), None, prompt, Vec::new(), Vec::new()).await?;
        std::thread::sleep(Duration::from_secs(2));
    }
    Ok(())
}
