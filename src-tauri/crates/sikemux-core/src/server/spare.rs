//! One shell started ahead of time for the project in front, at the size
//! terminals were last given, so a new terminal there opens on a prompt
//! instead of waiting on the person's rc files. At most one runs, and one
//! nobody takes is stopped after a while.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sikemux_process::clock::epoch_ms;
use sikemux_process::sync::Locked;

use crate::protocol::{SessionId, SpawnTarget, TerminalSpawn};
use sikemux_wire::pty::launch::PtyContext;

use super::wake::Wake;
use super::{session, Core};

/// How long the front project and the terminal size stay put before a shell
/// starts, so a burst of changes starts one.
const SETTLE_MS: u64 = 1_000;
/// A shell nobody takes in this long is stopped until something changes.
const IDLE_LIMIT_MS: u64 = 10 * 60 * 1_000;

#[derive(Default)]
pub(crate) struct Spares {
    pub(crate) wake: Wake,
    held: Mutex<Held>,
}

#[derive(Default)]
struct Held {
    size: Option<(u16, u16)>,
    spare: Option<Spare>,
    /// The pane id of a shell being started, hidden like a ready one.
    starting: Option<String>,
    changed_at: u64,
}

struct Spare {
    project: String,
    size: (u16, u16),
    id: SessionId,
    window_id: String,
    pane_id: String,
    started_at: u64,
}

/// A shell handed to a new terminal, with the ids it was started under for
/// the terminal to take as its own.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Taken {
    id: SessionId,
    window_id: String,
    pane_id: String,
}

#[derive(Deserialize)]
pub(crate) struct TakeParams {
    project: String,
}

impl Spares {
    /// Whether a session's pane is a shell started ahead, which no list shows.
    pub(crate) fn hides(&self, pane_id: Option<&str>) -> bool {
        let held = self.held.locked();
        pane_id.is_some_and(|pane| {
            held.starting.as_deref() == Some(pane)
                || held
                    .spare
                    .as_ref()
                    .is_some_and(|spare| spare.pane_id == pane)
        })
    }

    pub(crate) fn is_spare(&self, id: SessionId) -> bool {
        self.held
            .locked()
            .spare
            .as_ref()
            .is_some_and(|spare| spare.id == id)
    }

    /// A terminal was given this size; the next shell starts at it.
    pub(crate) fn note_size(&self, cols: u16, rows: u16) {
        let mut held = self.held.locked();
        if held.size != Some((cols, rows)) {
            held.size = Some((cols, rows));
            self.changed(&mut held);
        }
    }

    /// The project in front may have changed.
    pub(crate) fn front_changed(&self) {
        self.changed(&mut self.held.locked());
    }

    fn changed(&self, held: &mut Held) {
        held.changed_at = epoch_ms();
        self.wake.at(held.changed_at + SETTLE_MS);
    }
}

/// The shell started for `project`, for a new terminal there to take over.
pub(crate) fn take(core: &Core, params: TakeParams) -> Option<Taken> {
    let mut held = core.spares.held.locked();
    let fits = held.spare.as_ref()?.project == params.project
        && Some(held.spare.as_ref()?.size) == held.size;
    let spare = held.spare.take().filter(|_| fits)?;
    core.spares.changed(&mut held);
    Some(Taken {
        id: spare.id,
        window_id: spare.window_id,
        pane_id: spare.pane_id,
    })
}

/// Keeps one shell for the project in front at the last size, starting,
/// replacing or stopping it as they change and when it has gone untaken too long.
pub(crate) fn settle(core: &Arc<Core>) {
    let now = epoch_ms();
    let front = front_project(core);
    let mut held = core.spares.held.locked();
    if now < held.changed_at + SETTLE_MS {
        core.spares.wake.at(held.changed_at + SETTLE_MS);
        return;
    }
    let wanted = front.zip(held.size);
    if let Some(spare) = &held.spare {
        let fits = wanted.as_ref().is_some_and(|((project, _, _), size)| {
            *project == spare.project && *size == spare.size
        });
        let stale = now >= spare.started_at + IDLE_LIMIT_MS && spare.started_at >= held.changed_at;
        if fits && !stale {
            core.spares.wake.at(spare.started_at + IDLE_LIMIT_MS);
            return;
        }
        let id = spare.id;
        held.spare = None;
        stop(core, id);
        if stale {
            return;
        }
    }
    let (Some(((project, path, name), size)), None) = (wanted, &held.starting) else {
        return;
    };
    let pane_id = format!("pane-spare-{}", uuid::Uuid::new_v4().simple());
    held.starting = Some(pane_id.clone());
    drop(held);
    tokio::spawn(start(core.clone(), project, path, name, size, pane_id));
}

/// The project in front, by id, with its folder and name.
fn front_project(core: &Core) -> Option<(String, String, String)> {
    let id = super::layout::front_project(core)?;
    let workspace = core.workspace_view();
    let project = workspace
        .projects
        .into_iter()
        .find(|project| project.id == id)?;
    Some((
        id,
        project.path.to_string_lossy().into_owned(),
        project.name,
    ))
}

async fn start(
    core: Arc<Core>,
    project: String,
    path: String,
    name: String,
    size: (u16, u16),
    pane_id: String,
) {
    let window_id = format!("win-spare-{}", uuid::Uuid::new_v4().simple());
    let spawn = TerminalSpawn {
        cols: size.0,
        rows: size.1,
        cwd: Some(path.clone()),
        context: Some(PtyContext {
            session_id: project.clone(),
            session_name: name,
            session_kind: "project".into(),
            project: Some(path),
            window_id: Some(window_id.clone()),
            pane_id: Some(pane_id.clone()),
            agent_id: None,
            agent_type: None,
            initial_prompt_submitted: false,
            shell_integration: true,
        }),
        ..TerminalSpawn::default()
    };
    let target = Box::new(SpawnTarget::Terminal(spawn));
    let started =
        super::connection::spawn(core.clone(), core.launch_identity(), target, None).await;
    let pending = match started {
        Ok(pending) => pending,
        Err(error) => {
            eprintln!("sikemux core: a spare shell did not start: {error}");
            core.spares.held.locked().starting = None;
            return;
        }
    };
    let id = pending.id();
    pending.start(&core);
    let mut held = core.spares.held.locked();
    held.starting = None;
    held.spare = Some(Spare {
        project,
        size,
        id,
        window_id,
        pane_id,
        started_at: epoch_ms(),
    });
    drop(held);
    core.spares.wake.now();
}

/// Stops the shell started ahead, as before the core hands itself over.
pub(crate) fn discard(core: &Arc<Core>) {
    let spare = core.spares.held.locked().spare.take();
    if let Some(spare) = spare {
        stop(core, spare.id);
    }
}

fn stop(core: &Arc<Core>, id: SessionId) {
    if let Some(target) = core.take_session_for_kill(id) {
        let core = core.clone();
        tokio::task::spawn_blocking(move || session::kill(&core, &target));
    }
}
