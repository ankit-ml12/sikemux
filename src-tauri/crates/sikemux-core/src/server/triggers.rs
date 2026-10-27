//! Triggers an agent sets: each, when its time comes, once or again and again, sends the
//! agent's own chat a prompt, and goes when the chat closes, or adds a card to its project's
//! board, which Auto carries when it is on, and stays until it is removed.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sikemux_process::clock::epoch_ms;
use sikemux_process::sync::Locked;
use sikemux_wire::chat::{ChatTrigger, TriggerWhen};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use super::chat::reset::{calendar, when, WallClock, Zone};
use super::kept::Kept;
use super::messages;
use super::wake::Wake;
use super::Core;
use crate::cli::protocol::HarnessRequest;

const FILE: &str = "triggers.json";
const MAX_PER_AGENT: usize = 20;
const MAX_PER_BOARD: usize = 20;
const MAX_TRIGGERS: usize = 512;
const MAX_TITLE_CHARS: usize = 120;
const MAX_PROMPT_CHARS: usize = 8192;
const MIN_EVERY_SECONDS: u64 = 60;
const MAX_AHEAD_SECONDS: u64 = 366 * 24 * 60 * 60;
/// A repeating trigger whose time passed while the host was off fires once
/// when the core comes back within this long, and otherwise waits for its next time.
const LATE_GRACE_MS: u64 = 15 * 60 * 1000;
const WEEKDAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Trigger {
    id: u64,
    /// The chat that set it, which its prompt goes to unless it adds cards.
    agent_id: String,
    /// The project whose board it adds a card to when it fires, rather than prompting the chat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    board: Option<String>,
    title: String,
    prompt: String,
    when: TriggerWhen,
    next_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    next_id: u64,
    triggers: Vec<Trigger>,
}

pub(crate) struct Triggers {
    kept: Kept,
    state: Mutex<Saved>,
    pub(crate) wake: Wake,
}

/// A trigger whose time came, and whether it was due long before the core could send it.
struct Firing {
    trigger: Trigger,
    late_since: Option<u64>,
    last: bool,
}

impl Triggers {
    pub(crate) fn new(data_dir: Option<&std::path::Path>) -> Self {
        let kept = Kept::new(data_dir, FILE);
        let state = Mutex::new(kept.load());
        Self {
            kept,
            state,
            wake: Wake::default(),
        }
    }

    /// The chat's triggers, soonest first, as its clients show them.
    pub(crate) fn shown(&self, agent_id: &str) -> Vec<ChatTrigger> {
        self.shown_where(|trigger| trigger.board.is_none() && trigger.agent_id == agent_id)
    }

    /// The triggers that add cards to the project's board, soonest first.
    pub(crate) fn on_board(&self, project: &str) -> Vec<ChatTrigger> {
        self.shown_where(|trigger| trigger.board.as_deref() == Some(project))
    }

    fn shown_where(&self, keep: impl Fn(&Trigger) -> bool) -> Vec<ChatTrigger> {
        let state = self.state.locked();
        let mut shown: Vec<ChatTrigger> = state
            .triggers
            .iter()
            .filter(|trigger| keep(trigger))
            .map(|trigger| ChatTrigger {
                id: trigger.id,
                title: trigger.title.clone(),
                when: trigger.when.clone(),
                next_at: trigger.next_at,
            })
            .collect();
        shown.sort_by_key(|trigger| trigger.next_at);
        shown
    }

    /// When the soonest trigger that prompts the agent's chat fires.
    pub(crate) fn next_at(&self, agent_id: &str) -> Option<u64> {
        let state = self.state.locked();
        state
            .triggers
            .iter()
            .filter(|trigger| trigger.board.is_none() && trigger.agent_id == agent_id)
            .map(|trigger| trigger.next_at)
            .min()
    }

    fn change<T>(&self, change: impl FnOnce(&mut Saved) -> T) -> T {
        let mut state = self.state.locked();
        let out = change(&mut state);
        self.kept.save(&*state);
        drop(state);
        self.wake.now();
        out
    }

    /// Takes out the triggers whose time has come, moving repeating ones on
    /// to their next time, and says when the next one left is due.
    fn take_due(&self, now: u64) -> (Vec<Firing>, Option<u64>) {
        let mut state = self.state.locked();
        let mut firing = Vec::new();
        let mut changed = false;
        state.triggers.retain_mut(|trigger| {
            if trigger.next_at > now {
                return true;
            }
            changed = true;
            let due = trigger.next_at;
            let late = now.saturating_sub(due) > LATE_GRACE_MS;
            let next = next_after(&trigger.when, due, now);
            let last = next.is_none();
            if !late || last {
                firing.push(Firing {
                    trigger: trigger.clone(),
                    late_since: late.then_some(due),
                    last,
                });
            }
            match next {
                Some(next) => {
                    trigger.next_at = next;
                    true
                }
                None => false,
            }
        });
        if changed {
            self.kept.save(&*state);
        }
        let next = state.triggers.iter().map(|trigger| trigger.next_at).min();
        (firing, next)
    }

