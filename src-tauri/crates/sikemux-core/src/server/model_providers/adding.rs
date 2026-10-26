//! Adding a provider from what the person pastes. A provider in the catalogue
//! is found by its URL and connected; any other is checked against its model
//! list, asked which other APIs it serves, and kept as one the person added.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::launch::providers::{self, Custom, Kind, Provider, Providers};

use super::paste::{self, Pasted};
use super::{
    check_key, client, entry, listed_models, model_list, on_store, ModelProvider, ProviderParams,
    Unlisted, CHECK_TIMEOUT, RUNNER_TIMEOUT,
};
use crate::server::{Core, CoreResult};

const MAX_PASTE_BYTES: usize = 64 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
pub(crate) struct ReadParams {
    text: String,
}

#[derive(Deserialize)]
pub(crate) struct AddParams {
    text: String,
    /// The name the person gave it, in place of the one read from its URL.
    #[serde(default)]
    label: Option<String>,
}

/// What a paste holds, checked against the provider it names. Never the key.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Reading {
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    has_key: bool,
    /// The catalogue's provider at that URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    known_id: Option<String>,
    speaks: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runs_with: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    models: Option<Vec<String>>,
    /// Why it cannot be added as it stands.
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<String>,
}

/// The provider a paste names, the key it came with or was saved with, and
/// what the provider listed with it.
struct Examined {
    pasted: Pasted,
    provider: Option<Arc<Provider>>,
    known: bool,
    key: Option<String>,
    served: Result<Vec<String>, String>,
}

pub(crate) async fn read(core: &Arc<Core>, params: ReadParams) -> CoreResult<Reading> {
    let examined = examine(core, params.text, None).await?;
    let provider = examined.provider.as_deref();
    Ok(Reading {
        label: provider.map(|provider| provider.label.clone()),
        base_url: provider
            .map(|provider| provider.openai.clone())
            .or_else(|| examined.pasted.base_url.clone()),
        model: examined
            .pasted
            .model
            .clone()
            .or_else(|| provider?.model.clone()),
        has_key: examined.key.is_some(),
        known_id: provider
            .filter(|_| examined.known)
            .map(|provider| provider.id.clone()),
        speaks: provider.map(Provider::speaks).unwrap_or_default(),
        runs_with: provider.and_then(Provider::runs_with),
        models: examined.served.as_ref().ok().cloned(),
        failure: examined.served.err(),
    })
}

/// Connects the catalogue's provider the paste names, or keeps a new one with its key.
pub(crate) async fn add(core: &Arc<Core>, params: AddParams) -> CoreResult<ModelProvider> {
    let label = params
        .label
        .map(|label| label.trim().chars().take(60).collect::<String>())
        .filter(|label| !label.is_empty());
    let examined = examine(core, params.text, label).await?;
    let served = examined.served?;
    let provider = examined
        .provider
        .ok_or("Paste an API key with its base URL, or the provider's quickstart")?;
    let (known, pasted_key, key) = (examined.known, examined.pasted.key, examined.key);
    let (provider, feed) = on_store(core, move |store| {
        let provider = match (known, pasted_key) {
            (true, Some(key)) if provider.key.is_some() => {
                store.save_key(&provider.id, key)?;
                provider
            }
            (true, _) => provider,
            (false, _) => store.add_custom(Custom::of(&provider), key)?,
        };
        Ok::<_, String>((provider, store.feed()))
    })
    .await??;
    if !known {
        core.workspaces.model_checks.forget(&provider.id);
    }
    Ok(entry(core, provider, true, &feed, Some(served)))
}

pub(crate) async fn remove(core: &Arc<Core>, params: ProviderParams) -> CoreResult<()> {
    let id = params.id;
    let removing = id.clone();
    on_store(core, move |store| store.remove_custom(&removing)).await??;
    core.workspaces.model_checks.forget(&id);
    Ok(())
}

async fn examine(core: &Arc<Core>, text: String, label: Option<String>) -> CoreResult<Examined> {
    if text.len() > MAX_PASTE_BYTES {
        return Err("That is too long to be a key or a quickstart".into());
    }
    let pasted = paste::read(&text);
    let Some(base_url) = pasted.base_url.clone() else {
        let failure = match pasted.key {
            Some(_) => "Paste the provider's base URL too, such as https://api.example.com/v1",
            None => "Paste an API key with its base URL, or the provider's quickstart",
        };
        return Ok(Examined {
            pasted,
            provider: None,
            known: false,
            key: None,
            served: Err(failure.into()),
        });
    };
    let pasted_key = pasted.key.is_some();
    let (found, saved_key) = on_store(core, move |store| {
        let found = find(store, &base_url, label);
        let saved_key = match &found {
            Found::Known(provider) if !pasted_key => store.key(&provider.id),
            Found::New(custom) if !pasted_key => store.key(&custom.id),
            _ => None,
        };
        (found, saved_key)
    })
    .await?;
    let key = pasted.key.clone().or(saved_key);
    let (provider, known, served) = match found {
        Found::Known(provider) => {
            let served = serving(&provider, key.as_deref()).await;
            (provider, true, served)
        }
        Found::New(mut custom) => {
            let served = model_list(&custom.base_url, key.as_deref(), CHECK_TIMEOUT)
                .await
                .map_err(|unlisted| unlisted_failure(&custom, key.is_some(), unlisted));
            if served.is_ok() {
                (custom.responses, custom.anthropic) =
                    other_apis(&custom.base_url, key.as_deref()).await;
            }
            custom.keyed = key.is_some();
            custom.model = pasted.model.clone();
            (Arc::new(custom.provider()), false, served)
        }
    };
    Ok(Examined {
        pasted,
        provider: Some(provider),
        known,
        key,
        served,
    })
}

