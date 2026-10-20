use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use sikemux_client::client::{ClientEvent, CoreClient};
use sikemux_wire::protocol::{DeviceView, Event};
use tokio::sync::mpsc;

pub enum Update {
    /// The core's release, which the app ships with.
    Connected(String),
    View(Box<DeviceView>),
    Activity(Value),
    Lost(String),
}

/// What the client asks of the core beyond the view it is sent.
pub enum Ask {
    Activity { project: Option<String> },
}

pub struct Link {
    pub updates: mpsc::UnboundedReceiver<Update>,
    pub asks: mpsc::UnboundedSender<Ask>,
}

pub fn socket() -> PathBuf {
    std::env::var_os("SIKEMUX_CORE_SOCKET")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/sikemux/core.sock")))
        .unwrap_or_default()
}

/// Talks to the core on a thread of its own, so the window's thread only draws.
pub fn start(socket: PathBuf) -> Link {
    let (updates_tx, updates) = mpsc::unbounded_channel();
    let (asks, asks_rx) = mpsc::unbounded_channel();
    std::thread::Builder::new()
        .name("core".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime");
            runtime.block_on(run(socket, updates_tx, asks_rx));
        })
        .expect("core thread");
    Link { updates, asks }
}

async fn run(
    socket: PathBuf,
    updates: mpsc::UnboundedSender<Update>,
    mut asks: mpsc::UnboundedReceiver<Ask>,
) {
    let mut project = None;
    loop {
        match CoreClient::connect(&socket).await {
            Ok((client, mut events)) => {
                let client = Arc::new(client);
                let _ = updates.send(Update::Connected(client.hello().build.version.clone()));
                if let Err(error) = client.watch_view().await {
                    let _ = updates.send(Update::Lost(error.to_string()));
                }
                read_activity(&client, &project, &updates);
                loop {
                    tokio::select! {
                        event = events.recv() => match event {
                            Some(ClientEvent::Event(Event::DeviceView { view })) => {
                                let _ = updates.send(Update::View(Box::new(view)));
                            }
                            Some(ClientEvent::Event(Event::Changed { topic, .. })) if topic == "activity" => {
                                read_activity(&client, &project, &updates);
                            }
                            Some(_) => {}
                            None => break,
                        },
                        ask = asks.recv() => match ask {
                            Some(Ask::Activity { project: asked }) => {
                                project = asked;
                                read_activity(&client, &project, &updates);
                            }
                            None => return,
                        },
                    }
                }
                let _ = updates.send(Update::Lost("the core closed the connection".into()));
            }
            Err(error) => {
                let _ = updates.send(Update::Lost(error.to_string()));
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn read_activity(client: &Arc<CoreClient>, project: &Option<String>, updates: &mpsc::UnboundedSender<Update>) {
    let client = client.clone();
    let updates = updates.clone();
    let params = json!({ "utcOffsetMinutes": utc_offset_minutes(), "project": project });
    tokio::spawn(async move {
        if let Ok(summary) = client.call("activitySummary", params).await {
            let _ = updates.send(Update::Activity(summary));
        }
    });
}

fn utc_offset_minutes() -> i32 {
    chrono::Local::now().offset().local_minus_utc() / 60
}
