use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{div, px, AppContext, Context, Entity, Image, ImageFormat, InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Window};
use serde_json::{json, Value};
use sikemux_wire::protocol::{AgentStatus, DeviceView, Pane, PaneKind, ProjectInfo, RecentInfo, SessionInfo, Tab};
use tokio::sync::mpsc;

use crate::core::{utc_offset_minutes, Ask, Link, Update, Watch};
use crate::theme::{self, Tokens};
use crate::{agents, home, rail, tabs};

/// The one store: what the core last said, and what only this window decides.
pub struct Workbench {
    pub version: String,
    pub view: Option<DeviceView>,
    /// The latest answer to each watched call, by key.
    reads: HashMap<&'static str, Value>,
    pub lost: Option<String>,
    /// The project the per-project watches were last made for, by path.
    watched_project: Option<String>,
    /// The account's picture, and the hash the core named the one asked for by.
    picture: Option<Arc<Image>>,
    picture_hash: Option<String>,
    /// From asking the core to sign in until the browser is done or gives up.
    pub signing_in: bool,
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
        let _ = asks.send(Ask::Watch("usage", Watch::new("agentUsages", json!({}), &["usage"])));
        let _ = asks.send(Ask::Watch("plugins", Watch::new("plugins", json!({}), &["plugins"])));
        let _ = asks.send(Ask::Watch("hosts", Watch::new("hosts", json!({}), &["hosts"])));
        Self {
            version: String::new(),
            view: None,
            reads: HashMap::new(),
            lost: None,
            watched_project: None,
            picture: None,
            picture_hash: None,
            signing_in: false,
            all_projects: false,
            metric: 0,
            filter: None,
            filter_text: String::new(),
            asks,
            _filtering: None,
        }
    }

    fn apply(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::Connected(version) => {
                self.version = version;
                self.lost = None;
            }
            Update::View(view) => {
                self.view = Some(*view);
                self.watch_project();
                self.watch_picture();
            }
            Update::Read("picture", value) => self.picture = decode_picture(&value),
            Update::Read("sign-in", value) => match value["url"].as_str() {
                Some(url) => {
                    cx.open_url(url);
                    let _ = self.asks.send(Ask::Answer { key: "sign-in-finished", method: "accountSignInFinish", params: json!({}) });
                }
                None => self.signing_in = false,
            },
            Update::Read("sign-in-finished", _) => self.signing_in = false,
            Update::Refused(key, error) => {
                eprintln!("sikemux: the core refused {key}: {error}");
                if key.starts_with("sign-in") {
                    self.signing_in = false;
                }
            }
            Update::Read(key, value) => {
                self.reads.insert(key, value);
            }
            Update::Lost(reason) => self.lost = Some(reason),
        }
        cx.notify();
    }

    /// Points the watches that follow the project in front at it.
    fn watch_project(&mut self) {
        let path = self.project().map(|project| project.path.to_string_lossy().into_owned());
        if path == self.watched_project {
            return;
        }
        let activity = json!({ "utcOffsetMinutes": utc_offset_minutes(), "project": path });
        let _ = self.asks.send(Ask::Watch("activity", Watch::new("activitySummary", activity, &["activity"])));
        let _ = self.asks.send(Ask::Watch("tasks", Watch::new("tasks", json!({ "project": path }), &["tasks", "runs"])));
        self.watched_project = path;
    }

    /// Asks for the account's picture when it is one this window has not got.
    fn watch_picture(&mut self) {
        let hash = self.view.as_ref().and_then(|view| view.account.as_ref()?.picture_hash.clone());
        if hash == self.picture_hash {
            return;
        }
        self.picture = None;
        if let Some(hash) = &hash {
            let _ = self.asks.send(Ask::Watch("picture", Watch::new("accountPicture", json!({ "hash": hash }), &[])));
        }
        self.picture_hash = hash;
    }

    pub fn picture(&self) -> Option<Arc<Image>> {
        self.picture.clone()
    }

    pub fn read(&self, key: &str) -> Option<&Value> {
        self.reads.get(key)
    }

    fn call(&self, method: &'static str, params: Value) {
        let _ = self.asks.send(Ask::Call { method, params });
    }

    pub fn projects(&self) -> &[ProjectInfo] {
        self.view.as_ref().map_or(&[], |view| view.workspace.projects.as_slice())
    }

    /// The project in front, as the core's layout says.
    pub fn project(&self) -> Option<&ProjectInfo> {
        let active = self.view.as_ref()?.layout.active_project.as_deref();
        let projects = self.projects();
        active.and_then(|id| projects.iter().find(|project| project.id == id)).or(projects.first())
    }

    pub fn activate_project(&self, id: &str) {
        self.call("activateProject", json!({ "project": id }));
    }

    pub fn tabs(&self, project: &str) -> &[Tab] {
        self.view
            .as_ref()
            .and_then(|view| view.layout.projects.get(project))
            .map_or(&[], |layout| layout.tabs.as_slice())
    }

    /// The tab in front in the project in front.
    pub fn tab(&self) -> Option<&Tab> {
        let project = self.project()?;
        let layout = self.view.as_ref()?.layout.projects.get(&project.id)?;
        let tabs = layout.tabs.as_slice();
        layout.active.as_deref().and_then(|id| tabs.iter().find(|tab| tab.id == id)).or(tabs.first())
    }

    pub fn activate_tab(&self, project: &str, tab: &str) {
        self.call("activateTab", json!({ "project": project, "tab": tab }));
    }

    pub fn close_tab(&self, project: &str, tab: &str) {
        self.call("closeTab", json!({ "project": project, "tab": tab }));
    }

    /// Brings an agent's tab to the front.
    pub fn activate_agent(&self, agent: &AgentRow) {
        if let Some(tab) = self.tabs(&agent.project).iter().find(|tab| shows(tab, PaneKind::Agent) == Some(&agent.id)) {
            self.activate_project(&agent.project);
            self.activate_tab(&agent.project, &tab.id);
        }
    }

    /// The agent the tab in front shows.
    pub fn agent(&self) -> Option<AgentRow> {
        let project = self.project()?;
        let target = shows(self.tab()?, PaneKind::Agent)?;
        self.agents_of(&project.id).into_iter().find(|agent| &agent.id == target)
    }

    pub fn session(&self, id: &str) -> Option<&SessionInfo> {
        self.view.as_ref()?.sessions.iter().find(|session| session.id.to_string() == id)
    }

    /// Terminals started outside every project, which the core numbers.
    pub fn loose_terminals(&self) -> Vec<&SessionInfo> {
        self.view.as_ref().map_or(Vec::new(), |view| {
            view.sessions.iter().filter(|session| session.rail_name.is_some() && session.exit.is_none()).collect()
        })
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
                let session = view.sessions.iter().find(|session| session.agent_id.as_deref() == Some(agent.id.as_str()));
                AgentRow {
                    id: agent.id.clone(),
                    provider: agent.provider.clone(),
                    title: agent.title.clone(),
                    name: agent.name.clone(),
                    status: chat
                        .map(|chat| chat.status)
                        .or_else(|| session.and_then(|session| session.status))
                        .unwrap_or(if agent.asleep { AgentStatus::Asleep } else { AgentStatus::Idle }),
                    subagents: chat.map_or(0, |chat| chat.subagents),
                    tasks: chat.map_or(0, |chat| chat.tasks),
                    project: agent.project.clone(),
                }
            })
            .collect();
        rows.sort_by_key(|row| ranked.iter().position(|id| *id == row.id).unwrap_or(usize::MAX));
        rows
    }

    /// The rows the agent rail lists: this project's, or every project's.
    pub fn listed_agents(&self) -> Vec<AgentRow> {
        let mut rows = if self.all_projects {
            self.projects().iter().flat_map(|project| self.agents_of(&project.id)).collect()
        } else {
            self.project().map(|project| self.agents_of(&project.id)).unwrap_or_default()
        };
        rows.retain(|row| row.title.to_lowercase().contains(&self.filter_text));
        rows
    }

    /// The project's saved chats no agent holds, which the agent list already shows.
    pub fn recent_of(&self, project: &str) -> Vec<&RecentInfo> {
        self.view.as_ref().map_or(Vec::new(), |view| {
            view.recent
                .iter()
                .filter(|recent| recent.project == project && recent.agent_id.is_none())
                .filter(|recent| recent.title.to_lowercase().contains(&self.filter_text))
                .collect()
        })
    }

    pub fn sign_in(&mut self) {
        self.signing_in = true;
        let _ = self.asks.send(Ask::Answer { key: "sign-in", method: "accountSignIn", params: json!({}) });
    }

    /// Ends a sign-in the browser has not finished, which settles the one waiting on it.
    pub fn cancel_sign_in(&self) {
        self.call("accountSignInCancel", json!({}));
    }

    pub fn sign_out(&self) {
        self.call("accountSignOut", json!({}));
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
}

/// The picture `accountPicture` answers with: its type, and its bytes in base64.
fn decode_picture(value: &Value) -> Option<Arc<Image>> {
    use base64::Engine;
    let format = ImageFormat::from_mime_type(value["mime"].as_str()?)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(value["data"].as_str()?).ok()?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

/// What a tab's pane shows when it is one view of `kind`.
pub fn shows(tab: &Tab, kind: PaneKind) -> Option<&String> {
    match &tab.pane {
        Pane::View { kind: shown, target, .. } if *shown == kind => Some(target),
        _ => None,
    }
}

impl Render for Workbench {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Tokens::get(cx);
        div()
            .id("shell")
            .size_full()
            .flex()
            .flex_col()
            .pt(px(6.))
            .bg(t.surface_sunken)
            .font_family(theme::UI)
            .text_size(px(13.))
            .line_height(px(13. * 1.45))
            .text_color(t.text_secondary)
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