    /// The chats with triggers that prompt them.
    fn agents(&self) -> Vec<String> {
        let mut agents: Vec<String> = self
            .state
            .locked()
            .triggers
            .iter()
            .filter(|trigger| trigger.board.is_none())
            .map(|trigger| trigger.agent_id.clone())
            .collect();
        agents.sort();
        agents.dedup();
        agents
    }
}

/// When a trigger that was due at `due` fires next after `now`; none for one that fired its
/// only time.
fn next_after(when: &TriggerWhen, due: u64, now: u64) -> Option<u64> {
    match when {
        TriggerWhen::Once { .. } => None,
        TriggerWhen::Every { seconds } => {
            let step = seconds * 1000;
            let behind = now.saturating_sub(due) / step + 1;
            Some(due + behind * step)
        }
        TriggerWhen::Daily { minute, weekdays } => next_daily(*minute, weekdays, now),
    }
}

/// The first time after `now` that the host's clock reads `minute` past midnight on one of
/// `weekdays`, or on any day when there are none.
fn next_daily(minute: u32, weekdays: &[u8], now: u64) -> Option<u64> {
    let clock = WallClock {
        zone: Zone::Host,
        date: None,
        hour: (minute / 60) as i32,
        minute: (minute % 60) as i32,
    };
    (0..=7)
        .filter_map(|days_later| clock.on_day(now, days_later))
        .filter(|at| *at > now)
        .find(|at| {
            weekdays.is_empty()
                || calendar(*at, Zone::Host)
                    .is_some_and(|tm| weekdays.contains(&(tm.tm_wday as u8)))
        })
}

/// Sends what came due, closes the triggers of chats that are gone, and waits for the next.
pub(crate) fn settle(core: &Arc<Core>) {
    let gone: Vec<String> = core
        .triggers
        .agents()
        .into_iter()
        .filter(|agent_id| core.workspaces.closed(agent_id))
        .collect();
    if !gone.is_empty() {
        core.triggers.change(|state| {
            state
                .triggers
                .retain(|trigger| trigger.board.is_some() || !gone.contains(&trigger.agent_id))
        });
    }
    let (firing, next) = core.triggers.take_due(epoch_ms());
    if let Some(next) = next {
        core.triggers.wake.at(next);
    }
    for firing in &firing {
        show(core, &firing.trigger);
    }
    for firing in firing {
        tokio::spawn(fire(core.clone(), firing));
    }
}

async fn fire(core: Arc<Core>, firing: Firing) {
    let trigger = &firing.trigger;
    if let Some(project) = &trigger.board {
        if let Err(error) =
            super::board::add_for_trigger(&core, project, &trigger.title, &trigger.prompt)
        {
            eprintln!(
                "sikemux core: trigger {} added no card to {project}'s board: {error}",
                trigger.id
            );
        }
        return;
    }
    let mut text = format!("{}\n\n{}", trigger.title, trigger.prompt);
    if let Some(due) = firing.late_since {
        text.push_str(&format!(
            "\n\n(This was due {}, while this host was off.)",
            when(due)
        ));
    }
    let after = match firing.last {
        true => "It will not fire again.".to_owned(),
        false => core
            .triggers
            .shown(&trigger.agent_id)
            .iter()
            .find(|shown| shown.id == trigger.id)
            .map_or_else(String::new, |shown| {
                format!(
                    "It fires again {}; trigger_remove {} stops it.",
                    when(shown.next_at),
                    trigger.id
                )
            }),
    };
    let note = format!(
        "Sent by trigger {} that this chat set with trigger_add, not by the person or another agent. {after}",
        trigger.id
    );
    if let Err(error) = messages::from_trigger(
        &core,
        trigger.agent_id.clone(),
        text,
        note.trim().to_owned(),
    )
    .await
    {
        eprintln!(
            "sikemux core: trigger {} did not reach chat {}: {error}",
            trigger.id, trigger.agent_id
        );
    }
}

/// Tells the clients of the trigger's chat or board what triggers it has now, and every
/// device's view when the chat's next one fires.
fn show(core: &Core, trigger: &Trigger) {
    if let Some(project) = &trigger.board {
        super::board::triggers_changed(core, project);
        return;
    }
    if let Some(feed) = core.chats.heard(&trigger.agent_id) {
        feed.set_triggers(core.triggers.shown(&trigger.agent_id));
    }
    core.view_changed();
}

