use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    div, px, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Styled, Subscription, Window,
};
use serde_json::Value;
use sikemux_wire::protocol::{AgentStatus, DeviceView, ProjectInfo, RecentInfo};
use tokio::sync::mpsc;

use crate::core::{Ask, Link, Update};
use crate::theme::{self, Tokens};
use crate::{agents, home, rail, tabs};

pub struct Workbench {
    pub version: String,
    pub view: Option<DeviceView>,
    pub activity: Option<Value>,
    pub lost: Option<String>,
    pub project: Option<String>,
    pub agent: Option<String>,
    pub all_projects: bool,
    /// Which of the calendar's metrics it shows, an index into `home::METRICS`.
    pub metric: usize,
    pub filter: Option<Entity<InputState>>,
    pub filter_text: String,
    pub asks: mpsc::UnboundedSender<Ask>,
    _filtering: Option<Subscription>,
}

/// An agent as the rails and tabs show it.
pub struct AgentRow {
    pub id: String,
    pub provider: String,
    pub title: String,
    pub name: Option<String>,
    pub status: AgentStatus,
    pub subagents: u32,
    pub tasks: u32,
    pub project: String,
}

impl Workbench {
    pub fn new(link: Link, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Link { mut updates, asks } = link;
        cx.spawn(async move |this, cx| {
            while let Some(update) = updates.recv().await {
                if this.update(cx, |this, cx| this.apply(update, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            version: String::new(),
            view: None,
            activity: None,
            lost: None,
            project: None,
            agent: None,
            all_projects: false,
            metric: 0,
            filter: None,
            filter_text: String::new(),
            asks,
            _filtering: None,
        }
    }

    pub fn toggle_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filter.take().is_some() {
            self._filtering = None;
            self.filter_text.clear();
            return;
        }
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("filter recent chats…"));
        self._filtering = Some(cx.subscribe(&filter, |bench, filter, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                bench.filter_text = filter.read(cx).value().to_lowercase();
                cx.notify();
            }
        }));
        filter.update(cx, |state, cx| state.focus(window, cx));
        self.filter = Some(filter);
    }

    /// The rows the agent rail lists: this project's, or every project's.
    pub fn listed_agents(&self) -> Vec<AgentRow> {
        let mut rows = if self.all_projects {
            self.projects().iter().flat_map(|project| self.agents_of(&project.id)).collect()
        } else {
            self.project.as_deref().map(|project| self.agents_of(project)).unwrap_or_default()
        };
        rows.retain(|row| row.title.to_lowercase().contains(&self.filter_text));
        rows
    }

    fn apply(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::Connected(version) => {
                self.version = version;
                self.lost = None;
            }
            Update::View(view) => {
                self.view = Some(*view);
                if self.project().is_none() {
                    let first = self.projects().first().map(|project| project.id.clone());
                    self.select_project(first);
                }
            }
            Update::Activity(summary) => self.activity = Some(summary),
            Update::Lost(reason) => self.lost = Some(reason),
        }
        cx.notify();
    }

    pub fn projects(&self) -> &[ProjectInfo] {
        self.view
            .as_ref()
            .map_or(&[], |view| view.workspace.projects.as_slice())
    }

    pub fn project(&self) -> Option<&ProjectInfo> {
        let id = self.project.as_deref()?;
        self.projects().iter().find(|project| project.id == id)
    }

    pub fn select_project(&mut self, id: Option<String>) {
        self.project = id;
        self.agent = self
            .project
            .as_deref()
            .and_then(|project| self.agents_of(project).into_iter().next())
            .map(|agent| agent.id);
        let path = self
            .project()
            .map(|project| project.path.to_string_lossy().into_owned());
        let _ = self.asks.send(Ask::Activity { project: path });
    }

    /// The project's agents in the order the core ranks them for the person.
    pub fn agents_of(&self, project: &str) -> Vec<AgentRow> {
        let Some(view) = &self.view else {
            return Vec::new();
        };
        let ranked: Vec<&str> = view.inbox["agents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|agent| agent["agentId"].as_str())
            .collect();
        let mut rows: Vec<AgentRow> = view
            .workspace
            .agents
            .iter()
            .filter(|agent| agent.project == project)
            .map(|agent| {
                let chat = view.chats.iter().find(|chat| chat.agent_id == agent.id);
                let session = view
                    .sessions
                    .iter()
                    .find(|session| session.agent_id.as_deref() == Some(agent.id.as_str()));
                AgentRow {
                    id: agent.id.clone(),
                    provider: agent.provider.clone(),
                    title: agent.title.clone(),
                    name: agent
                        .name
                        .clone()
                        .or_else(|| chat.and_then(|chat| chat.name.clone()))
                        .or_else(|| session.and_then(|session| session.agent_name.clone())),
                    status: chat
                        .map(|chat| chat.status)
                        .or_else(|| session.and_then(|session| session.status))
                        .unwrap_or(if agent.asleep {
                            AgentStatus::Asleep
                        } else {
                            AgentStatus::Idle
                        }),
                    subagents: chat.map_or(0, |chat| chat.subagents),
                    tasks: chat.map_or(0, |chat| chat.tasks),
                    project: agent.project.clone(),
                }
            })
            .collect();
        rows.sort_by_key(|row| {
            ranked
                .iter()
                .position(|id| *id == row.id)
                .unwrap_or(usize::MAX)
        });
        rows
    }

    pub fn recent_of(&self, project: &str) -> Vec<&RecentInfo> {
        self.view.as_ref().map_or(Vec::new(), |view| {
            view.recent
                .iter()
                .filter(|recent| recent.project == project)
                .filter(|recent| recent.title.to_lowercase().contains(&self.filter_text))
                .collect()
        })
    }
}

impl Render for Workbench {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("shell")
            .size_full()
            .flex()
            .flex_col()
            .pt(px(6.))
            .bg(Tokens::get(cx).surface_sunken)
            .font_family(theme::UI)
            .text_size(px(13.))
            .line_height(px(13. * 1.45))
            .text_color(Tokens::get(cx).text_secondary)
            .child(tabs::top_bar(self, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(10.))
                    .pt(px(4.))
                    .px(px(10.))
                    .pb(px(10.))
                    .child(rail::side_rail(self, cx))
                    .child(home::stage(self, cx))
                    .child(agents::tab_rail(self, cx)),
            )
    }
}
