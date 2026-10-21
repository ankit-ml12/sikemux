//! An agent's desk, beside it in its tab: the pages its browser holds, one in front.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, Keystroke, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window,
};
use gpui_native_view::{native_view, Frame, NativeSurface, Surface};
use serde_json::Value;
use sikemux_browser::{
    BrowserBounds, BrowserDownload, BrowserManager, BrowserShortcut, BrowserSnapshot, Host,
    ParentView,
};
use sikemux_core::cli::protocol::{is_browser_method, is_sim_method, HarnessRequest};
use sikemux_wire::protocol::{CallId, Pane, PaneKind, WindowAnswer, WindowCall};
use tokio::sync::mpsc;

use crate::core::Ask;
use crate::icons::Icon;
use crate::theme::{Tokens, RADIUS_2};
use crate::ui::{self, IconButton};
use crate::workbench::{shows, Strip, Workbench};

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "desk.toggle")]
pub struct ToggleDesk;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "browser.tabNew")]
pub struct NewPage;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "simulator.new")]
pub struct NewSimulator;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "browser.address")]
pub struct FocusAddress;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "browser.reload")]
pub struct ReloadPage;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "browser.back")]
pub struct PageBack;

#[derive(Clone, Default, PartialEq, gpui::Action)]
#[action(name = "browser.forward")]
pub struct PageForward;

/// The browser engine and what the desks show of it.
pub struct Desks {
    engine: Option<Arc<BrowserManager>>,
    strips: HashMap<String, BrowserSnapshot>,
    pages: HashMap<String, Page>,
    /// Each agent's tabs as last heard, so only a tab it newly opened brings its desk out.
    known: HashMap<String, HashSet<String>>,
    /// What each agent's desk shows: its pages, or its simulator.
    kinds: HashMap<String, DeskKind>,
    _hearing: Option<gpui_kit::Task<()>>,
    _activation: Subscription,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum DeskKind {
    #[default]
    Pages,
    Simulator,
}

/// One desk's page area and address bar.
struct Page {
    surface: NativeSurface,
    focus: FocusHandle,
    address: Entity<InputState>,
    _typing: Subscription,
}

/// What the engine tells the window.
enum Heard {
    Tabs,
    Downloaded(BrowserDownload),
    Shortcut(BrowserShortcut),
}

impl Desks {
    pub fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let (heard, mut hearing) = mpsc::unbounded_channel();
        sikemux_core::runtime::set(runtime().handle().clone());
        let engine = window_pointer(window).map(|view| {
            BrowserManager::new(Arc::new(DeskHost {
                view,
                heard,
                home: sikemux_process::paths::config_dir(),
            }))
        });
        let task = engine.is_some().then(|| {
            cx.spawn_in(window, async move |this, cx| {
                while let Some(heard) = hearing.recv().await {
                    let heard =
                        this.update_in(cx, |bench, window, cx| bench.hear(heard, window, cx));
                    if heard.is_err() {
                        break;
                    }
                }
            })
        });
        let activation = cx.observe_window_activation(window, |bench, window, _| {
            if window.is_window_active() {
                let _ = bench.asks.send(Ask::RegisterWindow);
            }
        });
        Self {
            engine,
            strips: HashMap::new(),
            pages: HashMap::new(),
            known: HashMap::new(),
            kinds: HashMap::new(),
            _hearing: task,
            _activation: activation,
        }
    }
}

/// The browser's and the simulator's work, off the window's thread.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("engines")
            .enable_all()
            .build()
            .expect("the browser's runtime starts")
    })
}

pub fn spawn(work: impl std::future::Future<Output = ()> + Send + 'static) {
    runtime().spawn(work);
}

/// Runs `work` on the engines' threads and waits for it, from the window's.
pub async fn run<T: Send + 'static>(
    work: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T> {
    runtime().spawn(work).await.ok()
}

#[cfg(target_os = "macos")]
fn window_pointer(window: &Window) -> Option<usize> {
    gpui_native_view::window_view(window)
        .map(|view| gpui_native_view::Retained::as_ptr(&view) as usize)
}

