//! The git panel's methods, `git.*`, for every client, and the watch on each
//! project that tells them all when to read again.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use git2::Repository;
use serde::Deserialize;
use serde_json::Value;

use crate::git::{
    ai, blame, branches, changes, commit, log, remote, revisions, stash, status, watch, worktree,
};

use super::access::{Needs, Peer};
use super::{Core, CoreError, CoreResult};

/// What a client must be allowed to call `method`.
pub(crate) fn needs(method: &str) -> Needs {
    if READS.contains(&method) {
        Needs::Watch
    } else if matches!(method, "git.push") {
        Needs::Person
    } else {
        Needs::Full
    }
}

/// Whether only Sikemux's own window or notch may call `method` from this
/// host; another program here may only read.
pub(crate) fn for_person(method: &str) -> bool {
    !READS.contains(&method)
}

const READS: [&str; 17] = [
    "git.status",
    "git.discoverRepos",
    "git.overview",
    "git.diff",
    "git.branches",
    "git.worktrees",
    "git.log",
    "git.show",
    "git.fileAt",
    "git.fileDiff",
    "git.commitFiles",
    "git.compare",
    "git.blame",
    "git.pullRequestUrl",
    "git.stashList",
    "git.remotes",
    "git.remoteBranches",
];

/// Each method's parameters by the names clients send them under, and the
/// git function it runs.
macro_rules! methods {
    ($method:expr, $params:expr; $($name:literal => $run:path { $($arg:ident: $ty:ty),* $(,)? }),* $(,)?) => {
        match $method {
            $($name => {
                #[derive(Deserialize)]
                #[serde(rename_all = "camelCase")]
                struct Params { $($arg: $ty),* }
                let Params { $($arg),* } = serde_json::from_value($params).map_err(unfit)?;
                Ok(serde_json::to_value($run($($arg),*).await?)?)
            })*
            _ => Err(format!("this host does not know {}", $method).into()),
        }
    };
}

