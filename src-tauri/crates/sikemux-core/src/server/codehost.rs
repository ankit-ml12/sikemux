//! A project's pull requests, runs, issues and releases on its code host, as
//! clients list them, with what the core works out for each row. The core
//! keeps each list a connection shows, reads it again while one does, and
//! tells clients when it changed, so no client keeps time of its own.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use regex::Regex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sikemux_process::sync::Locked;
use sikemux_wire::codehost::{
    CheckTally, Issue, IssueList, IssuePage, ListedPull, ListedRun, Outcome, Pull, PullCounts,
    PullList, PullState, Release, ReleaseList, Run, RunList, RunPage, TrackedItem, TrackedKind,
};
use tokio::task::JoinSet;

use super::board::codehost::{of_project, Repo};
use super::changed::Topic;
use super::connection::ClientId;
use super::names::project_of;
use super::plugins::{Failure, Outcome as Answer};
use super::Core;

/// A list read this recently is answered as it is.
const FRESH: Duration = Duration::from_secs(60);
/// A list no connection shows any more is kept this long after it was asked for.
const GRACE: Duration = Duration::from_secs(60);
const TICK: Duration = Duration::from_secs(15);
const KEPT_AT_MOST: usize = 64;
/// The repository's latest runs, among which most open pull requests' commits are.
const RECENT_RUNS: u32 = 100;
/// Pull requests whose commits are past those that ask for their own runs.
const OWN_RUNS_AT_MOST: usize = 30;
/// A commit's runs, once all are over, are not asked for again this long.
const SETTLED_RUNS: Duration = Duration::from_secs(600);
const COMMITS_AT_MOST: usize = 500;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workflow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    event: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    head_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    per_page: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum List {
    Pulls { state: String },
    Runs(RunQuery),
    Issues { state: String, page: u32 },
    Releases,
    Tracked,
}