#[cfg(not(target_os = "macos"))]
fn window_pointer(_: &Window) -> Option<usize> {
    None
}

struct DeskHost {
    /// GPUI's own view of the window, which outlives the engine; only used on the main thread.
    view: usize,
    heard: mpsc::UnboundedSender<Heard>,
    home: Option<PathBuf>,
}

impl Host for DeskHost {
    fn window(&self) -> Result<ParentView, String> {
        Ok(ParentView::beneath(self.view as *mut std::ffi::c_void))
    }

    fn state_dir(&self) -> Option<PathBuf> {
        Some(self.home.as_ref()?.join("browser"))
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        Some(self.home.as_ref()?.join("cache"))
    }

    fn videos_dir(&self) -> Option<PathBuf> {
        Some(sikemux_process::paths::home()?.join("Movies"))
    }

    fn page_url(&self, _agent_id: &str, _html: &str) -> Result<String, String> {
        Err("this window cannot show an agent's page yet".into())
    }

    fn tabs_changed(&self) {
        let _ = self.heard.send(Heard::Tabs);
    }

    fn downloaded(&self, download: &BrowserDownload) {
        let _ = self.heard.send(Heard::Downloaded(download.clone()));
    }

    fn acting(&self, _agent_id: &str) {
        let _ = self.heard.send(Heard::Tabs);
    }

    fn shortcut(&self, shortcut: BrowserShortcut) {
        let _ = self.heard.send(Heard::Shortcut(shortcut));
    }
}

/// Where a desk's page shows: the engine lays the agent's tab in front over the frame.
struct DeskPage {
    engine: Arc<BrowserManager>,
    agent: String,
}

impl Surface for DeskPage {
    fn place(&self, frame: Option<Frame>) {
        let bounds = frame.map(|Frame { bounds, visible }| BrowserBounds {
            x: f64::from(bounds.left()),
            y: f64::from(bounds.top()),
            width: f64::from(bounds.size.width),
            height: f64::from(bounds.size.height),
            clip_left: f64::from(visible.left() - bounds.left()),
            clip_right: f64::from(bounds.right() - visible.right()),
            holes: Vec::new(),
            dim: 0.,
            opacity: 1.,
        });
        if let Err(error) = self.engine.set_bounds(&self.agent, bounds) {
            eprintln!("sikemux: the desk's page could not be placed: {error}");
        }
    }

    #[cfg(target_os = "macos")]
    fn responder(&self) -> Option<gpui_native_view::Retained<gpui_native_view::NSView>> {
        self.engine.active_view(&self.agent).ok()?.1.view_here()
    }
}

impl Workbench {
    fn hear(&mut self, heard: Heard, window: &mut Window, cx: &mut Context<Self>) {
        match heard {
            Heard::Tabs => self.read_strips(),
            Heard::Downloaded(download) => {
                eprintln!("sikemux: a page saved {}", download.path);
            }
            Heard::Shortcut(shortcut) => {
                let mut keys = String::from("cmd-");
                if shortcut.shift {
                    keys.push_str("shift-");
                }
                keys.push_str(&shortcut.key.to_lowercase());
                if let Ok(keystroke) = Keystroke::parse(&keys) {
                    window.dispatch_keystroke(keystroke, cx);
                }
            }
        }
        self.parts.redraw(&self.parts.stage, cx);
    }

