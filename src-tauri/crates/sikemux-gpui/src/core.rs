use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use sikemux_client::client::{Attached, ClientError, ClientEvent, CoreClient};
use sikemux_host::core_start::{self, StartError};
use sikemux_host::login_item::{self, CoreLaunch};
use sikemux_host::cli_paths;
use sikemux_wire::grid::{decode_frame, decode_lines, FindOptions, FindResult, Frame, GridPoint, Lines, TerminalInput};
use sikemux_wire::protocol::{ChatAttachment, ChatMark, DeviceView, Event, Request, SessionId, StreamId};
use tokio::sync::mpsc;

pub enum Update {
    /// The core's release, which the app ships with.
    Connected(String),
    View(Box<DeviceView>),
    /// A watched call's latest answer, under the key it was watched as.
    Read(&'static str, Value),
    /// The core turned down what was asked under the key.
    Refused(&'static str, String),
    Terminal(SessionId, Screenful),
    Lost(String),
    /// What came of a chat the window attached to, by its agent's id.
    Chat(String, ChatNews),
    /// The core said something under a followed topic moved; no paths means anything may have.
    Changed { topic: String, project: Option<String>, paths: Option<Vec<String>> },
}

pub enum ChatNews {
    Attached(ChatAttachment),
    /// The chat's changes numbered `seq`, a list of `ChatChange` as JSON.
    Changes { seq: u64, changes: Value },
    /// Earlier rows, a `ChatPage` as JSON, or null when the core had none to give.
    Page(Value),
    /// A `ToolDetail` as JSON.
    Detail { tool: String, detail: Value },
}

/// What an attached terminal sends, decoded on the core's thread.
pub enum Screenful {
    Frame(Frame),
    Lines(Lines),
    Copied(String),
    Found(FindResult),
}

/// What the window asks of one terminal.
pub enum TerminalAsk {
    /// Starts its frames, the first a whole screen.
    Attach,
    Detach,
    Input(TerminalInput),
    /// The window drew this frame, so the core may send the next.
    Ack(u32),
    Resize(u16, u16),
    Lines { start: u64, count: u32 },
    /// Its text between two cells, or all of it, sent back as `Screenful::Copied`.
    Copy(Option<(GridPoint, GridPoint)>),
    /// Matches of `query` in the screen and history, the current one the
    /// first after `from`, or before it going `backwards`. Sent back as `Screenful::Found`.
    Find { query: String, options: FindOptions, from: Option<GridPoint>, backwards: bool, inclusive: bool },
    Clear,
}

/// One of the core's `Watch` calls, read again whenever the core says one of
/// its topics changed.
#[derive(Clone)]
pub struct Watch {
    pub method: &'static str,
    pub params: Value,
    pub topics: &'static [&'static str],
    /// Counts the watches made under one key, so an answer to one it replaced is dropped.
    generation: u64,
}

impl Watch {
    pub fn new(method: &'static str, params: Value, topics: &'static [&'static str]) -> Self {
        Self { method, params, topics, generation: 0 }
    }
}

/// What the client asks of the core beyond the view it is sent.
pub enum Ask {
    /// Keeps `key` answered by `watch`, in place of whatever it watched before.
    Watch(&'static str, Watch),
    /// One of the core's methods, for something the person did. What it changes
    /// comes back with the next view.
    Call { method: &'static str, params: Value },
    /// A call whose answer the window waits for, sent back under `key`.
    Answer { key: &'static str, method: &'static str, params: Value },
    Terminal(SessionId, TerminalAsk),
    Chat(ChatAsk),
    /// Hands the window the core's changes under `topic`, with what moved.
    Follow(&'static str),
}

pub enum ChatAsk {
    /// Starts hearing a chat's changes, from `since` when the window held it before.
    Attach { agent: String, since: Option<ChatMark> },
    Detach { agent: String },
    History { agent: String, feed: String, before: u64, turns: u32 },
    Prompt { agent: String, text: String },
    Cancel { agent: String },
    /// The person's answer to a tool's request for permission; none turns it down.
    Permission { agent: String, request: String, option: Option<String> },
    Mode { agent: String, mode: String },
    /// One of the session's settings, such as its model, set to one of its choices.
    Config { agent: String, config: String, value: String },
    /// What a tool call changed and printed, which its row opens on.
    Detail { agent: String, tool: String },
}

pub struct Link {
    pub updates: mpsc::UnboundedReceiver<Update>,
    pub asks: mpsc::UnboundedSender<Ask>,
}

/// The app's own identifier, so its folders and its core's login item are never the Mac app's.
pub const IDENTIFIER: &str = if cfg!(debug_assertions) { "com.nodelike.sikemux.gpui.dev" } else { "com.nodelike.sikemux.gpui" };

/// Where this build's core listens, and how it is started when none does.
pub struct Home {
    pub socket: PathBuf,
    launch: Option<CoreLaunch>,
    manifest_dir: Option<PathBuf>,
}

impl Home {
    pub fn new() -> Self {
        let library = sikemux_process::paths::home().map(|home| home.join("Library"));
        let support = library.as_ref().map(|library| library.join("Application Support").join(IDENTIFIER));
        let socket = sikemux_core::default_socket_path().unwrap_or_default();
        let launch = cli_paths::cli_executable_path().map(|binary| CoreLaunch {
            binary,
            socket: socket.clone(),
            log: library.map_or_else(std::env::temp_dir, |library| library.join("Logs").join(IDENTIFIER)).join("core.log"),
            args: login_item::core_args(support.clone()),
        });
        Self { socket, launch, manifest_dir: support.map(|support| support.join("agent-detection")) }
    }
}

/// Talks to the core on a thread of its own, so the window's thread only draws.
pub fn start(home: Home) -> Link {
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (asks, asks_rx) = mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("core".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            runtime.block_on(run(home, updates_tx, asks_rx));
        })
        .expect("core thread");
    Link { updates, asks }
}

async fn run(home: Home, updates: mpsc::UnboundedSender<Update>, mut asks: mpsc::UnboundedReceiver<Ask>) {
    let mut watches: HashMap<&'static str, Watch> = HashMap::new();
    let mut followed: Vec<&'static str> = Vec::new();
    let current = Arc::new(Mutex::new(HashMap::<&'static str, u64>::new()));
    loop {
        if let Some(launch) = &home.launch {
            match core_start::start(IDENTIFIER, launch, &sikemux_host::build_identity(), false).await {
                Ok(()) | Err(StartError::Unreachable(_)) => {}
                Err(StartError::Incompatible { reason, .. }) => {
                    eprintln!("sikemux: the core cannot be used: {reason}");
                    let _ = updates.send(Update::Lost(reason));
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            }
        }
        match CoreClient::connect(&home.socket).await {
            Ok((client, mut events)) => {
                let client = Arc::new(client);
                let _ = updates.send(Update::Connected(client.hello().build.version.clone()));
                if let Err(error) = client.configure(home.manifest_dir.clone()).await {
                    eprintln!("sikemux: the core could not read the agent manifests: {error}");
                }
                if let Err(error) = client.watch_view().await {
                    let _ = updates.send(Update::Lost(error.to_string()));
                }
                for (key, watch) in &watches {
                    read(&client, key, watch, &current, &updates);
                }
                let mut streams = Streams::new(&client, &updates);
                loop {
                    tokio::select! {
                        settled = streams.settled.recv() => {
                            if let Some(settled) = settled {
                                streams.settle(settled);
                            }
                        }
                        event = events.recv() => match event {
                            Some(ClientEvent::Grid { stream, frame }) => streams.frame(stream, frame),
                            Some(ClientEvent::Event(Event::DeviceView { view })) => {
                                let _ = updates.send(Update::View(Box::new(view)));
                            }
                            Some(ClientEvent::Event(Event::Chat { agent_id, seq, changes })) => {
                                let _ = updates.send(Update::Chat(agent_id, ChatNews::Changes { seq, changes }));
                            }
                            Some(ClientEvent::Event(Event::Changed { topic, project, paths })) => {
                                for (key, watch) in watches.iter().filter(|(_, watch)| watch.topics.contains(&topic.as_str())) {
                                    read(&client, key, watch, &current, &updates);
                                }
                                if followed.contains(&topic.as_str()) {
                                    let _ = updates.send(Update::Changed { topic, project, paths });
                                }
                            }
                            Some(_) => {}
                            None => break,
                        },
                        ask = asks.recv() => match ask {
                            Some(Ask::Watch(key, mut watch)) => {
                                watch.generation = watches.get(key).map_or(0, |old| old.generation + 1);
                                current.lock().expect("watch generations").insert(key, watch.generation);
                                read(&client, key, &watch, &current, &updates);
                                watches.insert(key, watch);
                            }
                            Some(Ask::Follow(topic)) => {
                                if !followed.contains(&topic) {
                                    followed.push(topic);
                                }
                            }
                            Some(Ask::Answer { key, method, params }) => {
                                let (client, updates) = (client.clone(), updates.clone());
                                tokio::spawn(async move {
                                    let _ = updates.send(match client.call(method, params).await {
                                        Ok(value) => Update::Read(key, value),
                                        Err(error) => Update::Refused(key, error.to_string()),
                                    });
                                });
                            }
                            Some(Ask::Chat(ask)) => {
                                let (client, updates) = (client.clone(), updates.clone());
                                tokio::spawn(async move { chat(&client, ask, &updates).await });
                            }
                            Some(Ask::Call { method, params }) => {
                                let client = client.clone();
                                tokio::spawn(async move {
                                    if let Err(error) = client.call(method, params).await {
                                        eprintln!("sikemux: the core refused {method}: {error}");
                                    }
                                });
                            }
                            Some(Ask::Terminal(session, ask)) => streams.ask(session, ask),
                            None => return,
                        },
                    }
                }
                let _ = updates.send(Update::Lost("the core closed the connection".into()));
            }
            Err(error) => {
                eprintln!("sikemux: no core answers: {error}");
                let _ = updates.send(Update::Lost(error.to_string()));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn chat(client: &CoreClient, ask: ChatAsk, updates: &mpsc::UnboundedSender<Update>) {
    let (agent, news) = match ask {
        ChatAsk::Attach { agent, since } => match client.acp_attach_since(agent.clone(), since).await {
            Ok(attachment) => (agent, ChatNews::Attached(attachment)),
            Err(error) => return eprintln!("sikemux: the core refused to attach chat {agent}: {error}"),
        },
        ChatAsk::History { agent, feed, before, turns } => {
            let page = client.acp_history(agent.clone(), feed, before, turns).await;
            (agent, ChatNews::Page(page.unwrap_or_default()))
        }
        ChatAsk::Detach { agent } => return report("detach", client.acp_detach(agent).await),
        ChatAsk::Prompt { agent, text } => return report("prompt", client.acp_prompt(agent, None, text, Vec::new(), Vec::new()).await),
        ChatAsk::Cancel { agent } => return report("stop", client.acp_cancel(agent).await),
        ChatAsk::Permission { agent, request, option } => {
            return report("answer", client.acp_permission_reply(agent, request, option).await)
        }
        ChatAsk::Config { agent, config, value } => {
            return report("change the settings of", client.acp_set_config(agent, config, value).await.map(drop))
        }
        ChatAsk::Detail { agent, tool } => {
            let asked = json!({ "agentId": agent, "toolCallId": tool });
            match client.call("chatToolDetail", asked).await {
                Ok(detail) => (agent, ChatNews::Detail { tool, detail }),
                Err(error) => return eprintln!("sikemux: the core refused a tool's detail: {error}"),
            }
        }
        ChatAsk::Mode { agent, mode } => return report("switch the mode of", client.acp_set_permission_mode(agent, mode).await),
    };
    let _ = updates.send(Update::Chat(agent, news));
}

fn report(what: &str, done: Result<(), ClientError>) {
    if let Err(error) = done {
        eprintln!("sikemux: the core refused to {what} a chat: {error}");
    }
}

enum Settled {
    Attached(SessionId, Attached),
    Detached(StreamId),
}

/// The terminals attached on one connection: their streams, and frames that
/// arrived before the attach that started them was answered.
struct Streams {
    client: Arc<CoreClient>,
    updates: mpsc::UnboundedSender<Update>,
    sessions: HashMap<StreamId, SessionId>,
    streams: HashMap<SessionId, StreamId>,
    early: HashMap<StreamId, Vec<Vec<u8>>>,
    settle_tx: mpsc::UnboundedSender<Settled>,
    settled: mpsc::UnboundedReceiver<Settled>,
}

impl Streams {
    fn new(client: &Arc<CoreClient>, updates: &mpsc::UnboundedSender<Update>) -> Self {
        let (settle_tx, settled) = mpsc::unbounded_channel();
        Self {
            client: client.clone(),
            updates: updates.clone(),
            sessions: HashMap::new(),
            streams: HashMap::new(),
            early: HashMap::new(),
            settle_tx,
            settled,
        }
    }

    fn send(&self, session: SessionId, screenful: Screenful) {
        let _ = self.updates.send(Update::Terminal(session, screenful));
    }

    fn frame(&mut self, stream: StreamId, bytes: Vec<u8>) {
        match self.sessions.get(&stream) {
            Some(&session) => {
                if let Some(frame) = decode_frame(&bytes) {
                    self.send(session, Screenful::Frame(frame));
                }
            }
            None => self.early.entry(stream).or_default().push(bytes),
        }
    }

    fn settle(&mut self, settled: Settled) {
        match settled {
            Settled::Attached(session, attached) => {
                self.sessions.insert(attached.stream, session);
                self.streams.insert(session, attached.stream);
                self.frame(attached.stream, attached.frame);
                for bytes in self.early.remove(&attached.stream).unwrap_or_default() {
                    self.frame(attached.stream, bytes);
                }
            }
            Settled::Detached(stream) => {
                self.sessions.remove(&stream);
                self.early.remove(&stream);
            }
        }
    }

    fn ask(&mut self, session: SessionId, ask: TerminalAsk) {
        let client = self.client.clone();
        let stream = self.streams.get(&session).copied();
        match ask {
            TerminalAsk::Attach => {
                let settle = self.settle_tx.clone();
                tokio::spawn(async move {
                    match client.attach(session).await {
                        Ok(attached) => drop(settle.send(Settled::Attached(session, attached))),
                        Err(error) => eprintln!("sikemux: terminal {session} would not attach: {error}"),
                    }
                });
            }
            TerminalAsk::Detach => {
                let Some(stream) = self.streams.remove(&session) else {
                    return;
                };
                let settle = self.settle_tx.clone();
                tokio::spawn(async move {
                    let _ = client.detach(stream).await;
                    let _ = settle.send(Settled::Detached(stream));
                });
            }
            TerminalAsk::Ack(frame) => {
                if let Some(stream) = stream {
                    client.ack(stream, frame);
                }
            }
            TerminalAsk::Input(input) => drop(tokio::spawn(async move { client.input(session, input).await })),
            TerminalAsk::Resize(cols, rows) => drop(tokio::spawn(async move { client.resize(session, cols, rows).await })),
            TerminalAsk::Lines { start, count } => {
                let updates = self.updates.clone();
                tokio::spawn(async move {
                    if let Some(lines) = client.lines(session, start, count).await.ok().and_then(|bytes| decode_lines(&bytes)) {
                        let _ = updates.send(Update::Terminal(session, Screenful::Lines(lines)));
                    }
                });
            }
            TerminalAsk::Copy(range) => {
                let updates = self.updates.clone();
                tokio::spawn(async move {
                    if let Ok(text) = client.text(session, range).await {
                        let _ = updates.send(Update::Terminal(session, Screenful::Copied(text)));
                    }
                });
            }
            TerminalAsk::Find { query, options, from, backwards, inclusive } => {
                let updates = self.updates.clone();
                let request = Request::TerminalFind { id: session, query, options, from, backwards, inclusive };
                tokio::spawn(async move {
                    let found = client.find(request).await.unwrap_or_default();
                    let _ = updates.send(Update::Terminal(session, Screenful::Found(found)));
                });
            }
            TerminalAsk::Clear => drop(tokio::spawn(async move { client.clear(session).await })),
        }
    }
}

fn read(
    client: &Arc<CoreClient>,
    key: &'static str,
    watch: &Watch,
    current: &Arc<Mutex<HashMap<&'static str, u64>>>,
    updates: &mpsc::UnboundedSender<Update>,
) {
    let (client, updates, watch, current) = (client.clone(), updates.clone(), watch.clone(), current.clone());
    tokio::spawn(async move {
        let value = match client.call(watch.method, watch.params).await {
            Ok(value) => value,
            Err(error) => return eprintln!("sikemux: the core refused {}: {error}", watch.method),
        };
        if current.lock().expect("watch generations").get(key) == Some(&watch.generation) {
            let _ = updates.send(Update::Read(key, value));
        }
    });
}

pub fn utc_offset_minutes() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}