impl List {
    /// Which of a project's lists it is, whatever it was asked with.
    fn kind(&self) -> u8 {
        match self {
            Self::Pulls { .. } => 0,
            Self::Runs(_) => 1,
            Self::Issues { .. } => 2,
            Self::Releases => 3,
            Self::Tracked => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    project: String,
    list: List,
}

struct Kept {
    answer: Value,
    /// The repository it is on, as a `ci` change names it.
    repo: String,
    read: Instant,
    asked: Instant,
}

/// The runs on one commit, and when they were read.
struct CommitRuns {
    runs: Vec<Run>,
    read: Instant,
}

#[derive(Default)]
struct State {
    kept: HashMap<Key, Kept>,
    /// The list of each kind each connection last asked for, which it shows
    /// until it asks for another or leaves.
    shown: HashMap<(ClientId, String, u8), Key>,
    /// By repository and commit.
    commits: HashMap<(String, String), CommitRuns>,
}

#[derive(Default)]
pub(crate) struct Lists {
    state: Mutex<State>,
    ticking: AtomicBool,
}

fn listed_state(state: Option<String>) -> Result<String, Failure> {
    let state = state.unwrap_or_else(|| "open".to_owned());
    match state.as_str() {
        "open" | "closed" | "all" => Ok(state),
        _ => Err(Failure::new(
            "codehost",
            "invalid",
            format!("{state} is not open, closed or all"),
        )),
    }
}

#[derive(Deserialize)]
pub(crate) struct PullsParams {
    project: String,
    state: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct RunsParams {
    project: String,
    #[serde(flatten)]
    query: RunQuery,
}

#[derive(Deserialize)]
pub(crate) struct IssuesParams {
    project: String,
    state: Option<String>,
    page: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct ReleasesParams {
    project: String,
}

pub(crate) async fn pulls(core: &Arc<Core>, client: ClientId, params: PullsParams) -> Answer {
    match listed_state(params.state) {
        Ok(state) => answered(core, client, params.project, List::Pulls { state }).await,
        Err(failure) => Answer::Failed(failure),
    }
}

pub(crate) async fn runs(core: &Arc<Core>, client: ClientId, params: RunsParams) -> Answer {
    answered(core, client, params.project, List::Runs(params.query)).await
}

pub(crate) async fn issues(core: &Arc<Core>, client: ClientId, params: IssuesParams) -> Answer {
    match listed_state(params.state) {
        Ok(state) => {
            let page = params.page.unwrap_or(1).max(1);
            answered(core, client, params.project, List::Issues { state, page }).await
        }
        Err(failure) => Answer::Failed(failure),
    }
}

pub(crate) async fn tracked(core: &Arc<Core>, client: ClientId, params: ReleasesParams) -> Answer {
    answered(core, client, params.project, List::Tracked).await
}

pub(crate) async fn releases(core: &Arc<Core>, client: ClientId, params: ReleasesParams) -> Answer {
    answered(core, client, params.project, List::Releases).await
}

async fn answered(core: &Arc<Core>, client: ClientId, project: String, list: List) -> Answer {
    let project = project_of(Path::new(&project), &core.workspaces.project_paths());
    let key = Key { project, list };
    let kept = {
        let mut state = core.code_host_lists.state.locked();
        state
            .shown
            .insert((client, key.project.clone(), key.list.kind()), key.clone());
        state.kept.get_mut(&key).and_then(|kept| {
            kept.asked = Instant::now();
            (kept.read.elapsed() < FRESH).then(|| kept.answer.clone())
        })
    };
    if let Some(answer) = kept {
        return Answer::Value(answer);
    }
    match read(core, &key).await {
        Ok((answer, repo)) => {
            keep(core, key, answer.clone(), repo);
            Answer::Value(answer)
        }
        Err(failure) => Answer::Failed(failure),
    }
}

fn keep(core: &Arc<Core>, key: Key, answer: Value, repo: String) {
    {
        let mut state = core.code_host_lists.state.locked();
        let now = Instant::now();
        let kept = Kept {
            answer,
            repo,
            read: now,
            asked: now,
        };
        state.kept.insert(key, kept);
        while state.kept.len() > KEPT_AT_MOST {
            let Some(oldest) = state
                .kept
                .iter()
                .min_by_key(|(_, kept)| kept.asked)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            state.kept.remove(&oldest);
        }
    }
    follow(core);
}

/// Reads the lists connections still show again as each goes stale, until
/// none shows one.
fn follow(core: &Arc<Core>) {
    if core.code_host_lists.ticking.swap(true, Ordering::AcqRel) {
        return;
    }
    let weak = Arc::downgrade(core);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(TICK).await;
            let Some(core) = weak.upgrade() else { return };
            let connected: HashSet<ClientId> =
                core.clients().iter().map(|client| client.id).collect();
            let due = {
                let mut state = core.code_host_lists.state.locked();
                state
                    .shown
                    .retain(|(client, _, _), _| connected.contains(client));
                let shown: HashSet<Key> = state.shown.values().cloned().collect();
                state
                    .kept
                    .retain(|key, kept| shown.contains(key) || kept.asked.elapsed() < GRACE);
                state
                    .commits
                    .retain(|_, commit| commit.read.elapsed() < SETTLED_RUNS);
                if state.kept.is_empty() {
                    core.code_host_lists.ticking.store(false, Ordering::Release);
                    return;
                }
                state
                    .kept
                    .iter()
                    .filter(|(key, kept)| shown.contains(key) && kept.read.elapsed() >= FRESH)
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>()
            };
            refresh(&core, due).await;
        }
    });
}

/// Reads `keys` again, telling clients about each project whose lists changed.
async fn refresh(core: &Arc<Core>, keys: Vec<Key>) {
    let mut moved = HashSet::new();
    for key in keys {
        let Ok((answer, repo)) = read(core, &key).await else {
            continue;
        };
        let mut state = core.code_host_lists.state.locked();
        let Some(kept) = state.kept.get_mut(&key) else {
            continue;
        };
        if kept.answer != answer {
            moved.insert(key.project.clone());
        }
        kept.answer = answer;
        kept.repo = repo;
        kept.read = Instant::now();
    }
    for project in moved {
        core.changes.changed(Topic::CodeHost, Some(&project), None);
    }
}

/// The person picked another repository or account for the project at
/// `project`, so its lists are read afresh.
pub(crate) fn picked(core: &Core, project: &str) {
    let forgot = {
        let mut state = core.code_host_lists.state.locked();
        let before = state.kept.len();
        state.kept.retain(|key, _| key.project != project);
        state.kept.len() != before
    };
    if forgot {
        core.changes.changed(Topic::CodeHost, Some(project), None);
    }
}

/// The refs of `projects` moved, as a push, pull or commit moves them, so
/// their pull requests and runs are read again now.
pub(crate) fn refs_moved(core: &Arc<Core>, projects: &[String]) {
    let keys: Vec<Key> = core
        .code_host_lists
        .state
        .locked()
        .kept
        .keys()
        .filter(|key| {
            projects.contains(&key.project)
                && matches!(key.list, List::Pulls { .. } | List::Runs(_) | List::Tracked)
        })
        .cloned()
        .collect();
    if keys.is_empty() {
        return;
    }
    let core = core.clone();
    tokio::spawn(async move { refresh(&core, keys).await });
}

/// The runs of the repository `repo` moved, so its runs and the checks on its
/// pull requests are read again now.
pub(crate) fn ci_moved(core: &Arc<Core>, repo: &str) {
    let keys: Vec<Key> = core
        .code_host_lists
        .state
        .locked()
        .kept
        .iter()
        .filter(|(key, kept)| {
            kept.repo == repo && matches!(key.list, List::Pulls { .. } | List::Runs(_))
        })
        .map(|(key, _)| key.clone())
        .collect();
    if keys.is_empty() {
        return;
    }
    let core = core.clone();
    tokio::spawn(async move { refresh(&core, keys).await });
}

async fn read(core: &Arc<Core>, key: &Key) -> Result<(Value, String), Failure> {
    let repo = of_project(core, &key.project).await?;
    let answer = match &key.list {
        List::Pulls { state } => shaped(&repo, pull_list(core, &repo, state).await?),
        List::Runs(query) => shaped(&repo, run_list(&repo, query).await?),
        List::Issues { state, page } => shaped(&repo, issue_list(&repo, state, *page).await?),
        List::Releases => shaped(&repo, release_list(&repo).await?),
        List::Tracked => shaped(&repo, tracked_list(&repo).await?),
    }?;
    Ok((answer, repo.key()))
}

fn shaped(repo: &Repo, list: impl Serialize) -> Result<Value, Failure> {
    serde_json::to_value(list)
        .map_err(|error| Failure::new(&repo.key(), "invalid", error.to_string()))
}

fn typed<T: DeserializeOwned>(repo: &Repo, found: Value) -> Result<T, Failure> {
    serde_json::from_value(found).map_err(|error| {
        Failure::new(
            &repo.key(),
            "invalid",
            format!("the code host answered in an unknown shape: {error}"),
        )
    })
}

/// How many are open and closed, from hosts that count them cheaply.
async fn counts(repo: &Repo, method: &str) -> Option<PullCounts> {
    let found = repo.call(method, json!({})).await.ok()?;
    serde_json::from_value(found).ok()
}

async fn pull_list(core: &Core, repo: &Repo, state: &str) -> Result<PullList, Failure> {
    let (pulls, recent, counts) = tokio::join!(
        repo.call("pulls", json!({ "state": state })),
        repo.call("runs", json!({ "perPage": RECENT_RUNS })),
        counts(repo, "pullCounts"),
    );
    let pulls: Vec<Pull> = typed(repo, pulls?)?;
    let mut by_sha: HashMap<String, Vec<Run>> = HashMap::new();
    if let Ok(page) = recent.and_then(|found| typed::<RunPage>(repo, found)) {
        for run in page.runs {
            by_sha.entry(run.sha.clone()).or_default().push(run);
        }
    }
    let past: HashSet<String> = pulls
        .iter()
        .filter_map(|pull| pull.head_sha.clone())
        .filter(|sha| !by_sha.contains_key(sha))
        .take(OWN_RUNS_AT_MOST)
        .collect();
    by_sha.extend(own_runs(core, repo, past).await);
    let pulls = pulls
        .into_iter()
        .map(|pull| {
            let checks = pull
                .head_sha
                .as_ref()
                .and_then(|sha| by_sha.get(sha))
                .filter(|runs| !runs.is_empty())
                .map(|runs| tally(runs));
            ListedPull {
                state: state_of(&pull),
                closes: closes(&pull.body),
                own_branch: own_branch(&pull, repo.owner()),
                checks,
                pull,
            }
        })
        .collect();
    Ok(PullList { pulls, counts })
}

/// The runs on each of `shas`, which are past the repository's latest. A
/// commit whose runs are all over is not asked for again for a while.
async fn own_runs(core: &Core, repo: &Repo, shas: HashSet<String>) -> HashMap<String, Vec<Run>> {
    let mut runs = HashMap::new();
    let mut ask = Vec::new();
    {
        let state = core.code_host_lists.state.locked();
        for sha in shas {
            match state.commits.get(&(repo.key(), sha.clone())) {
                Some(settled) => {
                    runs.insert(sha, settled.runs.clone());
                }
                None => ask.push(sha),
            }
        }
    }
    let mut asked = JoinSet::new();
    for sha in ask {
        let repo = repo.clone();
        asked.spawn(async move {
            let found = repo
                .call("runs", json!({ "headSha": sha, "perPage": 30 }))
                .await;
            let page = found
                .ok()
                .and_then(|found| serde_json::from_value::<RunPage>(found).ok());
            (sha, page)
        });
    }
    let mut settled = Vec::new();
    while let Some(Ok((sha, page))) = asked.join_next().await {
        let Some(page) = page else { continue };
        if !page.runs.iter().any(unfinished) {
            settled.push((sha.clone(), page.runs.clone()));
        }
        runs.insert(sha, page.runs);
    }
    let mut state = core.code_host_lists.state.locked();
    for (sha, found) in settled {
        let read = Instant::now();
        state
            .commits
            .insert((repo.key(), sha), CommitRuns { runs: found, read });
    }
    while state.commits.len() > COMMITS_AT_MOST {
        let Some(oldest) = state
            .commits
            .iter()
            .min_by_key(|(_, commit)| commit.read)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        state.commits.remove(&oldest);
    }
    runs
}

fn own_branch(pull: &Pull, owner: &str) -> bool {
    pull.head.is_some()
        && pull
            .head_label
            .as_deref()
            .and_then(|label| label.split(':').next())
            .is_none_or(|head| head.is_empty() || head == owner)
}

fn state_of(pull: &Pull) -> PullState {
    if pull.state == "merged" || pull.merged_at.is_some() {
        PullState::Merged
    } else if pull.state == "open" && pull.draft {
        PullState::Draft
    } else if pull.state == "open" {
        PullState::Open
    } else {
        PullState::Closed
    }
}

/// The issues a description says it closes, as GitHub links them: "Fixes
/// #12", "closes owner/repo#3".
fn closes(body: &str) -> Vec<u64> {
    static CLOSING: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?iu)\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s*:?\s+(?:[\w.-]+/[\w.-]+)?#(\d+)",
        )
        .expect("the closing pattern compiles")
    });
    let mut found = Vec::new();
    for number in CLOSING
        .captures_iter(body)
        .filter_map(|caught| caught[1].parse().ok())
    {
        if !found.contains(&number) {
            found.push(number);
        }
    }
    found
}

