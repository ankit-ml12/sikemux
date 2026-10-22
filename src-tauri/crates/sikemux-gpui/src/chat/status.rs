//! How a chat stands, said at the end of its rows: what the agent is doing while a turn
//! runs, why the last turn failed and what can be done about it, and a session that
//! dropped.

use std::time::{Duration, Instant};

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, App, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Task, Window,
};
use serde_json::json;
use sikemux_wire::chat::{ChatConnection, ChatPart, FailureKind, ToolAction};

use super::Chat;
use crate::core::{Ask, ChatAsk};
use crate::icons::Icon;
use crate::theme::Tokens;
use crate::ui::{self, Button, Variant};
use crate::workbench::{AgentRow, Workbench};

/// The line under the last row while a turn runs: what the agent is doing, and for how
/// long. It keeps its own clock, and each second draws this line again in place, leaving the
/// chat around it as it was.
pub struct Activity {
    provider: String,
    label: Option<SharedString>,
    since: Instant,
    _ticking: Option<Task<()>>,
}

impl Activity {
    pub fn new(provider: &str) -> Self {
        Self {
            provider: provider.to_owned(),
            label: None,
            since: Instant::now(),
            _ticking: None,
        }
    }

    pub fn set(&mut self, label: Option<SharedString>, cx: &mut Context<Self>) {
        if label == self.label {
            return;
        }
        if self.label.is_none() {
            self.since = Instant::now();
        }
        self._ticking = label.is_some().then(|| {
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.repaint()).is_err() {
                    break;
                }
            })
        });
        self.label = label;
        cx.notify();
    }
}

impl Render for Activity {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(label) = self.label.clone() else {
            return div().into_any_element();
        };
        let t = Tokens::get(cx).clone();
        let seconds = self.since.elapsed().as_secs();
        centred(
            div()
                .px(px(22.))
                .py(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::mark(&self.provider, 17.))
                .child(ui::text(label, 13., FontWeight::NORMAL).text_color(t.text_tertiary))
                .when(seconds > 0, |line| {
                    line.child(ui::mono(elapsed(seconds), 10.5, cx).text_color(t.text_disabled))
                }),
        )
        .into_any_element()
    }
}

fn elapsed(seconds: u64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {:02}s", s / 60, s % 60),
        s => format!("{}h {:02}m", s / 3600, s / 60 % 60),
    }
}

impl Chat {
    /// What the line under the last row says: nothing while a permission card already says
    /// what the turn waits on, else the running call's words, a retry, or that it thinks;
    /// and while no turn runs, that the agent is still on its way.
    pub(super) fn activity(&self, cx: &App) -> Option<SharedString> {
        let head = self.state.as_ref()?;
        if !head.permissions.is_empty() {
            return None;
        }
        if head.running {
            if let Some(retrying) = &head.retrying {
                let what = if retrying.connection {
                    "Reconnecting…"
                } else {
                    "Retrying…"
                };
                let attempt = retrying
                    .attempt
                    .map(|attempt| format!(" (attempt {attempt})"));
                return Some(format!("{what}{}", attempt.unwrap_or_default()).into());
            }
            let running = head.active_tool.as_ref().and_then(|active| {
                let rows = self.transcript.read(cx).rows();
                let last = rows.last()?;
                last.message.parts.iter().find_map(|part| match part {
                    ChatPart::Tool { id, tool, .. } if id == active => Some(doing(&tool.row)),
                    _ => None,
                })
            });
            return Some(running.unwrap_or_else(|| "Thinking…".into()));
        }
        if self.is_empty(cx) {
            return None;
        }
        match head.connection {
            ChatConnection::Starting => Some("Starting agent…".into()),
            ChatConnection::Connecting | ChatConnection::Initializing => {
                Some("Connecting to agent session…".into())
            }
            _ => None,
        }
    }
}

