//! The person's accounts with their agents, as the core keeps them: who each
//! is signed in as, and signing one in through its agent's own login, which
//! opens the browser on this host.

use std::collections::{HashMap, HashSet};

use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, Window};
use serde::Deserialize;
use serde_json::{json, Value};
use sikemux_core::agent_accounts::{Book, Profile};

use crate::core;
use crate::workbench::Workbench;

/// Who one account is signed in as, in its agent's own words.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub signed_in: bool,
    pub name: Option<String>,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub organization: Option<String>,
}

/// An account's sign-in while it runs or after it failed.
pub enum SignIn {
    /// The browser is out; `url` is the page it opened, once the core has it,
    /// and `code` the field for the code some pages show.
    Waiting {
        url: Option<String>,
        code: Option<Entity<InputState>>,
    },
    Failed(String),
}

/// The add-account dialog: a name for a new account, then its sign-in.
pub struct Dialog {
    pub provider: String,
    pub name: Entity<InputState>,
    /// The account signing in, once it has one.
    pub account: Option<String>,
    /// The account this dialog made, taken away again if it closes before signing in.
    pub made: Option<String>,
    pub busy: bool,
}

#[derive(Default)]
pub struct Accounts {
    pub book: Option<Book>,
    asked_book: bool,
    status: HashMap<(String, Option<String>), Result<Status, String>>,
    asking: HashSet<(String, Option<String>)>,
    pub signing: HashMap<String, SignIn>,
    pub dialog: Option<Dialog>,
}

fn key(profile: &Profile) -> (String, Option<String>) {
    (profile.provider.clone(), profile.config_path.clone())
}

fn redraw(bench: &Workbench, cx: &mut Context<Workbench>) {
    bench.parts.redraw(&bench.parts.stage, cx);
}

impl Accounts {
    pub fn profile(&self, id: &str) -> Option<&Profile> {
        self.book
            .as_ref()?
            .profiles
            .iter()
            .find(|profile| profile.id == id)
    }

    /// The accounts `provider` has, in the order they were added.
    pub fn of(&self, provider: &str) -> Vec<&Profile> {
        self.book
            .iter()
            .flat_map(|book| &book.profiles)
            .filter(|profile| profile.provider == provider)
            .collect()
    }

    /// The account `provider` runs on now, else its first.
    pub fn current(&self, provider: &str) -> Option<&Profile> {
        let chosen = self.book.as_ref()?.selected.get(provider);
        let accounts = self.of(provider);
        chosen
            .and_then(|id| accounts.iter().find(|profile| profile.id == *id))
            .or(accounts.first())
            .copied()
    }

    pub fn auto_switch(&self, provider: &str) -> bool {
        self.book
            .as_ref()
            .and_then(|book| book.auto_switch.get(provider).copied())
            .unwrap_or(false)
    }
}

/// Reads the accounts the core keeps, the first time something shows them.
pub fn read(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    if !std::mem::replace(&mut bench.accounts.asked_book, true) {
        fetch(bench, cx);
    }
}

/// Reads the accounts again when the core says they changed, once something showed them.
pub fn book_moved(bench: &Workbench, cx: &mut Context<Workbench>) {
    if bench.accounts.asked_book {
        fetch(bench, cx);
    }
}

fn fetch(bench: &Workbench, cx: &mut Context<Workbench>) {
    let asks = bench.asks.clone();
    cx.spawn(async move |bench, cx| {
        let book = core::request(&asks, "agentAccounts", json!({})).await;
        let _ = bench.update(cx, |bench, cx| {
            adopt(bench, book.unwrap_or_default(), cx);
        });
    })
    .detach();
}

/// Keeps the accounts as the core answered them after a change.
fn adopt(bench: &mut Workbench, book: Value, cx: &mut Context<Workbench>) {
    if let Ok(book) = serde_json::from_value::<Book>(book) {
        bench.accounts.book = Some(book);
    }
    ask_statuses(bench, cx);
    redraw(bench, cx);
}

