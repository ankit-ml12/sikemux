//! OMP calls a tool server's tool by writing its arguments as JSON to the
//! path `xd://mcp__<server>_<tool>` with its own `write` tool. Such a write
//! is read as the tool call it is, titled `mcp__<server>__<tool>` as every
//! other agent's are, so the chat shows it, and a page it shows, alike.

use serde_json::{json, Value};

use crate::launch::tools::TOOLS_SERVER;

const TOOL_PATH: &str = "xd://mcp__";

/// Makes a `tool_call` or `tool_call_update` that writes to a tool's path into
/// that tool's call. `servers` are the tool servers the chat handed its agent.
pub(super) fn as_tool_call(update: &mut Value, servers: &[String]) {
    if !matches!(
        update["sessionUpdate"].as_str(),
        Some("tool_call" | "tool_call_update")
    ) {
        return;
    }
    let device = update.pointer("/rawOutput/details/xdev").cloned();
    let inner = device.as_ref().map(|device| &device["inner"]);
    let named = |pointer: &str, prefix: &str| {
        update
            .pointer(pointer)
            .and_then(Value::as_str)
            .and_then(|path| path.strip_prefix(prefix))
            .map(str::to_owned)
    };
    let written = named("/rawInput/path", TOOL_PATH);
    let (server, tool) = match inner.map(|inner| (&inner["serverName"], &inner["mcpToolName"])) {
        Some((Value::String(server), Value::String(tool))) => (server.clone(), tool.clone()),
        _ => match written
            .clone()
            .or_else(|| named("/rawOutput/details/xdev/tool", "mcp__"))
        {
            Some(name) => split(&name, servers),
            None => return,
        },
    };
    update["title"] = json!(format!("mcp__{server}__{tool}"));
    update["kind"] = json!("other");
    if written.is_some() {
        let content = update["rawInput"]["content"].as_str().unwrap_or_default();
        update["rawInput"] = serde_json::from_str::<Value>(content)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({ "input": content }));
    } else if let Some(args) = device.as_ref().map(|device| &device["args"]) {
        if args.is_object() {
            update["rawInput"] = args.clone();
        }
    }
    if let Some(blocks) = update.pointer("/rawOutput/content").cloned() {
        update["rawOutput"] = blocks;
    }
}

/// `sikemux_tools_page_show` as the server `sikemux-tools` and its tool
/// `page_show`: OMP joins a server's name, its dashes made underscores, to the
/// tool's with an underscore, so only the servers' own names tell where one ends.
fn split(name: &str, servers: &[String]) -> (String, String) {
    servers
        .iter()
        .map(String::as_str)
        .chain([TOOLS_SERVER])
        .filter_map(|server| {
            let joined: String = server
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            let tool = name.strip_prefix(&joined)?.strip_prefix('_')?;
            Some((server, tool))
        })
        .max_by_key(|(server, _)| server.len())
        .map(|(server, tool)| (server.to_owned(), tool.to_owned()))
        .unwrap_or_else(|| match name.split_once('_') {
            Some((server, tool)) => (server.to_owned(), tool.to_owned()),
            None => (String::new(), name.to_owned()),
        })
}