/// What a running call is doing, in its row's words.
fn doing(row: &sikemux_wire::chat::ToolRow) -> SharedString {
    if let Some(activity) = &row.activity {
        return activity.clone().into();
    }
    match row.action {
        ToolAction::Read => "Reading…",
        ToolAction::Edit => "Editing…",
        ToolAction::Delete => "Deleting…",
        ToolAction::Move => "Moving…",
        ToolAction::Search => "Searching…",
        ToolAction::Execute => "Running a command…",
        ToolAction::Think => "Thinking…",
        ToolAction::Fetch => "Fetching…",
        ToolAction::SwitchMode => "Switching mode…",
        ToolAction::Mcp | ToolAction::Other => "Working…",
    }
    .into()
}

/// Whose servers a provider's turns go to, for saying which the network lost.
fn served_by(provider: &str) -> &'static str {
    match provider {
        "claude" => "Anthropic",
        "codex" => "OpenAI",
        _ => "the model provider",
    }
}

impl Workbench {
    /// Starts the session of `agent`'s chat again after it dropped, and attaches to it.
    pub fn reconnect(&mut self, agent: &str) {
        self.ask_chat(ChatAsk::Restart {
            agent: agent.to_owned(),
        });
    }

    /// A new chat with the same agent and account, in the same project.
    fn start_over(&mut self, agent: &str) {
        let Some(saved) = self
            .view
            .as_ref()
            .and_then(|view| view.workspace.agents.iter().find(|saved| saved.id == agent))
        else {
            return;
        };
        let launcher = self.launchers().iter().find(|launcher| {
            launcher.provider == saved.provider
                && saved
                    .profile_id
                    .as_ref()
                    .is_none_or(|profile| launcher.id.ends_with(&format!(":{profile}")))
        });
        if let Some(launcher) = launcher {
            let _ = self.asks.send(Ask::StartChat {
                launcher: launcher.id.clone(),
                project: saved.project.clone(),
                started: None,
            });
        }
    }

    fn account_call(&self, method: &'static str, agent: &str) {
        let _ = self.asks.send(Ask::Call {
            method,
            params: json!({ "agentId": agent }),
        });
    }
}

/// What stands at the end of the rows when something went wrong: why the last turn
/// failed with what can be done about it, or that the session dropped.
pub(super) fn trouble(
    agent: &AgentRow,
    chat: &Chat,
    cx: &mut Context<Workbench>,
) -> Option<AnyElement> {
    let head = chat.state.as_ref()?;
    let t = Tokens::get(cx).clone();
    let this = cx.weak_entity();
    let act = |label: &'static str, id: &'static str, run: fn(&mut Workbench, &str)| {
        let (this, agent) = (this.clone(), agent.id.clone());
        Button::new(id, label, Variant::Outline).on_click(move |_, _, cx| {
            this.update(cx, |bench, cx| {
                run(bench, &agent);
                cx.notify();
            })
            .ok();
        })
    };
    let line = |icon: Icon, said: String| {
        div()
            .px(px(22.))
            .py(px(6.))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(ui::icon(icon, 14.).text_color(t.danger))
            .child(
                div()
                    .min_w_0()
                    .text_size(px(13.))
                    .text_color(t.text_secondary)
                    .child(said),
            )
    };
    let dropped = matches!(
        head.connection,
        ChatConnection::Error | ChatConnection::Stopped
    );
    if dropped && !chat.is_empty(cx) {
        let said = head
            .ended
            .as_ref()
            .and_then(|ended| ended.message.clone())
            .map_or_else(
                || "This session dropped.".to_owned(),
                |why| format!("This session dropped: {why}"),
            );
        let row = line(Icon::Plug, said)
            .child(act("Reconnect", "chat-reconnect", |bench, agent| {
                bench.reconnect(agent)
            }))
            .child(act("Start new chat", "chat-start-over", |bench, agent| {
                bench.start_over(agent)
            }));
        return Some(centred(row).into_any_element());
    }
    let error = head.error.clone()?;
    let failure = head.failure.as_ref();
    let network = failure.is_some_and(|failure| failure.kind == FailureKind::Network);
    let said = if network {
        format!(
            "Lost connection to {}. Sikemux sends it again when you're back online.",
            served_by(&agent.provider)
        )
    } else {
        error
    };
    let mut row = line(Icon::Warning, said);
    if network {
        row = row
            .child(act("Retry now", "chat-retry", |bench, agent| {
                bench.account_call("retryFailed", agent)
            }))
            .child(act("Discard", "chat-discard", |bench, agent| {
                bench.account_call("discardFailed", agent)
            }));
    }
    let resumes = failure.and_then(|failure| {
        failure
            .resumes_at
            .filter(|_| failure.kind == FailureKind::Limit)
    });
    if resumes.is_some_and(|at| at > now_ms()) {
        row = row.child(act("Don't resume", "chat-cancel-resume", |bench, agent| {
            bench.account_call("cancelResume", agent)
        }));
    }
    let details = failure
        .and_then(|failure| failure.details.clone())
        .filter(|_| network);
    let details = details.map(|details| {
        div().px(px(44.)).pb(px(6.)).child(
            ui::mono(details, 10.5, cx)
                .whitespace_normal()
                .text_color(t.text_tertiary),
        )
    });
    Some(centred(div().flex().flex_col().child(row).children(details)).into_any_element())
}

