//! The tabs desktop clients draw, kept by the core so every client shows the
//! same ones. Agents' tabs follow the workspace: a new agent gets a tab at the
//! end of its project's strip, and a tab goes once nothing it shows is left.

use std::collections::HashSet;
use std::sync::Mutex;

use serde::Deserialize;
use sikemux_process::sync::Locked;

use crate::protocol::{Layout, Pane, PaneKind, SessionId, SessionInfo, SplitDir, Tab, Workspace};

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
        PaneKind::Agent | PaneKind::Desk => agents.contains(target),
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
    layout.hosts.retain(|_, strip| {
        strip
            .tabs
            .retain_mut(|tab| keep_shown(&mut tab.pane, &shown));
        if !strip
            .active
            .as_ref()
            .is_some_and(|active| strip.tabs.iter().any(|tab| &tab.id == active))
        {
            strip.active = strip.tabs.first().map(|tab| tab.id.clone());
        }
        !strip.tabs.is_empty()
    });
    if !layout
        .active_host
        .as_ref()
        .is_some_and(|host| layout.hosts.contains_key(host))
    {
        layout.active_host = None;
    }
    layout
}

/// Drops the views nothing is left for, answering whether any remain.
fn keep_shown(pane: &mut Pane, shown: &impl Fn(PaneKind, &str) -> bool) -> bool {
    let keep = |view: &Pane| match view {
        Pane::View { kind, target, .. } => shown(*kind, target),
        Pane::Split { .. } => true,
    };
    keep(pane) && prune(pane, &keep)
}

/// Drops the views inside `pane` that `keep` refuses. A split left with one pane
/// becomes that pane, and its share goes to the panes that stay. Answers whether
/// anything remains.
fn prune(pane: &mut Pane, keep: &impl Fn(&Pane) -> bool) -> bool {
    let Pane::Split { panes, sizes, .. } = pane else {
        return true;
    };
    let mut kept = Vec::with_capacity(panes.len());
    let mut kept_sizes = Vec::with_capacity(panes.len());
    for (mut child, size) in panes.drain(..).zip(sizes.iter().copied()) {
        if keep(&child) && prune(&mut child, keep) {
            kept.push(child);
            kept_sizes.push(size);
        }
    }
    match kept.len() {
        0 => false,
        1 => {
            *pane = kept.remove(0);
            true
        }
        _ => {
            *panes = kept;
            *sizes = shares(&kept_sizes);
            true
        }
    }
}

/// `sizes` scaled to thousandths, every pane keeping some room.
fn shares(sizes: &[u16]) -> Vec<u16> {
    let total: u32 = sizes.iter().map(|&size| u32::from(size.max(1))).sum();
    let mut scaled: Vec<u16> = sizes
        .iter()
        .map(|&size| (u32::from(size.max(1)) * 1000 / total).max(1) as u16)
        .collect();
    let spare = 1000 - scaled.iter().map(|&size| i32::from(size)).sum::<i32>();
    if let Some(last) = scaled.last_mut() {
        *last = (i32::from(*last) + spare).max(1) as u16;
    }
    scaled
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

fn view_id() -> String {
    format!("pane-{}", uuid::Uuid::new_v4().simple())
}

/// Puts `new` beside the view `beside`, answering whether that view is in `pane`.
/// A split already running the same way takes it as one more pane.
fn insert_beside(
    pane: &mut Pane,
    beside: &str,
    dir: SplitDir,
    after: bool,
    new: &mut Option<Pane>,
) -> bool {
    match pane {
        Pane::View { id, .. } if id == beside => {
            let Some(new) = new.take() else {
                return true;
            };
            let old = std::mem::replace(
                pane,
                Pane::Split {
                    id: view_id(),
                    dir,
                    panes: Vec::new(),
                    sizes: vec![500, 500],
                },
            );
            if let Pane::Split { panes, .. } = pane {
                *panes = if after {
                    vec![old, new]
                } else {
                    vec![new, old]
                };
            }
            true
        }
        Pane::View { .. } => false,
        Pane::Split {
            dir: running,
            panes,
            sizes,
            ..
        } => {
            let at = panes
                .iter()
                .position(|child| matches!(child, Pane::View { id, .. } if id == beside));
            if let (Some(at), true) = (at, *running == dir) {
                let Some(new) = new.take() else {
                    return true;
                };
                let half = sizes[at] / 2;
                sizes[at] -= half;
                let to = if after { at + 1 } else { at };
                panes.insert(to, new);
                sizes.insert(to, half);
                *sizes = shares(sizes);
                return true;
            }
            panes
                .iter_mut()
                .any(|child| insert_beside(child, beside, dir, after, new))
        }
    }
}

fn find_split<'a>(pane: &'a mut Pane, split: &str) -> Option<&'a mut Pane> {
    match pane {
        Pane::Split { id, .. } if id == split => Some(pane),
        Pane::Split { panes, .. } => panes.iter_mut().find_map(|child| find_split(child, split)),
        Pane::View { .. } => None,
    }
}

