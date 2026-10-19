//! An agent that a linked host's agent started or messaged, or that an agent
//! like that started or messaged, uses none of this host's sign-ins until the
//! person allows that host.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::super::remote::{self, Pairing};
use super::super::{Core, CoreResult};

/// The linked hosts, by key, whose agents reached `agent_id` or an agent up
/// its chain of leads.
pub(crate) fn behind(core: &Core, agent_id: &str) -> BTreeSet<String> {
    let mut hosts = core.names.hosts_behind(agent_id);
    if let Some(foreign) = core.hosts.team.get(agent_id) {
        hosts.insert(foreign.host);
    }
    hosts
}

/// The names of the hosts behind `agent_id` that the person has not let use
/// this host's sign-ins.
fn barred(core: &Core, agent_id: &str) -> Vec<String> {
    let behind = behind(core, agent_id);
    if behind.is_empty() {
        return Vec::new();
    }
    let links = core.remote.links();
    behind
        .into_iter()
        .filter_map(|host| match links.iter().find(|link| link.id == host) {
            Some(link) if link.sign_ins => None,
            Some(link) => Some(link.name.clone()),
            None => Some("a host no longer linked with this one".to_owned()),
        })
        .collect()
}

pub(crate) fn allowed(core: &Core, agent_id: &str) -> bool {
    barred(core, agent_id).is_empty()
}

/// What `agent_id` is told when it tries `tool` while a host it may not use
/// sign-ins for is behind it.
pub(crate) fn refused(core: &Core, agent_id: &str, tool: &str) -> Option<String> {
    let barred = barred(core, agent_id);
    if barred.is_empty() {
        return None;
    }
    let hosts = barred.join(" and ");
    Some(format!(
        "{tool} is refused: an agent on {hosts} started or messaged you, or an agent above you, and the person has not allowed agents from {hosts} to use this host's sign-ins. Do this from an agent on {hosts} instead, or ask the person to allow it in Sikemux's Settings, under Devices."
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SignInsChange {
    id: String,
    allowed: bool,
}

/// The person lets agents from a linked host use this host's sign-ins, or
/// stops them.
pub(crate) fn set(core: &Core, change: SignInsChange) -> CoreResult<Pairing> {
    core.remote.set_link_sign_ins(&change.id, change.allowed)?;
    super::changed(core);
    Ok(remote::pairing(core))
}
