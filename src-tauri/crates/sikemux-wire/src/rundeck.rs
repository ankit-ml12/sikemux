//! What the Rundeck plugin's views hand a pane: the projects' folders, a
//! folder's jobs with how each last ran, and a job's or a project's runs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The projects the person can see, each with its folders of jobs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tree {
    pub projects: Vec<TreeProject>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TreeProject {
    pub name: String,
    /// Why its jobs could not be listed.
    pub error: Option<String>,
    /// Its jobs, in every folder.
    pub total: u32,
    /// Its jobs in no folder.
    pub ungrouped: u32,
    pub groups: Vec<GroupNode>,
}

/// A folder of jobs, by name, with the folders below it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupNode {
    pub name: String,
    /// Slash-joined from the project's root, in the folders' own case.
    pub path: String,
    /// Jobs in it and every folder below it.
    pub count: u32,
    pub children: Vec<GroupNode>,
}

/// How a run went, as a pane colours it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StatusKind {
    Succeeded,
    Failed,
    Running,
    Aborted,
    Unknown,
}

/// One run of a job, as lists show it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub execution_id: u64,
    pub status: String,
    pub status_kind: StatusKind,
    /// Still running, scheduled or queued.
    pub live: bool,
    pub user: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub branch: Option<String>,
}

/// A job in a folder, with its newest run and the newest that succeeded.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub job_id: String,
    pub name: String,
    pub group: Option<String>,
    pub enabled: bool,
    pub scheduled: bool,
    /// Why its runs could not be read; `timed out` when Rundeck was too slow.
    pub error: Option<String>,
    pub latest: Option<RunSummary>,
    pub deployed: Option<RunSummary>,
    /// The branch it runs: the newest run's, else the one last deployed.
    pub branch: Option<String>,
    /// What starting it is called: `Deploy` when it runs a branch, else `Run`.
    pub run_word: String,
    /// While a run of another branch goes on, the branch that is live.
    pub live_branch: Option<String>,
}

/// A run of jobs under one heading: a folder below the one shown, or the
/// folder itself.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    /// The folder below the one shown, or none for the jobs in it directly.
    pub segment: Option<String>,
    /// The folder's whole path from the project's root.
    pub path: String,
    /// What its heading says.
    pub label: String,
    /// The folder or project names production.
    pub prod: bool,
    /// Its jobs' ids, by name.
    pub jobs: Vec<String>,
}

/// A folder's jobs, with how each last ran.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Jobs {
    pub project: String,
    pub group: Option<String>,
    /// The folder's own name, or the project's at its root.
    pub title: String,
    /// Why the jobs could not be listed.
    pub error: Option<String>,
    /// Rundeck was too slow for some jobs, which show no runs.
    pub partial: bool,
    pub timed_out: u32,
    pub running: u32,
    pub failed: u32,
    /// By name.
    pub jobs: Vec<JobRow>,
    /// Each job under the path below the folder it sits in.
    pub list: Vec<Section>,
    /// Each job under the folder right below this one, to compare the same job across
    /// folders; empty unless more than one folder holds a job of the same name.
    pub compare: Vec<Section>,
    /// Job names found in more than one of those folders.
    pub shared: u32,
    /// Jobs that sit in this folder itself, which comparing leaves out.
    pub loose: u32,
    /// Every folder in the project, and the folders above it, holding a running job; the
    /// project itself is the empty path.
    pub live_paths: Vec<String>,
}

/// One run in a job's or a project's history, newest first.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub execution_id: u64,
    pub status: String,
    pub status_kind: StatusKind,
    pub live: bool,
    pub user: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    /// How long it took, once it ended.
    pub secs: Option<u64>,
    pub branch: Option<String>,
    pub job_id: Option<String>,
    pub job_name: Option<String>,
    pub job_group: Option<String>,
    /// How long its job's runs that succeeded usually take, of those read.
    pub usual_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StepState {
    Pending,
    Running,
    Ok,
    Fail,
    Skip,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveStep {
    pub name: String,
    pub state: StepState,
}

/// The run of a job going on now, step by step.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveRun {
    pub execution_id: u64,
    pub user: Option<String>,
    pub started_at: Option<String>,
    pub branch: Option<String>,
    pub steps: Vec<LiveStep>,
    pub completed: u32,
    pub total: u32,
}

/// A job with its recent runs and how they went.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobPage {
    pub job_id: String,
    pub name: String,
    pub group: Option<String>,
    pub project: String,
    pub description: Option<String>,
    pub permalink: Option<String>,
    pub execution_enabled: bool,
    pub node_filter: Option<String>,
    pub prod: bool,
    /// It takes a branch, so starting it is a deploy.
    pub runs_branch: bool,
    pub run_word: String,
    pub runs: Vec<Run>,
    pub live: Option<LiveRun>,
    /// The newest run that succeeded, with a branch when the job takes one.
    pub last_good: Option<Run>,
    /// That run's options, but the secure ones, to start it again with.
    pub repeat_options: BTreeMap<String, String>,
    pub finished: u32,
    pub succeeded: u32,
    pub usual_secs: Option<u64>,
    pub fastest_secs: Option<u64>,
    pub slowest_secs: Option<u64>,
    /// While a run goes on, how soon the core reads it again.
    pub read_again_secs: Option<u64>,
}

/// A project's recent runs of every job.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPage {
    pub project: String,
    /// Who the sign-in is, to pick out their runs.
    pub me: Option<String>,
    pub runs: Vec<Run>,
    /// Every run Rundeck holds for the project.
    pub total: Option<u64>,
    pub running: u32,
    pub failed: u32,
    pub mine: u32,
    /// While a run goes on, how soon the core reads it again.
    pub read_again_secs: Option<u64>,
}
