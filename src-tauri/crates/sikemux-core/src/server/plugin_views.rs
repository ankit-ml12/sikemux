//! What plugins' panes show, read and kept by the core. A plugin's manifest
//! names its views and how often each is read again while a pane shows it,
//! and its writes, after which its views are read again at once. Clients are
//! told when a view changed, so none keeps time of its own.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::changed::Topic;
use super::connection::ClientId;
use super::plugins::{self, Category, Failure, Outcome};
use super::shown_cache::{self, ShownCache, Shows};
use super::Core;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    plugin: String,
    method: String,
    /// The view's parameters as JSON, so equal ones share it.
    params: String,
}

impl Key {
    fn path(&self) -> String {
        format!("{}/{}", self.plugin, self.method)
    }
}

#[derive(Default)]
pub(crate) struct Views(ShownCache<Shown>);

/// The views panes show, each kept for the slot it is shown in.
struct Shown;

impl Shows for Shown {
    type Key = Key;
    type Slot = String;
    /// How often the manifest asks for the view to be read again.
    type With = Duration;
    const LONGEST_BACKOFF: Duration = Duration::from_secs(300);

    fn cache(core: &Core) -> &ShownCache<Self> {
        &core.plugin_views.0
    }

    async fn read(
        _core: Arc<Core>,
        key: Key,
        every: Duration,
    ) -> Result<(Value, Duration), Failure> {
        let params = serde_json::from_str(&key.params).unwrap_or_default();
        let answer = plugins::call(&key.plugin, &key.method, params).await?;
        let pace = pace(&answer, every);
        Ok((answer, pace))
    }

    fn moved(core: &Core, moved: Vec<(Key, Vec<String>)>) {
        let paths: HashSet<String> = moved.iter().map(|(key, _)| key.path()).collect();
        core.changes
            .changed(Topic::PluginView, None, Some(paths.into_iter().collect()));
    }
}

/// A view of a plugin. `slot` names where the client shows it, such as a pane,
/// so two panes showing two of the same plugin's views each keep theirs.
#[derive(Deserialize)]
pub(crate) struct ViewParams {
    plugin: String,
    method: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    slot: Option<String>,
}

/// What the plugin's manifest says of its views and writes.
fn manifest(plugin: &str) -> Option<Value> {
    plugins::manifests()
        .into_iter()
        .find(|manifest| manifest["id"] == plugin)
}

fn every(plugin: &str, method: &str) -> Option<Duration> {
    let secs = manifest(plugin)?["views"][method]["everySecs"].as_u64()?;
    Some(Duration::from_secs(secs.max(1)))
}

pub(crate) async fn view(core: &Arc<Core>, client: ClientId, asked: ViewParams) -> Outcome {
    let Some(every) = every(&asked.plugin, &asked.method) else {
        return Outcome::Failed(Failure::new(
            &asked.plugin,
            Category::Invalid,
            format!("{} is not a view of {}", asked.method, asked.plugin),
        ));
    };
    let slot = asked
        .slot
        .unwrap_or_else(|| format!("{}/{}", asked.plugin, asked.method));
    let key = Key {
        plugin: asked.plugin,
        method: asked.method,
        params: asked.params.to_string(),
    };
    if let Some(answer) = core.plugin_views.0.show(client, slot, &key) {
        return Outcome::Value(answer);
    }
    match plugins::call(&key.plugin, &key.method, asked.params).await {
        Ok(answer) => {
            let pace = pace(&answer, every);
            shown_cache::keep::<Shown>(core, key, answer.clone(), pace, every);
            Outcome::Value(answer)
        }
        Err(failure) => Outcome::Failed(failure),
    }
}

#[derive(Deserialize)]
pub(crate) struct AwayParams {
    slot: String,
}

/// The client no longer shows `slot` or any view under it, so they stop being read again for
/// it until it asks for one there.
pub(crate) fn away(core: &Core, client: ClientId, asked: AwayParams) {
    let under = format!("{}/", asked.slot);
    core.plugin_views.0.away(client, |slot| {
        *slot == asked.slot || slot.starts_with(&under)
    });
}

/// How long `answer` stays fresh: the manifest's pace, or sooner when the
/// view asks for it, such as while a run it shows goes on.
fn pace(answer: &Value, every: Duration) -> Duration {
    answer["readAgainSecs"]
        .as_u64()
        .map_or(every, |secs| Duration::from_secs(secs.max(1)).min(every))
}

/// The plugin `plugin` ran `method`; when the manifest names it as a write,
/// the plugin's views are read again at once, and again once the service has
/// settled.
pub(crate) fn wrote(core: &Arc<Core>, plugin: &str, method: &str) {
    let writes = manifest(plugin).is_some_and(|manifest| {
        manifest["writes"]
            .as_array()
            .is_some_and(|writes| writes.iter().any(|write| write == method))
    });
    if !writes {
        return;
    }
    let keys = core.plugin_views.0.keys(|key| key.plugin == plugin);
    shown_cache::read_again_settled::<Shown>(core, keys);
}

/// The views of `plugin` to read again now: each of `method`'s, or all.
#[derive(Deserialize)]
pub(crate) struct ReadAgain {
    plugin: String,
    #[serde(default)]
    method: Option<String>,
}

/// The person asked for a plugin's views to be read again, as after a change
/// made outside Sikemux; those that changed are sent.
pub(crate) fn read_again(core: &Arc<Core>, asked: ReadAgain) {
    let keys = core.plugin_views.0.keys(|key| {
        key.plugin == asked.plugin
            && asked
                .method
                .as_ref()
                .is_none_or(|method| *method == key.method)
    });
    shown_cache::read_again::<Shown>(core, keys);
}

/// An agent ran the tool `tool`, which may be one of a plugin's writes.
pub(crate) fn tool_ran(core: &Arc<Core>, tool: &str) {
    let found = plugins::manifests().into_iter().find_map(|manifest| {
        let method = manifest["tools"]
            .as_array()?
            .iter()
            .find(|each| each["name"] == tool)?["method"]
            .as_str()?
            .to_owned();
        Some((manifest["id"].as_str()?.to_owned(), method))
    });
    if let Some((plugin, method)) = found {
        wrote(core, &plugin, &method);
    }
}

/// The person signed in to or out of `plugin`, so what its views showed is
/// forgotten and read afresh.
pub(crate) fn signed(core: &Core, plugin: &str) {
    let forgot: HashSet<String> = core
        .plugin_views
        .0
        .forget(|key| key.plugin == plugin)
        .iter()
        .map(Key::path)
        .collect();
    if !forgot.is_empty() {
        core.changes
            .changed(Topic::PluginView, None, Some(forgot.into_iter().collect()));
    }
}
