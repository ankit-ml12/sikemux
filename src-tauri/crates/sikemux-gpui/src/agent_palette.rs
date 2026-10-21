//! ⌘⇧N, the agent palette: a new chat with any agent the host has, the agents
//! open in the project, and its pinned and recent chats to take up again.

use gpui_kit::{Context, Window};
use serde_json::json;
use sikemux_wire::protocol::AgentStatus;

use crate::core::Ask;
use crate::palette::{Mark, Row, Spec};
use crate::workbench::Workbench;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "agent.choose")]
pub struct ChooseAgent;

const NEW: &str = "new:";
const SIGN_IN: &str = "signin:";
const OPEN: &str = "open:";
const RESUME: &str = "resume:";

/// Agents whose accounts the core signs in to itself.
const SIGNS_IN: [&str; 2] = ["claude", "codex"];

impl Workbench {
    pub fn toggle_agent_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project().map(|project| project.id.clone()) else {
            return;
        };
        let mut rows = Vec::new();
        for launcher in self.launchers() {
            let row = match launcher.status.as_deref() {
                Some("signedOut") if SIGNS_IN.contains(&launcher.provider.as_str()) => Row::new(
                    format!("{SIGN_IN}{}", launcher.id),
                    format!("Sign in to {}", launcher.label),
                )
                .meta("signed out"),
                Some(status @ ("signedOut" | "missing" | "broken")) => {
                    Row::new(format!("{NEW}{}", launcher.id), launcher.label.clone())
                        .meta(match status {
                            "signedOut" => "signed out",
                            "missing" => "not installed",
                            _ => "won't start",
                        })
                        .disabled(true)
                }
                _ => Row::new(format!("{NEW}{}", launcher.id), launcher.label.clone())
                    .meta("new chat"),
            };
            rows.push(
                row.mark(Mark::Agent(launcher.provider.clone()))
                    .group("New")
                    .also(&launcher.provider),
            );
        }
        for agent in self.agents_of(&project) {
            let title = agent.name.clone().unwrap_or_else(|| agent.title.clone());
            rows.push(
                Row::new(format!("{OPEN}{}", agent.id), title)
                    .meta(status_word(agent.status))
                    .mark(Mark::Agent(agent.provider.clone()))
                    .group("Open")
                    .also(&agent.title)
                    .also(&agent.provider),
            );
        }
        let recent = self
            .view
            .as_ref()
            .map(|view| view.recent.as_slice())
            .unwrap_or_default();
        let saved = recent
            .iter()
            .filter(|recent| recent.project == project && recent.agent_id.is_none());
        let (pinned, rest): (Vec<_>, Vec<_>) = saved.partition(|recent| recent.pinned);
        for (group, chats) in [("Pinned", pinned), ("Recent", rest)] {
            for chat in chats {
                rows.push(
                    Row::new(format!("{RESUME}{}", chat.id), chat.title.clone())
                        .meta(if chat.active_at == 0 {
                            String::new()
                        } else {
                            crate::agents::age(chat.active_at)
                        })
                        .mark(Mark::Agent(chat.provider.clone()))
                        .group(group)
                        .also(&chat.provider),
                );
            }
        }
        let spec = Spec::new(
            "agent.choose",
            "start or find an agent…",
            rows,
            move |row, bench, _, _| {
                let key: &str = &row.key;
                if let Some(launcher) = key.strip_prefix(NEW) {
                    let _ = bench.asks.send(Ask::StartChat {
                        launcher: launcher.to_owned(),
                        project: project.clone(),
                        started: None,
                    });
                } else if let Some(launcher) = key.strip_prefix(SIGN_IN) {
                    bench.call("signInLauncher", json!({ "launcher": launcher }));
                } else if let Some(agent) = key.strip_prefix(OPEN) {
                    bench.open_agent(agent);
                } else if let Some(id) = key.strip_prefix(RESUME) {
                    let recent = bench
                        .view
                        .as_ref()
                        .and_then(|view| view.recent.iter().find(|recent| recent.id == id))
                        .cloned();
                    if let Some(recent) = recent {
                        bench.resume_chat(&recent);
                    }
                }
            },
        )
        .empty("no agents or chats match");
        self.toggle_palette(spec, window, cx);
    }
}

fn status_word(status: AgentStatus) -> &'static str {
    match status {
        AgentStatus::Working => "working",
        AgentStatus::Starting => "starting",
        AgentStatus::Blocked => "needs you",
        AgentStatus::Done => "done",
        AgentStatus::Idle => "idle",
        AgentStatus::Asleep => "asleep",
        AgentStatus::Stopped => "stopped",
    }
}