/// A line at the end of the rows, in the column the rows are read in.
fn centred(line: Div) -> Div {
    div()
        .w_full()
        .flex()
        .justify_center()
        .child(line.w_full().max_w(px(800.)))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// How much of a plan is shown before the rest is cut, so a runaway plan cannot slow the chat.
const MOST_PLAN: usize = 120_000;

/// The plan the agent keeps for the turn, folded behind its name until the reader opens it.
pub(super) fn plan(
    agent: &AgentRow,
    chat: &Chat,
    cx: &mut Context<Workbench>,
) -> Option<AnyElement> {
    let plan = chat.state.as_ref()?.plan.as_ref()?;
    let t = Tokens::get(cx).clone();
    let open = chat.plan_open;
    let this = cx.weak_entity();
    let agent = agent.id.clone();
    let summary = div()
        .id("chat-plan")
        .flex()
        .items_center()
        .gap(px(5.))
        .text_size(px(11.))
        .text_color(t.text_disabled)
        .cursor_pointer()
        .child(
            ui::icon(Icon::Chevron, 10.)
                .when(open, |chevron| {
                    chevron.with_transformation(gpui_kit::Transformation::rotate(
                        gpui_kit::radians(std::f32::consts::FRAC_PI_2),
                    ))
                })
                .text_color(t.text_disabled),
        )
        .child("Plan")
        .on_click(move |_, _, cx| {
            this.update(cx, |bench, cx| {
                if let Some(chat) = bench.chats.get_mut(&agent) {
                    chat.plan_open = !chat.plan_open;
                }
                bench.parts.redraw(&bench.parts.stage, cx);
            })
            .ok();
        });
    let shown = open.then(|| {
        let mut text = match plan {
            serde_json::Value::String(text) => text.clone(),
            plan => serde_json::to_string_pretty(plan).unwrap_or_default(),
        };
        if text.len() > MOST_PLAN {
            let cut = text.floor_char_boundary(MOST_PLAN);
            text.truncate(cut);
            text.push_str("\n… output truncated");
        }
        div()
            .id("chat-plan-text")
            .mt(px(8.))
            .max_h(px(420.))
            .overflow_y_scroll()
            .px(px(12.))
            .py(px(11.))
            .border_1()
            .border_color(t.line_soft)
            .rounded(crate::theme::RADIUS_2)
            .child(
                ui::mono(text, 11., cx)
                    .whitespace_normal()
                    .text_color(t.text_tertiary),
            )
    });
    Some(
        centred(
            div()
                .px(px(22.))
                .py(px(6.))
                .flex()
                .flex_col()
                .child(summary)
                .children(shown),
        )
        .into_any_element(),
    )
}