fn caller(request: &HarnessRequest, tool: &str) -> Result<String, String> {
    request
        .agent_id
        .clone()
        .ok_or_else(|| format!("{tool} needs the calling agent's id"))
}

pub(crate) fn add(core: &Arc<Core>, request: &HarnessRequest) -> Result<Value, String> {
    let agent_id = caller(request, "trigger_add")?;
    let params = &request.params;
    let title = text(params, "title", MAX_TITLE_CHARS)?;
    let prompt = text(params, "prompt", MAX_PROMPT_CHARS)?;
    let board = match params.get("board") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => None,
        Some(Value::Bool(true)) => Some(super::board::caller_project(core, request)),
        Some(_) => return Err("board must be a boolean".into()),
    };
    let now = epoch_ms();
    let (rule, next_at) = when_of(params, now)?;
    let trigger = core.triggers.change(|state| {
        let (own, most, whose) = match &board {
            Some(project) => (
                state
                    .triggers
                    .iter()
                    .filter(|trigger| trigger.board.as_ref() == Some(project))
                    .count(),
                MAX_PER_BOARD,
                "This project's board",
            ),
            None => (
                state
                    .triggers
                    .iter()
                    .filter(|trigger| trigger.board.is_none() && trigger.agent_id == agent_id)
                    .count(),
                MAX_PER_AGENT,
                "This chat",
            ),
        };
        if own >= most {
            return Err(format!(
                "{whose} already has {most} triggers; remove one with trigger_remove first"
            ));
        }
        if state.triggers.len() >= MAX_TRIGGERS {
            return Err(format!(
                "This host already holds {MAX_TRIGGERS} triggers, so it takes no more"
            ));
        }
        state.next_id += 1;
        let trigger = Trigger {
            id: state.next_id,
            agent_id: agent_id.clone(),
            board,
            title,
            prompt,
            when: rule,
            next_at,
        };
        state.triggers.push(trigger.clone());
        Ok(trigger)
    })?;
    show(core, &trigger);
    let note = match trigger.board {
        Some(_) => "It adds a card to this project's board then, titled as the trigger with its prompt as the details, which Auto carries when it is on. It stays after this chat closes.",
        None => "It sends this chat its prompt then, as a message you read like any other.",
    };
    Ok(json!({
        "trigger": trigger.id,
        "next": when(next_at),
        "nextAt": next_at,
        "note": note,
    }))
}

pub(crate) fn list(core: &Arc<Core>, request: &HarnessRequest) -> Result<Value, String> {
    let agent_id = caller(request, "trigger_list")?;
    let project = super::board::caller_project(core, request);
    let state = core.triggers.state.locked();
    let mut triggers: Vec<&Trigger> = state
        .triggers
        .iter()
        .filter(|trigger| yours(trigger, &agent_id, &project))
        .collect();
    triggers.sort_by_key(|trigger| trigger.next_at);
    let listed: Vec<Value> = triggers
        .into_iter()
        .map(|trigger| {
            json!({
                "trigger": trigger.id,
                "title": trigger.title,
                "prompt": trigger.prompt,
                "when": trigger.when,
                "board": trigger.board.is_some(),
                "next": when(trigger.next_at),
                "nextAt": trigger.next_at,
            })
        })
        .collect();
    Ok(json!({ "triggers": listed }))
}

pub(crate) fn remove_by_agent(core: &Arc<Core>, request: &HarnessRequest) -> Result<Value, String> {
    let agent_id = caller(request, "trigger_remove")?;
    let project = super::board::caller_project(core, request);
    let id = request
        .params
        .get("trigger")
        .and_then(Value::as_u64)
        .ok_or("trigger must be the number trigger_add or trigger_list gave")?;
    if remove_where(core, |trigger| {
        trigger.id == id && yours(trigger, &agent_id, &project)
    })
    .is_none()
    {
        return Err(format!(
            "Neither this chat nor its project's board has trigger {id}; trigger_list shows them"
        ));
    }
    Ok(json!({ "removed": id }))
}

/// A trigger of the agent's own chat, or of its project's board.
fn yours(trigger: &Trigger, agent_id: &str, project: &str) -> bool {
    match &trigger.board {
        Some(board) => board == project,
        None => trigger.agent_id == agent_id,
    }
}

/// Removes trigger `id` at the person's word, saying whether there was one.
pub(crate) fn remove(core: &Core, id: u64) -> bool {
    remove_where(core, |trigger| trigger.id == id).is_some()
}

