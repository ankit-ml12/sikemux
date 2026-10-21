//! The tabs desktop clients draw, kept by the core so every client shows the
//! same ones. Agents' tabs follow the workspace: a new agent gets a tab at the
//! end of its project's strip, and a tab goes once nothing it shows is left.

use std::collections::HashSet;
use std::sync::Mutex;

use serde::Deserialize;
use sikemux_process::sync::Locked;

use crate::protocol::{Layout, Pane, PaneKind, SessionInfo, Tab, Workspace};

use super::kept::Kept;
use super::{Core, CoreError, CoreResult};

const FILE: &str = "layout.json";
const MAX_TABS: usize = 64;

pub(crate) struct Layouts {
    layout: Mutex<Layout>,
    kept: Kept,
}

impl Layouts {
    pub(crate) fn new(data_dir: Option<&std::path::Path>) -> Self {
        let kept = Kept::new(data_dir, FILE);
        Self {
            layout: Mutex::new(kept.load()),
            kept,
        }
    }
}

/// The layout as clients are sent it, fitted to the workspace and sessions.
pub(crate) fn view(core: &Core, workspace: &Workspace, sessions: &[SessionInfo]) -> Layout {
    fitted(core.layouts.layout.locked().clone(), workspace, sessions)
}

fn fitted(mut layout: Layout, workspace: &Workspace, sessions: &[SessionInfo]) -> Layout {
    let agents: HashSet<&str> = workspace
        .agents
        .iter()
        .map(|agent| agent.id.as_str())
        .collect();
    let terminals: HashSet<String> = sessions
        .iter()
        .map(|session| session.id.to_string())
        .collect();
    let shown = |kind: PaneKind, target: &str| match kind {
        PaneKind::Agent => agents.contains(target),
        PaneKind::Terminal => terminals.contains(target),
        _ => true,
    };
    layout
        .projects
        .retain(|id, _| workspace.projects.iter().any(|project| &project.id == id));
    for project in &workspace.projects {
        let strip = layout.projects.entry(project.id.clone()).or_default();
        strip
            .tabs
            .retain_mut(|tab| keep_shown(&mut tab.pane, &shown));
        let mut created: Vec<_> = workspace
            .agents
            .iter()
            .filter(|agent| agent.project == project.id)
            .filter(|agent| {
                !strip
                    .tabs
                    .iter()
                    .any(|tab| holds(&tab.pane, PaneKind::Agent, &agent.id))
            })
            .collect();
        created.sort_by_key(|agent| agent.created_at);
        for agent in created
            .into_iter()
            .take(MAX_TABS.saturating_sub(strip.tabs.len()))
        {
            strip.tabs.push(tab(
                format!("agent:{}", agent.id),
                PaneKind::Agent,
                agent.id.clone(),
            ));
        }
        let active_shown = strip
            .active
            .as_ref()
            .is_some_and(|active| strip.tabs.iter().any(|tab| &tab.id == active));
        if !active_shown {
            strip.active = strip.tabs.first().map(|tab| tab.id.clone());
        }
    }
    let active_shown = layout
        .active_project
        .as_ref()
        .is_some_and(|active| layout.projects.contains_key(active));
    if !active_shown {
        layout.active_project = workspace.projects.first().map(|project| project.id.clone());
    }
    layout
}

/// Drops the views nothing is left for, answering whether any remain.
fn keep_shown(pane: &mut Pane, shown: &impl Fn(PaneKind, &str) -> bool) -> bool {
    match pane {
        Pane::View { kind, target, .. } => shown(*kind, target),
        Pane::Split { panes, .. } => {
            panes.retain_mut(|pane| keep_shown(pane, shown));
            !panes.is_empty()
        }
    }
}

fn holds(pane: &Pane, kind: PaneKind, target: &str) -> bool {
    match pane {
        Pane::View {
            kind: held,
            target: held_target,
            ..
        } => *held == kind && held_target == target,
        Pane::Split { panes, .. } => panes.iter().any(|pane| holds(pane, kind, target)),
    }
}

fn agents_in(pane: &Pane, found: &mut Vec<String>) {
    match pane {
        Pane::View {
            kind: PaneKind::Agent,
            target,
            ..
        } => found.push(target.clone()),
        Pane::View { .. } => {}
        Pane::Split { panes, .. } => panes.iter().for_each(|pane| agents_in(pane, found)),
    }
}

