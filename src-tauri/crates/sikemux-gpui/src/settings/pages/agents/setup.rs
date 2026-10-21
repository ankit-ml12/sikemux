//! Every agent Sikemux can run, with what it needs next: nothing, an install, or a sign-in.

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, px, AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled,
};
use serde_json::{json, Value};
use sikemux_core::agent_accounts::status::AgentStatus;

use crate::core;
use crate::settings::rows;
use crate::ui;
use crate::workbench::Workbench;

/// One agent as the core found it.
pub struct Found {
    kind: String,
    label: String,
    about: String,
    install: Option<String>,
    featured: bool,
    status: AgentStatus,
}

#[derive(Default)]
pub struct Setup {
    found: Option<Vec<Found>>,
}

/// Asks the core, for every agent it knows, whether it is installed and signed in.
pub fn read(bench: &Workbench, cx: &mut Context<Workbench>) {
    let kinds: Vec<Value> = bench
        .read("catalog")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let accounts: Vec<Option<String>> = kinds
        .iter()
        .map(|kind| {
            let provider = kind["type"].as_str().unwrap_or_default();
            bench
                .accounts
                .current(provider)
                .and_then(|profile| profile.config_path.clone())
        })
        .collect();
    let asks = bench.asks.clone();
    cx.spawn(async move |bench, cx| {
        let mut found = Vec::new();
        for (kind, config) in kinds.iter().zip(accounts) {
            let provider = kind["type"].as_str().unwrap_or_default().to_owned();
            let asked = json!({ "provider": provider, "configPath": config });
            let status = core::request(&asks, "agentAvailability", asked)
                .await
                .ok()
                .and_then(|answer| serde_json::from_value(answer["status"].clone()).ok())
                .unwrap_or(AgentStatus::Missing);
            let installable = kind["install"].as_str().is_some();
            if matches!(status, AgentStatus::Missing) && !installable {
                continue;
            }
            found.push(Found {
                kind: provider,
                label: kind["label"].as_str().unwrap_or_default().to_owned(),
                about: kind["about"].as_str().unwrap_or_default().to_owned(),
                install: kind["install"].as_str().map(str::to_owned),
                featured: kind["featured"].as_bool() == Some(true),
                status,
            });
        }
        let order = |found: &Found| {
            let ready = !matches!(
                found.status,
                AgentStatus::Missing | AgentStatus::Broken { .. } | AgentStatus::SignedOut
            );
            (!ready, !found.featured)
        };
        found.sort_by_key(order);
        let _ = bench.update(cx, |bench, cx| {
            if let Some(fields) = super::fields_of(bench) {
                fields.setup.found = Some(found);
            }
            bench.parts.redraw(&bench.parts.stage, cx);
        });
    })
    .detach();
}

fn status_label(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Missing => "Not installed",
        AgentStatus::Broken { .. } => "Not working",
        AgentStatus::SignedOut => "Signed out",
        AgentStatus::Ready { account } => match account.as_deref() {
            Some("apiKey") => "Ready · API key",
            Some("subscription") => "Ready · subscription",
            _ => "Ready",
        },
        AgentStatus::Unknown => "Found",
    }
}

/// Runs the agent's own installer in a terminal of the project in front, where the
/// person can watch it, then shows that terminal.
fn install(
    bench: &mut Workbench,
    command: String,
    window: &mut gpui_kit::Window,
    cx: &mut Context<Workbench>,
) {
    let Some(project) = bench.project().map(|project| project.id.clone()) else {
        return;
    };
    let asks = bench.asks.clone();
    cx.spawn(async move |_, _| {
        let Ok(id) = core::request(&asks, "newTerminal", json!({ "project": project })).await
        else {
            return;
        };
        let typed = json!({ "id": id, "draft": format!("{command}\r") });
        let _ = core::request(&asks, "typeWhenReady", typed).await;
    })
    .detach();
    crate::settings::close(bench, window, cx);
}

/// Signs the agent in on the account it runs on now.
fn sign_in(bench: &mut Workbench, provider: &str, cx: &mut Context<Workbench>) {
    if let Some(id) = bench
        .accounts
        .current(provider)
        .map(|profile| profile.id.clone())
    {
        crate::accounts::sign_in(bench, &id, cx);
    }
}

pub fn render(
    bench: &Workbench,
    setup: &Setup,
    lit: &dyn Fn(&str) -> bool,
    this: &Entity<Workbench>,
    cx: &mut Context<Workbench>,
) -> AnyElement {
    let body: AnyElement = match &setup.found {
        None => rows::note("Looking for agents…", cx),
        Some(found) => div()
            .children(found.iter().map(|agent| {
                let detail: Option<SharedString> = match &agent.status {
                    AgentStatus::Broken { reason } => Some(reason.clone().into()),
                    AgentStatus::Missing => Some(agent.about.clone().into()),
                    _ => None,
                };
                let about: SharedString = match &detail {
                    Some(detail) => format!("{} · {detail}", status_label(&agent.status)).into(),
                    None => status_label(&agent.status).into(),
                };
                let signed_out = matches!(agent.status, AgentStatus::SignedOut)
                    && bench.accounts.current(&agent.kind).is_some();
                let broken = matches!(agent.status, AgentStatus::Broken { .. });
                let missing = matches!(agent.status, AgentStatus::Missing);
                let actions =
                    div()
                        .flex()
                        .gap(px(8.))
                        .children(signed_out.then(|| {
                            let (this, provider) = (this.clone(), agent.kind.clone());
                            ui::Button::new(
                                SharedString::from(format!("agent-sign-in-{}", agent.kind)),
                                "Sign in",
                                ui::Variant::Outline,
                            )
                            .on_click(move |_, _, cx| {
                                let provider = provider.clone();
                                this.update(cx, |bench, cx| sign_in(bench, &provider, cx))
                            })
                        }))
                        .children(agent.install.clone().filter(|_| missing || broken).map(
                            |command| {
                                let this = this.clone();
                                ui::Button::new(
                                    SharedString::from(format!("agent-install-{}", agent.kind)),
                                    if broken { "Reinstall" } else { "Install" },
                                    ui::Variant::Outline,
                                )
                                .on_click(move |_, window, cx| {
                                    let command = command.clone();
                                    this.update(cx, |bench, cx| install(bench, command, window, cx))
                                })
                            },
                        ));
                rows::row_with(
                    Some(ui::mark(&agent.kind, 18.)),
                    agent.label.clone(),
                    Some(about),
                    actions,
                    cx,
                )
                .when(missing, |row| row.opacity(0.75))
                .into_any_element()
            }))
            .into_any_element(),
    };
    let ready = setup.found.as_ref().map(|found| {
        let ready = found
            .iter()
            .filter(|agent| matches!(agent.status, AgentStatus::Ready { .. }))
            .count();
        SharedString::from(format!("{ready} ready"))
    });
    rows::section(
        "Agents on this machine",
        ready,
        Some("Install an agent here and it lands where its own updater expects, so it works in your terminal too.".into()),
        lit("Agents on this machine"),
        body,
        cx,
    )
}
