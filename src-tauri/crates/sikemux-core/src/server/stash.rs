//! The account's Stash: what the person shared into Sikemux from another app, kept on the
//! accounts server for every device and host on the account. An item shared to one of this
//! host's chats lands in that chat's draft as soon as the host hears of it; any other waits
//! until the person attaches it from a composer.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sikemux_process::sync::Locked;

use crate::accounts::protocol::{
    AccountEvent, AccountEventType, StashItem, StashKind, StashList, StashTaken, StashTarget,
    StashUpdate,
};

use super::changed::Topic;
use super::connection::blocking;
use super::drafts::Delivery;
use super::hosts::account::{as_host, host_request};
use super::{attachments, Core, CoreError, CoreResult};

/// The most a stash item can hold, as the accounts server takes it.
const MAX_ITEM_BYTES: u64 = 25 * 1024 * 1024;
/// The largest picture fetched only to be shown small.
const MAX_PICTURE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListParams {
    #[serde(default)]
    taken: Option<StashTaken>,
    #[serde(default)]
    cursor: Option<String>,
}

/// The stash as the apps draw it: none while this host is signed out.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Shown {
    signed_in: bool,
    #[serde(flatten)]
    list: StashList,
}

/// One page of the account's stash, newest first; by default only what has not been used.
pub(crate) async fn list(core: &Core, params: ListParams) -> CoreResult<Shown> {
    if core.remote.signed_in_api().is_none() {
        return Ok(Shown {
            signed_in: false,
            list: StashList {
                items: Vec::new(),
                cursor: None,
            },
        });
    }
    let taken = params.taken.unwrap_or(StashTaken::False);
    let list = page(core, taken, params.cursor.as_deref()).await?;
    Ok(Shown {
        signed_in: true,
        list,
    })
}

async fn page(core: &Core, taken: StashTaken, cursor: Option<&str>) -> CoreResult<StashList> {
    let taken = serde_json::to_value(taken)?;
    let mut path = format!("/v1/stash?taken={}", taken.as_str().unwrap_or("false"));
    if let Some(cursor) = cursor {
        path.push_str("&cursor=");
        path.extend(url::form_urlencoded::byte_serialize(cursor.as_bytes()));
    }
    as_host(core, Method::GET, &path, None).await
}

/// An item of the stash by its id, as the server writes it in a path.
fn item_path(id: &str) -> CoreResult<String> {
    let usable = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_'));
    if !usable {
        return Err(CoreError::from("that is not an item of the stash"));
    }
    Ok(format!("/v1/stash/{id}"))
}

async fn item(core: &Core, id: &str) -> CoreResult<StashItem> {
    as_host(core, Method::GET, &item_path(id)?, None).await
}