/// Changes the accounts in the core with `method`, then keeps what it answers.
fn change(bench: &Workbench, method: &'static str, params: Value, cx: &mut Context<Workbench>) {
    let asks = bench.asks.clone();
    cx.spawn(async move |bench, cx| {
        let Ok(book) = core::request(&asks, method, params).await else {
            return;
        };
        let _ = bench.update(cx, |bench, cx| adopt(bench, book, cx));
    })
    .detach();
}

pub fn save(bench: &Workbench, profile: &Profile, cx: &mut Context<Workbench>) {
    change(bench, "saveAgentAccount", json!({ "profile": profile }), cx);
}

pub fn remove(bench: &Workbench, id: &str, cx: &mut Context<Workbench>) {
    change(bench, "removeAgentAccount", json!({ "id": id }), cx);
}

/// Makes the account the one its agent runs on; the core moves open chats there.
pub fn select(bench: &Workbench, profile: &Profile, cx: &mut Context<Workbench>) {
    let params = json!({ "provider": profile.provider, "id": profile.id });
    change(bench, "selectAgentAccount", params, cx);
}

pub fn set_auto_switch(bench: &Workbench, provider: &str, on: bool, cx: &mut Context<Workbench>) {
    let params = json!({ "provider": provider, "on": on });
    change(bench, "setAccountAutoSwitch", params, cx);
}

/// Who the account is signed in as, once its agent has said.
pub fn status<'a>(bench: &'a Workbench, profile: &Profile) -> Option<&'a Result<Status, String>> {
    bench.accounts.status.get(&key(profile))
}

/// Asks each account's agent who it is signed in as, where nobody has yet.
fn ask_statuses(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    let unasked: Vec<(String, Option<String>)> = bench
        .accounts
        .book
        .iter()
        .flat_map(|book| &book.profiles)
        .map(key)
        .filter(|key| {
            !bench.accounts.status.contains_key(key) && !bench.accounts.asking.contains(key)
        })
        .collect();
    for asked in unasked {
        bench.accounts.asking.insert(asked.clone());
        let asks = bench.asks.clone();
        let params = json!({ "provider": asked.0, "configPath": asked.1 });
        cx.spawn(async move |bench, cx| {
            let answer = core::request(&asks, "agentAccountStatus", params).await;
            let status = answer.and_then(|status| {
                serde_json::from_value::<Status>(status).map_err(|error| error.to_string())
            });
            let _ = bench.update(cx, |bench, cx| {
                bench.accounts.asking.remove(&asked);
                bench.accounts.status.insert(asked, status);
                redraw(bench, cx);
            });
        })
        .detach();
    }
}

/// Marks the account to be asked about again, since its sign-in changed.
fn forget(bench: &mut Workbench, profile: &Profile, cx: &mut Context<Workbench>) {
    let key = key(profile);
    bench.accounts.status.remove(&key);
    let asks = bench.asks.clone();
    let params = json!({ "provider": key.0, "configPath": key.1 });
    cx.spawn(async move |bench, cx| {
        let _ = core::request(&asks, "forgetAgentStatus", params).await;
        let _ = bench.update(cx, |bench, cx| {
            ask_statuses(bench, cx);
            redraw(bench, cx);
        });
    })
    .detach();
}

fn account_params(profile: &Profile) -> Value {
    json!({
        "agent": profile.provider,
        "executablePath": profile.executable_path,
        "configPath": profile.config_path,
    })
}