fn tally(runs: &[Run]) -> CheckTally {
    let mut tally = CheckTally {
        total: runs.len() as u32,
        ..CheckTally::default()
    };
    for run in runs {
        match run.outcome {
            Outcome::Success | Outcome::Skipped => tally.passed += 1,
            Outcome::Failure => tally.failing += 1,
            Outcome::Running => tally.running += 1,
            Outcome::Queued => tally.queued += 1,
            Outcome::Blocked => tally.waiting += 1,
            Outcome::Cancelled => tally.cancelled += 1,
            Outcome::Unknown => tally.unknown += 1,
        }
    }
    tally.outcome = if tally.failing > 0 {
        Outcome::Failure
    } else if tally.running > 0 {
        Outcome::Running
    } else if tally.queued > 0 {
        Outcome::Queued
    } else if tally.waiting > 0 {
        Outcome::Blocked
    } else {
        Outcome::Success
    };
    tally
}

fn unfinished(run: &Run) -> bool {
    run.status != "completed"
        && matches!(
            run.outcome,
            Outcome::Running | Outcome::Queued | Outcome::Blocked
        )
}

async fn run_list(repo: &Repo, query: &RunQuery) -> Result<RunList, Failure> {
    let asked = serde_json::to_value(query).unwrap_or_default();
    let page: RunPage = typed(repo, repo.call("runs", asked).await?)?;
    Ok(RunList {
        runs: page
            .runs
            .into_iter()
            .map(|run| ListedRun {
                unfinished: unfinished(&run),
                run,
            })
            .collect(),
        total: page.total,
        next_page: page.next_page,
    })
}