fn remove_where(core: &Core, matches: impl Fn(&Trigger) -> bool) -> Option<Trigger> {
    let removed = core.triggers.change(|state| {
        let at = state.triggers.iter().position(&matches)?;
        Some(state.triggers.remove(at))
    })?;
    show(core, &removed);
    Some(removed)
}

/// Puts the chat's triggers in its feed, as it starts or wakes.
pub(crate) fn seed(core: &Core, feed: &super::chat::feed::Feed, agent_id: &str) {
    feed.set_triggers(core.triggers.shown(agent_id));
}

fn text(params: &Value, key: &str, max: usize) -> Result<String, String> {
    let value = params
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} must be text"))?;
    if value.chars().count() > max {
        return Err(format!("{key} must be at most {max} characters"));
    }
    Ok(value.to_owned())
}

/// The one time rule the call names, and when it first fires.
fn when_of(params: &Value, now: u64) -> Result<(TriggerWhen, u64), String> {
    let given: Vec<&str> = ["in", "at", "every", "daily"]
        .into_iter()
        .filter(|key| params.get(*key).is_some_and(|value| !value.is_null()))
        .collect();
    let [key] = given.as_slice() else {
        return Err("Give exactly one of in, at, every or daily".into());
    };
    let value = &params[*key];
    let latest = now + MAX_AHEAD_SECONDS * 1000;
    match *key {
        "in" => {
            let seconds = seconds(value, "in")?;
            let at = now + seconds * 1000;
            Ok((TriggerWhen::Once { at }, at))
        }
        "at" => {
            let at = value
                .as_str()
                .and_then(|text| OffsetDateTime::parse(text, &Rfc3339).ok())
                .ok_or("at must be a time such as 2026-10-28T09:30:00+05:30")?;
            let at = u64::try_from(at.unix_timestamp())
                .ok()
                .and_then(|seconds| seconds.checked_mul(1000))
                .filter(|at| *at > now)
                .ok_or("at must be later than now")?;
            if at > latest {
                return Err("at must be within a year".into());
            }
            Ok((TriggerWhen::Once { at }, at))
        }
        "every" => {
            let seconds = seconds(value, "every")?;
            Ok((TriggerWhen::Every { seconds }, now + seconds * 1000))
        }
        _ => {
            let (minute, weekdays) = daily(value, params.get("weekdays"))?;
            let at = next_daily(minute, &weekdays, now)
                .ok_or("That time does not come on this host's clock")?;
            Ok((TriggerWhen::Daily { minute, weekdays }, at))
        }
    }
}

fn seconds(value: &Value, key: &str) -> Result<u64, String> {
    value
        .as_u64()
        .filter(|seconds| (MIN_EVERY_SECONDS..=MAX_AHEAD_SECONDS).contains(seconds))
        .ok_or_else(|| {
            format!("{key} must be a whole number of seconds from {MIN_EVERY_SECONDS} to a year")
        })
}

fn daily(value: &Value, weekdays: Option<&Value>) -> Result<(u32, Vec<u8>), String> {
    let minute = value
        .as_str()
        .and_then(|text| text.split_once(':'))
        .and_then(|(hour, minute)| Some((hour.parse::<u32>().ok()?, minute.parse::<u32>().ok()?)))
        .filter(|(hour, minute)| *hour < 24 && *minute < 60)
        .map(|(hour, minute)| hour * 60 + minute)
        .ok_or("daily must be a time on this host's clock such as 09:30")?;
    let mut days = Vec::new();
    for day in weekdays.and_then(Value::as_array).into_iter().flatten() {
        let day = day
            .as_str()
            .map(str::to_lowercase)
            .and_then(|day| WEEKDAYS.iter().position(|known| day.starts_with(known)))
            .ok_or("weekdays must be days such as mon, tue or sat")?;
        days.push(day as u8);
    }
    days.sort_unstable();
    days.dedup();
    if days.len() == WEEKDAYS.len() {
        days.clear();
    }
    Ok((minute, days))
}

/// A request one of this host's webhooks received, as its sender made it.
pub(crate) struct HookDelivery {
    pub hook_id: String,
    pub delivery_id: String,
    /// Milliseconds since the Unix epoch.
    pub received_at: u64,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub query: String,
    pub body: Vec<u8>,
}

/// Takes a webhook's request. An error leaves it with the accounts server, which sends it again
/// later.
pub(crate) fn delivered(_core: &Arc<Core>, delivery: HookDelivery) -> Result<(), String> {
    eprintln!(
        "sikemux core: webhook {} received request {} at {}: {} with {} headers, query {:?} and {} bytes",
        delivery.hook_id,
        delivery.delivery_id,
        delivery.received_at,
        delivery.method,
        delivery.headers.len(),
        delivery.query,
        delivery.body.len()
    );
    Ok(())
}
