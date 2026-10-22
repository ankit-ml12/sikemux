//! The host the person is on (ADR 0018): this one, another host on the account, or an SSH host
//! without Sikemux. The rail, the top bar and the tab list follow the one chosen.

use gpui_kit::{Context, SharedString};
use serde_json::Value;

use crate::workbench::{Strip, Workbench};

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

    /// The host in front, as the core's layout says.
    pub fn chosen_host(&self) -> HostChoice {
        match self.strip() {
            Some(Strip::Host(alias)) => HostChoice::Ssh(alias),
            _ => HostChoice::This,
        }
    }

    /// Puts the person on `choice`, which the rail and the tab list then follow.
    pub fn choose_host(&mut self, choice: HostChoice, cx: &mut Context<Self>) {
        match choice {
            HostChoice::This => {
                if let Some(project) = self.project().map(|project| project.id.clone()) {
                    self.activate_project(&project);
                }
            }
            HostChoice::Ssh(alias) => {
                let strip = Strip::Host(alias.clone());
                match self.front_tab_of(&strip) {
                    Some(tab) => self.activate_tab(&strip, &tab),
                    None => self.open_ssh_terminal(&alias),
                }
            }
            HostChoice::Account(_) => {}
        }
        cx.notify();
    }
}
