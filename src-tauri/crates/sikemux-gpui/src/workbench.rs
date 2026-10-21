use std::collections::HashMap;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Timelike;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::{div, px, AppContext, Context, Entity, Image, ImageFormat, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Window};
use serde_json::{json, Value};
use sikemux_wire::protocol::{AgentFace, AgentGroup, AgentStatus, DeviceView, LauncherInfo, Pane, PaneKind, ProjectInfo, RecentInfo, SessionInfo, Tab};
use tokio::sync::mpsc;

use crate::battery::Battery;
use crate::chat::{Chat, Composing, Question};
use crate::files::FileTree;
use crate::finder::{Finder, OpenFile, ProjectFiles};
use crate::core::{utc_offset_minutes, Ask, ChatAsk, ChatNews, Link, Update, Watch};
use crate::theme::{self, Tokens};
use crate::{agents, home, rail, tabs, terminal};

/// The one store: what the core last said, and what only this window decides.
pub struct Workbench {
    pub version: String,
    pub view: Option<DeviceView>,
    /// The latest answer to each watched call, by key.
    reads: HashMap<&'static str, Value>,
    pub lost: Option<String>,
    /// The project the per-project watches were last made for, by path.
    watched_project: Option<String>,
    pub battery: Option<Battery>,
    /// The repository and branch whose latest CI run the top bar shows.
    pub watched_ci: Option<CiRepo>,
    /// The account's picture, and the hash the core named the one asked for by.
    picture: Option<Arc<Image>>,
    picture_hash: Option<String>,
    /// From asking the core to sign in until the browser is done or gives up.
    pub signing_in: bool,
    pub all_projects: bool,
    /// Which of its pages the tab rail shows.
    pub rail_page: RailPage,
    pub files: FileTree,
    pub finder: Option<Finder>,
    pub project_files: ProjectFiles,
    /// The agent the rail was switched to, else the one the project last started.
    pub launcher_choice: Option<String>,
    /// The rail lists the recent chats of every agent, not one's.
    pub all_harnesses: bool,
    /// Which of the calendar's metrics it shows, an index into `home::METRICS`.
    pub metric: usize,
    pub filter: Option<Entity<InputState>>,
    pub filter_text: String,
    pub asks: mpsc::UnboundedSender<Ask>,
    /// The chats of the agents with a tab open, by agent id.
    pub chats: HashMap<String, Chat>,
    /// What agents asked the person, and the assumptions they went on.
    pub questions: Vec<Question>,
    pub terminals: terminal::Terminals,
    /// Holds the keyboard while nothing in the window does, so the window's shortcuts reach it.
    pub focus: FocusHandle,
    _filtering: Option<Subscription>,
}

