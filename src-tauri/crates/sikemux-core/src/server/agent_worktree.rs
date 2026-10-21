//! Taking away the worktree an agent was given. A client first asks what
//! removing it would cost, so it can ask the person; then it removes it with
//! their answers, and the agent goes back to the main checkout.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::connection::blocking;
use super::sandbox::AgentWorktree;
use super::{Core, CoreError, CoreResult};
use crate::git::Git;
use crate::protocol::WorkspaceAgent;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorktreeParams {
    agent_id: String,
}

/// What removing an agent's worktree would cost.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Removal {
    branch: String,
    path: PathBuf,
    /// The agent is at work, so it keeps its worktree until its turn ends.
    busy: bool,
    /// Other agents and projects in the worktree, by name, which keep it.
    shared_with: Vec<String>,
    /// Files with changes not committed, which removing it loses.
    dirty_files: usize,
    /// Commits on its branch its base does not have, which deleting the
    /// branch loses.
    unmerged_commits: usize,
}

fn worktree_of(core: &Core, agent_id: &str) -> CoreResult<(WorkspaceAgent, AgentWorktree)> {
    let agent = core
        .workspaces
        .agent(agent_id)
        .ok_or_else(|| CoreError::from("that agent is no longer on this host"))?;
    let worktree = agent
        .worktree
        .clone()
        .and_then(|worktree| serde_json::from_value(worktree).ok())
        .ok_or_else(|| CoreError::from("that agent works in the main checkout"))?;
    Ok((agent, worktree))
}

fn sharers(core: &Core, agent_id: &str, worktree: &Path) -> Vec<String> {
    let projects = core
        .workspaces
        .project_paths()
        .into_iter()
        .filter(|path| path.starts_with(worktree))
        .map(|path| path.to_string_lossy().into_owned());
    let agents = core
        .workspaces
        .agent_ids()
        .into_iter()
        .filter(|id| id != agent_id)
        .filter_map(|id| core.workspaces.agent(&id))
        .filter(|other| other.cwd.starts_with(worktree))
        .map(|other| core.names.name(&other.id).unwrap_or(other.title));
    projects.chain(agents).collect()
}

fn count(output: Result<String, String>) -> usize {
    output.map_or(0, |text| {
        text.lines().filter(|line| !line.is_empty()).count()
    })
}

fn costs(worktree: &AgentWorktree) -> (usize, usize) {
    let dirty = count(Git::at(&worktree.path).text(&["status", "--porcelain"]));
    let base = worktree.base.as_deref().unwrap_or(&worktree.start_sha);
    let unmerged = Git::at(&worktree.repo)
        .text(&[
            "rev-list",
            "--count",
            &format!("{base}..{}", worktree.branch),
        ])
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(1);
    (dirty, unmerged)
}

fn busy(core: &Core, agent_id: &str) -> bool {
    core.chats
        .get(agent_id)
        .is_some_and(|chat| chat.state_now() != "idle")
}

pub(crate) async fn removal(core: &Core, params: WorktreeParams) -> CoreResult<Removal> {
    let (_, worktree) = worktree_of(core, &params.agent_id)?;
    let shared_with = sharers(core, &params.agent_id, &worktree.path);
    let busy = busy(core, &params.agent_id);
    let (dirty_files, unmerged_commits) = {
        let worktree = worktree.clone();
        blocking(move || Ok(costs(&worktree))).await?
    };
    Ok(Removal {
        branch: worktree.branch,
        path: worktree.path,
        busy,
        shared_with,
        dirty_files,
        unmerged_commits,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoveParams {
    agent_id: String,
    /// The person agreed to lose the worktree's uncommitted changes.
    #[serde(default)]
    discard_changes: bool,
    /// The person agreed to lose commits only its branch has.
    #[serde(default)]
    delete_branch: bool,
}

/// What removing did to the branch.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Removed {
    branch_deleted: bool,
}

/// The agent's folder once the worktree is gone: the same place in the main
/// checkout.
fn outside(worktree: &AgentWorktree, cwd: &Path) -> PathBuf {
    match cwd.strip_prefix(&worktree.path) {
        Ok(inside) => worktree.repo.join(inside),
        Err(_) => worktree.repo.clone(),
    }
}

pub(crate) async fn remove(core: &Core, params: RemoveParams) -> CoreResult<Removed> {
    let (agent, worktree) = worktree_of(core, &params.agent_id)?;
    if busy(core, &params.agent_id) {
        return Err("stop the agent's turn before removing its worktree".into());
    }
    if !sharers(core, &params.agent_id, &worktree.path).is_empty() {
        return Err("another agent or project still uses this worktree".into());
    }
    let (discard, delete) = (params.discard_changes, params.delete_branch);
    let deleted = {
        let worktree = worktree.clone();
        blocking(move || {
            let (dirty, unmerged) = costs(&worktree);
            if dirty > 0 && !discard {
                return Err(CoreError::from(format!(
                    "{dirty} uncommitted change{} would be lost",
                    if dirty == 1 { "" } else { "s" }
                )));
            }
            let path = worktree.path.to_string_lossy().into_owned();
            let mut args = vec!["worktree", "remove"];
            if discard {
                args.push("--force");
            }
            args.push(&path);
            Git::at(&worktree.repo)
                .text(&args)
                .map_err(CoreError::from)?;
            let deleted = unmerged == 0 || delete;
            if deleted {
                Git::at(&worktree.repo)
                    .text(&["branch", "-D", &worktree.branch])
                    .map_err(CoreError::from)?;
            }
            Ok(deleted)
        })
        .await?
    };
    let mut patch = Map::new();
    patch.insert("worktree".into(), Value::Null);
    patch.insert(
        "cwd".into(),
        Value::String(
            outside(&worktree, &agent.cwd)
                .to_string_lossy()
                .into_owned(),
        ),
    );
    core.workspaces.update_agent(&params.agent_id, patch)?;
    core.view_changed();
    Ok(Removed {
        branch_deleted: deleted,
    })
}