fn find_view<'a>(pane: &'a Pane, view: &str) -> Option<&'a Pane> {
    match pane {
        Pane::View { id, .. } if id == view => Some(pane),
        Pane::View { .. } => None,
        Pane::Split { panes, .. } => panes.iter().find_map(|child| find_view(child, view)),
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
        preview: false,
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
    /// Opens it to look at, in place of the strip's tab opened that way.
    #[serde(default)]
    preview: bool,
}

/// Brings the tab showing `target` to the front, opening one at the end of
/// the strip when none does, or in place of the strip's preview when it
/// opens to look at. A tab brought forward to keep is kept. Answers its id.
pub(crate) fn open_tab(core: &Core, params: OpenTab) -> CoreResult<String> {
    edit(core, |layout| {
        layout.active_project = Some(params.project.clone());
        layout.active_host = None;
        let strip = strip(layout, &params.project)?;
        let shown = strip
            .tabs
            .iter()
            .position(|tab| holds(&tab.pane, params.kind, &params.target));
        let replaced = strip.tabs.iter().position(|tab| tab.preview);
        let id = match (shown, replaced) {
            (Some(at), _) => {
                let tab = &mut strip.tabs[at];
                tab.preview &= params.preview;
                tab.id.clone()
            }
            (None, Some(at)) if params.preview => {
                let tab = &mut strip.tabs[at];
                let view = format!("view-{}", uuid::Uuid::new_v4().simple());
                tab.pane = Pane::View {
                    id: view,
                    kind: params.kind,
                    target: params.target,
                };
                tab.title = params.title;
                tab.id.clone()
            }
            (None, _) => {
                if strip.tabs.len() >= MAX_TABS {
                    return Err("that project already has as many tabs as the core keeps".into());
                }
                let mut opened = tab(
                    format!("tab-{}", uuid::Uuid::new_v4().simple()),
                    params.kind,
                    params.target,
                );
                opened.title = params.title;
                opened.preview = params.preview;
                let id = opened.id.clone();
                strip.tabs.push(opened);
                id
            }
        };
        strip.active = Some(id.clone());
        Ok(id)
    })
}

/// The strip a tab call names: an SSH host's, or a project's.
fn strip_of<'a>(
    layout: &'a mut Layout,
    params: &TabParams,
) -> CoreResult<&'a mut crate::protocol::ProjectLayout> {
    strip_at(layout, &params.project, params.host.as_deref())
}

fn strip_at<'a>(
    layout: &'a mut Layout,
    project: &str,
    host: Option<&str>,
) -> CoreResult<&'a mut crate::protocol::ProjectLayout> {
    match host {
        Some(host) => layout
            .hosts
            .get_mut(host)
            .ok_or_else(|| CoreError::from("no terminal is open to that host")),
        None => strip(layout, project),
    }
}

fn tab_in<'a>(strip: &'a mut crate::protocol::ProjectLayout, tab: &str) -> CoreResult<&'a mut Tab> {
    strip
        .tabs
        .iter_mut()
        .find(|held| held.id == tab)
        .ok_or_else(|| CoreError::from("that tab is not open"))
}

/// Opens a tab on the terminal `id` at the end of the SSH host's strip and
/// brings the host to the front. Answers the tab's id.
pub(crate) fn show_ssh_terminal(core: &Core, host: String, id: SessionId) -> CoreResult<String> {
    edit(core, |layout| {
        let strip = layout.hosts.entry(host.clone()).or_default();
        if strip.tabs.len() >= MAX_TABS {
            return Err("that host already has as many tabs as the core keeps".into());
        }
        let opened = tab(
            format!("tab-{}", uuid::Uuid::new_v4().simple()),
            PaneKind::Terminal,
            id.to_string(),
        );
        let tab_id = opened.id.clone();
        strip.tabs.push(opened);
        strip.active = Some(tab_id.clone());
        layout.active_host = Some(host);
        Ok(tab_id)
    })
}

