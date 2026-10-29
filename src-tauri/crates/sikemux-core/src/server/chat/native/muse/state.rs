//! What the chat knows of a Muse session while it runs: which turn answers
//! the prompt, which items are on screen, and its subagents. Each
//! notification comes back as the effects it has on the chat.

use sikemux_fold::update::{agent_text, notice};
use sikemux_fold::UpdateKind;
use std::collections::HashSet;

use serde_json::{json, Value};

use crate::acp::account::{Failure, FailureKind};

use super::super::super::states::{self, Outcome, Subagent};
use super::super::json::{text, text_or_empty};
use super::super::session::TurnEnd;
use super::super::turns::{self, Ended, Started, Turns};
use super::items::{Follow, Items};

/// What one notification does to the chat.
pub(super) enum Effect {
    /// A session update, for the session with this id.
    Update(String, Value),
    End(u64, TurnEnd),
    WorkStarted,
    WorkEnded(&'static str),
    Follow(Follow),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Session {
    pub session_id: String,
    pub cwd: String,
    /// Muse begins a turn on its own too, such as after a background task ended.
    pub turns: Turns,
    items: Items,
    /// The latest running token totals.
    pub totals: Option<Value>,
    /// How full the context window is.
    pub context: Option<Value>,
    /// The approval stages already put to the person.
    asked: HashSet<(String, String)>,
}

/// The failure a turn ended with, from Muse's error for it.
pub(super) fn failure(error: &Value) -> Failure {
    let kind = if !error["quota"].is_null() {
        FailureKind::Limit
    } else if text_or_empty(error, "kind") == "authRequired" {
        FailureKind::SignIn
    } else {
        FailureKind::Other
    };
    Failure {
        kind,
        title: text(error, "message").unwrap_or("Turn failed").to_owned(),
        details: None,
        resets_at: None,
    }
}

/// The tokens a turn used, as the chat reads a prompt's answer.
fn turn_usage(usage: &Value) -> Option<Value> {
    let count = |key: &str| usage[key].as_u64().unwrap_or(0);
    usage.is_object().then(|| {
        json!({
            "totalTokens": count("inputTokens") + count("outputTokens"),
            "inputTokens": count("inputTokens").saturating_sub(count("cachedTokens")),
            "cachedReadTokens": count("cachedTokens"),
            "outputTokens": count("outputTokens"),
            "thoughtTokens": count("reasoningTokens"),
        })
    })
}

/// Muse's to-do list as the chat's plan.
fn plan(params: &Value) -> Value {
    let entries: Vec<Value> = params["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|todo| {
            let status = match text_or_empty(todo, "status") {
                "inProgress" => "in_progress",
                "completed" | "cancelled" => "completed",
                _ => "pending",
            };
            json!({ "content": todo["text"], "status": status, "priority": "medium" })
        })
        .collect();
    json!({ "sessionUpdate": UpdateKind::Plan, "entries": entries })
}

impl Session {
    pub fn new(session_id: String, cwd: String) -> Self {
        Self {
            session_id,
            cwd,
            turns: Turns::default(),
            items: Items::default(),
            totals: None,
            context: None,
            asked: HashSet::new(),
        }
    }

    /// Muse took the interrupt of turn `id`. The turn is over for the chat
    /// now, and what Muse still says of it is dropped.
    pub fn interrupted(&mut self, id: &str) -> Vec<Effect> {
        match self.turns.interrupted(id) {
            Ended::Prompted(turn) => {
                let mut effects = self.finish_subagents(Subagent::Cancelled);
                effects.push(Effect::End(
                    turn.number,
                    TurnEnd::Completed(json!({ "stopReason": "cancelled" })),
                ));
                effects
            }
            Ended::Unprompted => {
                let mut effects = self.finish_subagents(Subagent::Cancelled);
                effects.push(Effect::WorkEnded("cancelled"));
                effects
            }
            Ended::Neither => Vec::new(),
        }
    }

    /// The conversation goes on in `session_id`, a copy of this one up to
    /// before turn `turn_id`.
    pub fn moved(&mut self, session_id: String, turn_id: &str) {
        self.session_id = session_id;
        self.turns.rewound(turn_id);
    }

    /// Whether this stage of an approval is new to the person.
    pub fn ask(&mut self, key: (String, String)) -> bool {
        self.asked.insert(key)
    }

    /// The chat session an approval belongs in: the subagent's that asked,
    /// when the chat has a card for it.
    pub fn session_for(&self, params: &Value) -> String {
        params["subagentOrigin"]["childSessionId"]
            .as_str()
            .filter(|child| self.items.is_child(child))
            .unwrap_or(&self.session_id)
            .to_owned()
    }

    fn finish_subagents(&mut self, state: Subagent) -> Vec<Effect> {
        self.items
            .finish_outstanding(state)
            .into_iter()
            .map(|(session, update)| Effect::Update(session, update))
            .collect()
    }

