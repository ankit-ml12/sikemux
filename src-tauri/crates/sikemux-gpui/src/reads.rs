//! What the core's watches last answered, each under a field of its own and decoded once
//! where it arrives, so a screen reads a field rather than looking an answer up by its key.

use serde_json::Value;
use sikemux_plugin_api::{Listed, TokenForm};
use sikemux_wire::catalog::CatalogAgent;

/// The agents the core can run, as its catalog lists them.
#[derive(Default, PartialEq)]
pub struct Catalog(Vec<CatalogAgent>);

impl Catalog {
    pub fn agents(&self) -> impl Iterator<Item = &CatalogAgent> {
        self.0.iter()
    }

    /// The catalog's entry for the agent kind `provider`, such as `claude`.
    pub fn kind(&self, provider: &str) -> Option<&CatalogAgent> {
        self.0.iter().find(|agent| agent.kind == provider)
    }

    /// What the agent kind is called, such as "Claude", or its id until the catalog is read.
    pub fn label<'a>(&'a self, provider: &'a str) -> &'a str {
        self.kind(provider).map_or(provider, |agent| &agent.label)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The plugins built in, as the core last listed them.
#[derive(Default, PartialEq)]
pub struct Plugins(Vec<Listed>);

impl Plugins {
    pub fn all(&self) -> impl Iterator<Item = &Listed> {
        self.0.iter()
    }

    /// Those switched on.
    pub fn on(&self) -> impl Iterator<Item = &Listed> {
        self.0.iter().filter(|plugin| plugin.enabled)
    }

    /// Whether the person is signed in to `id`; none for a plugin with no sign-in.
    pub fn signed_in(&self, id: &str) -> Option<bool> {
        self.0.iter().find(|plugin| plugin.id == id)?.signed_in
    }

    /// The form `id` is signed in to with a token, when it has one.
    pub fn token_form(&self, id: &str) -> Option<&TokenForm> {
        self.0
            .iter()
            .find(|plugin| plugin.id == id)?
            .token_form
            .as_ref()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Default)]
pub struct Reads {
    pub catalog: Catalog,
    pub plugins: Plugins,
    pub hosts: Option<Value>,
    pub update: Option<Value>,
    pub usage: Option<Value>,
    pub shares: Option<Value>,
    pub stash: Option<Value>,
    pub themes: Option<Value>,
    /// This host's trust questions, and those of the account host in front.
    pub trust: Option<Value>,
    pub host_trust: Option<Value>,
    /// The composer drafts kept on this host, and on the account host in front.
    pub drafts: Option<Value>,
    pub host_drafts: Option<Value>,
    /// The CI runs of the branch in front, and of the commit its newest run is for.
    pub ci: Option<Value>,
    pub ci_head: Option<Value>,
    /// What the project in front holds: its tasks, their runs, what listens on its ports,
    /// its activity, and the repository its git pane shows.
    pub tasks: Option<Value>,
    pub runs: Option<Value>,
    pub ports: Option<Value>,
    pub activity: Option<Value>,
    pub git_project_repo: Option<Value>,
    pub git_overview: Option<Value>,
    pub git_draft: Option<Value>,
    /// Activity across every project, for Settings; null when the core refused it.
    pub activity_everywhere: Option<Value>,
    /// The worktree in front's pull requests, and whether its branch is merged.
    pub worktree_pulls: Option<Value>,
    pub worktree_merged: Option<Value>,
    /// What the project and host pickers offer: folders under the project roots, and SSH hosts.
    pub picker_folders: Option<Value>,
    pub ssh_hosts: Option<Value>,
}

/// Which field keeps the answer to each watch, by the watch's key.
macro_rules! slots {
    ($($key:pat => $field:ident),* $(,)?) => {
        impl Reads {
            fn slot(&self, key: &str) -> Option<&Option<Value>> {
                Some(match key {
                    $($key => &self.$field,)*
                    _ => return None,
                })
            }

            fn slot_mut(&mut self, key: &str) -> Option<&mut Option<Value>> {
                Some(match key {
                    $($key => &mut self.$field,)*
                    _ => return None,
                })
            }
        }
    };
}

slots! {
    "hosts" => hosts,
    "update" => update,
    "usage" => usage,
    "shares" => shares,
    "stash" => stash,
    "themes" => themes,
    "trust" => trust,
    "host-trust" => host_trust,
    "drafts" => drafts,
    "host-drafts" => host_drafts,
    "ci" => ci,
    "ci-head" => ci_head,
    "tasks" => tasks,
    "runs" => runs,
    "ports" => ports,
    "activity" => activity,
    "git-project-repo" => git_project_repo,
    "git-overview" => git_overview,
    "git-draft" => git_draft,
    crate::settings::ACTIVITY => activity_everywhere,
    crate::worktree_head::PULLS => worktree_pulls,
    crate::worktree_head::MERGED => worktree_merged,
    "pickerFolders" => picker_folders,
    "sshHosts" => ssh_hosts,
}

impl Reads {
    /// Keeps the agent catalog the core sent, and says whether it differs from the one kept.
    pub fn keep_catalog(&mut self, value: Value) -> bool {
        match serde_json::from_value::<Vec<CatalogAgent>>(value) {
            Ok(agents) if agents == self.catalog.0 => false,
            Ok(agents) => {
                self.catalog = Catalog(agents);
                true
            }
            Err(error) => {
                eprintln!("sikemux: the core's agent catalog does not read: {error}");
                false
            }
        }
    }

    /// Keeps the plugins the core listed, and says whether they differ from those kept.
    pub fn keep_plugins(&mut self, value: Value) -> bool {
        match serde_json::from_value::<Vec<Listed>>(value) {
            Ok(plugins) if plugins == self.plugins.0 => false,
            Ok(plugins) => {
                self.plugins = Plugins(plugins);
                true
            }
            Err(error) => {
                eprintln!("sikemux: the core's plugins do not read: {error}");
                false
            }
        }
    }

    /// Keeps what `key`'s watch answered. Gives the answer back when no field holds `key`.
    pub fn keep(&mut self, key: &str, value: Value) -> Result<(), Value> {
        match self.slot_mut(key) {
            Some(slot) => {
                *slot = Some(value);
                Ok(())
            }
            None => Err(value),
        }
    }

    /// Whether `key` already holds `value`, so a push carrying it changes nothing.
    pub fn holds(&self, key: &str, value: &Value) -> bool {
        self.slot(key)
            .is_some_and(|slot| slot.as_ref() == Some(value))
    }

    /// Forgets what `key` answered, so nothing shows the answer of what it watched before.
    pub fn forget(&mut self, key: &str) {
        if let Some(slot) = self.slot_mut(key) {
            *slot = None;
        }
    }
}
