//! What the Rundeck pane shows: the projects' folders, and a folder's jobs with
//! how each last ran. The core keeps each and reads it again on the pace the
//! manifest gives.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use sikemux_plugin_api::rundeck::{
    GroupNode, JobRow, Jobs, Section, StatusKind, Tree, TreeProject,
};

use crate::client::Session;
use crate::config::Settings;
use crate::error::RundeckResult;
use crate::projects::{self, MatrixCell};
use crate::shape::{group_segments, is_production, run_summary};

pub async fn tree_view(session: &Session) -> RundeckResult<Tree> {
    let projects = projects::job_index(session)
        .await?
        .into_iter()
        .map(|entry| {
            let groups: Vec<Option<&str>> =
                entry.jobs.iter().map(|job| job.group.as_deref()).collect();
            let (groups, ungrouped) = folders(&groups);
            TreeProject {
                name: entry.project,
                error: entry.error,
                total: entry.jobs.len() as u32,
                ungrouped,
                groups,
            }
        })
        .collect();
    Ok(Tree { projects })
}

/// The folders the jobs in `groups` sit in, nested and counted, and how many sit in none.
fn folders(groups: &[Option<&str>]) -> (Vec<GroupNode>, u32) {
    let mut root = GroupNode {
        name: String::new(),
        path: String::new(),
        count: 0,
        children: Vec::new(),
    };
    let mut ungrouped = 0;
    for group in groups {
        let segments = group_segments(*group);
        if segments.is_empty() {
            ungrouped += 1;
            continue;
        }
        let mut node = &mut root;
        for segment in segments {
            let at = match node.children.iter().position(|child| child.name == segment) {
                Some(at) => at,
                None => {
                    let path = match node.path.is_empty() {
                        true => segment.to_owned(),
                        false => format!("{}/{segment}", node.path),
                    };
                    node.children.push(GroupNode {
                        name: segment.to_owned(),
                        path,
                        count: 0,
                        children: Vec::new(),
                    });
                    node.children.len() - 1
                }
            };
            node = &mut node.children[at];
            node.count += 1;
        }
    }
    sort(&mut root.children);
    (root.children, ungrouped)
}

fn sort(nodes: &mut [GroupNode]) {
    nodes.sort_by(|a, b| a.name.cmp(&b.name));
    for node in nodes {
        sort(&mut node.children);
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobsRequest {
    project: String,
    #[serde(default)]
    group: Option<String>,
}

pub async fn jobs_view(
    session: &Session,
    settings: &Settings,
    request: JobsRequest,
) -> RundeckResult<Jobs> {
    let matrix =
        projects::branches_matrix(session, request.project.clone(), &settings.branch_options)
            .await?;
    Ok(jobs_of(
        &request.project,
        request.group.as_deref(),
        matrix.cells,
        matrix.error,
        matrix.partial,
        &settings.prod_envs,
    ))
}

fn jobs_of(
    project: &str,
    group: Option<&str>,
    cells: Vec<MatrixCell>,
    error: Option<String>,
    partial: bool,
    prod_envs: &[String],
) -> Jobs {
    let here = group_segments(group);
    let mut live_paths = BTreeSet::new();
    for cell in cells.iter().filter(|cell| {
        cell.latest
            .as_ref()
            .is_some_and(|run| crate::shape::is_live(run.status.as_deref()))
    }) {
        let segments = group_segments(cell.group.as_deref());
        for depth in 0..=segments.len() {
            live_paths.insert(segments[..depth].join("/"));
        }
    }
    let mut cells: Vec<MatrixCell> = cells
        .into_iter()
        .filter(|cell| group_segments(cell.group.as_deref()).starts_with(&here))
        .collect();
    cells.sort_by(|a, b| a.name.cmp(&b.name));
    let path_of = |below: &[&str]| -> String { [here.as_slice(), below].concat().join("/") };
    let rows: Vec<JobRow> = cells.iter().map(row).collect();

    let below = |cell: &MatrixCell| -> Vec<String> {
        group_segments(cell.group.as_deref())[here.len()..]
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect()
    };
    let sections = |key: &dyn Fn(&MatrixCell) -> Option<String>| -> Vec<Section> {
        let mut by: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
        for cell in &cells {
            by.entry(key(cell)).or_default().push(cell.job_id.clone());
        }
        by.into_iter()
            .map(|(segment, jobs)| {
                let path = match &segment {
                    Some(segment) => path_of(&segment.split('/').collect::<Vec<_>>()),
                    None => here.join("/"),
                };
                Section {
                    label: match (&segment, path.is_empty()) {
                        (None, true) => project.to_owned(),
                        _ => path.clone(),
                    },
                    prod: is_production(
                        project,
                        Some(&path)
                            .filter(|path| !path.is_empty())
                            .map(String::as_str),
                        prod_envs,
                    ),
                    segment,
                    path,
                    jobs,
                }
            })
            .collect()
    };
    let list = sections(&|cell| {
        let below = below(cell);
        (!below.is_empty()).then(|| below.join("/"))
    });
    let mut compare = sections(&|cell| below(cell).first().cloned());
    let loose = compare
        .iter()
        .find(|section| section.segment.is_none())
        .map_or(0, |section| section.jobs.len() as u32);
    compare.retain(|section| section.segment.is_some());
    let mut seen: BTreeMap<&str, u32> = BTreeMap::new();
    for section in &compare {
        let names: BTreeSet<&str> = section
            .jobs
            .iter()
            .filter_map(|id| cells.iter().find(|cell| &cell.job_id == id))
            .map(|cell| cell.name.as_str())
            .collect();
        for name in names {
            *seen.entry(name).or_default() += 1;
        }
    }
    let shared = seen.values().filter(|count| **count > 1).count() as u32;
    if compare.len() < 2 || shared == 0 {
        compare.clear();
    }

    Jobs {
        project: project.to_owned(),
        group: group.map(str::to_owned),
        title: here
            .last()
            .map_or_else(|| project.to_owned(), |name| (*name).to_owned()),
        error,
        partial,
        timed_out: cells
            .iter()
            .filter(|cell| cell.error.as_deref() == Some("timed out"))
            .count() as u32,
        running: rows
            .iter()
            .filter(|row| row.latest.as_ref().is_some_and(|run| run.live))
            .count() as u32,
        failed: rows
            .iter()
            .filter(|row| {
                row.latest
                    .as_ref()
                    .is_some_and(|run| run.status_kind == StatusKind::Failed)
            })
            .count() as u32,
        jobs: rows,
        list,
        compare,
        shared,
        loose,
        live_paths: live_paths.into_iter().collect(),
    }
}

fn row(cell: &MatrixCell) -> JobRow {
    let latest = cell.latest.as_ref().map(run_summary);
    let deployed = cell.deployed.as_ref().map(run_summary);
    let branch = cell
        .latest
        .as_ref()
        .and_then(|run| run.branch.clone())
        .or_else(|| cell.deployed.as_ref().and_then(|run| run.branch.clone()));
    let live_branch = match (&latest, &deployed) {
        (Some(latest), Some(deployed))
            if latest.live && latest.branch.is_some() && deployed.branch != latest.branch =>
        {
            deployed.branch.clone()
        }
        _ => None,
    };
    JobRow {
        job_id: cell.job_id.clone(),
        name: cell.name.clone(),
        group: cell.group.clone(),
        enabled: cell.enabled != Some(false),
        scheduled: cell.scheduled == Some(true),
        error: cell.error.clone(),
        run_word: if branch.is_some() { "Deploy" } else { "Run" }.to_owned(),
        branch,
        latest,
        deployed,
        live_branch,
    }
}
