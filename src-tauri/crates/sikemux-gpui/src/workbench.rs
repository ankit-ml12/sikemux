use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use chrono::Timelike;

use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, AnyWindowHandle, App, AppContext, Context, DragMoveEvent, Entity,
    EventEmitter, FocusHandle, Focusable, Image, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, StyleRefinement, Styled, Subscription, Task,
    WeakEntity, Window,
};
use serde_json::{json, Value};
use sikemux_wire::protocol::{
    AgentFace, AgentGroup, AgentStatus, DeviceView, LauncherInfo, MidTurnSend, Pane, PaneKind,
    Preferences, ProjectInfo, ProjectLayout, RecentInfo, RemoteStatus, SessionId, SessionInfo,
    SpaceInfo, Tab,
};
use tokio::sync::mpsc;

use crate::battery::Battery;
use crate::chat::{Chat, Composing, Question, Sent};
use crate::codehost::{Area, HostArea, OpenArea};
use crate::core::{utc_offset_minutes, Ask, ChatAsk, ChatNews, Link, Update, Watch};
use crate::desk::{NewPage, NewSimulator, ToggleDesk};
use crate::editor::OpenAt;
use crate::files::FileTree;
use crate::finder::{Finder, OpenFile, ProjectFiles};
use crate::git::{FocusGit, GitPane};
use crate::search::{FocusSearch, FocusSearchWindow, SearchPane};
use crate::terminal::NewTerminal;
use crate::theme::{self, Tokens};
use crate::{agents, home, rail, tabs, terminal};

/// How long a peeking rail stays after the pointer leaves it.
const PEEK_CLOSE: Duration = Duration::from_millis(180);
/// How close to the window's edge the pointer brings out a closed rail.
const PEEK_REACH: f32 = 28.;

