//! Text handed to an agent from elsewhere in the window, such as a terminal's selection or an
//! editor's problem: to one open in the project, or a new chat started for it.

use std::future::Future;

use gpui_kit::{AppContext, Context, SharedString, Task, Window};
use tokio::sync::oneshot;

use crate::workbench::Workbench;

/// An agent text can go to.
#[derive(Clone)]
pub struct Choice {
    pub label: SharedString,
    pub provider: String,
    pub to: Recipient,
}

#[derive(Clone)]
pub enum Recipient {
    Agent(String),
    NewChat { launcher: String, project: String },
}

/// The project's agents, then a new chat with each agent there is.
pub fn choices(bench: &Workbench) -> Vec<Choice> {
    let Some(project) = bench.project() else {
        return Vec::new();
    };
    let open = bench
        .agents_of(&project.id)
        .into_iter()
        .map(|agent| Choice {
            label: agent.title.into(),
            provider: agent.provider,
            to: Recipient::Agent(agent.id),
        });
    let fresh = bench.launchers().iter().map(|launcher| Choice {
        label: format!("New {} chat", launcher.label).into(),
        provider: launcher.provider.clone(),
        to: Recipient::NewChat {
            launcher: launcher.id.clone(),
            project: project.id.clone(),
        },
    });
    open.chain(fresh).collect()
}

/// Puts `text` in the composer of the agent `to` names, starting its chat first when it is a
/// new one, and brings the agent forward. Nothing goes when `text` comes to nothing.
pub fn send(
    bench: &mut Workbench,
    to: Recipient,
    text: impl Future<Output = Option<String>> + 'static,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let agent = match to {
        Recipient::Agent(agent) => Task::ready(Some(agent)),
        Recipient::NewChat { launcher, project } => {
            let (started, agent) = oneshot::channel();
            bench
                .target()
                .start_chat(launcher, project, None, Some(started));
            cx.background_spawn(async move { agent.await.ok() })
        }
    };
    cx.spawn_in(window, async move |bench, cx| {
        let (Some(text), Some(agent)) = (text.await, agent.await) else {
            return;
        };
        let _ = bench.update_in(cx, |bench, window, cx| {
            bench.deliver(&agent, text, window, cx)
        });
    })
    .detach();
}

/// `text` in a fenced code block whose fence no run of backticks in it can close.
pub fn code_fence(text: &str) -> String {
    let longest = text
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat((longest + 1).max(3));
    format!("{fence}\n{}\n{fence}", text.trim_end_matches('\n'))
}