#[derive(Clone, PartialEq)]
pub struct CiRepo {
    pub host: String,
    pub owner: String,
    pub name: String,
    pub account: Option<String>,
    pub branch: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RailPage {
    Agents,
    Files,
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
    /// The face it wears, else none and its agent's mark is drawn.
    pub face: Option<AgentFace>,
    pub group: AgentGroup,
}

impl Workbench {
    pub fn new(link: Link, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Link { mut updates, asks } = link;
        crate::faces::install(cx);
        crate::file_icons::install(cx);
        crate::finder::install(cx);
        let (power_changed, mut power) = mpsc::unbounded_channel();
        crate::battery::watch(move || {
            let _ = power_changed.send(());
        });
        cx.spawn(async move |this, cx| {
            while power.recv().await.is_some() {
                let read = |bench: &mut Self, cx: &mut Context<Self>| {
                    bench.battery = crate::battery::read();
                    cx.notify();
                };
                if this.update(cx, read).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            while let Some(update) = updates.recv().await {
                let mut batch = vec![update];
                while let Ok(update) = updates.try_recv() {
                    batch.push(update);
                }
                if this.update(cx, |this, cx| batch.into_iter().for_each(|update| this.apply(update, cx))).is_err() {
                    break;
                }
            }
        })
        .detach();
        let _ = asks.send(Ask::TerminalColors(Tokens::get(cx).terminal.clone()));
        let _ = asks.send(Ask::Watch("plugins", Watch::new("plugins", json!({}), &["plugins"])));
        let _ = asks.send(Ask::Watch("hosts", Watch::new("hosts", json!({}), &["hosts"])));
        let _ = asks.send(Ask::Watch("runs", Watch::new("taskRuns", json!({}), &["runs"])));
        let _ = asks.send(Ask::Watch("questions", Watch::new("questions", json!({}), &["questions"])));
        let _ = asks.send(Ask::Watch("catalog", Watch::new("agentCatalog", json!({}), &[])));
        let _ = asks.send(Ask::Follow("git"));
        cx.spawn(async move |this, cx| loop {
            let Ok(wait) = this.update(cx, |bench, _| bench.until_next_tick()) else {
                break;
            };
            cx.background_executor().timer(wait).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        })
        .detach();
        let bench = Self {
            version: String::new(),
            view: None,
            reads: HashMap::new(),
            lost: None,
            watched_project: None,
            watched_ci: None,
            battery: crate::battery::read(),
            picture: None,
            picture_hash: None,
            signing_in: false,
            all_projects: false,
            rail_page: RailPage::Agents,
            files: FileTree::default(),
            finder: None,
            project_files: ProjectFiles::default(),
            launcher_choice: None,
            all_harnesses: false,
            metric: 0,
            filter: None,
            filter_text: String::new(),
            asks,
            chats: HashMap::new(),
            questions: Vec::new(),
            terminals: terminal::Terminals::default(),
            focus: cx.focus_handle(),
            _filtering: None,
        };
        bench.focus.focus(window, cx);
        bench.read_usage();
        bench
    }

    /// Reads every account's plan limits again, as they are whenever usage changes.
    pub fn read_usage(&self) {
        let _ = self.asks.send(Ask::Watch("usage", Watch::new("agentUsages", json!({}), &["usage"])));
    }

    fn apply(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::Connected(version) => {
                self.version = version;
                self.lost = None;
                for (agent, chat) in &self.chats {
                    self.ask_chat(ChatAsk::Attach { agent: agent.clone(), since: chat.mark.clone() });
                }
                self.terminals.reattach(&self.asks);
            }
            Update::View(view) => {
                self.view = Some(*view);
                let sessions = &self.view.as_ref().map_or(&[][..], |view| view.sessions.as_slice());
                self.terminals.keep(|id| sessions.iter().any(|session| session.id == id), &self.asks);
                self.watch_project();
                self.watch_picture();
                self.watch_ci();
                self.close_chats();
                self.open_chat(cx);
            }
            Update::Chat(agent, news) => {
                let Some(chat) = self.chats.get_mut(&agent) else {
                    return;
                };
                match news {
                    ChatNews::Attached(attachment) => chat.attached(attachment, cx),
                    ChatNews::Changes { seq, changes } => {
                        if chat.changed(seq, changes, cx) {
                            let since = chat.mark.clone();
                            self.ask_chat(ChatAsk::Attach { agent, since });
                        }
                    }
                    ChatNews::Page(page) => chat.page(page, cx),
                }
                return;
            }
            Update::Read("plugins", value) => {
                self.reads.insert("plugins", value);
                self.watch_ci();
            }
            Update::Read("questions", value) => match serde_json::from_value(value) {
                Ok(questions) => self.questions = questions,
                Err(error) => eprintln!("sikemux: the core's questions do not read: {error}"),
            },
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
            Update::Read("files.list", value) => self.files.listed(&value, &self.asks),
            Update::Read("projectFiles", value) => {
                if self.project_files.answered(&value) {
                    self.rank(cx);
                }
            }
            Update::Read(key, value) => {
                self.reads.insert(key, value);
            }
            Update::Terminal(session, screenful) => self.terminals.apply(session, screenful, cx),
            Update::Lost(reason) => self.lost = Some(reason),
            Update::Changed { topic, project, paths } => {
                let root = self.files.root.as_ref().map(|root| root.to_string_lossy().into_owned());
                if topic == "git" && (project.is_none() || project == root) {
                    self.files.changed(paths.as_deref(), &self.asks);
                    if self.finder.is_some() {
                        self.project_files.read(root.as_deref(), &self.asks);
                    }
                }
            }
        }
        cx.notify();
    }

    pub fn ask_chat(&self, ask: ChatAsk) {
        let _ = self.asks.send(Ask::Chat(ask));
    }

    /// Holds the chat of the agent the tab in front shows, attaching to it the first time.
    fn open_chat(&mut self, cx: &mut Context<Self>) {
        let Some(AgentRow { id: agent, provider, .. }) = self.agent() else {
            return;
        };
        if self.chats.contains_key(&agent) {
            return;
        }
        let chat = Chat::new(agent.clone(), &provider, cx.weak_entity(), cx);
        self.chats.insert(agent.clone(), chat);
        self.ask_chat(ChatAsk::Attach { agent, since: None });
    }