/// The one store: what the core last said, and what only this window decides.
pub struct Workbench {
    pub version: String,
    pub view: Option<DeviceView>,
    /// The latest answer to each watched call, by key.
    reads: HashMap<&'static str, Value>,
    pub lost: Option<String>,
    /// Opening has taken long enough to say what the window waits for.
    pub slow_start: bool,
    /// Dictation, heard into the chat or terminal holding the keyboard.
    pub voice: Entity<crate::voice::Dictation>,
    _voice_heard: [Subscription; 2],
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
    /// Where remote access stands, once the core has said.
    pub remote: Option<RemoteStatus>,
    pub accounts: crate::accounts::Accounts,
    /// The number the Dock icon's badge shows.
    pub badge: usize,
    /// A tab being dragged along its strip, and where it would land.
    pub reorder: Option<crate::drag::Reorder>,
    pub all_projects: bool,
    /// The task each project's Run button starts, when the person chose one, by project path.
    pub picked_tasks: HashMap<String, String>,
    /// The space whose projects the rail shows, else every project.
    pub space: Option<String>,
    /// Which rails stay open; a closed one comes out over the stage while the pointer is at its edge.
    pub side_rail_open: bool,
    pub tab_rail_open: bool,
    side_rail_width: f32,
    tab_rail_width: f32,
    /// The handles beside the rails, which arrow keys move once focused.
    resizers: [FocusHandle; 2],
    pub peeking: Option<Edge>,
    unpeek: Option<Task<()>>,
    run_clock: Option<Task<()>>,
    /// Which of its pages the tab rail shows.
    pub rail_page: RailPage,
    pub files: FileTree,
    pub finder: Option<Finder>,
    pub palette: Option<crate::palette::Palette>,
    pub switcher: Option<crate::switcher::Switcher>,
    /// The command deck's keys, the last used first.
    pub recent_commands: Vec<SharedString>,
    pub project_files: ProjectFiles,
    /// The agent the rail was switched to, else the one the project last started.
    pub launcher_choice: Option<String>,
    /// The rail lists the recent chats of every agent, not one's.
    pub all_harnesses: bool,
    /// Which of the calendar's metrics it shows, an index into `home::METRICS`.
    pub metric: usize,
    /// The calendar day the pointer is over, counted from the Unix epoch.
    pub metric_day: Option<i64>,
    pub filter: Option<Entity<InputState>>,
    pub filter_text: String,
    pub asks: mpsc::UnboundedSender<Ask>,
    /// The chats of the agents with a tab open, by agent id.
    pub chats: HashMap<String, Chat>,
    /// Text handed to agents whose composer is not made yet, put in once it is.
    deliveries: HashMap<String, String>,
    /// What agents asked the person, and the assumptions they went on.
    pub questions: Vec<Question>,
    pub terminals: terminal::Terminals,
    pub editors: crate::editor::Editors,
    /// The Git pane of the project whose Git tab was last in front.
    pub git: Option<Entity<GitPane>>,
    /// What the Git tab shows: the local workbench, or one of the code host's areas.
    pub area: Area,
    /// The code host's areas of the project whose Git tab last showed one.
    pub host: Option<Entity<HostArea>>,
    /// The Search pane of the project whose Search tab was last in front.
    pub search: Option<Entity<SearchPane>>,
    /// Settings, while they take the stage's place.
    pub settings: Option<crate::settings::Settings>,
    /// The agent whose title is being edited in place in its header.
    pub retitling: Option<Retitling>,
    /// The question the shell is putting to the person, over the window.
    asking: Option<Asking>,
    /// The stage's tabs while they move between one another.
    pub(crate) track: crate::track::Slide<String>,
    /// The tab rail's pages while they move between one another.
    pub(crate) rail_slide: crate::track::Slide<RailPage>,
    /// The task the person is writing in the Add a task dialog, while it is open.
    pub adding_task: Option<crate::add_task::AddTask>,
    /// The core's trust questions already put to the person, by id.
    trust_asked: HashSet<String>,
    /// Holds the keyboard while nothing in the window does, so the window's shortcuts reach it.
    pub focus: FocusHandle,
    /// The window the store draws, for what is said in it from outside a draw.
    window: AnyWindowHandle,
    pub parts: Parts,
    pub panes: crate::panes::Panes,
    pub desks: crate::desk::Desks,
    pub sims: crate::sim_pane::Simulators,
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

/// Whose tabs a strip holds: a project's, or those of an SSH host shown instead of one.
#[derive(Clone, PartialEq, Eq)]
pub enum Strip {
    Project(String),
    Host(String),
}

impl Strip {
    /// `params` naming this strip, as the core's tab calls take it.
    pub(crate) fn with(&self, mut params: Value) -> Value {
        let (key, id) = match self {
            Strip::Project(id) => ("project", id),
            Strip::Host(alias) => ("host", alias),
        };
        params[key] = json!(id);
        params
    }
}

/// A side of the window a rail lives on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// How wide a rail can be dragged, and how wide it starts.
#[derive(Clone, Copy)]
pub struct RailWidth {
    pub min: f32,
    pub max: f32,
    pub initial: f32,
}

type Confirmed = Rc<dyn Fn(&mut Workbench, bool, &mut Window, &mut Context<Workbench>)>;
type Typed = Rc<dyn Fn(&mut Workbench, String, &mut Window, &mut Context<Workbench>)>;

/// A question the shell puts to the person over the window, and what its answer does.
enum Asking {
    Confirm {
        title: SharedString,
        body: SharedString,
        yes: SharedString,
        destructive: bool,
        then: Confirmed,
    },
    Prompt {
        title: SharedString,
        initial: String,
        field: Option<Entity<InputState>>,
        then: Typed,
    },
}

/// An agent's title while it is edited in place, in a field holding it.
pub struct Retitling {
    pub agent: String,
    pub input: Entity<InputState>,
    title: String,
    _events: Subscription,
}

/// A rail's handle while it is dragged.
struct DraggedRail(Edge);

impl Render for DraggedRail {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui_kit::Empty
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RailPage {
    Agents,
    Terminals,
    Files,
}

impl RailPage {
    /// The pages in the order they sit, left to right.
    pub const ALL: [RailPage; 3] = [RailPage::Agents, RailPage::Terminals, RailPage::Files];
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
    /// When it last worked, in Unix milliseconds; 0 when the core does not know.
    pub active_at: u64,
}

impl Workbench {
    pub fn new(link: Link, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Link { mut updates, asks } = link;
        cx.defer_in(window, |bench, _, cx| crate::accounts::read(bench, cx));
        crate::faces::install(cx);
        crate::file_icons::install(cx);
        crate::syntax::install(cx);
        crate::finder::install(cx);
        crate::palette::install(cx);
        crate::keymap::install(cx);
        crate::chat::install(cx);
        crate::terminal::install(cx);
        crate::editor::install(cx);
        crate::search::install(cx);
        crate::settings::install(cx);
        crate::project_switcher::install(cx);
        crate::model_picker::install(cx);
        crate::ui::grain::animate(window, cx);
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
                if this
                    .update(cx, |this, cx| {
                        batch.into_iter().for_each(|update| this.apply(update, cx))
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let _ = asks.send(Ask::Watch(
            "theme",
            Watch::new("theme", json!({}), &["preferences"]),
        ));
        let _ = asks.send(Ask::Watch(
            "plugins",
            Watch::new("plugins", json!({}), &["plugins"]),
        ));
        let _ = asks.send(Ask::Watch(
            "hosts",
            Watch::new("hosts", json!({}), &["hosts"]),
        ));
        let _ = asks.send(Ask::Watch(
            "update",
            Watch::new("updateStatus", json!({}), &["update"]),
        ));
        let _ = asks.send(Ask::Watch(
            "runs",
            Watch::new("taskRuns", json!({}), &["runs"]),
        ));
        let _ = asks.send(Ask::Watch(
            "trust",
            Watch::new("trustAsks", json!({}), &["trust"]),
        ));
        let _ = asks.send(Ask::Watch(
            "questions",
            Watch::new("questions", json!({}), &["questions"]),
        ));
        let _ = asks.send(Ask::Watch(
            "shares",
            Watch::new("shares", json!({}), &["shares"]),
        ));
        let _ = asks.send(Ask::Watch(
            "catalog",
            Watch::new("agentCatalog", json!({}), &[]),
        ));
        let _ = asks.send(Ask::Watch(
            "preferences",
            Watch::new("preferences", json!({}), &["preferences"]),
        ));
        let _ = asks.send(Ask::Follow("git"));
        let _ = asks.send(Ask::Follow("files"));
        let _ = asks.send(Ask::Follow("lsp"));
        let _ = asks.send(Ask::Follow("agentSignIn"));
        let _ = asks.send(Ask::Follow("agentAccounts"));
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
        let voice = cx.new(|_| crate::voice::Dictation::default());
        let voice_heard = [
            cx.subscribe_in(&voice, window, |bench, _, heard, window, cx| {
                bench.dictated(heard, window, cx)
            }),
            cx.observe_window_activation(window, |bench, window, cx| {
                if !window.is_window_active() {
                    bench.voice.update(cx, |voice, cx| voice.abandon_hold(cx));
                }
            }),
        ];
        let bench = Self {
            version: String::new(),
            view: None,
            reads: HashMap::new(),
            lost: None,
            slow_start: false,
            voice,
            _voice_heard: voice_heard,
            watched_project: None,
            watched_ci: None,
            battery: crate::battery::read(),
            picture: None,
            picture_hash: None,
            signing_in: false,
            remote: None,
            accounts: Default::default(),
            badge: 0,
            reorder: None,
            all_projects: false,
            picked_tasks: HashMap::new(),
            space: None,
            side_rail_open: true,
            tab_rail_open: true,
            side_rail_width: rail::SIDE_RAIL.initial,
            tab_rail_width: agents::TAB_RAIL.initial,
            resizers: [cx.focus_handle(), cx.focus_handle()],
            retitling: None,
            asking: None,
            track: crate::track::Slide::default(),
            rail_slide: crate::track::Slide::default(),
            adding_task: None,
            trust_asked: HashSet::new(),
            peeking: None,
            unpeek: None,
            run_clock: None,
            rail_page: RailPage::Agents,
            files: FileTree::default(),
            finder: None,
            palette: None,
            switcher: None,
            recent_commands: Vec::new(),
            project_files: ProjectFiles::default(),
            launcher_choice: None,
            all_harnesses: false,
            metric: 0,
            metric_day: None,
            filter: None,
            filter_text: String::new(),
            asks,
            chats: HashMap::new(),
            deliveries: HashMap::new(),
            questions: Vec::new(),
            terminals: terminal::Terminals::default(),
            editors: crate::editor::Editors::default(),
            git: None,
            area: Area::Local,
            host: None,
            search: None,
            settings: None,
            focus: cx.focus_handle(),
            window: window.window_handle(),
            parts: Parts::new(cx),
            panes: crate::panes::Panes::default(),
            desks: crate::desk::Desks::new(window, cx),
            sims: crate::sim_pane::Simulators::new(cx),
            _filtering: None,
        };
        bench.focus.focus(window, cx);
        bench.read_usage();
        bench.wait_patiently(cx);
        bench
    }

    /// Reads every account's plan limits again, as they are whenever usage changes.
    pub fn read_usage(&self) {
        let _ = self.asks.send(Ask::Watch(
            "usage",
            Watch::new("agentUsages", json!({}), &["usage"]),
        ));
    }

    fn apply(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::Connected(version) => {
                self.version = version;
                self.lost = None;
                for (agent, chat) in &self.chats {
                    self.ask_chat(ChatAsk::Attach {
                        agent: agent.clone(),
                        since: chat.mark.clone(),
                    });
                }
                self.terminals.reattach(&self.asks);
                self.files.relist_shown(&self.asks);
            }
            Update::View(view) => {
                self.view = Some(*view);
                let sessions = &self
                    .view
                    .as_ref()
                    .map_or(&[][..], |view| view.sessions.as_slice());
                self.terminals.keep(
                    |id| sessions.iter().any(|session| session.id == id),
                    &self.asks,
                );
                crate::editor::keep(self);
                crate::notifications::badge(self);
                self.watch_project();
                self.watch_picture();
                self.watch_ci();
                self.close_chats();
                self.open_chat(cx);
                self.front_moved(cx);
            }
            Update::Chat(agent, news) => {
                let Some(chat) = self.chats.get_mut(&agent) else {
                    return;
                };
                match news {
                    ChatNews::Attached(attachment) => chat.attached(attachment, cx),
                    ChatNews::Changes { seq, changes } => {
                        let gap = chat.changed(seq, changes, cx);
                        let (since, wanted) = (chat.mark.clone(), chat.wanted());
                        if gap {
                            self.ask_chat(ChatAsk::Attach {
                                agent: agent.clone(),
                                since,
                            });
                        }
                        for tool in wanted {
                            self.ask_chat(ChatAsk::Detail {
                                agent: agent.clone(),
                                tool,
                            });
                        }
                    }
                    ChatNews::Page(page) => chat.page(page, cx),
                    ChatNews::Detail { tool, detail } => chat.detail(tool, detail, cx),
                    ChatNews::Refused { error, text } => {
                        if let Some(composing) = chat.composer.as_mut() {
                            composing.refusal = Some(error.into());
                            let held = composing
                                .sent
                                .take()
                                .filter(|sent| sent.content.text().trim() == text);
                            composing.returned = Some(held.unwrap_or_else(|| Sent {
                                content: text.into(),
                                attachments: Vec::new(),
                                earlier: Vec::new(),
                            }));
                        }
                        return self.parts.redraw(&self.parts.stage, cx);
                    }
                }
                let restage = self
                    .chats
                    .get_mut(&agent)
                    .is_some_and(Chat::take_head_moved);
                if restage {
                    self.parts.redraw(&self.parts.stage, cx);
                }
                return;
            }
            Update::Read("plugins", value) => {
                self.reads.insert("plugins", value);
                self.watch_ci();
            }
            Update::Read("update", value) => {
                let ready = |value: &Value| value["state"] == "ready";
                let already = self.read("update").is_some_and(ready);
                let now = ready(&value);
                self.reads.insert("update", value);
                if now && !already {
                    // The core put the update in place of this app; opening the app again starts it.
                    cx.restart();
                }
            }
            Update::Read("questions", value) => match serde_json::from_value(value) {
                Ok(questions) => self.questions = questions,
                Err(error) => eprintln!("sikemux: the core's questions do not read: {error}"),
            },
            Update::Read("theme", value) => {
                if let Ok(palette) = serde_json::from_value(value) {
                    theme::apply(&palette, cx);
                    crate::syntax::restyle(cx);
                    cx.refresh_windows();
                }
            }
            Update::Read("picture", value) => self.picture = decode_picture(&value),
            Update::Read("sign-in", value) => match value["url"].as_str() {
                Some(url) => {
                    cx.open_url(url);
                    let _ = self.asks.send(Ask::Answer {
                        key: "sign-in-finished",
                        method: "accountSignInFinish",
                        params: json!({}),
                    });
                }
                None => self.signing_in = false,
            },
            Update::Read("sign-in-finished", _) => self.signing_in = false,
            Update::Failed(error) => {
                return self.notify(Notification::error(error).autohide(false), cx);
            }
            Update::Refused(key, error) => {
                eprintln!("sikemux: the core refused {key}: {error}");
                if key.starts_with("sign-in") {
                    self.signing_in = false;
                }
                if key == "files.list" {
                    self.files.relist_shown(&self.asks);
                }
                if key == crate::settings::ACTIVITY {
                    self.reads.insert(key, Value::Null);
                    self.parts.redraw(&self.parts.stage, cx);
                }
            }
            Update::Read("files.list", value) => {
                self.files.listed(&value, &self.asks);
                return self.parts.redraw(&self.parts.tabs, cx);
            }
            Update::Read("git-overview", value) => {
                self.files.mark(&value);
                if let Some(pane) = &self.git {
                    pane.update(cx, |pane, cx| pane.overview(&value, cx));
                }
                self.reads.insert("git-overview", value);
                self.parts.redraw(&self.parts.top, cx);
                return self.parts.redraw(&self.parts.tabs, cx);
            }
            Update::Read("projectFiles", value) => {
                if self.project_files.answered(&value) {
                    self.rank(cx);
                    self.files_listed(cx);
                }
                return;
            }
            Update::Read("preferences", value) => {
                if let Ok(preferences) = serde_json::from_value::<Preferences>(value.clone()) {
                    crate::keymap::bind(&preferences.keybinding_overrides, cx);
                    let on = preferences.voice_dictation;
                    self.voice.update(cx, |voice, cx| voice.follow(on, cx));
                    crate::ui::grain::set(&preferences, cx);
                }
                self.reads.insert("preferences", value);
                crate::notifications::introduce(self, cx);
            }
            Update::Read("drafts", value) => {
                if let Some(pane) = &self.git {
                    pane.update(cx, |pane, cx| pane.drafts(&value, cx));
                }
                self.drafts(&value, cx);
                self.reads.insert("drafts", value);
            }
            Update::Read("catalog", value) => {
                if let Some(pane) = &self.git {
                    pane.update(cx, |pane, cx| pane.catalog(&value, cx));
                }
                self.reads.insert("catalog", value);
            }
            Update::Read("trust", value) => {
                self.reads.insert("trust", value);
                self.ask_trust(cx);
            }
            Update::Read(key @ ("runs" | "ports"), value) => {
                self.reads.insert(key, value);
                self.time_run(cx);
            }
            Update::Read(key, value) if crate::codehost::watches(key) => {
                if let Some(host) = &self.host {
                    host.update(cx, |host, cx| host.read(key, &value, cx));
                }
            }
            Update::Read(key, value) => {
                self.reads.insert(key, value);
                if matches!(key, "pickerFolders" | "sshHosts") {
                    self.picker_answered(cx);
                }
            }
            Update::Terminal(session, screenful) => {
                self.terminals.apply(session, screenful, cx);
                return self.parts.redraw(&self.parts.stage, cx);
            }
            Update::Lost(reason) => self.lost = Some(reason),
            Update::ShutDown => return cx.quit(),
            Update::Notice { title, body, agent } => {
                return crate::notifications::notice(self, title, body, agent, cx);
            }
            Update::Remote(status) => {
                self.remote = Some(*status);
                crate::settings::remote_moved(self, cx);
                return self.parts.redraw(&self.parts.stage, cx);
            }
            Update::WindowCall(call_id, call) => self.answer_window_call(call_id, call),
            Update::Changed {
                topic,
                project,
                paths,
            } => {
                let root = self
                    .files
                    .root
                    .as_ref()
                    .map(|root| root.to_string_lossy().into_owned());
                if topic == "git" && (project.is_none() || project == root) {
                    self.files.changed(paths.as_deref(), &self.asks);
                    if self.finder.is_some() {
                        self.project_files.read(root.as_deref(), &self.asks);
                    }
                }
                if topic == "files" {
                    crate::editor::changed(self, paths.as_deref().unwrap_or_default(), cx);
                    if let Some(search) = &self.search {
                        search.update(cx, |search, cx| search.changed(cx));
                    }
                }
                if topic == "lsp" {
                    crate::editor::diagnostics_changed(
                        self,
                        paths.as_deref().unwrap_or_default(),
                        cx,
                    );
                }
                if topic == "agentSignIn" {
                    crate::accounts::pages_moved(self, cx);
                }
                if topic == "agentAccounts" {
                    crate::accounts::book_moved(self, cx);
                }
                if topic == "git" {
                    crate::editor::git_changed(self, paths.as_deref(), cx);
                    cx.emit(crate::git::Changed { project, paths });
                }
                return;
            }
        }
        cx.notify();
    }

    pub fn ask_chat(&self, ask: ChatAsk) {
        let _ = self.asks.send(Ask::Chat(ask));
    }

    /// Holds the chat of the agent the tab in front shows, attaching to it the first time.
    fn open_chat(&mut self, cx: &mut Context<Self>) {
        let Some(AgentRow {
            id: agent,
            provider,
            ..
        }) = self.agent()
        else {
            return;
        };
        if self.chats.contains_key(&agent) {
            return;
        }
        let chat = Chat::new(
            agent.clone(),
            &provider,
            cx.weak_entity(),
            self.asks.clone(),
            cx,
        );
        self.chats.insert(agent.clone(), chat);
        self.ask_chat(ChatAsk::Attach { agent, since: None });
    }

    /// Makes the composer of the chat in front, which needs the window.
    fn compose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(agent) = self.agent().map(|agent| agent.id) else {
            return;
        };
        let Some(chat) = self
            .chats
            .get_mut(&agent)
            .filter(|chat| chat.composer.is_none())
        else {
            return;
        };
        let draft = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 12)
                .submit_on_enter(true)
        });
        let secret = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Type the answer")
        });
        let sending = {
            let agent = agent.clone();
            let was_empty = Cell::new(true);
            cx.subscribe_in(
                &draft,
                window,
                move |bench, draft, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter {
                        shift: false,
                        secondary,
                    } => bench.send(&agent, *secondary, window, cx),
                    InputEvent::Change => {
                        let empty = draft.read(cx).value().is_empty();
                        if was_empty.replace(empty) != empty {
                            cx.notify();
                        }
                        bench.offer(&agent, cx);
                        bench.typed_draft(&agent, cx);
                    }
                    _ => {}
                },
            )
        };
        let keeping = {
            let agent = agent.clone();
            cx.subscribe_in(
                &secret,
                window,
                move |bench, _, event: &InputEvent, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        bench.keep_secret(&agent, window, cx);
                    }
                },
            )
        };
        let mic = {
            let (voice, toggle) = (
                self.voice.clone(),
                crate::chat::dictate(&agent, cx.weak_entity()),
            );
            let agent = agent.clone();
            cx.new(|cx| crate::voice::MicButton::new(voice, agent, toggle, cx))
        };
        chat.composer = Some(Composing {
            draft,
            secret,
            mic,
            hint: SharedString::default(),
            attachments: Vec::new(),
            menu: None,
            earlier: Vec::new(),
            refusal: None,
            sent: None,
            returned: None,
            synced: json!({ "text": "" }),
            arrived: None,
            _keeping: Task::ready(()),
            _heard: [sending, keeping],
        });
        if let (Some(composing), Some(drafts)) = (chat.composer.as_mut(), self.reads.get("drafts"))
        {
            composing.start_from(drafts, &agent, window, cx);
        }
        self.watch_drafts();
        if let Some(text) = self.deliveries.remove(&agent) {
            self.deliver(&agent, text, window, cx);
        }
    }

    /// Hands `agent` text from elsewhere in the app, such as a terminal's selection: it goes
    /// at the end of the draft after a blank line, and the agent's tab comes to the front
    /// with the draft focused.
    pub fn deliver(
        &mut self,
        agent: &str,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = self.view.as_ref().and_then(|view| {
            let saved = view
                .workspace
                .agents
                .iter()
                .find(|saved| saved.id == agent)?;
            Some(saved.project.clone())
        });
        if let Some(project) = project {
            self.call(
                "openTab",
                json!({ "project": project, "kind": "agent", "target": agent }),
            );
        }
        let Some(draft) = self
            .chats
            .get(agent)
            .and_then(|chat| Some(chat.composer.as_ref()?.draft.clone()))
        else {
            self.deliveries
                .entry(agent.to_owned())
                .and_modify(|held| *held = format!("{held}\n\n{text}"))
                .or_insert(text);
            return;
        };
        append(&draft, &text, window, cx);
        draft.read(cx).focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Stops the turn running in `agent`'s chat. The messages waiting behind it come back
    /// to the draft rather than going out after it.
    pub fn stop(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(chat) = self.chats.get(agent) else {
            return;
        };
        let waiting = chat.waiting().join("\n\n");
        if let Some(composing) = chat.composer.as_ref().filter(|_| !waiting.is_empty()) {
            append(&composing.draft, &waiting, window, cx);
        }
        self.ask_chat(ChatAsk::Cancel {
            agent: agent.to_owned(),
        });
        cx.notify();
    }

    /// Keeps what the empty draft of the chat in front says in step with how the chat stands.
    fn hint(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(agent) = self.agent().map(|agent| agent.id) else {
            return;
        };
        let resumes = self.view.as_ref().is_some_and(|view| {
            view.workspace
                .agents
                .iter()
                .any(|saved| saved.id == agent && saved.resume_id.is_some())
        });
        let Some(chat) = self.chats.get_mut(&agent) else {
            return;
        };
        if let Some(composing) = chat.composer.as_mut() {
            let empty = composing.draft.read(cx).value().trim().is_empty();
            if let Some(returned) = composing.returned.take().filter(|_| empty) {
                composing.draft.update(cx, |draft, cx| {
                    draft.set_value(returned.content, window, cx)
                });
                composing.attachments = returned.attachments;
                composing.earlier = returned.earlier;
            }
            composing.take_arrived(window, cx);
        }
        let hint = SharedString::from(chat.hint(&agent, &self.questions, resumes));
        let Some(composing) = chat
            .composer
            .as_mut()
            .filter(|composing| composing.hint != hint)
        else {
            return;
        };
        composing.hint = hint.clone();
        composing
            .draft
            .update(cx, |draft, cx| draft.set_placeholder(hint, window, cx));
    }

    /// Sends what the person wrote: the answer to a question it answers, else words for
    /// the agent, which join a running turn or wait behind it as the person prefers,
    /// and ⌘ does the other. ⌘ with nothing written sends the questions' answers, else
    /// steers in the messages waiting.
    pub fn send(
        &mut self,
        agent: &str,
        primary: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let waits = self
            .voice
            .update(cx, |voice, cx| voice.send_after_words(agent, primary, cx));
        if waits {
            return;
        }
        let Some(composing) = self
            .chats
            .get(agent)
            .and_then(|chat| chat.composer.as_ref())
        else {
            return;
        };
        let draft = composing.draft.clone();
        let outgoing = composing.outgoing(cx);
        let text = draft.read(cx).value().trim().to_owned();
        if text.is_empty() && outgoing.0.is_empty() {
            if primary {
                self.answer(agent, |chat, open| {
                    chat.send_answers(open)
                        .or_else(|| chat.steer_waiting(agent))
                });
                cx.notify();
            }
            return;
        }
        let sent = Sent {
            content: draft.read(cx).content(),
            attachments: composing.attachments.clone(),
            earlier: composing.earlier.clone(),
        };
        let steers = self.preferences().mid_turn_send == MidTurnSend::Steer;
        self.answer(agent, |chat, open| {
            if let Some(composing) = chat.composer.as_mut() {
                composing.sent = Some(sent);
            }
            chat.sent(agent, text, outgoing, primary == steers, open)
        });
        draft.update(cx, |draft, cx| draft.set_value("", window, cx));
        cx.notify();
    }

    pub fn keep_secret(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self
            .chats
            .get(agent)
            .and_then(|chat| Some(chat.composer.as_ref()?.secret.clone()))
        else {
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
    pub fn answer(
        &mut self,
        agent: &str,
        change: impl FnOnce(&mut Chat, &[&Question]) -> Option<Ask>,
    ) {
        let open: Vec<&Question> = self
            .questions
            .iter()
            .filter(|question| question.of(agent, "open"))
            .collect();
        if let Some(ask) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| change(chat, &open))
        {
            let _ = self.asks.send(ask);
        }
    }

    /// Whether the agent asks before it acts, kept for its next session and changed in the one running.
    pub fn switch_mode(&self, agent: &str, mode: &str) {
        let _ = self.asks.send(Ask::Call {
            method: "updateAgent",
            params: json!({ "agentId": agent, "patch": { "permissionMode": mode } }),
        });
        self.ask_chat(ChatAsk::Mode {
            agent: agent.to_owned(),
            mode: mode.to_owned(),
        });
    }

    /// Lets go of the chats whose agents no longer have a tab.
    fn close_chats(&mut self) {
        let Some(view) = &self.view else {
            return;
        };
        let tabs = view
            .layout
            .projects
            .values()
            .flat_map(|layout| &layout.tabs);
        let shown: Vec<&String> = tabs.filter_map(|tab| shows(tab, PaneKind::Agent)).collect();
        let gone: Vec<String> = self
            .chats
            .keys()
            .filter(|agent| !shown.contains(agent))
            .cloned()
            .collect();
        let closed = !gone.is_empty();
        for agent in gone {
            self.chats.remove(&agent);
            self.ask_chat(ChatAsk::Detach { agent });
        }
        if closed {
            self.watch_drafts();
        }
    }

    /// Asks for the turns before the earliest the chat holds.
    pub fn ask_earlier(&mut self, agent: &str) {
        if let Some(ask) = self
            .chats
            .get_mut(agent)
            .and_then(|chat| chat.earlier(agent))
        {
            self.ask_chat(ask);
        }
    }

    /// Points the watches that follow the project in front at it.
    fn watch_project(&mut self) {
        let path = self
            .project()
            .map(|project| project.path.to_string_lossy().into_owned());
        if path == self.watched_project {
            return;
        }
        let activity = json!({ "utcOffsetMinutes": utc_offset_minutes(), "project": path });
        let _ = self.asks.send(Ask::Watch(
            "activity",
            Watch::new("activitySummary", activity, &["activity"]),
        ));
        let _ = self.asks.send(Ask::Watch(
            "tasks",
            Watch::new("tasks", json!({ "project": path }), &["tasks", "runs"]),
        ));
        let _ = self.asks.send(Ask::Watch(
            "ports",
            Watch::new("ports", json!({ "project": path }), &["ports"]),
        ));
        let _ = self.asks.send(Ask::Watch(
            "git-overview",
            Watch::new("git.overview", json!({ "repo": path }), &["git"]),
        ));
        self.files.show(path.clone().map(PathBuf::from), &self.asks);
        self.watched_project = path;
        self.watch_drafts();
    }

    /// Keeps the core answering the drafts on show: the commit message of the project in
    /// front, and every open chat's.
    fn watch_drafts(&self) {
        let git = self
            .watched_project
            .iter()
            .map(|path| format!("git:{path}"));
        let chats = self
            .chats
            .iter()
            .filter(|(_, chat)| chat.composer.is_some())
            .map(|(agent, _)| crate::chat::draft_key(agent));
        let keys: Vec<String> = git.chain(chats).collect();
        let _ = self.asks.send(Ask::Watch(
            "drafts",
            Watch::new("drafts", json!({ "keys": keys }), &["drafts"]),
        ));
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
        let _ = self.asks.send(Ask::Watch(
            "ci-shown",
            Watch::new("showCi", json!({ "repos": repos }), &[]),
        ));
        if let Some(ci) = &shown {
            let runs = json!({ "owner": ci.owner, "name": ci.name, "account": ci.account, "branch": ci.branch, "perPage": 1 });
            let call = json!({ "plugin": format!("sikemux.{}", ci.host), "method": "runs", "params": runs });
            let _ = self
                .asks
                .send(Ask::Watch("ci", Watch::new("plugin", call, &["ci"])));
        }
        self.watched_ci = shown;
    }

    fn ci_repo(&self) -> Option<CiRepo> {
        let project = self.project()?;
        let host = project
            .code_host
            .as_ref()
            .filter(|host| host.host == "github")?;
        let plugin = format!("sikemux.{}", host.host);
        let signed_in = self
            .read("plugins")?
            .as_array()?
            .iter()
            .any(|entry| entry["id"] == plugin.as_str() && entry["signedIn"] == true);
        let (owner, name) = host.repo.split_once('/')?;
        if !signed_in {
            return None;
        }
        Some(CiRepo {
            host: host.host.clone(),
            owner: owner.to_owned(),
            name: name.to_owned(),
            account: project
                .code_hosts
                .get(&host.host)
                .and_then(|choice| choice.account.clone()),
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
        let hash = self
            .view
            .as_ref()
            .and_then(|view| view.account.as_ref()?.picture_hash.clone());
        if hash == self.picture_hash {
            return;
        }
        self.picture = None;
        if let Some(hash) = &hash {
            let _ = self.asks.send(Ask::Watch(
                "picture",
                Watch::new("accountPicture", json!({ "hash": hash }), &[]),
            ));
        }
        self.picture_hash = hash;
    }

    /// The clock moves each minute, and a run's time each second while it goes.
    fn until_next_tick(&self) -> Duration {
        let now = chrono::Local::now();
        let into_second = Duration::from_nanos(u64::from(now.timestamp_subsec_nanos()));
        let counting = self.person_runs().0.is_some() || self.chats.values().any(Chat::running);
        let seconds = if counting {
            1
        } else {
            60 - u64::from(now.second())
        };
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
        let active = |run: &Value| {
            matches!(
                run["status"].as_str(),
                Some("awaiting-trust" | "starting" | "running" | "stopping")
            )
        };
        (
            mine.iter().rev().find(|run| active(run)).copied(),
            mine.last().copied(),
        )
    }

    /// Counts the seconds on the Run chip while the person's task runs with no port to show.
    fn time_run(&mut self, cx: &mut Context<Self>) {
        let listening = |run: &Value| {
            self.read("ports")
                .and_then(Value::as_array)
                .is_some_and(|ports| {
                    ports
                        .iter()
                        .any(|port| port["owner"]["executionId"] == run["executionId"])
                })
        };
        let counting = self.person_runs().0.is_some_and(|run| !listening(run));
        if !counting {
            self.run_clock = None;
        } else if self.run_clock.is_none() {
            self.run_clock = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let tick = this.update(cx, |bench, cx| bench.parts.redraw(&bench.parts.top, cx));
                if tick.is_err() {
                    break;
                }
            }));
        }
    }

    pub fn start_task(&self, task: &str) {
        if let Some(project) = &self.watched_project {
            self.call("taskStart", json!({ "project": project, "taskId": task }));
        }
    }

    /// Makes `task` the one the project's Run button starts.
    pub fn pick_task(&mut self, task: &str, cx: &mut Context<Self>) {
        if let Some(project) = self.watched_project.clone() {
            self.picked_tasks.insert(project, task.to_owned());
            cx.notify();
        }
    }

    /// Opens the Add a task dialog for the project whose tasks the Run menu lists.
    pub fn add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.watched_project.clone() {
            self.adding_task = Some(crate::add_task::AddTask::new(project, window, cx));
            cx.notify();
        }
    }

    /// Brings forward the project's tab showing `target`, opening one when none does.
    pub fn show(&self, kind: &str, target: &str, title: Option<&str>) {
        if let Some(project) = self.project() {
            let tab =
                json!({ "project": project.id, "kind": kind, "target": target, "title": title });
            self.call("openTab", tab);
        }
    }

    /// The path of the project whose tasks, runs and ports the store watches.
    pub fn watched_project(&self) -> Option<&str> {
        self.watched_project.as_deref()
    }

    pub fn stop_run(&self, execution: &str) {
        self.call("taskStop", json!({ "executionId": execution }));
    }

    pub fn picture(&self) -> Option<Arc<Image>> {
        self.picture.clone()
    }

    pub fn preferences(&self) -> Preferences {
        self.read("preferences")
            .and_then(|read| serde_json::from_value(read.clone()).ok())
            .unwrap_or_default()
    }

    pub fn read(&self, key: &str) -> Option<&Value> {
        self.reads.get(key)
    }

    /// Shows `note` in the window's notifications.
    pub fn notify(&self, note: Notification, cx: &mut Context<Self>) {
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| window.push_notification(note, cx));
        });
    }

    pub fn call(&self, method: &'static str, params: Value) {
        let _ = self.asks.send(Ask::Call { method, params });
    }

    pub fn projects(&self) -> &[ProjectInfo] {
        self.view
            .as_ref()
            .map_or(&[], |view| view.workspace.projects.as_slice())
    }

    /// The project in front, as the core's layout says.
    pub fn project(&self) -> Option<&ProjectInfo> {
        let active = self.view.as_ref()?.layout.active_project.as_deref();
        let projects = self.projects();
        active
            .and_then(|id| projects.iter().find(|project| project.id == id))
            .or(projects.first())
    }

    pub fn launchers(&self) -> &[LauncherInfo] {
        self.view
            .as_ref()
            .map_or(&[], |view| view.workspace.launchers.as_slice())
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
        let account = self
            .read("usage")?
            .as_array()?
            .iter()
            .find(|account| account["launcher"] == launcher)?;
        let windows = account["usage"]["windows"].as_array()?;
        windows
            .iter()
            .filter_map(|window| window["usedPercent"].as_f64())
            .map(|used| used.clamp(0., 100.))
            .reduce(f64::max)
    }

    /// The file the tab in front shows, when it is an editor.
    pub fn front_file(&self) -> Option<PathBuf> {
        let (kind, target) = crate::tabs::first_view(&self.tab()?.pane)?;
        (kind == PaneKind::Files).then(|| PathBuf::from(target))
    }

    /// Brings the file's editor tab to the front, opening one when none shows it.
    pub fn open_file(&self, path: &Path) {
        self.show_file(path, false);
    }

    /// Opens the file's editor to look at, in place of the one opened that way last.
    pub fn preview_file(&self, path: &Path) {
        self.show_file(path, true);
    }

    fn show_file(&self, path: &Path, preview: bool) {
        if let Some(project) = self.project() {
            self.call(
                "openTab",
                json!({
                    "project": project.id,
                    "kind": "files",
                    "target": path.to_string_lossy(),
                    "preview": preview,
                }),
            );
        }
    }

    /// Opens settings on `page`, as a link to one of them does.
    pub fn open_settings(
        &mut self,
        page: crate::settings::Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        crate::settings::open(self, page, window, cx);
    }

    /// A terminal tab in the project in front, on the shell the core keeps ready there.
    /// Starts a chat with the agent the rail is set to, in the project in front.
    pub fn new_chat(&self) {
        if let (Some(project), Some(launcher)) = (self.project(), self.launcher()) {
            let _ = self.asks.send(Ask::StartChat {
                launcher: launcher.id.clone(),
                project: project.id.clone(),
                started: None,
            });
        }
    }

    /// Keeps a rail open, or closes it; a peeking rail that is kept stays where it is.
    pub fn toggle_rail(&mut self, edge: Edge, cx: &mut Context<Self>) {
        let open = match edge {
            Edge::Start => &mut self.side_rail_open,
            Edge::End => &mut self.tab_rail_open,
        };
        *open = !*open;
        self.peeking = None;
        cx.notify();
    }

    /// Swaps an agent's title in its header for a field holding it, which Enter or
    /// clicking away saves and Escape leaves unchanged.
    pub fn retitle(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(title) = self.view.as_ref().and_then(|view| {
            view.workspace
                .agents
                .iter()
                .find(|kept| kept.id == agent)
                .map(|kept| kept.title.clone())
        }) else {
            return;
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title.clone()));
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        let events = cx.subscribe(&input, |bench, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                bench.finish_retitle(true, cx);
            }
        });
        self.retitling = Some(Retitling {
            agent: agent.to_owned(),
            input,
            title,
            _events: events,
        });
        self.parts.redraw(&self.parts.stage, cx);
    }

    pub fn finish_retitle(&mut self, save: bool, cx: &mut Context<Self>) {
        let Some(retitling) = self.retitling.take() else {
            return;
        };
        let title: String = retitling
            .input
            .read(cx)
            .value()
            .trim()
            .chars()
            .take(72)
            .collect();
        if save && !title.is_empty() && title != retitling.title {
            self.call(
                "updateAgent",
                json!({ "agentId": retitling.agent, "patch": { "title": title, "renamed": true } }),
            );
        }
        self.parts.redraw(&self.parts.stage, cx);
    }

    /// Asks the person whether something goes ahead; `then` hears yes or no.
    pub fn confirm(
        &mut self,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        yes: impl Into<SharedString>,
        destructive: bool,
        then: impl Fn(&mut Workbench, bool, &mut Window, &mut Context<Workbench>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.asking = Some(Asking::Confirm {
            title: title.into(),
            body: body.into(),
            yes: yes.into(),
            destructive,
            then: Rc::new(then),
        });
        cx.notify();
    }

    /// Asks the person for a line of text, starting from `initial`; `then` hears what they
    /// typed, and nothing is done when they give up.
    pub fn prompt(
        &mut self,
        title: impl Into<SharedString>,
        initial: impl Into<String>,
        then: impl Fn(&mut Workbench, String, &mut Window, &mut Context<Workbench>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.asking = Some(Asking::Prompt {
            title: title.into(),
            initial: initial.into(),
            field: None,
            then: Rc::new(then),
        });
        cx.notify();
    }

    /// The question being asked, as a dialog, making a prompt's field the first time.
    fn asked(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let bench = cx.entity();
        let answered = move |yes: bool| {
            let bench = bench.clone();
            move |window: &mut Window, cx: &mut App| {
                bench.update(cx, |bench, cx| {
                    if let Some(Asking::Confirm { then, .. }) = bench.asking.take() {
                        then(bench, yes, window, cx);
                    }
                    bench.ask_trust(cx);
                    cx.notify();
                })
            }
        };
        Some(match self.asking.as_mut()? {
            Asking::Confirm {
                title,
                body,
                yes,
                destructive,
                ..
            } => crate::ui::Confirm::new(title.clone(), body.clone(), yes.clone())
                .destructive(*destructive)
                .on_yes(answered(true))
                .on_no(answered(false))
                .into_any_element(),
            Asking::Prompt {
                title,
                initial,
                field,
                ..
            } => {
                let field = field
                    .get_or_insert_with(|| {
                        let initial = initial.clone();
                        cx.new(|cx| {
                            let mut state = InputState::new(window, cx);
                            state.set_value(initial, window, cx);
                            state.focus(window, cx);
                            state.select_all(window, cx);
                            state
                        })
                    })
                    .clone();
                let (typing, giving_up) = (cx.entity(), cx.entity());
                crate::ui::Prompt::new(title.clone(), &field)
                    .on_yes(move |typed, window, cx| {
                        typing.update(cx, |bench, cx| {
                            if let Some(Asking::Prompt { then, .. }) = bench.asking.take() {
                                then(bench, typed.trim().to_owned(), window, cx);
                            }
                            cx.notify();
                        })
                    })
                    .on_no(move |_, cx| {
                        giving_up.update(cx, |bench, cx| {
                            bench.asking = None;
                            cx.notify();
                        })
                    })
                    .into_any_element()
            }
        })
    }

    /// Puts the core's next trust question to the person, each once, while nothing else is asked.
    fn ask_trust(&mut self, cx: &mut Context<Self>) {
        let asks: Vec<Value> = self
            .read("trust")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        self.trust_asked
            .retain(|id| asks.iter().any(|ask| ask["id"] == id.as_str()));
        if self.asking.is_some() {
            return;
        }
        let Some(ask) = asks.into_iter().find(|ask| {
            ask["id"]
                .as_str()
                .is_some_and(|id| !self.trust_asked.contains(id))
        }) else {
            return;
        };
        let text = |value: &Value| value.as_str().unwrap_or_default().to_owned();
        let id = text(&ask["id"]);
        self.trust_asked.insert(id.clone());
        let commands: Vec<String> = ask["commands"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|command| format!("{}: {}", text(&command["label"]), text(&command["command"])))
            .collect();
        let body: Vec<String> = [text(&ask["body"]), commands.join("\n")]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect();
        self.confirm(
            text(&ask["title"]),
            body.join("\n\n"),
            text(&ask["confirmLabel"]),
            false,
            move |bench, trust, _, _| {
                bench.call("answerTrust", json!({ "id": id, "trust": trust }));
            },
            cx,
        );
    }

    /// Removes an agent's worktree once the person knows what that loses: uncommitted
    /// changes, and commits only its branch has.
    pub fn remove_worktree(&mut self, agent: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (asks, agent) = (self.asks.clone(), agent.to_owned());
        cx.spawn_in(window, async move |bench, cx| {
            let cost =
                crate::core::request(&asks, "worktreeRemoval", json!({ "agentId": agent })).await;
            let _ = bench.update_in(cx, |bench, window, cx| match cost {
                Ok(cost) => bench.confirm_removal(agent, &cost, window, cx),
                Err(error) => window.push_notification(
                    Notification::error(format!("Could not remove the worktree: {error}")),
                    cx,
                ),
            });
        })
        .detach();
    }

    fn confirm_removal(
        &mut self,
        agent: String,
        cost: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if cost["sharedWith"]
            .as_array()
            .is_some_and(|shared| !shared.is_empty())
        {
            return window.push_notification(
                "Another agent or project still uses this worktree. Close it before removing the worktree.",
                cx,
            );
        }
        if cost["busy"] == true {
            return window
                .push_notification("Stop the agent's turn before removing its worktree", cx);
        }
        let branch = cost["branch"].as_str().unwrap_or_default().to_owned();
        let path = cost["path"].as_str().unwrap_or_default().to_owned();
        let dirty = cost["dirtyFiles"].as_u64().unwrap_or(0);
        let unmerged = cost["unmergedCommits"].as_u64().unwrap_or(0) > 0;
        let named = branch.clone();
        let then_branch = move |bench: &mut Workbench,
                                discard: bool,
                                window: &mut Window,
                                cx: &mut Context<Workbench>| {
            let (agent, named) = (agent.clone(), named.clone());
            if !unmerged {
                return bench.finish_removal(agent, named, discard, false, window, cx);
            }
            bench.confirm(
                format!("Delete branch {named} too?"),
                "It has commits that are not in the branch it started from. Deleting it loses any that were not pushed.",
                "Delete branch",
                true,
                move |bench, delete, window, cx| {
                    bench.finish_removal(agent.clone(), named.clone(), discard, delete, window, cx)
                },
                cx,
            );
        };
        if dirty == 0 {
            return then_branch(self, false, window, cx);
        }
        let changes = if dirty == 1 { "change" } else { "changes" };
        self.confirm(
            format!("Remove worktree {branch}?"),
            format!("{dirty} uncommitted {changes} in {path} will be lost."),
            "Remove",
            true,
            move |bench, remove, window, cx| {
                if remove {
                    then_branch(bench, true, window, cx);
                }
            },
            cx,
        );
    }

    fn finish_removal(
        &self,
        agent: String,
        branch: String,
        discard: bool,
        delete_branch: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let params =
            json!({ "agentId": agent, "discardChanges": discard, "deleteBranch": delete_branch });
        let removing = crate::core::request(&self.asks, "removeAgentWorktree", params);
        cx.spawn_in(window, async move |_, cx| {
            let removed = removing.await;
            let _ = cx.update(|window, cx| {
                window.push_notification(
                    match removed {
                        Ok(answer) if answer["branchDeleted"] == true => {
                            Notification::success(format!("Removed worktree and branch {branch}"))
                        }
                        Ok(_) => Notification::success(format!("Removed worktree {branch}")),
                        Err(error) => Notification::error(format!(
                            "Could not remove worktree {branch}: {error}"
                        )),
                    },
                    cx,
                )
            });
        })
        .detach();
    }

    /// The terminal an agent runs in, while it runs in one rather than as a chat.
    pub fn agent_terminal(&self, agent: &str) -> Option<SessionId> {
        self.view
            .as_ref()?
            .workspace
            .agents
            .iter()
            .find(|kept| kept.id == agent)?
            .pty_id
    }

    /// Moves an agent between its chat and its own CLI in a terminal, carrying what was
    /// typed and not sent.
    pub fn switch_view(&self, agent: &str, terminal: bool) {
        let to = if terminal { "terminal" } else { "chat" };
        self.call("switchAgentView", json!({ "agentId": agent, "to": to }));
    }

    pub fn rail_width(&self, edge: Edge) -> f32 {
        match edge {
            Edge::Start => self.side_rail_width,
            Edge::End => self.tab_rail_width,
        }
    }

    /// Sets a rail's width, within what that rail allows, and draws it at once.
    fn resize_rail(&mut self, edge: Edge, width: f32, window: &mut Window) {
        let (kept, range) = match edge {
            Edge::Start => (&mut self.side_rail_width, rail::SIDE_RAIL),
            Edge::End => (&mut self.tab_rail_width, agents::TAB_RAIL),
        };
        let width = width.clamp(range.min, range.max).round();
        if *kept != width {
            *kept = width;
            window.refresh();
        }
    }

    /// Moves the edge between a rail and the stage `by` points to the right.
    fn nudge_rail(&mut self, edge: Edge, by: f32, window: &mut Window) {
        let outward = match edge {
            Edge::Start => by,
            Edge::End => -by,
        };
        self.resize_rail(edge, self.rail_width(edge) + outward, window);
    }

    /// The handle between a rail and the stage, dragged or moved by arrow keys to resize the rail.
    fn rail_resizer(&self, edge: Edge, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::keymap::{EdgeFarLeft, EdgeFarRight, EdgeLeft, EdgeRight};
        div()
            .id(match edge {
                Edge::Start => "resize-side-rail",
                Edge::End => "resize-tab-rail",
            })
            .flex_none()
            .w(px(10.))
            .h_full()
            .cursor_col_resize()
            .key_context("RailResizer")
            .track_focus(&self.resizers[edge as usize])
            .tooltip(crate::ui::tooltip("Drag or use arrow keys to resize"))
            .on_click(cx.listener(move |bench, _, window, cx| {
                bench.resizers[edge as usize].focus(window, cx)
            }))
            .on_action(cx.listener(move |bench, _: &EdgeLeft, window, _| {
                bench.nudge_rail(edge, -16., window)
            }))
            .on_action(cx.listener(move |bench, _: &EdgeRight, window, _| {
                bench.nudge_rail(edge, 16., window)
            }))
            .on_action(cx.listener(move |bench, _: &EdgeFarLeft, window, _| {
                bench.nudge_rail(edge, -40., window)
            }))
            .on_action(cx.listener(move |bench, _: &EdgeFarRight, window, _| {
                bench.nudge_rail(edge, 40., window)
            }))
            .on_drag(DraggedRail(edge), |rail, _, _, cx| {
                cx.new(|_| DraggedRail(rail.0))
            })
    }

    /// Opens both rails when either is closed, else closes both.
    pub fn toggle_focus_mode(&mut self, cx: &mut Context<Self>) {
        let open = !(self.side_rail_open && self.tab_rail_open);
        self.side_rail_open = open;
        self.tab_rail_open = open;
        self.peeking = None;
        cx.notify();
    }

    /// Shows a closed rail over the stage while the pointer is at its edge or over it, and puts
    /// it away a moment after the pointer leaves.
    fn peek(&mut self, edge: Edge, inside: bool, cx: &mut Context<Self>) {
        if inside {
            self.unpeek = None;
            if self.peeking != Some(edge) {
                self.peeking = Some(edge);
                cx.notify();
            }
            return;
        }
        self.unpeek = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PEEK_CLOSE).await;
            let _ = this.update(cx, |bench, cx| {
                if bench.peeking == Some(edge) {
                    bench.peeking = None;
                    cx.notify();
                }
            });
        }));
    }

    /// The space the rail shows, while it still exists.
    pub fn space(&self) -> Option<&SpaceInfo> {
        let spaces = &self.view.as_ref()?.workspace.spaces;
        spaces
            .iter()
            .find(|space| Some(&space.id) == self.space.as_ref())
    }

    /// Shows one space's projects, or every project, and leaves a project the space hides
    /// for the first one it shows.
    pub fn show_space(&mut self, space: Option<String>, cx: &mut Context<Self>) {
        self.space = space;
        let shown = |project: &ProjectInfo| {
            self.space.is_none() || project.space.as_ref() == self.space.as_ref()
        };
        if !self.project().is_some_and(shown) {
            if let Some(first) = self.projects().iter().find(|project| shown(project)) {
                self.activate_project(&first.id);
            }
        }
        cx.notify();
    }

    /// Takes up a recent chat again, in the project it ran in.
    pub fn resume_chat(&self, recent: &RecentInfo) {
        let _ = self.asks.send(Ask::ResumeChat {
            recent: recent.id.clone(),
            project: recent.project.clone(),
        });
    }

    /// Brings forward the tab `by` places from the one in front, among those `same`
    /// accepts, going round the strip.
    fn step_tab(&self, by: isize, same: impl Fn(&Tab, &Tab) -> bool) {
        let (Some(strip), Some(front)) = (self.strip(), self.tab()) else {
            return;
        };
        let tabs: Vec<&Tab> = self
            .tabs(&strip)
            .iter()
            .filter(|tab| same(tab, front))
            .collect();
        let Some(at) = tabs.iter().position(|tab| tab.id == front.id) else {
            return;
        };
        let to = (at as isize + by).rem_euclid(tabs.len() as isize) as usize;
        self.activate_tab(&strip, &tabs[to].id);
    }

    /// Brings forward the tab at `place` in the strip, counting from one, or the last tab.
    fn go_to_tab(&self, place: Option<usize>) {
        let Some(strip) = self.strip() else {
            return;
        };
        let tabs = self.tabs(&strip);
        if let Some(tab) = place.map_or(tabs.last(), |place| tabs.get(place - 1)) {
            self.activate_tab(&strip, &tab.id);
        }
    }

    fn close_front_tab(&self) {
        if let (Some(strip), Some(tab)) = (self.strip(), self.tab()) {
            self.close_tab(&strip, &tab.id);
        }
    }

    /// Brings forward the first terminal tab in the strip in front, else opens a terminal.
    fn focus_terminal(&self) {
        let terminal = self.strip().and_then(|strip| {
            let tab = self.tabs(&strip).iter().find(|tab| {
                tabs::first_view(&tab.pane).is_some_and(|(kind, _)| kind == PaneKind::Terminal)
            })?;
            Some((strip.clone(), tab.id.clone()))
        });
        match terminal {
            Some((strip, tab)) => self.activate_tab(&strip, &tab),
            None => self.new_terminal(),
        }
    }

    /// Makes the Git pane when the project's Git tab comes to the front.
    fn git_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let front = self.tab().and_then(|tab| tabs::first_view(&tab.pane));
        let (Some((PaneKind::Git, _)), Some(project)) = (front, self.project()) else {
            return;
        };
        let repo = project.path.to_string_lossy().into_owned();
        if self
            .git
            .as_ref()
            .is_some_and(|pane| pane.read(cx).repo == repo)
        {
            return;
        }
        let asks = self.asks.clone();
        let bench = cx.entity().downgrade();
        let pane = cx.new(|cx| GitPane::new(asks, repo, bench, window, cx));
        if let Some(overview) = self.reads.get("git-overview") {
            pane.update(cx, |pane, cx| pane.overview(overview, cx));
        }
        if let Some(catalog) = self.reads.get("catalog") {
            pane.update(cx, |pane, cx| pane.catalog(catalog, cx));
        }
        if let Some(drafts) = self.reads.get("drafts") {
            pane.update(cx, |pane, cx| pane.drafts(drafts, cx));
        }
        self.git = Some(pane);
    }

    /// Makes the Search pane when the project's Search tab comes to the front.
    fn search_pane(&mut self, cx: &mut Context<Self>) {
        let front = self.tab().and_then(|tab| tabs::first_view(&tab.pane));
        let (Some((PaneKind::Search, _)), Some(project)) = (front, self.project()) else {
            return;
        };
        let project = project.path.to_string_lossy().into_owned();
        if self
            .search
            .as_ref()
            .is_some_and(|pane| pane.read(cx).project == project)
        {
            return;
        }
        let asks = self.asks.clone();
        self.search = Some(cx.new(|cx| SearchPane::new(asks, project, cx)));
    }

    /// Brings the project's Search tab to the front with the keyboard in its find field,
    /// opening one when it has none.
    fn open_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.project() else {
            return;
        };
        self.call(
            "openTab",
            json!({ "project": project.id, "kind": "search", "target": project.path.to_string_lossy() }),
        );
        let path = project.path.to_string_lossy();
        if let Some(search) = self
            .search
            .as_ref()
            .filter(|search| search.read(cx).project == path)
        {
            search.update(cx, |search, cx| search.focus_find(window, cx));
        }
    }

    /// Makes the code host's areas when the Git tab in front shows one, and keeps them
    /// on the project's repository and branch.
    fn host_area(&mut self, cx: &mut Context<Self>) {
        let front = self.tab().and_then(|tab| tabs::first_view(&tab.pane));
        let (Some((PaneKind::Git, _)), Some(project)) = (front, self.project()) else {
            return;
        };
        let Some(host) = project.code_host.clone() else {
            self.area = Area::Local;
            return;
        };
        if self.area == Area::Local {
            return;
        }
        let (folder, branch) = (
            project.path.to_string_lossy().into_owned(),
            crate::codehost::branch_of(project),
        );
        let area = self.area;
        match &self.host {
            Some(shown) if shown.read(cx).project == folder => {
                shown.update(cx, |shown, cx| {
                    shown.project_moved(&host, branch, cx);
                    shown.show(area, cx);
                });
            }
            _ => {
                let asks = self.asks.clone();
                let shown = cx.new(|_| HostArea::new(asks, folder, host, branch));
                shown.update(cx, |shown, cx| shown.show(area, cx));
                self.host = Some(shown);
            }
        }
    }

    /// Brings the project's Git tab to the front showing `area`.
    fn open_area(&mut self, area: Area, cx: &mut Context<Self>) {
        self.area = area;
        self.open_git();
        cx.notify();
    }

    /// Brings the project's Git tab to the front, opening one when it has none.
    fn open_git(&self) {
        if let Some(project) = self.project() {
            self.call(
                "openTab",
                json!({ "project": project.id, "kind": "git", "target": project.path.to_string_lossy() }),
            );
        }
    }

    pub fn new_terminal(&self) {
        if let Some(project) = self.project() {
            self.call("newTerminal", json!({ "project": project.id }));
        }
    }

    pub fn activate_project(&self, id: &str) {
        self.call("activateProject", json!({ "project": id }));
    }

    /// The strip in front: the SSH host shown instead of a project, else the project in front.
    pub fn strip(&self) -> Option<Strip> {
        let layout = &self.view.as_ref()?.layout;
        match layout
            .active_host
            .as_ref()
            .filter(|host| layout.hosts.contains_key(*host))
        {
            Some(host) => Some(Strip::Host(host.clone())),
            None => self
                .project()
                .map(|project| Strip::Project(project.id.clone())),
        }
    }

    fn layout_of(&self, strip: &Strip) -> Option<&ProjectLayout> {
        let layout = &self.view.as_ref()?.layout;
        match strip {
            Strip::Project(id) => layout.projects.get(id),
            Strip::Host(alias) => layout.hosts.get(alias),
        }
    }

    pub fn tabs(&self, strip: &Strip) -> &[Tab] {
        self.layout_of(strip)
            .map_or(&[], |layout| layout.tabs.as_slice())
    }

    /// The tab in front in the strip in front.
    pub fn tab(&self) -> Option<&Tab> {
        let layout = self.layout_of(&self.strip()?)?;
        let tabs = layout.tabs.as_slice();
        layout
            .active
            .as_deref()
            .and_then(|id| tabs.iter().find(|tab| tab.id == id))
            .or(tabs.first())
    }

    pub fn activate_tab(&self, strip: &Strip, tab: &str) {
        self.call("activateTab", strip.with(json!({ "tab": tab })));
    }

    pub fn close_tab(&self, strip: &Strip, tab: &str) {
        self.call("closeTab", strip.with(json!({ "tab": tab })));
    }

    /// Moves a tab to `to` in its strip, counted as the strip is now.
    pub fn move_tab(&self, strip: &Strip, tab: &str, to: usize) {
        self.call("moveTab", strip.with(json!({ "tab": tab, "to": to })));
    }

    /// Keeps a tab opened to look at, so the next one opens beside it.
    pub fn keep_tab(&self, strip: &Strip, tab: &str) {
        self.call("keepTab", strip.with(json!({ "tab": tab })));
    }

    pub fn close_tabs(&self, strip: &Strip, tabs: &[String]) {
        self.call("closeTabs", strip.with(json!({ "tabs": tabs })));
    }

    /// Names a tab; an empty name gives back the one its view gives it.
    pub fn rename_tab(&self, strip: &Strip, tab: &str, title: &str) {
        self.call(
            "renameTab",
            strip.with(json!({ "tab": tab, "title": title })),
        );
    }

    /// Opens a copy of a tab beside it, with new shells for its terminals.
    pub fn duplicate_tab(&self, strip: &Strip, tab: &str) {
        self.call("duplicateTab", strip.with(json!({ "tab": tab })));
    }

    /// Opens a terminal to an SSH host, in that host's strip.
    pub fn open_ssh_terminal(&self, alias: &str) {
        self.call("openSshTerminal", json!({ "host": alias }));
    }

    /// Brings an agent's tab to the front.
    /// Brings the tab of the agent `id` to the front, in its project.
    pub fn open_agent(&self, id: &str) {
        let Some(project) = self.view.as_ref().and_then(|view| {
            view.workspace
                .agents
                .iter()
                .find(|agent| agent.id == id)
                .map(|agent| agent.project.clone())
        }) else {
            return;
        };
        let strip = Strip::Project(project.clone());
        if let Some(tab) = self
            .tabs(&strip)
            .iter()
            .find(|tab| shows(tab, PaneKind::Agent).is_some_and(|shown| shown == id))
        {
            self.activate_project(&project);
            self.activate_tab(&strip, &tab.id);
        }
    }

    /// The agent the tab in front shows.
    pub fn agent(&self) -> Option<AgentRow> {
        let project = self.project()?;
        let target = shows(self.tab()?, PaneKind::Agent)?;
        self.agents_of(&project.id)
            .into_iter()
            .find(|agent| &agent.id == target)
    }

    pub fn session(&self, id: &str) -> Option<&SessionInfo> {
        self.view
            .as_ref()?
            .sessions
            .iter()
            .find(|session| session.id.to_string() == id)
    }

    /// Terminals started outside every project, which the core numbers.
    pub fn loose_terminals(&self) -> Vec<&SessionInfo> {
        self.view.as_ref().map_or(Vec::new(), |view| {
            view.sessions
                .iter()
                .filter(|session| session.rail_name.is_some() && session.exit.is_none())
                .collect()
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
                let session = view
                    .sessions
                    .iter()
                    .find(|session| session.agent_id.as_deref() == Some(agent.id.as_str()));
                let active_at = view.inbox["agents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|row| row["agentId"] == agent.id.as_str())
                    .and_then(|row| row["activeAt"].as_u64())
                    .unwrap_or(0);
                AgentRow {
                    active_at,
                    id: agent.id.clone(),
                    provider: agent.provider.clone(),
                    title: agent.title.clone(),
                    name: agent.name.clone(),
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
                    face: agent.face.clone(),
                    group: chat
                        .map(|chat| chat.group)
                        .or_else(|| session.and_then(|session| session.group))
                        .unwrap_or_default(),
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

    /// The rows the agent rail lists: this project's, or every project's.
    pub fn listed_agents(&self) -> Vec<AgentRow> {
        let mut rows = if self.all_projects {
            self.projects()
                .iter()
                .flat_map(|project| self.agents_of(&project.id))
                .collect()
        } else {
            self.project()
                .map(|project| self.agents_of(&project.id))
                .unwrap_or_default()
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
                .filter(|recent| {
                    self.all_harnesses
                        || self
                            .launcher()
                            .is_some_and(|launcher| launcher.provider == recent.provider)
                })
                .filter(|recent| recent.title.to_lowercase().contains(&self.filter_text))
                .collect()
        })
    }

    pub fn sign_in(&mut self) {
        self.signing_in = true;
        let _ = self.asks.send(Ask::Answer {
            key: "sign-in",
            method: "accountSignIn",
            params: json!({}),
        });
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
        self._filtering = Some(
            cx.subscribe(&filter, |bench, filter, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    bench.filter_text = filter.read(cx).value().to_lowercase();
                    cx.notify();
                }
            }),
        );
        filter.update(cx, |state, cx| state.focus(window, cx));
        self.filter = Some(filter);
    }
}

impl EventEmitter<crate::git::Changed> for Workbench {}

/// The picture `accountPicture` answers with: its type, and its bytes in base64.
fn decode_picture(value: &Value) -> Option<Arc<Image>> {
    crate::picture::decode(value["mime"].as_str()?, value["data"].as_str()?)
}

/// Whether two tabs show the same kind of pane, as ⌃Tab steps between them.
fn same_kind(tab: &Tab, front: &Tab) -> bool {
    let kind = |tab: &Tab| tabs::first_view(&tab.pane).map(|(kind, _)| kind);
    kind(tab) == kind(front)
}

/// What the tab's first view of `kind` shows, among its splits.
pub fn shows(tab: &Tab, kind: PaneKind) -> Option<&String> {
    shown_in(&tab.pane, kind)
}

fn shown_in(pane: &Pane, kind: PaneKind) -> Option<&String> {
    match pane {
        Pane::View {
            kind: shown,
            target,
            ..
        } if *shown == kind => Some(target),
        Pane::View { .. } => None,
        Pane::Split { panes, .. } => panes.iter().find_map(|pane| shown_in(pane, kind)),
    }
}

/// One part of the window, drawn as a view of its own so that it draws again only when the
/// store changes everywhere or names this part.
pub struct Part {
    bench: WeakEntity<Workbench>,
    draw: fn(&mut Workbench, &mut Window, &mut Context<Workbench>) -> AnyElement,
    column: bool,
    _store: Subscription,
}

impl Render for Part {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let draw = self.draw;
        let drawn = self
            .bench
            .update(cx, |bench, cx| draw(bench, window, cx))
            .unwrap_or_else(|_| div().into_any_element());
        div()
            .size_full()
            .flex()
            .when(self.column, |part| part.flex_col())
            .child(drawn)
    }
}