enum Found {
    Known(Arc<Provider>),
    New(Custom),
}

/// The catalogue's provider at `base_url`, else the one the person added
/// there, else a new one named `label` or after its domain.
fn find(store: &Providers, base_url: &str, label: Option<String>) -> Found {
    let at = |url: &str| same_api(base_url, url);
    let known = providers::catalogue().iter().find(|provider| {
        at(&provider.openai) || provider.anthropic.as_ref().is_some_and(|api| at(&api.url))
    });
    if let Some(provider) = known {
        return Found::Known(provider.clone());
    }
    let all = store.all();
    let added = all
        .iter()
        .find(|provider| provider.custom && provider.openai == base_url);
    let (id, label) = match added {
        Some(provider) => (
            provider.id.clone(),
            label.unwrap_or_else(|| provider.label.clone()),
        ),
        None => {
            let label = label.unwrap_or_else(|| paste::label(base_url));
            let taken: HashSet<_> = all.iter().map(|provider| provider.id.as_str()).collect();
            let wanted = paste::id(&label);
            let id = std::iter::once(wanted.clone())
                .chain((2..).map(|n| format!("{wanted}-{n}")))
                .find(|id| !taken.contains(id.as_str()))
                .unwrap_or(wanted);
            (id, label)
        }
    };
    Found::New(Custom {
        id,
        label,
        base_url: base_url.to_owned(),
        keyed: false,
        model: None,
        responses: false,
        anthropic: None,
    })
}

/// Whether two base URLs are one API: the same host, one path within the other.
fn same_api(pasted: &str, url: &str) -> bool {
    let (Ok(pasted), Ok(url)) = (Url::parse(pasted), Url::parse(url)) else {
        return false;
    };
    let (a, b) = (
        pasted.path().trim_end_matches('/'),
        url.path().trim_end_matches('/'),
    );
    pasted.host_str() == url.host_str()
        && pasted.port_or_known_default() == url.port_or_known_default()
        && (a.starts_with(b) || b.starts_with(a))
}

/// What a catalogue provider serves with `key`, once it takes the key.
async fn serving(provider: &Provider, key: Option<&str>) -> Result<Vec<String>, String> {
    match (provider.kind, key) {
        (Kind::Local, _) => listed_models(provider, None, RUNNER_TIMEOUT)
            .await
            .ok_or_else(|| format!("{} is not running on this host", provider.label)),
        (Kind::Api | Kind::Plan, None) => Err(format!("Paste your {} key too", provider.label)),
        (Kind::Api | Kind::Plan, Some(key)) => {
            check_key(provider, key)
                .await
                .map_err(|error| error.to_string())?;
            Ok(listed_models(provider, Some(key), CHECK_TIMEOUT)
                .await
                .unwrap_or_default())
        }
    }
}

fn unlisted_failure(custom: &Custom, keyed: bool, unlisted: Unlisted) -> String {
    let (label, base_url) = (&custom.label, &custom.base_url);
    match unlisted {
        Unlisted::Refused if keyed => format!("{label} did not accept that key"),
        Unlisted::Refused => format!("Paste your {label} key too"),
        Unlisted::Unreachable => format!("Could not reach {label} at {base_url}"),
        Unlisted::Answered(status) => format!("{label} answered {status} at {base_url}/models"),
        Unlisted::NotAList => {
            format!("{base_url} does not answer as an OpenAI-compatible API")
        }
    }
}

/// Whether the API at `base_url` also serves OpenAI's Responses API, and the
/// base URL of the Anthropic-compatible API beside it, when it serves one.
async fn other_apis(base_url: &str, key: Option<&str>) -> (bool, Option<String>) {
    let anthropic = base_url.strip_suffix("/v1");
    let (responses, messages) =
        tokio::join!(answers(format!("{base_url}/responses"), key), async {
            match anthropic {
                Some(_) => answers(format!("{base_url}/messages"), key).await,
                None => false,
            }
        });
    (responses, anthropic.filter(|_| messages).map(str::to_owned))
}

/// Whether `url` takes requests: an empty one is refused as malformed, not as missing.
async fn answers(url: String, key: Option<&str>) -> bool {
    let mut request = client()
        .post(url)
        .timeout(PROBE_TIMEOUT)
        .header("anthropic-version", "2023-06-01")
        .json(&serde_json::json!({}));
    if let Some(key) = key {
        request = request.bearer_auth(key);
    }
    request.send().await.is_ok_and(|response| {
        matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        )
    })
}