pub(crate) async fn call(core: &Core, peer: &Peer, method: &str, params: Value) -> CoreResult<Value> {
    if matches!(peer, Peer::Device { .. }) {
        let asked = ["repo", "root"]
            .into_iter()
            .find_map(|key| params.get(key).and_then(Value::as_str))
            .map(PathBuf::from)
            .ok_or("the call names no repository")?;
        let projects = core.workspaces.project_paths();
        if !super::connection::blocking(move || Ok(of_a_project(&asked, &projects))).await? {
            return Err("a device may only look at the workspace's projects".into());
        }
    }
    methods!(method, params;
        "git.status" => status::git_status { repo: String },
        "git.discoverRepos" => status::git_discover_repos { root: String },
        "git.overview" => log::git_overview { repo: String },
        "git.log" => log::git_log { repo: String },
        "git.diff" => changes::git_diff { repo: String, path: String, staged: bool },
        "git.stage" => changes::git_stage { repo: String, path: String },
        "git.unstage" => changes::git_unstage { repo: String, path: String },
        "git.stagePaths" => changes::git_stage_paths { repo: String, paths: Vec<String> },
        "git.unstagePaths" => changes::git_unstage_paths { repo: String, paths: Vec<String> },
        "git.stageAll" => changes::git_stage_all { repo: String },
        "git.unstageAll" => changes::git_unstage_all { repo: String },
        "git.discardFiles" => changes::git_discard_files { repo: String, paths: Vec<String>, mode: String },
        "git.branches" => branches::git_branches { repo: String },
        "git.checkout" => branches::git_checkout { repo: String, branch: String },
        "git.checkoutSmart" => branches::git_checkout_smart { repo: String, branch: String },
        "git.branchCreate" => branches::git_branch_create { repo: String, name: String, start_point: Option<String> },
        "git.branchDelete" => branches::git_branch_delete { repo: String, name: String, force: bool },
        "git.branchRename" => branches::git_branch_rename { repo: String, old_name: String, new_name: String },
        "git.merge" => branches::git_merge { repo: String, branch: String },
        "git.mergeSquash" => branches::git_merge_squash { repo: String, branch: String },
        "git.reset" => branches::git_reset { repo: String, rev: String, mode: String },
        "git.revert" => branches::git_revert { repo: String, rev: String },
        "git.worktrees" => worktree::git_worktree_list { repo: String },
        "git.worktreeCreate" => worktree::git_worktree_create { repo: String, path: String, branch: String, create_branch: bool, start_point: Option<String> },
        "git.worktreeRemove" => worktree::git_worktree_remove { repo: String, path: String, force: bool },
        "git.show" => revisions::git_show { repo: String, rev: String },
        "git.fileAt" => revisions::git_file_at { repo: String, rev: String, path: String },
        "git.fileDiff" => revisions::git_file_diff { repo: String, path: String, base_rev: String, head_rev: Option<String>, full: bool },
        "git.commitFiles" => revisions::git_commit_files { repo: String, rev: String },
        "git.compare" => revisions::git_compare { repo: String, base: String, head: String },
        "git.blame" => blame::git_blame { repo: String, path: String, contents: Option<String> },
        "git.push" => commit::git_push { repo: String },
        "git.pull" => commit::git_pull { repo: String },
        "git.pullRequestUrl" => remote::pull_request_url { repo: String },
        "git.aiMessage" => ai::git_ai_message { repo: String, provider: Option<String>, model: Option<String> },
        "git.stashList" => stash::git_stash_list { repo: String },
        "git.stashPush" => stash::git_stash_push { repo: String, message: Option<String>, mode: String },
        "git.stashApply" => stash::git_stash_apply { repo: String, refname: String, sha: String },
        "git.stashPop" => stash::git_stash_pop { repo: String, refname: String, sha: String },
        "git.stashDrop" => stash::git_stash_drop { repo: String, refname: String, sha: String },
        "git.stashBranch" => stash::git_stash_branch { repo: String, refname: String, sha: String, name: String },
        "git.stashRename" => stash::git_stash_rename { repo: String, refname: String, sha: String, new_message: String },
        "git.remotes" => remote::git_remotes { repo: String },
        "git.remoteAdd" => remote::git_remote_add { repo: String, name: String, url: String },
        "git.remoteRemove" => remote::git_remote_remove { repo: String, name: String },
        "git.remoteRename" => remote::git_remote_rename { repo: String, old_name: String, new_name: String },
        "git.remoteSetUrl" => remote::git_remote_set_url { repo: String, name: String, url: String },
        "git.fetch" => remote::git_fetch { repo: String, remote: Option<String> },
        "git.fetchRef" => remote::git_fetch_ref { repo: String, remote: String, source: String, branch: String },
        "git.remoteBranches" => remote::git_remote_branches { repo: String, remote: String },
        "git.checkoutRemoteBranch" => remote::git_checkout_remote_branch { repo: String, remote: String, branch: String, local_name: Option<String> },
        "git.deleteRemoteBranch" => remote::git_delete_remote_branch { repo: String, remote: String, branch: String },
        "git.setUpstream" => remote::git_set_upstream { repo: String, branch: String, upstream: Option<String> },
    )
}

/// Whether `asked` lies inside one of the projects, or is a worktree of one
/// of their repositories.
fn of_a_project(asked: &Path, projects: &[PathBuf]) -> bool {
    let Ok(asked) = asked.canonicalize() else {
        return false;
    };
    let shared = |path: &Path| {
        Repository::discover(path)
            .ok()
            .and_then(|repo| repo.commondir().canonicalize().ok())
    };
    let asked_shared = shared(&asked);
    projects.iter().any(|project| {
        let Ok(project) = project.canonicalize() else {
            return false;
        };
        asked.starts_with(&project)
            || asked_shared
                .as_ref()
                .is_some_and(|common| shared(&project).as_ref() == Some(common))
    })
}

fn unfit(error: serde_json::Error) -> CoreError {
    format!("the call's params do not fit: {error}").into()
}

/// Watches the workspace's projects as they are now, telling every client
/// when one moves.
pub(crate) fn follow_projects(core: &Core) {
    let Some(core) = core.me.upgrade() else {
        return;
    };
    let weak: Weak<Core> = core.me.clone();
    let changed: watch::Changed = Arc::new(move |projects, paths| {
        let Some(core) = weak.upgrade() else { return };
        for project in projects {
            core.changes.changed("git", Some(project), paths.clone());
        }
    });
    tokio::task::spawn_blocking(move || {
        core.git_watches
            .follow(|| core.workspaces.project_paths(), &changed);
    });
}
