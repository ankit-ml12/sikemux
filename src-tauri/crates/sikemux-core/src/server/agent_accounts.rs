//! The core's calls about the person's agent accounts, which the app's
//! settings and rail make.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use serde::{Deserialize, Serialize};

use super::changed::Topic;
use super::connection::blocking;
use super::workspace::refresh_launchers;
use super::{Core, CoreResult};
use crate::agent_accounts::{chat_account, folders, status, usage, Book, Profile};
use crate::flights::{shared, Flights};
use crate::launch::discovery;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Account {
    provider: String,
    config_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Forget {
    provider: Option<String>,
    config_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Resuming {
    agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Authenticate {
    agent_id: String,
    method_id: String,
}

#[derive(Deserialize)]
pub(super) struct Saving {
    profile: Profile,
}

#[derive(Deserialize)]
pub(super) struct Removing {
    id: String,
}

#[derive(Deserialize)]
pub(super) struct Choosing {
    provider: String,
    id: String,
}

#[derive(Deserialize)]
pub(super) struct Switching {
    provider: String,
    on: bool,
}

#[derive(Deserialize)]
pub(super) struct Adding {
    provider: String,
    name: String,
}

/// The program the account's agent runs as, found afresh.
async fn program(core: &Core, account: &Account) -> CoreResult<PathBuf> {
    let spec = core
        .workspaces
        .account_spec(&account.provider, account.config_path.as_deref());
    Ok(spec.program().await?)
}

/// Changes the person's accounts, offers agents on them afresh and tells
/// every client, answering with the accounts as they are now.
async fn change(
    core: &Arc<Core>,
    edit: impl FnOnce(&mut Book) -> Result<(), String>,
) -> CoreResult<Book> {
    let mut book = core.workspaces.accounts().unwrap_or_default();
    edit(&mut book)?;
    core.workspaces.set_accounts(book.clone())?;
    refresh_launchers(core.clone()).await;
    core.changes.changed(Topic::AgentAccounts, None, None);
    Ok(book)
}

/// Moves `provider`'s chats onto the account `id` the person chose, where it
/// keeps their chats too, with a turn at work carrying on there. Agents in a
/// terminal stay where they are.
fn move_chats(core: &Core, book: &Book, provider: &str, id: &str) {
    let Some(target) = book
        .profile(id)
        .filter(|profile| profile.provider == provider)
    else {
        return;
    };
    let chosen = book.chosen(provider);
    for agent_id in core.workspaces.agent_ids() {
        let Some(agent) = core.workspaces.agent(&agent_id) else {
            continue;
        };
        let on = agent.profile_id.clone().unwrap_or_else(|| chosen.clone());
        if agent.provider != provider || agent.pty_id.is_some() || on == id {
            continue;
        }
        let from = book
            .profile(&on)
            .and_then(|profile| profile.config_path.as_deref());
        if !folders::sessions_shared(provider, from, target.config_path.as_deref()) {
            continue;
        }
        let account = chat_account(
            provider,
            &target.id,
            &target.name,
            target.config_path.as_deref(),
        );
        if let Err(error) = super::chat::switch_account(core, &agent_id, account) {
            eprintln!("sikemux core: {agent_id} stayed on its account: {error}");
        }
    }
}

pub(super) async fn set(core: &Arc<Core>, book: Book) -> CoreResult<()> {
    core.workspaces.set_accounts(book)?;
    refresh_launchers(core.clone()).await;
    Ok(())
}

pub(super) async fn save(core: &Arc<Core>, saving: Saving) -> CoreResult<Book> {
    change(core, |book| {
        book.save(saving.profile);
        Ok(())
    })
    .await
}

pub(super) async fn remove(core: &Arc<Core>, removing: Removing) -> CoreResult<Book> {
    change(core, |book| book.remove(&removing.id)).await
}

pub(super) async fn select(core: &Arc<Core>, choosing: Choosing) -> CoreResult<Book> {
    let (provider, id) = (choosing.provider.clone(), choosing.id.clone());
    let book = change(core, move |book| {
        book.selected.insert(provider, id);
        Ok(())
    })
    .await?;
    move_chats(core, &book, &choosing.provider, &choosing.id);
    Ok(book)
}

pub(super) async fn set_auto_switch(core: &Arc<Core>, switching: Switching) -> CoreResult<Book> {
    change(core, move |book| {
        book.auto_switch.insert(switching.provider, switching.on);
        Ok(())
    })
    .await
}

pub(super) async fn add(adding: Adding) -> CoreResult<String> {
    blocking(move || Ok(folders::add(&adding.provider, &adding.name)?)).await
}

/// Where an account's agent program is, what was tried and failed on the
/// way, and whether it is signed in. The program comes from the person's own
/// profile, never from the caller.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Availability {
    program: Option<PathBuf>,
    failures: Vec<String>,
    status: status::AgentStatus,
}

/// Asks an account again, once however many ask together, and tells every
/// client when the answer changed.
async fn recheck(
    core: Arc<Core>,
    provider: String,
    config_path: Option<String>,
    program: PathBuf,
) -> status::Checked {
    static CHECKING: LazyLock<Flights<String, status::Checked>> = LazyLock::new(Flights::default);
    let generation = status::generation();
    let flight = format!(
        "{generation}\0{provider}\0{}",
        config_path.as_deref().unwrap_or("")
    );
    shared(&CHECKING, flight, async {
        let keys = {
            let (core, provider) = (core.clone(), provider.clone());
            blocking(move || Ok(core.workspaces.provider_keys(&provider)))
                .await
                .unwrap_or_default()
        };
        let config_path = config_path.as_deref();
        let checked = status::ask(&provider, &program, config_path, keys).await;
        if status::keep(&provider, config_path, &checked, generation) {
            core.workspaces
                .note_status(&provider, config_path, &checked.status);
            core.view_changed();
            core.changes.changed(Topic::AgentStatus, None, None);
        }
        checked
    })
    .await
}

/// What is known of an account, at once whenever anything is: an answer that
/// no longer holds is given while the account is asked again behind it.
async fn checked(
    core: &Arc<Core>,
    provider: &str,
    config_path: Option<&str>,
    program: &Path,
) -> status::Checked {
    let ask = || {
        recheck(
            core.clone(),
            provider.to_owned(),
            config_path.map(str::to_owned),
            program.to_owned(),
        )
    };
    match status::known(provider, config_path) {
        Some((checked, true)) => checked,
        Some((checked, false)) => {
            tokio::spawn(ask());
            checked
        }
        None => ask().await,
    }
}

pub(super) async fn availability(core: &Arc<Core>, account: Account) -> CoreResult<Availability> {
    let (provider, config_path) = (&account.provider, account.config_path.as_deref());
    let spec = core.workspaces.account_spec(provider, config_path);
    let (program, failures) = status::locate(&spec).await;
    let status = match &program {
        Some(program) => checked(core, provider, config_path, program).await.status,
        None => status::unfound(&failures),
    };
    Ok(Availability {
        program,
        failures,
        status,
    })
}

pub(super) async fn status(core: &Arc<Core>, account: Account) -> CoreResult<status::AgentStatus> {
    Ok(availability(core, account).await?.status)
}

pub(super) async fn account_status(
    core: &Arc<Core>,
    account: Account,
) -> CoreResult<status::AccountStatus> {
    if !matches!(account.provider.as_str(), "claude" | "codex") {
        return Err("Only Claude and Codex keep separate accounts".into());
    }
    let program = program(core, &account).await?;
    let (provider, config_path) = (&account.provider, account.config_path.as_deref());
    let mut known = checked(core, provider, config_path, &program).await;
    if known.account.is_none() {
        known = recheck(core.clone(), provider.clone(), account.config_path, program).await;
    }
    Ok(known
        .account
        .ok_or("Claude or Codex did not say who is signed in")??)
}

pub(super) async fn usage(core: &Arc<Core>, account: Account) -> CoreResult<usage::Usage> {
    let program = program(core, &account).await?;
    let config_path = account.config_path.as_deref();
    Ok(usage::usage(&account.provider, &program, config_path).await?)
}

pub(super) fn forget(core: &Arc<Core>, forget: Forget) {
    let (provider, config_path) = (forget.provider.as_deref(), forget.config_path.as_deref());
    status::forget(provider, config_path);
    if let Some(provider) = provider {
        usage::forget(provider, config_path);
    }
    discovery::forget_failures();
    tokio::spawn(refresh_launchers(core.clone()));
}

pub(super) fn mark_signed_out(core: &Core, account: Account) {
    let (provider, config_path) = (&account.provider, account.config_path.as_deref());
    let signed_out = status::AgentStatus::SignedOut;
    core.workspaces
        .note_status(provider, config_path, &signed_out);
    status::remember(provider, config_path, signed_out);
    core.view_changed();
}

pub(super) fn cancel_resume(core: &Arc<Core>, resuming: Resuming) {
    super::chat::reset::cancel_by_person(core, &resuming.agent_id);
}

fn failed_chat(core: &Core, failed: &Resuming) -> CoreResult<Arc<super::chat::Chat>> {
    Ok(core
        .chats
        .get(&failed.agent_id)
        .ok_or_else(|| format!("no chat {} is running", failed.agent_id))?)
}

pub(super) fn retry_failed(core: &Core, failed: Resuming) -> CoreResult<()> {
    failed_chat(core, &failed)?.retry_failed()
}

pub(super) fn discard_failed(core: &Core, failed: Resuming) -> CoreResult<()> {
    failed_chat(core, &failed)?.discard_failed()
}

pub(super) async fn authenticate(core: &Arc<Core>, params: Authenticate) -> CoreResult<()> {
    super::chat::authenticate(core, &params.agent_id, params.method_id).await?;
    Ok(())
}
