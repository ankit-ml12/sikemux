//! How the pane reads Rundeck's names: folders from a job's group, which
//! folders and projects are production, and how a run's status went.

use sikemux_plugin_api::rundeck::{RunSummary, StatusKind};

use crate::executions::ExecSummary;

/// A job's group as its folders, top first, without empty ones.
pub fn group_segments(group: Option<&str>) -> Vec<&str> {
    group
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect()
}

/// Whether `name`, or a word of it, is one of `prod_envs`.
fn names_production(name: &str, prod_envs: &[String]) -> bool {
    let lower = name.to_lowercase();
    std::iter::once(lower.as_str())
        .chain(
            lower
                .split(['-', '_', '.', '/', ' '])
                .filter(|token| !token.is_empty()),
        )
        .any(|token| {
            prod_envs
                .iter()
                .any(|env| env.trim().eq_ignore_ascii_case(token))
        })
}

/// Whether a job in `group` of `project` deploys to production: its top folder
/// or its project says so.
pub fn is_production(project: &str, group: Option<&str>, prod_envs: &[String]) -> bool {
    group_segments(group)
        .first()
        .is_some_and(|top| names_production(top, prod_envs))
        || names_production(project, prod_envs)
}

pub fn is_live(status: Option<&str>) -> bool {
    status.is_some_and(|status| {
        matches!(
            status.to_ascii_lowercase().as_str(),
            "running" | "scheduled" | "queued"
        )
    })
}

pub fn status_kind(status: Option<&str>) -> StatusKind {
    let Some(status) = status else {
        return StatusKind::Unknown;
    };
    match status.to_ascii_lowercase().as_str() {
        "succeeded" => StatusKind::Succeeded,
        "failed" | "failed-with-retry" | "timedout" | "other-failed" | "error" => {
            StatusKind::Failed
        }
        "aborted" => StatusKind::Aborted,
        live if is_live(Some(live)) => StatusKind::Running,
        _ => StatusKind::Unknown,
    }
}

/// What a status reads as: Rundeck's own custom status where it set one.
pub fn display_status(status: Option<&str>, custom: Option<&str>) -> String {
    match custom {
        Some(custom) if status.is_none_or(|status| status.eq_ignore_ascii_case("other")) => {
            custom.to_owned()
        }
        _ => status.unwrap_or("unknown").to_owned(),
    }
}

pub fn run_summary(run: &ExecSummary) -> RunSummary {
    RunSummary {
        execution_id: run.execution_id,
        status: display_status(run.status.as_deref(), run.custom_status.as_deref()),
        status_kind: status_kind(run.status.as_deref()),
        live: is_live(run.status.as_deref()),
        user: run.user.clone(),
        started_at: run.started_at.clone(),
        ended_at: run.ended_at.clone(),
        branch: run.branch.clone(),
    }
}