    /// Reads every agent's tabs again, and brings out the desk of an agent that opened one.
    fn read_strips(&mut self) {
        let Some(engine) = self.desks.engine.clone() else {
            return;
        };
        let agents: Vec<String> = self
            .view
            .as_ref()
            .map(|view| {
                view.workspace
                    .agents
                    .iter()
                    .map(|agent| agent.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        for agent in agents {
            let Ok(strip) = engine.snapshot(&agent) else {
                continue;
            };
            let now: HashSet<String> = strip.tabs.iter().map(|tab| tab.id.clone()).collect();
            let before = self.desks.known.insert(agent.clone(), now.clone());
            let opened = before.is_some_and(|before| now.iter().any(|tab| !before.contains(tab)));
            if opened && !self.desk_open_anywhere(&agent) {
                self.reveal_desk(&agent);
            }
            self.desks.strips.insert(agent, strip);
        }
    }

    /// The project tab holding the agent, its strip and its view there, wherever it is.
    fn agent_place(&self, agent: &str) -> Option<(Strip, String, String)> {
        let layout = &self.view.as_ref()?.layout;
        layout.projects.iter().find_map(|(project, strip)| {
            strip.tabs.iter().find_map(|tab| {
                view_of(&tab.pane, PaneKind::Agent, agent)
                    .map(|view| (Strip::Project(project.clone()), tab.id.clone(), view))
            })
        })
    }

    fn desk_open_anywhere(&self, agent: &str) -> bool {
        self.view.as_ref().is_some_and(|view| {
            view.layout.projects.values().any(|strip| {
                strip
                    .tabs
                    .iter()
                    .any(|tab| view_of(&tab.pane, PaneKind::Desk, agent).is_some())
            })
        })
    }

    fn reveal_desk(&self, agent: &str) {
        if let Some((strip, tab, view)) = self.agent_place(agent) {
            self.split_pane(&strip, &tab, &view, "right", PaneKind::Desk, agent);
        }
    }

    /// Turns the agent's desk to its simulator, bringing the desk out if it is closed.
    pub(crate) fn show_simulator(&mut self, agent: &str) {
        self.desks
            .kinds
            .insert(agent.to_owned(), DeskKind::Simulator);
        if !self.desk_open_anywhere(agent) {
            self.reveal_desk(agent);
        }
    }

    fn show_kind(&mut self, agent: &str, kind: DeskKind) {
        self.desks.kinds.insert(agent.to_owned(), kind);
    }

    /// The desk view of `agent` in `tab`, by id.
    pub fn desk_of(&self, tab: &str, agent: &str) -> Option<String> {
        let strip = self.strip()?;
        let tab = self.tabs(&strip).iter().find(|held| held.id == tab)?;
        view_of(&tab.pane, PaneKind::Desk, agent)
    }

    /// Opens the agent's desk beside its view, or closes it when it is open.
    pub fn toggle_desk(&mut self, tab: &str, view: &str, agent: &str) {
        let Some(strip) = self.strip() else {
            return;
        };
        match self.desk_of(tab, agent) {
            Some(desk) => self.close_pane(&strip, tab, &desk),
            None => {
                self.split_pane(&strip, tab, view, "right", PaneKind::Desk, agent);
                self.open_first_page(agent);
            }
        }
    }

    /// The agent in the pane the person last used, else the tab's first agent, with its view.
    fn front_agent(&self) -> Option<(String, String, String)> {
        let tab = self.tab()?;
        let focused = self
            .panes
            .focused
            .as_deref()
            .and_then(|focused| target_of(&tab.pane, focused));
        let agent = match focused {
            Some((PaneKind::Agent | PaneKind::Desk, agent)) => agent,
            _ => shows(tab, PaneKind::Agent)?.clone(),
        };
        let view = view_of(&tab.pane, PaneKind::Agent, &agent)?;
        Some((tab.id.clone(), view, agent))
    }

    pub fn toggle_front_desk(&mut self) {
        if let Some((tab, view, agent)) = self.front_agent() {
            self.toggle_desk(&tab, &view, &agent);
        }
    }

    /// The front agent's desk, turned to its simulator.
    pub fn front_simulator(&mut self) {
        if let Some((_, _, agent)) = self.front_agent() {
            self.show_simulator(&agent);
        }
    }

    /// A new page on the front agent's desk, which opens if it is closed.
    pub fn new_front_page(&mut self) {
        let Some((tab, view, agent)) = self.front_agent() else {
            return;
        };
        if let (None, Some(strip)) = (self.desk_of(&tab, &agent), self.strip()) {
            self.split_pane(&strip, &tab, &view, "right", PaneKind::Desk, &agent);
        }
        self.new_page(&agent);
    }

    /// The front agent, while its desk is open on its pages.
    fn front_pages(&self) -> Option<String> {
        let (tab, _, agent) = self.front_agent()?;
        self.desk_of(&tab, &agent)?;
        let kind = self.desks.kinds.get(&agent).copied().unwrap_or_default();
        (kind == DeskKind::Pages).then_some(agent)
    }

    /// Closes what the front desk shows, its page or its simulator, and says whether it had one
    /// to close; with neither, ⌘W closes the tab as it would anywhere else.
    pub fn close_shown_page(&mut self) -> bool {
        let Some((tab, _, agent)) = self.front_agent() else {
            return false;
        };
        if self.desk_of(&tab, &agent).is_none() {
            return false;
        }
        match self.desks.kinds.get(&agent).copied().unwrap_or_default() {
            DeskKind::Simulator => {
                self.show_kind(&agent, DeskKind::Pages);
                true
            }
            DeskKind::Pages => {
                let shown = self
                    .desks
                    .strips
                    .get(&agent)
                    .and_then(|strip| strip.active_tab_id.clone());
                match shown {
                    Some(page) => {
                        self.with_engine(|engine| engine.close_tab(&agent, &page));
                        true
                    }
                    None => false,
                }
            }
        }
    }

    /// Puts the front desk's address in the person's hands, all of it selected to type over.
    pub fn focus_front_address(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self
            .front_pages()
            .and_then(|agent| self.desks.pages.get(&agent))
        else {
            return;
        };
        page.address.update(cx, |address, cx| {
            address.focus(window, cx);
            address.select_all(window, cx);
        });
    }

    pub fn reload_front_page(&self) {
        if let Some(agent) = self.front_pages() {
            self.with_engine(|engine| engine.reload(&agent));
        }
    }

    /// Steps the front desk's page back or forward through its history.
    pub fn step_front_page(&self, by: i32) {
        if let Some(agent) = self.front_pages() {
            self.with_engine(|engine| engine.history(&agent, by));
        }
    }

    /// A desk opened on an agent with no pages starts on a blank one, ready for an address.
    fn open_first_page(&self, agent: &str) {
        let Some(engine) = self.desks.engine.clone() else {
            return;
        };
        if self
            .desks
            .strips
            .get(agent)
            .is_some_and(|strip| !strip.tabs.is_empty())
        {
            return;
        }
        let agent = agent.to_owned();
        runtime().spawn(async move {
            if let Err(error) = engine.open_tab(&agent, None).await {
                eprintln!("sikemux: the desk could not open a page: {error}");
            }
        });
    }

    /// Opens `url` in a new page on `agent`'s desk, bringing the agent and its desk to the
    /// front. Answers false when this window has no browser, so the caller can open it elsewhere.
    pub fn open_on_desk(&mut self, agent: &str, url: &str) -> bool {
        let Some(engine) = self.desks.engine.clone() else {
            return false;
        };
        self.open_agent(agent);
        if !self.desk_open_anywhere(agent) {
            self.reveal_desk(agent);
        }
        let (agent, url) = (agent.to_owned(), url.to_owned());
        runtime().spawn(async move {
            if let Err(error) = engine.open_tab(&agent, Some(&url)).await {
                eprintln!("sikemux: the desk could not open {url}: {error}");
            }
        });
        true
    }

    fn new_page(&self, agent: &str) {
        let Some(engine) = self.desks.engine.clone() else {
            return;
        };
        let agent = agent.to_owned();
        runtime().spawn(async move {
            if let Err(error) = engine.open_tab(&agent, None).await {
                eprintln!("sikemux: the desk could not open a page: {error}");
            }
        });
    }

    fn with_engine(&self, act: impl FnOnce(&BrowserManager) -> sikemux_browser::BrowserResult<()>) {
        if let Some(engine) = &self.desks.engine {
            if let Err(error) = act(engine) {
                eprintln!("sikemux: the desk's browser refused: {error}");
            }
        }
    }
}

/// Makes the page areas and address bars of the desks the tab in front shows.
pub fn prepare(bench: &mut Workbench, window: &mut Window, cx: &mut Context<Workbench>) {
    let mut agents = Vec::new();
    if let Some(tab) = bench.tab() {
        desks_in(&tab.pane, &mut agents);
    }
    for agent in &agents {
        if bench.desks.kinds.get(agent) == Some(&DeskKind::Simulator) && bench.sims.holds(agent) {
            bench.start_screen(agent, cx);
        }
    }
    let Some(engine) = bench.desks.engine.clone() else {
        return;
    };
    for agent in agents {
        if bench.desks.pages.contains_key(&agent) {
            continue;
        }
        if !bench.desks.strips.contains_key(&agent) {
            if let Ok(strip) = engine.snapshot(&agent) {
                let tabs = strip.tabs.iter().map(|tab| tab.id.clone()).collect();
                bench.desks.known.insert(agent.clone(), tabs);
                bench.desks.strips.insert(agent.clone(), strip);
            }
        }
        let address =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search or enter an address"));
        let typed = agent.clone();
        let typing = cx.subscribe_in(
            &address,
            window,
            move |bench, address, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    let input = address.read(cx).value().trim().to_string();
                    if input.is_empty() {
                        return;
                    }
                    bench.with_engine(|engine| engine.navigate_typed(&typed, &input));
                    if let Some(page) = bench.desks.pages.get(&typed) {
                        page.focus.focus(window, cx);
                    }
                }
            },
        );
        bench.desks.pages.insert(
            agent.clone(),
            Page {
                surface: NativeSurface::new(DeskPage {
                    engine: engine.clone(),
                    agent,
                }),
                focus: cx.focus_handle(),
                address,
                _typing: typing,
            },
        );
    }
    let shown: Vec<(String, String)> = bench
        .desks
        .pages
        .iter()
        .filter_map(|(agent, page)| {
            let url = bench.desks.strips.get(agent).and_then(|strip| {
                strip
                    .tabs
                    .iter()
                    .find(|tab| Some(&tab.id) == strip.active_tab_id.as_ref())
                    .map(|tab| shown_address(&tab.url))
            })?;
            let typing = page.address.read(cx).focus_handle(cx).is_focused(window);
            (!typing && page.address.read(cx).value() != url.as_str()).then(|| (agent.clone(), url))
        })
        .collect();
    for (agent, url) in shown {
        if let Some(page) = bench.desks.pages.get(&agent) {
            page.address
                .update(cx, |address, cx| address.set_value(url, window, cx));
        }
    }
}

fn shown_address(url: &str) -> String {
    if url == sikemux_browser::BLANK_URL {
        String::new()
    } else {
        url.to_owned()
    }
}

fn desks_in(pane: &Pane, found: &mut Vec<String>) {
    match pane {
        Pane::View {
            kind: PaneKind::Desk,
            target,
            ..
        } => found.push(target.clone()),
        Pane::View { .. } => {}
        Pane::Split { panes, .. } => panes.iter().for_each(|pane| desks_in(pane, found)),
    }
}

fn view_of(pane: &Pane, kind: PaneKind, target: &str) -> Option<String> {
    match pane {
        Pane::View {
            id,
            kind: shown,
            target: shown_target,
        } => (*shown == kind && shown_target == target).then(|| id.clone()),
        Pane::Split { panes, .. } => panes.iter().find_map(|pane| view_of(pane, kind, target)),
    }
}

fn target_of(pane: &Pane, view: &str) -> Option<(PaneKind, String)> {
    match pane {
        Pane::View { id, kind, target } => (id == view).then(|| (*kind, target.clone())),
        Pane::Split { panes, .. } => panes.iter().find_map(|pane| target_of(pane, view)),
    }
}

/// The desk of `agent`, as the view `view` of `tab`.
pub fn pane(
    bench: &Workbench,
    tab: &str,
    view: &str,
    agent: &str,
    _window: &Window,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let kind = bench.desks.kinds.get(agent).copied().unwrap_or_default();
    if kind == DeskKind::Simulator {
        return simulator_desk(bench, tab, view, agent, cx);
    }
    let strip = bench.desks.strips.get(agent);
    let front = strip.and_then(|strip| {
        strip
            .tabs
            .iter()
            .find(|tab| Some(&tab.id) == strip.active_tab_id.as_ref())
    });
    let mut head = div()
        .h(px(36.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .border_b_1()
        .border_color(t.border)
        .child(kinds(agent, kind, cx));
    for page in strip.map(|strip| strip.tabs.as_slice()).unwrap_or_default() {
        let label = if !page.title.is_empty() {
            page.title.clone()
        } else if page.url == sikemux_browser::BLANK_URL {
            "New tab".to_owned()
        } else {
            page.url.clone()
        };
        let (switch_agent, switch_tab) = (agent.to_owned(), page.id.clone());
        let (close_agent, close_tab) = (agent.to_owned(), page.id.clone());
        head = head.child(
            div()
                .id(SharedString::from(format!("desk-tab-{}", page.id)))
                .min_w_0()
                .max_w(px(200.))
                .h(px(26.))
                .pl(px(8.))
                .pr(px(2.))
                .flex()
                .items_center()
                .gap(px(4.))
                .rounded(RADIUS_2)
                .when(page.active, |tab| tab.bg(t.surface_chosen))
                .hover(|tab| tab.bg(t.surface_hover))
                .text_color(if page.active {
                    t.text_primary
                } else {
                    t.text_secondary
                })
                .on_click(cx.listener(move |bench, _, _, _| {
                    bench.with_engine(|engine| engine.switch_tab(&switch_agent, &switch_tab))
                }))
                .when(page.acting, |tab| {
                    tab.child(ui::icon(Icon::Pointer, 11.).text_color(t.accent))
                })
                .child(
                    ui::text(label, 12., FontWeight::NORMAL)
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap(),
                )
                .child(
                    IconButton::new(
                        SharedString::from(format!("desk-close-{}", page.id)),
                        Icon::Close,
                        "Close page",
                    )
                    .glyph(10.)
                    .on_click(cx.listener(move |bench, _, _, _| {
                        bench.with_engine(|engine| engine.close_tab(&close_agent, &close_tab))
                    })),
                ),
        );
    }
    let new_agent = agent.to_owned();
    head = head
        .child(
            IconButton::new("desk-new-page", Icon::Plus, "New page")
                .glyph(12.)
                .on_click(cx.listener(move |bench, _, _, _| bench.new_page(&new_agent))),
        )
        .child(div().flex_1())
        .child(close_desk(tab, view, cx));
    let page = bench.desks.pages.get(agent);
    let (back, forward, reload) = (agent.to_owned(), agent.to_owned(), agent.to_owned());
    let bar = div()
        .h(px(34.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .child(
            IconButton::new("desk-back", Icon::Chevron, "Back")
                .glyph(11.)
                .turned(std::f32::consts::PI)
                .disabled(!front.is_some_and(|tab| tab.can_go_back))
                .on_click(cx.listener(move |bench, _, _, _| {
                    bench.with_engine(|engine| engine.history(&back, -1))
                })),
        )
        .child(
            IconButton::new("desk-forward", Icon::Chevron, "Forward")
                .glyph(11.)
                .disabled(!front.is_some_and(|tab| tab.can_go_forward))
                .on_click(cx.listener(move |bench, _, _, _| {
                    bench.with_engine(|engine| engine.history(&forward, 1))
                })),
        )
        .child(
            IconButton::new("desk-reload", Icon::Refresh, "Reload")
                .glyph(12.)
                .on_click(cx.listener(move |bench, _, _, _| {
                    bench.with_engine(|engine| engine.reload(&reload))
                })),
        )
        .children(page.map(|page| {
            div()
                .flex_1()
                .min_w_0()
                .child(Input::new(&page.address).appearance(false))
        }));
    let body = div()
        .flex_1()
        .min_h_0()
        .relative()
        .key_context("BrowserPage")
        .children(page.map(|page| {
            native_view(
                SharedString::from(format!("desk-page-{agent}")),
                &page.surface,
            )
            .track_focus(&page.focus)
            .size_full()
        }))
        .when(strip.is_none_or(|strip| strip.tabs.is_empty()), |body| {
            body.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(t.text_tertiary)
                    .child(ui::text("No pages yet", 12.5, FontWeight::NORMAL)),
            )
        });
    ui::pane(cx)
        .child(
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(head)
                .child(bar)
                .child(body),
        )
        .into_any_element()
}

/// The desk's switch between its pages and its simulator.
fn kinds(agent: &str, shown: DeskKind, cx: &mut Context<Workbench>) -> impl IntoElement {
    let choice = |id: &'static str,
                  icon: Icon,
                  label: &'static str,
                  kind: DeskKind,
                  cx: &mut Context<Workbench>| {
        let agent = agent.to_owned();
        IconButton::new(id, icon, label)
            .glyph(13.)
            .pressed(shown == kind)
            .on_click(cx.listener(move |bench, _, _, cx| {
                bench.show_kind(&agent, kind);
                bench.parts.redraw(&bench.parts.stage, cx);
            }))
    };
    div()
        .flex()
        .items_center()
        .gap(px(2.))
        .pr(px(6.))
        .child(choice(
            "desk-pages",
            Icon::Globe,
            "Pages",
            DeskKind::Pages,
            cx,
        ))
        .child(choice(
            "desk-simulator",
            Icon::Phone,
            "iOS Simulator",
            DeskKind::Simulator,
            cx,
        ))
}

fn close_desk(tab: &str, view: &str, cx: &mut Context<Workbench>) -> impl IntoElement {
    let (tab, view) = (tab.to_owned(), view.to_owned());
    IconButton::new("desk-close", Icon::PanelRight, "Close the desk")
        .glyph(13.)
        .on_click(cx.listener(move |bench, _, _, _| {
            if let Some(strip) = bench.strip() {
                bench.close_pane(&strip, &tab, &view);
            }
        }))
}

fn simulator_desk(
    bench: &Workbench,
    tab: &str,
    view: &str,
    agent: &str,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let t = Tokens::get(cx).clone();
    let head = div()
        .h(px(36.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(4.))
        .px(px(6.))
        .border_b_1()
        .border_color(t.border)
        .child(kinds(agent, DeskKind::Simulator, cx))
        .children(bench.sims.device_name(agent).map(|name| {
            ui::text(name.to_owned(), 12., FontWeight::MEDIUM).text_color(t.text_secondary)
        }))
        .when(bench.sims.acting(agent), |head| {
            head.child(ui::icon(Icon::Pointer, 11.).text_color(t.accent))
        })
        .child(crate::sim_pane::tools(bench, agent, cx))
        .child(div().flex_1())
        .child(close_desk(tab, view, cx));
    ui::pane(cx)
        .child(
            div().size_full().flex().flex_col().child(head).child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(crate::sim_pane::body(bench, agent, cx)),
            ),
        )
        .into_any_element()
}

impl Workbench {
    /// Answers a call the core hands this window: an agent's browser tools run on the
    /// engine, on a thread of their own, since most answers wait on a page.
    pub fn answer_window_call(&self, call_id: CallId, call: WindowCall) {
        let asks = self.asks.clone();
        let answer = move |answer: Result<Value, String>| {
            let _ = asks.send(Ask::AnswerWindow(call_id, WindowAnswer::from(answer)));
        };
        let WindowCall::Harness { request } = call else {
            return answer(Err("this window cannot open that yet".into()));
        };
        let request: HarnessRequest = match serde_json::from_value(request) {
            Ok(request) => request,
            Err(error) => {
                return answer(Err(format!(
                    "Sikemux could not read the tool call: {error}"
                )))
            }
        };
        let run: Box<dyn FnOnce() -> Result<Value, String> + Send> =
            if is_sim_method(&request.method) {
                let engine = Arc::clone(self.sims.engine());
                Box::new(move || sikemux_simulator::tools::execute(&engine, &request))
            } else if let Some(engine) = self
                .desks
                .engine
                .clone()
                .filter(|_| is_browser_method(&request.method))
            {
                Box::new(move || sikemux_browser::tools::execute(&engine, &request))
            } else {
                return answer(Err(format!(
                    "this window cannot answer {} yet",
                    request.method
                )));
            };
        let spawned = std::thread::Builder::new()
            .name("sikemux-window-call".into())
            .spawn(move || answer(run()));
        if let Err(error) = spawned {
            eprintln!("sikemux: a tool call could not start: {error}");
        }
    }
}
