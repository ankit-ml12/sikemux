//! The core's calls about the person's agent accounts, which the app's
//! settings and rail make.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::connection::blocking;
use super::workspace::refresh_launchers;
use super::{Core, CoreError, CoreResult};
use crate::agent_accounts::{chat_account, folders, status, usage, Book, Profile};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    provider: String,
    #[serde(default)]
    config_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Forget {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    config_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resuming {
    agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Authenticate {
    agent_id: String,
    method_id: String,
}

#[derive(Deserialize)]
struct Saving {
    profile: Profile,
}

#[derive(Deserialize)]
struct Removing {
    id: String,
}

#[derive(Deserialize)]
struct Choosing {
    provider: String,
    id: String,
}

#[derive(Deserialize)]
struct Switching {
    provider: String,
    on: bool,
}

#[derive(Deserialize)]
struct Adding {
    provider: String,
    name: String,
}

fn parse<T: for<'de> Deserialize<'de>>(params: Value) -> CoreResult<T> {
    serde_json::from_value(params)
        .map_err(|error| CoreError::from(format!("the call's params do not fit: {error}")))
}

fn answer(result: impl Serialize) -> CoreResult<Value> {
    Ok(serde_json::to_value(result)?)
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
    core.changes.changed("agentAccounts", None, None);
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

pub(crate) async fn call(core: &Arc<Core>, method: &str, params: Value) -> CoreResult<Value> {
    match method {
        "agentAccounts" => answer(core.workspaces.accounts()),
        "setAgentAccounts" => {
            core.workspaces.set_accounts(parse(params)?)?;
            refresh_launchers(core.clone()).await;
            answer(())
        }
        "saveAgentAccount" => {
            let saving: Saving = parse(params)?;
            answer(change(core, |book| Ok(book.save(saving.profile))).await?)
        }
        "removeAgentAccount" => {
            let removing: Removing = parse(params)?;
            answer(change(core, |book| book.remove(&removing.id)).await?)
        }
        "selectAgentAccount" => {
            let choosing: Choosing = parse(params)?;
            let (provider, id) = (choosing.provider.clone(), choosing.id.clone());
            let book = change(core, move |book| {
                book.selected.insert(provider, id);
                Ok(())
            })
            .await?;
            move_chats(core, &book, &choosing.provider, &choosing.id);
            answer(book)
        }
        "setAccountAutoSwitch" => {
            let switching: Switching = parse(params)?;
            answer(
                change(core, move |book| {
                    book.auto_switch.insert(switching.provider, switching.on);
                    Ok(())
                })
                .await?,
            )
        }
        "addAgentAccount" => {
            let adding: Adding = parse(params)?;
            let added = blocking(move || Ok(folders::add(&adding.provider, &adding.name)?)).await?;
            answer(added)
        }
        "agentStatus" => {
            let account: Account = parse(params)?;
            let (provider, config_path) = (&account.provider, account.config_path.as_deref());
            let spec = core.workspaces.account_spec(provider, config_path);
            let status = match status::find(&spec).await {
                Ok(program) => {
                    let keys = {
                        let (core, provider) = (core.clone(), provider.clone());
                        blocking(move || Ok(core.workspaces.provider_keys(&provider))).await?
                    };
                    status::signed_in(provider, &program, config_path, keys).await
                }
                Err(status) => status,
            };
            core.workspaces.note_status(provider, config_path, &status);
            core.publish_device_view();
            answer(status)
        }
        "agentAccountStatus" => {
            let account: Account = parse(params)?;
            let program = program(core, &account).await?;
            let config_path = account.config_path.as_deref();
            answer(status::account(&account.provider, &program, config_path).await?)
        }
        "agentUsage" => {
            let account: Account = parse(params)?;
            let program = program(core, &account).await?;
            let config_path = account.config_path.as_deref();
            answer(usage::usage(&account.provider, &program, config_path).await?)
        }
        "forgetAgentStatus" => {
            let forget: Forget = parse(params)?;
            let (provider, config_path) =
                (forget.provider.as_deref(), forget.config_path.as_deref());
            status::forget(provider, config_path);
            if let Some(provider) = provider {
                usage::forget(provider, config_path);
            }
            tokio::spawn(refresh_launchers(core.clone()));
            answer(())
        }
        "markAgentSignedOut" => {
            let account: Account = parse(params)?;
            let (provider, config_path) = (&account.provider, account.config_path.as_deref());
            let signed_out = status::AgentStatus::SignedOut;
            core.workspaces
                .note_status(provider, config_path, &signed_out);
            status::remember(provider, config_path, signed_out);
            core.publish_device_view();
            answer(())
        }
        "cancelResume" => {
            let resuming: Resuming = parse(params)?;
            super::chat::reset::cancel_by_person(core, &resuming.agent_id);
            answer(())
        }
        "retryFailed" | "discardFailed" => {
            let failed: Resuming = parse(params)?;
            let chat = core
                .chats
                .get(&failed.agent_id)
                .ok_or_else(|| format!("no chat {} is running", failed.agent_id))?;
            if method == "retryFailed" {
                chat.retry_failed().map_err(|error| error.to_string())?;
            } else {
                chat.discard_failed().map_err(|error| error.to_string())?;
            }
            answer(())
        }
        "authenticateChat" => {
            let params: Authenticate = parse(params)?;
            super::chat::authenticate(core, &params.agent_id, params.method_id).await?;
            answer(())
        }
        _ => Err(format!("this host does not know {method}").into()),
    }
}