    fn turn_completed(&mut self, params: &Value) -> Vec<Effect> {
        let terminal = text_or_empty(params, "terminal");
        let outcome = states::outcome(terminal).unwrap_or(Outcome::Failed);
        match self.turns.completed(text_or_empty(params, "turnId")) {
            Ended::Unprompted => {
                let (subagents, reason) = turns::unprompted_end(outcome);
                let mut effects = subagents
                    .map(|state| self.finish_subagents(state))
                    .unwrap_or_default();
                effects.push(Effect::WorkEnded(reason));
                effects
            }
            Ended::Prompted(mut turn) => {
                let mut effects: Vec<Effect> = self
                    .turns
                    .title(&mut turn)
                    .map(|title| Effect::Update(self.session_id.clone(), title))
                    .into_iter()
                    .collect();
                let mut payload = json!({ "stopReason": "end_turn" });
                if let Some(usage) = turn_usage(&params["usage"]) {
                    payload["usage"] = usage;
                }
                let (subagents, end) = turns::prompted_end(outcome, payload, || {
                    let error = Some(params["error"].clone())
                        .filter(|error| error.is_object())
                        .unwrap_or_else(|| {
                            json!({
                                "kind": "hostDeath",
                                "message": text(params, "reason").unwrap_or("Turn failed"),
                                "retryable": true,
                            })
                        });
                    failure(&error)
                });
                if let Some(state) = subagents {
                    effects.extend(self.finish_subagents(state));
                }
                effects.push(Effect::End(turn.number, end));
                effects
            }
            Ended::Neither => Vec::new(),
        }
    }

    /// The effects of one notification from Muse.
    pub fn handle(&mut self, method: &str, params: &Value) -> Vec<Effect> {
        let session = text_or_empty(params, "sessionId").to_owned();
        let root = session == self.session_id;
        if !root && !self.items.is_child(&session) {
            return Vec::new();
        }
        let stale = params["turnId"]
            .as_str()
            .or_else(|| params["item"]["turnId"].as_str())
            .is_some_and(|turn| root && self.turns.is_stale(turn));
        let mut effects = Vec::new();
        let mut follow = Vec::new();
        let cwd = self.cwd.clone();
        match method {
            "item/started" | "item/updated" if !stale => {
                let item = &params["item"];
                if item["kind"] != "userMessage" {
                    effects.extend(
                        self.items
                            .opened(&session, item, &cwd, &mut follow)
                            .into_iter()
                            .map(|(session, update)| Effect::Update(session, update)),
                    );
                }
            }
            "item/completed" if !stale => {
                let item = &params["item"];
                if item["kind"] != "userMessage" {
                    effects.extend(
                        self.items
                            .completed(&session, item, &cwd, &mut follow)
                            .into_iter()
                            .map(|(session, update)| Effect::Update(session, update)),
                    );
                }
            }
            "item/delta" if !stale => {
                if let Some(update) = self.items.delta(&session, params) {
                    effects.push(Effect::Update(session.clone(), update));
                }
            }
            "turn/started" if root => {
                if let Started::Unprompted = self.turns.began(text_or_empty(params, "turnId")) {
                    effects.push(Effect::WorkStarted);
                }
            }
            "turn/completed" if root => effects.extend(self.turn_completed(params)),
            "turn/retryScheduled" if root && !stale => {
                effects.push(Effect::Update(
                    session.clone(),
                    notice(
                        "warning",
                        &format!(
                            "Retrying (attempt {} of {}): {}",
                            params["nextAttempt"],
                            params["maxAttempts"],
                            text_or_empty(params, "reason")
                        ),
                        None,
                    ),
                ));
            }
            "session/modelRouteUnserved" if root => {
                effects.push(Effect::Update(
                    session.clone(),
                    notice(
                        "warning",
                        "Muse could not reach the chosen model and used another",
                        None,
                    ),
                ));
            }
            "session/tokenUsage" if root => {
                self.totals = Some(params["cumulative"].clone());
            }
            "session/contextUsage" if root => {
                self.context = Some(params.clone());
                if let (Some(used), Some(size)) = (
                    params["usedTokens"].as_u64(),
                    params["windowTokens"].as_u64(),
                ) {
                    effects.push(Effect::Update(
                        session.clone(),
                        json!({ "sessionUpdate": UpdateKind::UsageUpdate, "used": used, "size": size }),
                    ));
                }
            }
            "session/nameChanged" if root => {
                if let Some(name) = text(params, "name") {
                    self.turns.titled = true;
                    effects.push(Effect::Update(
                        session.clone(),
                        json!({ "sessionUpdate": UpdateKind::SessionInfoUpdate, "title": name }),
                    ));
                }
            }
            "session/todoListChanged" => {
                effects.push(Effect::Update(session.clone(), plan(params)));
            }
            _ => {}
        }
        effects.extend(follow.into_iter().map(Effect::Follow));
        effects
    }

    /// Tells a subagent's card its conversation, read after it ended.
    pub fn told_subagent(&mut self, child: &str, items: &[Value]) -> Vec<Effect> {
        let cwd = self.cwd.clone();
        let mut effects = Vec::new();
        let mut follow = Vec::new();
        for item in items {
            if item["kind"] == "userMessage" {
                continue;
            }
            effects.extend(
                self.items
                    .completed(child, item, &cwd, &mut follow)
                    .into_iter()
                    .map(|(session, update)| Effect::Update(session, update)),
            );
        }
        effects
    }

    pub fn subagent_said(&mut self, child: &str, result: &str) -> Vec<Effect> {
        self.items.child_spoke(child);
        vec![Effect::Update(child.to_owned(), agent_text(result, None))]
    }
}
