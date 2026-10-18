//! When the core puts a chat to sleep and wakes it. A chat no client has on
//! screen sleeps once it has been idle for a while, the oldest first when more
//! than a few sit idle, and one a client brings on screen wakes again. A chat
//! at work, or one the person keeps alive, is never put to sleep.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::remote::unix_ms;
use super::{launching, Core, CoreResult};

/// How long a chat nobody looks at stays idle before it sleeps.
const IDLE_SLEEP_MS: u64 = 10 * 60_000;
/// How many idle chats nobody looks at stay awake, so the ones switched
/// back to most come back at once.
const MAX_WARM: usize = 3;
const CHECK_EVERY: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct Lifecycle {
    /// Agents whose chat is starting, so two clients waking one start it once.
    starting: Mutex<HashSet<String>>,
}

/// Holds an agent's place among those starting until it is dropped.
struct Starting<'a> {
    lifecycle: &'a Lifecycle,
    agent_id: String,
}

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        if let Ok(mut starting) = self.lifecycle.starting.lock() {
            starting.remove(&self.agent_id);
        }
    }
}

impl Lifecycle {
    fn claim(&self, agent_id: &str) -> Option<Starting<'_>> {
        let mut starting = self.starting.lock().ok()?;
        starting.insert(agent_id.to_owned()).then(|| Starting {
            lifecycle: self,
            agent_id: agent_id.to_owned(),
        })
    }
}

/// Starts the agent's chat as its record says unless it runs already or
/// another client is starting it, and answers once it runs.
pub(crate) async fn ensure_running(core: &Arc<Core>, agent_id: &str) -> CoreResult<()> {
    loop {
        if core.chats.get(agent_id).is_some() {
            return Ok(());
        }
        if let Some(_starting) = core.lifecycle.claim(agent_id) {
            if core.chats.get(agent_id).is_none() {
                launching::start_agent(core, None, agent_id).await?;
            }
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Wakes the sleeping agents among those a client just brought on screen.
pub(crate) fn wake_shown(core: &Arc<Core>, shown: Vec<String>) {
    for agent_id in shown {
        let asleep = core
            .workspaces
            .agent(&agent_id)
            .is_some_and(|agent| agent.asleep);
        if !asleep {
            continue;
        }
        let core = core.clone();
        tokio::spawn(async move {
            if let Err(error) = ensure_running(&core, &agent_id).await {
                eprintln!("sikemux core: {agent_id} did not wake: {error}");
            }
        });
    }
}

/// Puts the agent's chat to sleep unless something in it is still running,
/// keeping how it ran so it wakes on its saved conversation. Answers whether
/// it is asleep now.
pub(crate) fn sleep(core: &Core, agent_id: &str) -> bool {
    let chat = core.chats.get(agent_id);
    if let Some(chat) = &chat {
        core.workspaces.note_stopping(chat);
    }
    let stopped = core.chats.stop_if_idle(agent_id);
    if stopped && chat.is_some() {
        core.workspaces.set_asleep(agent_id, true);
        core.note_agent_gone(agent_id);
    }
    stopped
}

/// Puts every idle chat to sleep now, on screen or not, but those the person
/// keeps alive. Answers how many slept.
pub(crate) fn sleep_idle(core: &Core) -> usize {
    core.chats
        .ids()
        .into_iter()
        .filter(|agent_id| kept(core, agent_id) && idle(core, agent_id))
        .filter(|agent_id| sleep(core, agent_id))
        .count()
}

/// Puts to sleep the chats that have sat idle off every screen long enough.
pub(crate) async fn serve(core: Arc<Core>) {
    let began = unix_ms();
    let mut ticker = tokio::time::interval(CHECK_EVERY);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        for agent_id in due(&core, began, unix_ms()) {
            sleep(&core, &agent_id);
        }
    }
}

/// The chats to sleep now: every one idle off screen for [`IDLE_SLEEP_MS`],
/// and the oldest beyond [`MAX_WARM`] of the rest.
fn due(core: &Core, began: u64, now: u64) -> Vec<String> {
    let mut idle: Vec<(u64, String)> = core
        .chats
        .ids()
        .into_iter()
        .filter(|agent_id| may_sleep(core, agent_id))
        .map(|agent_id| {
            let quiet = core.seen.quiet_since(&agent_id).unwrap_or(began).max(began);
            (quiet, agent_id)
        })
        .collect();
    idle.sort();
    let stale = idle
        .iter()
        .filter(|(quiet, _)| now.saturating_sub(*quiet) >= IDLE_SLEEP_MS)
        .count();
    let sleeping = stale.max(idle.len().saturating_sub(MAX_WARM));
    idle.into_iter()
        .take(sleeping)
        .map(|(_, agent_id)| agent_id)
        .collect()
}

/// Whether the chat could sleep now: it is in the workspace, the person does
/// not keep it alive, no client has it on screen, and it rests on a session
/// it can be woken on.
fn may_sleep(core: &Core, agent_id: &str) -> bool {
    kept(core, agent_id) && !core.seen.viewed(agent_id) && idle(core, agent_id)
}

fn kept(core: &Core, agent_id: &str) -> bool {
    core.workspaces
        .agent(agent_id)
        .is_some_and(|agent| !agent.keep_alive)
}

fn idle(core: &Core, agent_id: &str) -> bool {
    core.chats
        .get(agent_id)
        .is_some_and(|chat| chat.state_now() == "idle")
}
