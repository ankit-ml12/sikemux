//! The host the person is on (ADR 0018): this one, another host on the account, or an SSH host
//! without Sikemux. The rail, the top bar and the tab list follow the one chosen.

use std::path::Path;
use std::time::Duration;

use gpui_kit::component::notification::Notification;
use gpui_kit::{AppContext, Context, SharedString, WeakEntity, Window};
use serde_json::{json, Value};

use crate::core;
use crate::workbench::{Strip, Workbench};

/// How often a reached host's load is read again while the window is in front; the core sends
/// no news of it.
const LOAD_EVERY: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum HostChoice {
    #[default]
    This,
    /// Another host on the account, by its core's key.
    Account(String),
    /// An alias from ~/.ssh/config that no host on the account was installed on.
    Ssh(String),
}

impl HostChoice {
    /// The same across launches: "this", "host:<key>" or "ssh:<alias>".
    pub fn key(&self) -> SharedString {
        match self {
            HostChoice::This => "this".into(),
            HostChoice::Account(key) => format!("host:{key}").into(),
            HostChoice::Ssh(alias) => format!("ssh:{alias}").into(),
        }
    }
}

/// Whose core a pane talks to: this host's, or another host's on the account. An SSH host
/// without Sikemux has no core, so its strip is this host's.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Place {
    #[default]
    This,
    Account(String),
}

pub struct HostOption {
    pub choice: HostChoice,
    pub label: SharedString,
}

impl Workbench {
    /// This host, then the account's hosts in the order they were linked, then the SSH hosts
    /// no account host was installed on, as the core lists them.
    pub fn host_choices(&self) -> Vec<HostOption> {
        let listed = self
            .read("hosts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|host| {
                let choice = match (host["id"].as_str(), host["ssh"].as_str()) {
                    (Some(key), _) => HostChoice::Account(key.to_owned()),
                    (None, Some(alias)) => HostChoice::Ssh(alias.to_owned()),
                    (None, None) => return None,
                };
                let label = host["name"].as_str().unwrap_or_default().to_owned();
                Some(HostOption {
                    choice,
                    label: label.into(),
                })
            });
        std::iter::once(HostOption {
            choice: HostChoice::This,
            label: "This host".into(),
        })
        .chain(listed)
        .collect()
    }

    /// The name of the account host `host`, as the core lists it.
    pub fn host_name(&self, host: &str) -> SharedString {
        self.host_choices()
            .into_iter()
            .find(|option| option.choice == HostChoice::Account(host.to_owned()))
            .map_or_else(|| "another host".into(), |option| option.label)
    }

    /// The host in front, as the core's layout says.
    pub fn chosen_host(&self) -> HostChoice {
        if let Some(key) = self
            .view
            .as_ref()
            .and_then(|view| view.layout.chosen_host.clone())
        {
            return HostChoice::Account(key);
        }
        match self.strip() {
            Some(Strip::Host(alias)) => HostChoice::Ssh(alias),
            _ => HostChoice::This,
        }
    }

    /// Whose core the panes in front talk to.
    pub fn place(&self) -> Place {
        match self.chosen_host() {
            HostChoice::Account(key) => Place::Account(key),
            _ => Place::This,
        }
    }

    /// Opens the project at `path` on the account host `host`, which clones it from its remote
    /// or takes it from this host as a bundle, then puts the person on that host.
    pub fn open_on(
        &mut self,
        path: &Path,
        host: &str,
        label: SharedString,
        cx: &mut Context<Self>,
    ) {
        let (asks, host) = (self.asks.clone(), host.to_owned());
        let params = json!({ "path": path, "host": host });
        self.notify(Notification::info(format!("Opening on {label}…")), cx);
        cx.spawn(async move |bench, cx| {
            let opened = core::request(&asks, "openOnHost", params).await;
            let _ = bench.update(cx, |bench, cx| match opened {
                Ok(_) => bench.choose_host(HostChoice::Account(host), cx),
                Err(error) => bench.notify(
                    Notification::error(format!("It did not open on {label}: {error}")),
                    cx,
                ),
            });
        })
        .detach();
    }

    /// Installs Sikemux on the SSH host `alias` over the person's own ssh, in a terminal of its
    /// strip, and signs it in to the account; its row says how it goes.
    pub fn install_host(&mut self, alias: &str, cx: &mut Context<Self>) {
        let (asks, alias) = (self.asks.clone(), alias.to_owned());
        let params = json!({ "alias": alias, "work": "install" });
        cx.spawn(async move |bench, cx| {
            if let Err(error) = core::request(&asks, "installHost", params).await {
                let _ = bench.update(cx, |bench, cx| {
                    bench.notify(
                        Notification::error(format!(
                            "Sikemux was not installed on {alias}: {error}"
                        )),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    /// Puts the person on `choice`, which the rail and the tab list then follow.
    pub fn choose_host(&mut self, choice: HostChoice, cx: &mut Context<Self>) {
        self.call("chooseHost", json!({ "host": choice.key() }));
        cx.notify();
    }
}

/// Keeps each reached host's load fresh while the window is in front.
pub fn watch_loads(window: &mut Window, cx: &mut Context<Workbench>) {
    cx.spawn_in(window, async move |bench, cx| loop {
        let Ok(hosts) = bench.update_in(cx, |bench, window, _| {
            if window.is_window_active() {
                bench.hosts_reached.iter().cloned().collect()
            } else {
                Vec::new()
            }
        }) else {
            break;
        };
        for host in hosts {
            read_load(&bench, host, cx).await;
        }
        cx.background_executor().timer(LOAD_EVERY).await;
    })
    .detach();
}

/// Reads how busy `host` is now, for its row.
pub fn read_load_now(host: String, cx: &mut Context<Workbench>) {
    cx.spawn(async move |bench, cx| read_load(&bench, host, cx).await)
        .detach();
}

async fn read_load(bench: &WeakEntity<Workbench>, host: String, cx: &mut impl AppContext) {
    let Ok(asks) = bench.update(cx, |bench, _| bench.asks.clone()) else {
        return;
    };
    let Ok(load) = core::request_on(&asks, &host, "hostLoad", Value::Null).await else {
        return;
    };
    let cpu = load["cpu"].as_u64().unwrap_or_default();
    let label = match load["memoryUsed"].as_u64() {
        Some(bytes) => format!("{cpu}% · {:.1} GB", bytes as f64 / 1024f64.powi(3)),
        None => format!("{cpu}%"),
    };
    let _ = bench.update(cx, |bench, cx| {
        if bench.hosts_reached.contains(&host) {
            bench.host_loads.insert(host, label.into());
            bench.parts.redraw(&bench.parts.rail, cx);
        }
    });
}