/// The window's parts. A plain `cx.notify()` on the store draws them all again; output that
/// touches one part, such as a terminal's, draws only that one.
#[derive(Clone)]
pub struct Parts {
    pub top: Entity<Part>,
    pub rail: Entity<Part>,
    pub stage: Entity<Part>,
    pub tabs: Entity<Part>,
    pub finder: Entity<Part>,
}

impl Parts {
    fn new(cx: &mut Context<Workbench>) -> Self {
        let bench = cx.entity();
        let mut part =
            |draw: fn(&mut Workbench, &mut Window, &mut Context<Workbench>) -> AnyElement,
             column: bool| {
                cx.new(|cx| Part {
                    bench: bench.downgrade(),
                    draw,
                    column,
                    _store: cx.observe(&bench, |_, _, cx| cx.notify()),
                })
            };
        Self {
            top: part(
                |bench, _, cx| tabs::top_bar(bench, cx).into_any_element(),
                true,
            ),
            rail: part(
                |bench, _, cx| rail::side_rail(bench, cx).into_any_element(),
                false,
            ),
            stage: part(
                |bench, window, cx| {
                    bench.compose(window, cx);
                    bench.hint(window, cx);
                    bench.git_pane(window, cx);
                    bench.host_area(cx);
                    bench.search_pane(cx);
                    crate::desk::prepare(bench, window, cx);
                    home::stage(bench, window, cx)
                },
                false,
            ),
            tabs: part(|bench, _, cx| agents::tab_rail(bench, cx), false),
            finder: part(
                |bench, window, cx| {
                    crate::switcher::overlay(bench, cx)
                        .or_else(|| crate::palette::overlay(bench, window, cx))
                        .or_else(|| crate::finder::overlay(bench, window, cx))
                        .unwrap_or_else(|| div().into_any_element())
                },
                true,
            ),
        }
    }

