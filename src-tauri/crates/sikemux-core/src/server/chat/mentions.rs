//! What a message tells its agent about each agent it names with `@`. A client sends
//! only who it names, as a context item with no text, and the core writes the note.

use crate::protocol::ChatContext;
use crate::server::{display, Core};

const AGENT_URI: &str = "sikemux://agent/";

/// Writes the note of each agent `context` names that came without one, for the
/// message `caller` is about to get. A named agent the core no longer has is dropped.
pub(crate) fn fill(core: &Core, caller: &str, context: &mut Vec<ChatContext>) {
    context.retain_mut(|item| {
        if !item.text.is_empty() {
            return true;
        }
        let Some(id) = item.uri.strip_prefix(AGENT_URI) else {
            return true;
        };
        match note(core, caller, id) {
            Some(note) => {
                item.text = note;
                true
            }
            None => false,
        }
    });
}

fn note(core: &Core, caller: &str, id: &str) -> Option<String> {
    let mate = core.workspaces.agent(id)?;
    let name = core.names.name(id)?;
    let elsewhere = core
        .workspaces
        .agent(caller)
        .filter(|caller| caller.project != mate.project)
        .map(|_| {
            core.workspaces
                .project_info(&mate.project)
                .map_or_else(|| mate.project.clone(), |project| project.name)
        });
    let cli = crate::agent_catalog::agent(&mate.provider)
        .map_or(mate.provider.as_str(), |entry| entry.label);
    let mut lines = vec![match &elsewhere {
        Some(project) => {
            format!("@{name} is an agent in another project, {project}, not the person.")
        }
        None => format!("@{name} is another agent in this project, not the person."),
    }];
    lines.push(format!("Agent id: {id}"));
    lines.push(format!("CLI: {cli}"));
    lines.push(format!("Status: {}", status(core, id)));
    lines.push(format!("Working on: {}", mate.title));
    let to = if elsewhere.is_some() { id } else { &name };
    lines.push(format!("To message it, call agent_send with to \"{to}\"."));
    Some(lines.join("\n"))
}

/// The word every client shows beside the agent, or Asleep while it is not running.
fn status(core: &Core, id: &str) -> &'static str {
    if let Some(chat) = core
        .chat_infos()
        .into_iter()
        .find(|chat| chat.agent_id == id)
    {
        return if chat.asleep {
            "Asleep"
        } else {
            display::group_label(chat.group)
        };
    }
    core.session_infos()
        .into_iter()
        .find(|session| session.agent_id.as_deref() == Some(id))
        .and_then(|session| session.group)
        .map_or("Asleep", display::group_label)
}