    /// Makes the composer of the chat in front, which needs the window.
    fn compose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(agent) = self.agent().map(|agent| agent.id) else {
            return;
        };
        let Some(chat) = self.chats.get_mut(&agent).filter(|chat| chat.composer.is_none()) else {
            return;
        };
        let draft = cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 12).submit_on_enter(true).placeholder("Ask about this project"));
        let secret = cx.new(|cx| InputState::new(window, cx).masked(true).placeholder("Type the answer"));
        let sending = {
            let agent = agent.clone();
            cx.subscribe_in(&draft, window, move |bench, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { shift: false, secondary } = event {
                    bench.send(&agent, *secondary, window, cx);
                }
            })
        };
        let keeping = cx.subscribe_in(&secret, window, move |bench, _, event: &InputEvent, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                bench.keep_secret(&agent, window, cx);
            }
        });
        chat.composer = Some(Composing { draft, secret, _heard: [sending, keeping] });
    }

    /// Sends what the person wrote: the answer to a question it answers, else words for
    /// the agent, which steer a running turn, or with ⌘ wait behind it. ⌘ with nothing
    /// written sends the questions' answers, else steers in the messages waiting.
    pub fn send(&mut self, agent: &str, queue: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.chats.get(agent).and_then(|chat| Some(chat.composer.as_ref()?.draft.clone())) else {
            return;
        };
        let text = draft.read(cx).value().trim().to_owned();
        if text.is_empty() {
            if queue {
                self.answer(agent, |chat, open| chat.send_answers(open).or_else(|| chat.steer_waiting(agent)));
                cx.notify();
            }
            return;
        }
        self.answer(agent, |chat, open| chat.sent(agent, text, queue, open));
        draft.update(cx, |draft, cx| draft.set_value("", window, cx));
        cx.notify();
    }

    pub fn keep_secret(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.chats.get(agent).and_then(|chat| Some(chat.composer.as_ref()?.secret.clone())) else {
            return;
        };
        let text = field.read(cx).value().to_string();
        if text.is_empty() {
            return;
        }
        self.answer(agent, |chat, open| chat.keep_secret(text, open));
        field.update(cx, |field, cx| field.set_value("", window, cx));
        cx.notify();
    }

    /// Changes the chat of `agent` with its open questions to hand, and asks the core what that asks.
    pub fn answer(&mut self, agent: &str, change: impl FnOnce(&mut Chat, &[&Question]) -> Option<Ask>) {
        let open: Vec<&Question> = self.questions.iter().filter(|question| question.of(agent, "open")).collect();
        if let Some(ask) = self.chats.get_mut(agent).and_then(|chat| change(chat, &open)) {
            let _ = self.asks.send(ask);
        }
    }

    /// Whether the agent asks before it acts, kept for its next session and changed in the one running.
    pub fn switch_mode(&self, agent: &str, mode: &str) {
        let _ = self.asks.send(Ask::Call { method: "updateAgent", params: json!({ "agentId": agent, "patch": { "permissionMode": mode } }) });
        self.ask_chat(ChatAsk::Mode { agent: agent.to_owned(), mode: mode.to_owned() });
    }

    /// Lets go of the chats whose agents no longer have a tab.
    fn close_chats(&mut self) {
        let Some(view) = &self.view else {
            return;
        };
        let tabs = view.layout.projects.values().flat_map(|layout| &layout.tabs);
        let shown: Vec<&String> = tabs.filter_map(|tab| shows(tab, PaneKind::Agent)).collect();
        let gone: Vec<String> = self.chats.keys().filter(|agent| !shown.contains(agent)).cloned().collect();
        for agent in gone {
            self.chats.remove(&agent);
            self.ask_chat(ChatAsk::Detach { agent });
        }
    }

    /// Asks for the turns before the earliest the chat holds.
    pub fn ask_earlier(&mut self, agent: &str) {
        if let Some(ask) = self.chats.get_mut(agent).and_then(|chat| chat.earlier(agent)) {
            self.ask_chat(ask);
        }
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
        let _ = self.asks.send(Ask::Watch("ports", Watch::new("ports", json!({ "project": path }), &["ports"])));
        let _ = self.asks.send(Ask::Watch("git-overview", Watch::new("git.overview", json!({ "repo": path }), &["git"])));
        self.files.show(path.clone().map(PathBuf::from), &self.asks);
        self.watched_project = path;
    }

    /// Keeps the core checking the CI of the repository in front while its host is signed in.
    fn watch_ci(&mut self) {
        let shown = self.ci_repo();
        if shown == self.watched_ci {
            return;
        }
        let repos: Vec<Value> = shown
            .iter()
            .map(|ci| json!({ "provider": ci.host, "owner": ci.owner, "name": ci.name, "account": ci.account, "host": null }))
            .collect();
        let _ = self.asks.send(Ask::Watch("ci-shown", Watch::new("showCi", json!({ "repos": repos }), &[])));
        if let Some(ci) = &shown {
            let runs = json!({ "owner": ci.owner, "name": ci.name, "account": ci.account, "branch": ci.branch, "perPage": 1 });
            let call = json!({ "plugin": format!("sikemux.{}", ci.host), "method": "runs", "params": runs });
            let _ = self.asks.send(Ask::Watch("ci", Watch::new("plugin", call, &["ci"])));
        }
        self.watched_ci = shown;
    }

    fn ci_repo(&self) -> Option<CiRepo> {
        let project = self.project()?;
        let host = project.code_host.as_ref().filter(|host| host.host == "github")?;
        let plugin = format!("sikemux.{}", host.host);
        let signed_in = self.read("plugins")?.as_array()?.iter().any(|entry| entry["id"] == plugin.as_str() && entry["signedIn"] == true);
        let (owner, name) = host.repo.split_once('/')?;
        if !signed_in {
            return None;
        }
        Some(CiRepo {
            host: host.host.clone(),
            owner: owner.to_owned(),
            name: name.to_owned(),
            account: project.code_hosts.get(&host.host).and_then(|choice| choice.account.clone()),
            branch: project.git.as_ref()?.branch.clone(),
        })
    }

    /// The latest CI run of the branch in front, while the core is checking it.
    pub fn latest_run(&self) -> Option<&Value> {
        self.watched_ci.as_ref()?;
        self.read("ci")?["runs"].as_array()?.first()
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

    /// The clock moves each minute, and a run's time each second while it goes.
    fn until_next_tick(&self) -> Duration {
        let now = chrono::Local::now();
        let into_second = Duration::from_nanos(u64::from(now.timestamp_subsec_nanos()));
        let seconds = if self.person_runs().0.is_some() { 1 } else { 60 - u64::from(now.second()) };
        Duration::from_secs(seconds) - into_second
    }

    /// The run of the project in front's tasks the person started that is going, and the last.
    pub fn person_runs(&self) -> (Option<&Value>, Option<&Value>) {
        let Some(path) = self.watched_project.as_deref() else {
            return (None, None);
        };
        let mine: Vec<&Value> = self
            .read("runs")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|run| run["project"] == path && run["agentId"].is_null())
            .collect();
        let active = |run: &Value| matches!(run["status"].as_str(), Some("awaiting-trust" | "starting" | "running" | "stopping"));
        (mine.iter().rev().find(|run| active(run)).copied(), mine.last().copied())
    }

    pub fn start_task(&self, task: &str) {
        if let Some(project) = &self.watched_project {
            self.call("taskStart", json!({ "project": project, "taskId": task }));
        }
    }

    pub fn stop_run(&self, execution: &str) {
        self.call("taskStop", json!({ "executionId": execution }));
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

    pub fn launchers(&self) -> &[LauncherInfo] {
        self.view.as_ref().map_or(&[], |view| view.workspace.launchers.as_slice())
    }

    /// What a new chat started from the rail starts with.
    pub fn launcher(&self) -> Option<&LauncherInfo> {
        let launchers = self.launchers();
        let chosen = |id: &str| launchers.iter().find(|launcher| launcher.id == id);
        self.launcher_choice
            .as_deref()
            .and_then(chosen)
            .or_else(|| self.project()?.launcher.as_deref().and_then(chosen))
            .or(launchers.first())
    }

    /// The most of any window of its plan the launcher's account has used.
    pub fn usage_peak(&self, launcher: &str) -> Option<f64> {
        let account = self.read("usage")?.as_array()?.iter().find(|account| account["launcher"] == launcher)?;
        let windows = account["usage"]["windows"].as_array()?;
        windows.iter().filter_map(|window| window["usedPercent"].as_f64()).map(|used| used.clamp(0., 100.)).reduce(f64::max)
    }

    /// The file the tab in front shows, when it is an editor.
    pub fn front_file(&self) -> Option<PathBuf> {
        let (kind, target) = crate::tabs::first_view(&self.tab()?.pane)?;
        (kind == PaneKind::Files).then(|| PathBuf::from(target))
    }

    /// Brings the file's editor tab to the front, opening one when none shows it.
    pub fn open_file(&self, path: &Path) {
        if let Some(project) = self.project() {
            self.call("openTab", json!({ "project": project.id, "kind": "files", "target": path.to_string_lossy() }));
        }
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
                    face: agent.face.clone(),
                    group: chat.map(|chat| chat.group).or_else(|| session.and_then(|session| session.group)).unwrap_or_default(),
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
                .filter(|recent| self.all_harnesses || self.launcher().is_some_and(|launcher| launcher.provider == recent.provider))
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.compose(window, cx);
        let t = Tokens::get(cx);
        div()
            .id("shell")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .on_action(cx.listener(|bench, _: &OpenFile, window, cx| {
                bench.toggle_finder(window, cx);
                cx.notify();
            }))
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
                    .child(home::stage(self, window, cx))
                    .child(agents::tab_rail(self, cx)),
            )
            .children(crate::finder::overlay(self, window, cx))
    }
}