/// Runs the account's own sign-in. The page it opens is kept for when no browser came up.
pub fn sign_in(bench: &mut Workbench, id: &str, cx: &mut Context<Workbench>) {
    let Some(profile) = bench.accounts.profile(id).cloned() else {
        return;
    };
    if matches!(bench.accounts.signing.get(id), Some(SignIn::Waiting { .. })) {
        return;
    }
    bench.accounts.signing.insert(
        id.to_owned(),
        SignIn::Waiting {
            url: None,
            code: None,
        },
    );
    redraw(bench, cx);
    let asks = bench.asks.clone();
    let params = account_params(&profile);
    cx.spawn(async move |bench, cx| {
        let answer = core::request(&asks, "agentSignIn", params).await;
        let _ = bench.update(cx, |bench, cx| {
            match answer {
                Err(message) if message != "Sign-in stopped" => {
                    bench
                        .accounts
                        .signing
                        .insert(profile.id.clone(), SignIn::Failed(message));
                }
                _ => {
                    bench.accounts.signing.remove(&profile.id);
                }
            }
            forget(bench, &profile, cx);
        });
    })
    .detach();
}

/// Hands each waiting sign-in the page its agent opened, when the core says one did.
pub fn pages_moved(bench: &Workbench, cx: &mut Context<Workbench>) {
    if !bench
        .accounts
        .signing
        .values()
        .any(|signing| matches!(signing, SignIn::Waiting { url: None, .. }))
    {
        return;
    }
    let asks = bench.asks.clone();
    cx.spawn(async move |bench, cx| {
        let Ok(pages) = core::request(&asks, "agentSignIns", json!({})).await else {
            return;
        };
        let _ = bench.update(cx, |bench, cx| {
            let Workbench { accounts, .. } = bench;
            for page in pages.as_array().into_iter().flatten() {
                let config = page["configPath"].as_str();
                let opened_for =
                    accounts
                        .book
                        .iter()
                        .flat_map(|book| &book.profiles)
                        .find(|profile| {
                            page["agent"] == profile.provider.as_str()
                                && profile.config_path.as_deref() == config
                        });
                let Some(id) = opened_for.map(|profile| profile.id.clone()) else {
                    continue;
                };
                if let Some(SignIn::Waiting { url, .. }) = accounts.signing.get_mut(&id) {
                    *url = page["url"].as_str().map(str::to_owned);
                }
            }
            redraw(bench, cx);
        });
    })
    .detach();
}

pub fn cancel_sign_in(bench: &Workbench, id: &str) {
    let Some(profile) = bench.accounts.profile(id) else {
        return;
    };
    let _ = bench.asks.send(core::Ask::Call {
        method: "cancelAgentSignIn",
        params: json!({ "agent": profile.provider, "configPath": profile.config_path }),
    });
}

pub fn dismiss_failure(bench: &mut Workbench, id: &str) {
    if matches!(bench.accounts.signing.get(id), Some(SignIn::Failed(_))) {
        bench.accounts.signing.remove(id);
    }
}

/// Opens the field for the code the sign-in page showed.
pub fn ask_code(bench: &mut Workbench, id: &str, window: &mut Window, cx: &mut Context<Workbench>) {
    let field = cx.new(|cx| InputState::new(window, cx).placeholder("Code from the page"));
    field.update(cx, |field, cx| field.focus(window, cx));
    if let Some(SignIn::Waiting { code, .. }) = bench.accounts.signing.get_mut(id) {
        *code = Some(field);
    }
    redraw(bench, cx);
}

/// Hands the running sign-in the code typed into its field.
pub fn submit_code(bench: &mut Workbench, id: &str, cx: &mut Context<Workbench>) {
    let Some(profile) = bench.accounts.profile(id).cloned() else {
        return;
    };
    let Some(SignIn::Waiting {
        code: Some(field), ..
    }) = bench.accounts.signing.get(id)
    else {
        return;
    };
    let code = field.read(cx).value().trim().to_owned();
    if code.is_empty() {
        return;
    }
    let asks = bench.asks.clone();
    let params =
        json!({ "agent": profile.provider, "configPath": profile.config_path, "code": code });
    cx.spawn(async move |bench, cx| {
        if core::request(&asks, "agentSignInCode", params)
            .await
            .is_ok()
        {
            let _ = bench.update(cx, |bench, cx| {
                if let Some(SignIn::Waiting { code, .. }) =
                    bench.accounts.signing.get_mut(&profile.id)
                {
                    *code = None;
                }
                redraw(bench, cx);
            });
        }
    })
    .detach();
}