/// Brings the tab showing the terminal `id` to the front in `project`,
/// opening one when none does. Answers its id.
pub(crate) fn show_terminal(
    core: &Core,
    project: String,
    id: SessionId,
    title: Option<String>,
) -> CoreResult<String> {
    open_tab(
        core,
        OpenTab {
            project,
            kind: PaneKind::Terminal,
            target: id.to_string(),
            title,
            preview: false,
        },
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TabParams {
    #[serde(default)]
    project: String,
    /// The SSH host whose strip holds the tab, in place of a project.
    host: Option<String>,
    tab: String,
    /// Where `moveTab` puts it, counted in the strip as it is now.
    to: Option<usize>,
}

/// Closes a tab. Agents it shows close with it, since an agent always has a
/// tab; terminals keep running until a client ends them.
pub(crate) fn close_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    let agents = edit(core, |layout| {
        let strip = strip_of(layout, &params)?;
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

/// Keeps a tab opened to look at, so the next one opens beside it.
pub(crate) fn keep_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    edit(core, |layout| {
        let strip = strip_of(layout, &params)?;
        let tab = strip
            .tabs
            .iter_mut()
            .find(|tab| tab.id == params.tab)
            .ok_or("that tab is not open")?;
        tab.preview = false;
        Ok(())
    })
}

pub(crate) fn activate_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    edit(core, |layout| {
        match &params.host {
            Some(host) => layout.active_host = Some(host.clone()),
            None => {
                layout.active_project = Some(params.project.clone());
                layout.active_host = None;
            }
        }
        let strip = strip_of(layout, &params)?;
        if !strip.tabs.iter().any(|tab| tab.id == params.tab) {
            return Err("that tab is not open".into());
        }
        strip.active = Some(params.tab);
        Ok(())
    })
}

pub(crate) fn move_tab(core: &Core, params: TabParams) -> CoreResult<()> {
    edit(core, |layout| {
        let strip = strip_of(layout, &params)?;
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
        layout.active_host = None;
        Ok(())
    })
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SplitPane {
    #[serde(default)]
    project: String,
    host: Option<String>,
    tab: String,
    /// The view the new one goes beside, by id.
    pane: String,
    side: Side,
    kind: Option<PaneKind>,
    target: Option<String>,
    /// A tab of the same strip whose pane moves into the split, closing the tab.
    from_tab: Option<String>,
}

/// Puts a view beside one of a tab's views: a new view of `kind` showing
/// `target`, or the pane of `from_tab`. Brings the tab to the front and
/// answers the id of the view it added, or of the first moved one.
pub(crate) fn split_pane(core: &Core, params: SplitPane) -> CoreResult<String> {
    edit(core, |layout| {
        let strip = strip_at(layout, &params.project, params.host.as_deref())?;
        let new = match (&params.from_tab, params.kind, &params.target) {
            (Some(from), _, _) => {
                if from == &params.tab {
                    return Err("a tab cannot be split into itself".into());
                }
                let at = strip
                    .tabs
                    .iter()
                    .position(|tab| &tab.id == from)
                    .ok_or("that tab is not open")?;
                strip.tabs.remove(at).pane
            }
            (None, Some(kind), Some(target)) => Pane::View {
                id: view_id(),
                kind,
                target: target.clone(),
            },
            _ => return Err("a split needs a kind and target, or a tab to move".into()),
        };
        let added = first_view_id(&new);
        let tab = tab_in(strip, &params.tab)?;
        let (dir, after) = match params.side {
            Side::Left => (SplitDir::Row, false),
            Side::Right => (SplitDir::Row, true),
            Side::Top => (SplitDir::Column, false),
            Side::Bottom => (SplitDir::Column, true),
        };
        let mut new = Some(new);
        if !insert_beside(&mut tab.pane, &params.pane, dir, after, &mut new) {
            return Err("that view is not in the tab".into());
        }
        strip.active = Some(params.tab.clone());
        Ok(added)
    })
}

fn first_view_id(pane: &Pane) -> String {
    match pane {
        Pane::View { id, .. } => id.clone(),
        Pane::Split { panes, .. } => panes.first().map(first_view_id).unwrap_or_default(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaneParams {
    #[serde(default)]
    project: String,
    host: Option<String>,
    tab: String,
    pane: String,
}

/// Closes one view of a tab; a tab left with nothing closes as `closeTab` does.
/// An agent shown in it closes with it.
pub(crate) fn close_pane(core: &Core, params: PaneParams) -> CoreResult<()> {
    let closed = edit(core, |layout| {
        let strip = strip_at(layout, &params.project, params.host.as_deref())?;
        let tab = tab_in(strip, &params.tab)?;
        let view = find_view(&tab.pane, &params.pane)
            .cloned()
            .ok_or("that view is not in the tab")?;
        let keep = |pane: &Pane| !matches!(pane, Pane::View { id, .. } if id == &params.pane);
        let left = keep(&tab.pane) && prune(&mut tab.pane, &keep);
        Ok((view, left))
    })?;
    match closed {
        (_, false) => close_tab(
            core,
            TabParams {
                project: params.project,
                host: params.host,
                tab: params.tab,
                to: None,
            },
        ),
        (view, true) => {
            let mut agents = Vec::new();
            agents_in(&view, &mut agents);
            if !agents.is_empty() {
                super::workspace::close(core, &agents)?;
            }
            Ok(())
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResizeSplit {
    #[serde(default)]
    project: String,
    host: Option<String>,
    tab: String,
    split: String,
    /// Each pane's share, in any unit; the core keeps them in thousandths.
    sizes: Vec<u16>,
}

pub(crate) fn resize_split(core: &Core, params: ResizeSplit) -> CoreResult<()> {
    edit(core, |layout| {
        let strip = strip_at(layout, &params.project, params.host.as_deref())?;
        let tab = tab_in(strip, &params.tab)?;
        let Some(Pane::Split { panes, sizes, .. }) = find_split(&mut tab.pane, &params.split)
        else {
            return Err("that split is not in the tab".into());
        };
        if params.sizes.len() != panes.len() {
            return Err("a split needs one size for each of its panes".into());
        }
        *sizes = shares(&params.sizes);
        Ok(())
    })
}