async fn issue_list(repo: &Repo, state: &str, page: u32) -> Result<IssueList, Failure> {
    let (issues, counts) = tokio::join!(
        repo.call("issues", json!({ "state": state, "page": page })),
        counts(repo, "issueCounts"),
    );
    let issues: IssuePage = typed(repo, issues?)?;
    Ok(IssueList {
        issues: issues.issues,
        total: issues.total,
        next_page: issues.next_page,
        counts,
    })
}

/// The open issues and pull requests together, newest first. A host that
/// lists pull requests among its issues has each kept once, as a pull request.
async fn tracked_list(repo: &Repo) -> Result<Vec<TrackedItem>, Failure> {
    let (pulls, issues) = tokio::join!(
        repo.call("pulls", json!({ "state": "open" })),
        repo.call("issues", json!({ "state": "open", "page": 1 })),
    );
    let pulls: Vec<Pull> = typed(repo, pulls?)?;
    let issues = issues
        .ok()
        .and_then(|found| serde_json::from_value::<IssuePage>(found).ok())
        .map(|page| page.issues)
        .unwrap_or_default();
    Ok(tracked_items(pulls, issues))
}

fn tracked_items(pulls: Vec<Pull>, issues: Vec<Issue>) -> Vec<TrackedItem> {
    let numbers: HashSet<u64> = pulls.iter().map(|pull| pull.number).collect();
    let pulls = pulls.into_iter().map(|pull| TrackedItem {
        kind: TrackedKind::Pull,
        number: pull.number,
        title: pull.title,
        created_at: pull.created_at,
        url: pull.url,
    });
    let issues = issues
        .into_iter()
        .filter(|issue| !numbers.contains(&issue.number))
        .map(|issue| TrackedItem {
            kind: TrackedKind::Issue,
            number: issue.number,
            title: issue.title,
            created_at: issue.created_at,
            url: issue.url,
        });
    let mut items: Vec<TrackedItem> = pulls.chain(issues).collect();
    items.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then(b.number.cmp(&a.number))
    });
    items
}