pub fn sign_out(bench: &Workbench, id: &str, cx: &mut Context<Workbench>) {
    let Some(profile) = bench.accounts.profile(id).cloned() else {
        return;
    };
    let asks = bench.asks.clone();
    let params = account_params(&profile);
    cx.spawn(async move |bench, cx| {
        let _ = core::request(&asks, "agentSignOut", params).await;
        let _ = bench.update(cx, |bench, cx| forget(bench, &profile, cx));
    })
    .detach();
}

/// Opens the dialog that adds an account to `provider`.
pub fn open_dialog(
    bench: &mut Workbench,
    provider: &str,
    window: &mut Window,
    cx: &mut Context<Workbench>,
) {
    let name = cx.new(|cx| InputState::new(window, cx).placeholder("Work"));
    name.update(cx, |name, cx| name.focus(window, cx));
    bench.accounts.dialog = Some(Dialog {
        provider: provider.to_owned(),
        name,
        account: None,
        made: None,
        busy: false,
    });
    redraw(bench, cx);
}

/// Signs an account already there in again, from the dialog.
pub fn sign_in_again(bench: &mut Workbench, id: &str, cx: &mut Context<Workbench>) {
    if let Some(dialog) = bench.accounts.dialog.as_mut() {
        dialog.account = Some(id.to_owned());
    }
    dismiss_failure(bench, id);
    sign_in(bench, id, cx);
}

/// Makes the account named in the dialog: a folder of its own sharing the default one's
/// chats and settings, then signs it in.
pub fn add(bench: &mut Workbench, brand: String, cx: &mut Context<Workbench>) {
    let Some(dialog) = bench.accounts.dialog.as_mut() else {
        return;
    };
    let name = dialog.name.read(cx).value().trim().to_owned();
    if name.is_empty() || dialog.busy {
        return;
    }
    dialog.busy = true;
    let provider = dialog.provider.clone();
    let asks = bench.asks.clone();
    redraw(bench, cx);
    cx.spawn(async move |bench, cx| {
        let made = async {
            let added = json!({ "provider": provider, "name": name });
            let folder = core::request(&asks, "addAgentAccount", added).await?;
            let now = chrono::Utc::now().timestamp_millis();
            let profile = Profile {
                id: format!("profile-{now:x}"),
                name,
                provider,
                accent: brand,
                executable_path: None,
                config_path: folder.as_str().map(str::to_owned),
                environment_keys: Vec::new(),
            };
            let book =
                core::request(&asks, "saveAgentAccount", json!({ "profile": profile })).await?;
            Ok::<_, String>((profile.id, book))
        }
        .await;
        let _ = bench.update(cx, |bench, cx| {
            if let Some(dialog) = bench.accounts.dialog.as_mut() {
                dialog.busy = false;
            }
            let Ok((id, book)) = made else {
                return redraw(bench, cx);
            };
            adopt(bench, book, cx);
            if let Some(dialog) = bench.accounts.dialog.as_mut() {
                dialog.made = Some(id.clone());
                dialog.account = Some(id.clone());
            }
            sign_in(bench, &id, cx);
        });
    })
    .detach();
}

/// Closes the dialog, stopping a sign-in still waiting and taking away an account it
/// made that never signed in.
pub fn close_dialog(bench: &mut Workbench, cx: &mut Context<Workbench>) {
    let Some(dialog) = bench.accounts.dialog.take() else {
        return;
    };
    if let Some(id) = &dialog.account {
        let waiting = matches!(bench.accounts.signing.get(id), Some(SignIn::Waiting { .. }));
        if waiting {
            cancel_sign_in(bench, id);
        }
        let unfinished = bench.accounts.signing.contains_key(id);
        if dialog.made.as_deref() == Some(id.as_str()) && unfinished {
            remove(bench, id, cx);
        }
    }
    redraw(bench, cx);
}