async fn take(core: &Core, id: &str) -> CoreResult<()> {
    let path = item_path(id)?;
    let update = StashUpdate {
        note: None,
        taken: Some(true),
    };
    let body = Some(serde_json::to_value(update)?);
    let _: StashItem = as_host(core, Method::PATCH, &path, body).await?;
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct ItemParams {
    id: String,
}

/// Takes an item out of the account's stash for good, on every device.
pub(crate) async fn remove(core: &Core, params: ItemParams) -> CoreResult<()> {
    host_request(core, Method::DELETE, &item_path(&params.id)?, None).await?;
    core.changes.changed(Topic::Stash, None, None);
    Ok(())
}

/// A picture in the stash, in base64, for an app to show small.
pub(crate) async fn picture(core: &Core, params: ItemParams) -> CoreResult<Value> {
    let item = item(core, &params.id).await?;
    let mime = item.mime.clone().unwrap_or_default();
    if item.kind != StashKind::Image || !mime.starts_with("image/") {
        return Err(CoreError::from("that item is not a picture"));
    }
    if item
        .size
        .is_some_and(|size| size as u64 > MAX_PICTURE_BYTES)
    {
        return Err(CoreError::from("that picture is too large to show small"));
    }
    let bytes = file(core, &item, MAX_PICTURE_BYTES).await?;
    Ok(json!({ "mime": mime, "data": STANDARD.encode(bytes) }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AttachParams {
    id: String,
    agent_id: String,
    /// False when the app keeps its own draft: the item's file is only kept here, and the app
    /// adds it to its message and marks the item used once it is sent.
    #[serde(default)]
    draft: Option<bool>,
}

/// What attaching an item put in the chat's draft.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Attached {
    agent_id: String,
    /// Where the item's file was kept on this host, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
}

/// Puts a stash item in the draft of one of this host's chats, unsent, and marks it used; or
/// with `draft` false only keeps its file where the chat's agent can read it.
pub(crate) async fn attach(core: &Core, params: AttachParams) -> CoreResult<Attached> {
    if core.workspaces.agent(&params.agent_id).is_none() {
        return Err(CoreError::from("that chat is no longer on this host"));
    }
    let item = item(core, &params.id).await?;
    if params.draft == Some(false) {
        let path = match has_file(&item) {
            true => Some(download(core, &item).await?),
            false => None,
        };
        return Ok(Attached {
            agent_id: params.agent_id,
            path,
        });
    }
    let _delivering = Delivering::start(&item.id)
        .ok_or_else(|| CoreError::from("that item is already on its way to a chat"))?;
    let path = into_draft(core, &item, &params.agent_id).await?;
    take(core, &item.id).await?;
    core.changes.changed(Topic::Stash, None, None);
    Ok(Attached {
        agent_id: params.agent_id,
        path,
    })
}

/// Downloads the item's file, when it has one, and adds it with the item's words to the
/// draft of `agent_id`'s chat.
async fn into_draft(core: &Core, item: &StashItem, agent_id: &str) -> CoreResult<Option<PathBuf>> {
    let path = match has_file(item) {
        true => Some(download(core, item).await?),
        false => None,
    };
    let key = super::drafts::chat_key(agent_id);
    let delivery = Delivery {
        path: path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        words: words(item),
    };
    core.drafts.deliver(&key, delivery)?;
    super::drafts::changed(&core, vec![key]);
    Ok(path)
}

fn has_file(item: &StashItem) -> bool {
    matches!(
        item.kind,
        StashKind::Image | StashKind::Video | StashKind::File
    )
}

/// The item's note, and its link or text, as they go after what the draft says.
fn words(item: &StashItem) -> String {
    let said = match item.kind {
        StashKind::Link => item.url.as_deref(),
        StashKind::Text => item.text.as_deref(),
        _ => None,
    };
    [item.note.as_deref(), said]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|words| !words.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn download(core: &Core, item: &StashItem) -> CoreResult<PathBuf> {
    let dir = core
        .listening
        .get()
        .and_then(|listening| listening.config.attachment_dir.clone())
        .ok_or_else(|| CoreError::from("this host has nowhere to keep files"))?;
    let bytes = file(core, item, MAX_ITEM_BYTES).await?;
    let name = item.name.clone().unwrap_or_default();
    let mime = item.mime.clone().unwrap_or_default();
    blocking(move || attachments::save_bytes(&dir, &name, &mime, &bytes)).await
}

/// The item's file, refused once it runs past `most` bytes.
async fn file(core: &Core, item: &StashItem, most: u64) -> CoreResult<Vec<u8>> {
    let too_large = || CoreError::from("that item is larger than it may be here");
    if item.size.is_some_and(|size| size as u64 > most) {
        return Err(too_large());
    }
    let path = format!("{}/file", item_path(&item.id)?);
    let mut response = host_request(core, Method::GET, &path, None).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| CoreError::from(error.to_string()))?
    {
        if (bytes.len() + chunk.len()) as u64 > most {
            return Err(too_large());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// The items being put in a chat's draft right now, so an event and a catch-up that both
/// hear of one deliver it once.
struct Delivering(String);

impl Delivering {
    fn held() -> &'static Mutex<HashSet<String>> {
        static HELD: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
        HELD.get_or_init(Default::default)
    }

    fn start(id: &str) -> Option<Self> {
        Self::held()
            .locked()
            .insert(id.to_owned())
            .then(|| Self(id.to_owned()))
    }
}

impl Drop for Delivering {
    fn drop(&mut self) {
        Self::held().locked().remove(&self.0);
    }
}

/// What the live connection heard about the stash: every app reads it again, and an item
/// shared to one of this host's chats is delivered there.
pub(crate) fn heard(core: &Arc<Core>, events: &[AccountEvent]) {
    let ids: Vec<String> = events
        .iter()
        .filter(|event| {
            matches!(
                event.r#type,
                AccountEventType::StashAdded | AccountEventType::StashChanged
            )
        })
        .filter_map(|event| event.stash_id.clone())
        .collect();
    let stash_moved = events.iter().any(|event| {
        matches!(
            event.r#type,
            AccountEventType::StashAdded
                | AccountEventType::StashChanged
                | AccountEventType::StashRemoved
        )
    });
    if stash_moved {
        core.changes.changed(Topic::Stash, None, None);
    }
    if ids.is_empty() {
        return;
    }
    let core = core.clone();
    tokio::spawn(async move {
        for id in ids {
            match item(&core, &id).await {
                Ok(item) => deliver(&core, item).await,
                Err(error) => eprintln!("sikemux core: could not read stash item {id}: {error}"),
            }
        }
    });
}

/// Delivers every item shared to this host's chats while it was away, once its live
/// connection is up again.
pub(crate) fn catch_up(core: &Arc<Core>) {
    let core = core.clone();
    tokio::spawn(async move {
        let mut cursor = None;
        loop {
            let list = match page(&core, StashTaken::False, cursor.as_deref()).await {
                Ok(list) => list,
                Err(error) => {
                    eprintln!("sikemux core: could not read the stash: {error}");
                    return;
                }
            };
            for item in list.items {
                deliver(&core, item).await;
            }
            match list.cursor {
                Some(next) => cursor = Some(next),
                None => return,
            }
        }
    });
}

/// Puts `item` in the chat it was shared to, when that chat is on this host, and marks it
/// used. One shared to a chat since closed, or a project no longer open here, stays in the
/// stash for the person to attach.
async fn deliver(core: &Arc<Core>, item: StashItem) {
    if item.taken_at.is_some() {
        return;
    }
    let Some(target) = item.target.as_ref() else {
        return;
    };
    if core.remote.core_id().as_deref() != Some(target.host_key.as_str()) {
        return;
    }
    let Some(_delivering) = Delivering::start(&item.id) else {
        return;
    };
    if let Err(error) = deliver_to(core, &item, target).await {
        eprintln!(
            "sikemux core: could not put stash item {} in its chat: {error}",
            item.id
        );
    }
}

async fn deliver_to(core: &Arc<Core>, item: &StashItem, target: &StashTarget) -> CoreResult<()> {
    let agent_id = match (&target.chat_id, &target.project_path) {
        (Some(chat), _) if core.workspaces.agent(chat).is_some() => chat.clone(),
        (Some(_), _) => return Ok(()),
        (None, Some(project)) if target.new_chat == Some(true) => {
            match new_chat(core, project).await? {
                Some(agent_id) => agent_id,
                None => return Ok(()),
            }
        }
        _ => return Ok(()),
    };
    into_draft(core, item, &agent_id).await?;
    take(core, &item.id).await?;
    core.changes.changed(Topic::Stash, None, None);
    Ok(())
}

/// Opens a new chat in the project at `folder`, as a new chat there opens: it waits for the
/// person, so its agent starts only once they write or send. None when that project is not
/// open on this host or no agent here can chat.
async fn new_chat(core: &Arc<Core>, folder: &str) -> CoreResult<Option<String>> {
    let view = core.workspace_view();
    let Some(project) = view
        .projects
        .iter()
        .find(|project| project.path.to_string_lossy() == folder)
    else {
        return Ok(None);
    };
    let Some(launcher) = project.launcher.clone() else {
        return Ok(None);
    };
    let agent_id = format!("agent-{}", uuid::Uuid::new_v4().simple());
    let chosen = super::launching::Launch {
        agent_id: agent_id.clone(),
        launcher,
        cwd: project.path.clone(),
        permission_mode: None,
        model: None,
        model_provider: None,
        effort: None,
        resume_id: None,
        account: None,
        fallbacks: Vec::new(),
        program: None,
        waits_for_person: true,
    };
    super::launching::start(core, None, chosen).await?;
    let tab = json!({ "project": project.id, "kind": "agent", "target": agent_id });
    if let Ok(tab) = serde_json::from_value(tab) {
        let _ = super::layout::open_tab(core, tab);
    }
    Ok(Some(agent_id))
}