fn tab(id: String, kind: PaneKind, target: String) -> Tab {
    Tab {
        id: id.clone(),
        title: None,
        pane: Pane::View { id, kind, target },
    }
}

/// Changes the layout as clients see it now, keeps it and tells them.
fn edit<T>(core: &Core, change: impl FnOnce(&mut Layout) -> CoreResult<T>) -> CoreResult<T> {
    let workspace = core.workspace_view();
    let sessions = core.session_infos();
    let answer = {
        let mut kept = core.layouts.layout.locked();
        let mut layout = fitted(kept.clone(), &workspace, &sessions);
        let answer = change(&mut layout)?;
        *kept = layout;
        core.layouts.kept.save(&*kept);
        answer
    };
    core.view_changed();
    core.spares.front_changed();
    Ok(answer)
}

/// The project the person has in front, by id, as the layout last said.
pub(crate) fn front_project(core: &Core) -> Option<String> {
    core.layouts.layout.locked().active_project.clone()
}

fn strip<'a>(
    layout: &'a mut Layout,
    project: &str,
) -> CoreResult<&'a mut crate::protocol::ProjectLayout> {
    layout
        .projects
        .get_mut(project)
        .ok_or_else(|| CoreError::from("that project is not on this host"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenTab {
    project: String,
    kind: PaneKind,
    target: String,
    title: Option<String>,
}

/// Brings the tab showing `target` to the front, opening one at the end of
/// the strip when none does. Answers its id.
pub(crate) fn open_tab(core: &Core, params: OpenTab) -> CoreResult<String> {
    edit(core, |layout| {
        layout.active_project = Some(params.project.clone());
        let strip = strip(layout, &params.project)?;
        let id = match strip
            .tabs
            .iter()
            .find(|tab| holds(&tab.pane, params.kind, &params.target))
        {
            Some(tab) => tab.id.clone(),
            None => {
                if strip.tabs.len() >= MAX_TABS {
                    return Err("that project already has as many tabs as the core keeps".into());
                }
                let mut opened = tab(
                    format!("tab-{}", uuid::Uuid::new_v4().simple()),
                    params.kind,
                    params.target,
                );
                opened.title = params.title;
                let id = opened.id.clone();
                strip.tabs.push(opened);
                id
            }
        };
        strip.active = Some(id.clone());
        Ok(id)
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabParams {
    project: String,
    tab: String,
    /// Where `moveTab` puts it, counted in the strip as it is now.
    to: Option<usize>,
}

/// Closes a tab. Agents it shows close with it, since an agent always has a
/// tab; terminals keep running until a client ends them.
pub(crate) fn close_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    let agents = edit(core, |layout| {
        let strip = strip(layout, &params.project)?;
        let at = strip
            .tabs
            .iter()
            .position(|tab| tab.id == params.tab)
            .ok_or("that tab is not open")?;
        let closed = strip.tabs.remove(at);
        if strip.active.as_deref() == Some(params.tab.as_str()) {
            strip.active = strip
                .tabs
                .get(at.min(strip.tabs.len().saturating_sub(1)))
                .map(|tab| tab.id.clone());
        }
        let mut agents = Vec::new();
        agents_in(&closed.pane, &mut agents);
        Ok(agents)
    })?;
    if !agents.is_empty() {
        super::workspace::close(core, &agents)?;
    }
    Ok(())
}

pub(crate) fn activate_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    edit(core, |layout| {
        layout.active_project = Some(params.project.clone());
        let strip = strip(layout, &params.project)?;
        if !strip.tabs.iter().any(|tab| tab.id == params.tab) {
            return Err("that tab is not open".into());
        }
        strip.active = Some(params.tab);
        Ok(())
    })
}

pub(crate) fn move_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    edit(core, |layout| {
        let strip = strip(layout, &params.project)?;
        let at = strip
            .tabs
            .iter()
            .position(|tab| tab.id == params.tab)
            .ok_or("that tab is not open")?;
        let moved = strip.tabs.remove(at);
        let to = params.to.unwrap_or(strip.tabs.len()).min(strip.tabs.len());
        strip.tabs.insert(to, moved);
        Ok(())
    })
}

#[derive(Deserialize)]
pub(crate) struct ProjectParams {
    project: String,
}

pub(crate) fn activate_project(core: &Core, params: ProjectParams) -> CoreResult<()> {
    edit(core, |layout| {
        strip(layout, &params.project)?;
        layout.active_project = Some(params.project);
        Ok(())
    })
}