    pub fn redraw(&self, part: &Entity<Part>, cx: &mut App) {
        part.update(cx, |_, cx| cx.notify());
    }
}

impl Workbench {
    /// A closed rail's edge of the window, which brings the rail out over the stage.
    fn rail_edge(&self, edge: Edge, cx: &mut Context<Self>) -> impl IntoElement {
        let (part, width) = match edge {
            Edge::Start => (&self.parts.rail, self.side_rail_width),
            Edge::End => (&self.parts.tabs, self.tab_rail_width),
        };
        let peeking = self.peeking == Some(edge);
        let zone = div()
            .id(match edge {
                Edge::Start => "peek-start",
                Edge::End => "peek-end",
            })
            .absolute()
            .top(px(44.))
            .bottom(px(10.))
            .w(px(if peeking { width + 20. } else { PEEK_REACH }))
            .px(px(10.))
            .on_hover(cx.listener(move |bench, inside: &bool, _, cx| bench.peek(edge, *inside, cx)))
            .when(peeking, |zone| {
                zone.child(
                    part.clone()
                        .cached(StyleRefinement::default().w(px(width)).h_full()),
                )
            });
        match edge {
            Edge::Start => zone.left_0(),
            Edge::End => zone.right_0(),
        }
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::keymap::*;
        let asked = self.asked(window, cx);
        if window.focused(cx).is_none() {
            self.focus.focus(window, cx);
        }
        let t = Tokens::get(cx);
        let parts = &self.parts;
        div()
            .id("shell")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .on_modifiers_changed(
                cx.listener(|bench, event, _, cx| bench.modifiers_changed(event, cx)),
            )
            .capture_key_down(cx.listener(|bench, event, _, cx| bench.switcher_key(event, cx)))
            .on_action(
                cx.listener(|bench, _: &crate::switcher::NextPlace, window, cx| {
                    bench.step_place(1, window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::switcher::PreviousPlace, window, cx| {
                    bench.step_place(-1, window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::commands::OpenCommands, window, cx| {
                    bench.toggle_commands(window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::agent_palette::ChooseAgent, window, cx| {
                    bench.toggle_agent_palette(window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::picker::OpenProject, window, cx| {
                    bench.toggle_picker(crate::picker::Picking::Projects, window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::picker::OpenHost, window, cx| {
                    bench.toggle_picker(crate::picker::Picking::Hosts, window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::picker::OpenPlace, window, cx| {
                    bench.toggle_picker(crate::picker::Picking::Both, window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::new_tab::NewTab, window, cx| {
                    bench.toggle_new_tab(window, cx);
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::theme_palette::SwitchTheme, window, cx| {
                    bench.toggle_theme_palette(window, cx);
                }),
            )
            .on_modifiers_changed(
                cx.listener(|bench, event, window, cx| bench.voice_keys(event, window, cx)),
            )
            .capture_key_down(cx.listener(|bench, _: &gpui_kit::KeyDownEvent, _, cx| {
                bench.voice.update(cx, |voice, cx| voice.abandon_hold(cx))
            }))
            .on_action(cx.listener(|bench, _: &OpenFile, window, cx| {
                bench.toggle_finder(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|bench, _: &NewTerminal, _, _| bench.new_terminal()))
            .on_action(cx.listener(|bench, _: &FocusGit, _, _| bench.open_git()))
            .on_action(cx.listener(|bench, open: &OpenArea, _, cx| bench.open_area(open.area, cx)))
            .on_action(
                cx.listener(|bench, _: &FocusSearch, window, cx| bench.open_search(window, cx)),
            )
            .on_action(
                cx.listener(|bench, _: &FocusSearchWindow, window, cx| {
                    bench.open_search(window, cx)
                }),
            )
            .on_action(
                cx.listener(|bench, _: &crate::settings::ToggleSettings, window, cx| {
                    crate::settings::toggle(bench, window, cx)
                }),
            )
            .on_action(cx.listener(|bench, _: &NewChat, _, _| bench.new_chat()))
            .on_action(cx.listener(|bench, _: &FocusTerminal, _, _| bench.focus_terminal()))
            .on_action(cx.listener(|bench, _: &ToggleDesk, _, _| bench.toggle_front_desk()))
            .on_action(cx.listener(|bench, _: &NewPage, _, _| bench.new_front_page()))
            .on_action(cx.listener(|bench, _: &NewSimulator, _, cx| {
                bench.front_simulator();
                bench.parts.redraw(&bench.parts.stage, cx);
            }))
            .on_action(cx.listener(|bench, _: &PreviousTab, _, _| bench.step_tab(-1, |_, _| true)))
            .on_action(cx.listener(|bench, _: &NextTab, _, _| bench.step_tab(1, |_, _| true)))
            .on_action(cx.listener(|bench, _: &PreviousOfKind, _, _| bench.step_tab(-1, same_kind)))
            .on_action(cx.listener(|bench, _: &NextOfKind, _, _| bench.step_tab(1, same_kind)))
            .on_action(cx.listener(|bench, _: &GoToTab1, _, _| bench.go_to_tab(Some(1))))
            .on_action(cx.listener(|bench, _: &GoToTab2, _, _| bench.go_to_tab(Some(2))))
            .on_action(cx.listener(|bench, _: &GoToTab3, _, _| bench.go_to_tab(Some(3))))
            .on_action(cx.listener(|bench, _: &GoToTab4, _, _| bench.go_to_tab(Some(4))))
            .on_action(cx.listener(|bench, _: &GoToTab5, _, _| bench.go_to_tab(Some(5))))
            .on_action(cx.listener(|bench, _: &GoToTab6, _, _| bench.go_to_tab(Some(6))))
            .on_action(cx.listener(|bench, _: &GoToTab7, _, _| bench.go_to_tab(Some(7))))
            .on_action(cx.listener(|bench, _: &GoToTab8, _, _| bench.go_to_tab(Some(8))))
            .on_action(cx.listener(|bench, _: &GoToLastTab, _, _| bench.go_to_tab(None)))
            .on_action(cx.listener(|bench, _: &CloseTab, _, _| bench.close_front_tab()))
            .on_action(cx.listener(|bench, _: &FocusMode, _, cx| bench.toggle_focus_mode(cx)))
            .on_action(cx.listener(|bench, _: &QuitAndStopEverything, _, _| {
                let _ = bench.asks.send(Ask::Shutdown);
            }))
            .on_action(|_: &Minimize, window, _| window.minimize_window())
            .on_action(|_: &Zoom, window, _| window.zoom_window())
            .on_action(|_: &ToggleFullScreen, window, _| window.toggle_fullscreen())
            .on_action(
                cx.listener(|bench, open: &OpenAt, _, cx| {
                    crate::editor::go(bench, open.clone(), cx)
                }),
            )
            .flex()
            .flex_col()
            .pt(px(6.))
            .bg(t.surface_sunken)
            .font_family(theme::UI)
            .text_size(px(13.))
            .line_height(px(13. * 1.45))
            .text_color(t.text_secondary)
            .child(
                parts
                    .top
                    .clone()
                    .cached(StyleRefinement::default().w_full().h(px(34.)).flex_none()),
            )
            .child(
                div()
                    .id("shell-row")
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .pt(px(4.))
                    .px(px(10.))
                    .pb(px(10.))
                    .on_drag_move(cx.listener(
                        |bench, event: &DragMoveEvent<DraggedRail>, window, cx| {
                            let edge = event.drag(cx).0;
                            let (x, bounds) = (event.event.position.x, event.bounds);
                            let width = match edge {
                                Edge::Start => x - bounds.left(),
                                Edge::End => bounds.right() - x,
                            };
                            bench.resize_rail(edge, f32::from(width) - 15., window);
                        },
                    ))
                    .when(self.side_rail_open, |row| {
                        row.child(
                            parts.rail.clone().cached(
                                StyleRefinement::default()
                                    .w(px(self.side_rail_width))
                                    .h_full()
                                    .flex_none(),
                            ),
                        )
                        .child(self.rail_resizer(Edge::Start, cx))
                    })
                    .child(
                        parts
                            .stage
                            .clone()
                            .cached(StyleRefinement::default().flex_1().min_w_0().h_full()),
                    )
                    .when(self.tab_rail_open, |row| {
                        row.child(self.rail_resizer(Edge::End, cx)).child(
                            parts.tabs.clone().cached(
                                StyleRefinement::default()
                                    .w(px(self.tab_rail_width))
                                    .h_full()
                                    .flex_none(),
                            ),
                        )
                    }),
            )
            .when(!self.side_rail_open, |shell| {
                shell.child(self.rail_edge(Edge::Start, cx))
            })
            .when(!self.tab_rail_open, |shell| {
                shell.child(self.rail_edge(Edge::End, cx))
            })
            .children(asked)
            .children(crate::add_task::view(self, cx))
            .children(crate::boot::reconnecting(self, cx))
            .children(crate::boot::opening(self, cx))
            .when(
                self.finder.is_some() || self.palette.is_some() || self.switcher.is_some(),
                |shell| {
                    shell.child(
                        parts
                            .finder
                            .clone()
                            .cached(StyleRefinement::default().absolute().inset_0()),
                    )
                },
            )
    }
}

/// Puts `text` at the end of the draft after a blank line, keeping what it holds, chips
/// and all, with the caret after it.
fn append(draft: &Entity<TextareaState>, text: &str, window: &mut Window, cx: &mut App) {
    draft.update(cx, |draft, cx| {
        let held = draft.value();
        let kept = held.trim_end().len();
        let joined = if kept == 0 {
            text.to_owned()
        } else {
            format!("\n\n{text}")
        };
        draft.set_selected_range(kept..held.len(), cx);
        draft.replace(joined, window, cx);
    });
}