async fn release_list(repo: &Repo) -> Result<ReleaseList, Failure> {
    let releases: Vec<Release> = typed(repo, repo.call("releases", json!({})).await?)?;
    let latest = releases
        .iter()
        .find(|release| !release.draft && !release.prerelease)
        .map(|release| release.id);
    Ok(ReleaseList { releases, latest })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(outcome: Outcome) -> Run {
        serde_json::from_value(json!({
            "id": "1", "name": "CI", "title": "", "workflowId": "1", "path": null,
            "runNumber": 1, "attempt": 1, "event": "push", "status": "completed",
            "conclusion": null, "outcome": outcome, "branch": null, "sha": "abc",
            "shortSha": "abc", "actor": null, "avatarUrl": null, "createdAt": "",
            "startedAt": null, "updatedAt": "", "pullRequests": [], "url": ""
        }))
        .expect("a run")
    }

    fn pull(number: u64, head_label: &str, created_at: &str) -> Pull {
        serde_json::from_value(json!({
            "number": number, "title": format!("pull {number}"), "body": "", "state": "open",
            "draft": false, "author": null, "avatarUrl": null, "authorAssociation": null,
            "head": "main", "headLabel": head_label, "base": "main", "headSha": null,
            "createdAt": created_at, "updatedAt": created_at, "comments": null, "additions": null,
            "deletions": null, "changedFiles": null, "mergeable": null, "mergeState": null,
            "labels": [], "reviewers": [], "assignees": [], "milestone": null, "commits": null,
            "mergedAt": null, "mergedBy": null, "mergeCommitSha": null, "avatars": {}, "url": ""
        }))
        .expect("a pull request")
    }

    fn issue(number: u64, created_at: &str) -> Issue {
        serde_json::from_value(json!({
            "number": number, "title": format!("issue {number}"), "body": "", "state": "open",
            "stateReason": null, "author": null, "avatarUrl": null, "createdAt": created_at,
            "updatedAt": created_at, "closedAt": null, "comments": 0, "labels": [],
            "assignees": [], "url": ""
        }))
        .expect("an issue")
    }

    #[test]
    fn a_forks_branch_only_shares_a_name_with_one_here() {
        assert!(own_branch(&pull(1, "nodelike:main", ""), "nodelike"));
        assert!(own_branch(&pull(1, "", ""), "nodelike"));
        assert!(!own_branch(&pull(49, "Sujal85526:main", ""), "nodelike"));
    }

    #[test]
    fn tracked_items_are_newest_first_and_a_pull_request_listed_as_an_issue_once() {
        let items = tracked_items(
            vec![pull(2, "", "2026-02-01")],
            vec![
                issue(1, "2026-01-01"),
                issue(2, "2026-02-01"),
                issue(3, "2026-03-01"),
            ],
        );
        let shown: Vec<_> = items.iter().map(|item| (item.kind, item.number)).collect();
        assert_eq!(
            shown,
            [
                (TrackedKind::Issue, 3),
                (TrackedKind::Pull, 2),
                (TrackedKind::Issue, 1)
            ]
        );
    }

    #[test]
    fn a_commit_fails_on_any_failure_and_runs_while_anything_goes() {
        let failing = tally(&[
            run(Outcome::Success),
            run(Outcome::Failure),
            run(Outcome::Running),
        ]);
        assert_eq!(
            (failing.outcome, failing.passed, failing.total),
            (Outcome::Failure, 1, 3)
        );
        assert_eq!(
            tally(&[run(Outcome::Success), run(Outcome::Running)]).outcome,
            Outcome::Running
        );
        assert_eq!(
            tally(&[run(Outcome::Success), run(Outcome::Queued)]).outcome,
            Outcome::Queued
        );
        let passed = tally(&[run(Outcome::Success), run(Outcome::Skipped)]);
        assert_eq!((passed.outcome, passed.passed), (Outcome::Success, 2));
    }
}
