//! How the person wants Sikemux to behave, kept here so every app follows
//! the same choices. Each one's default is `Preferences::default()`.

use std::path::Path;
use std::sync::Mutex;

use serde_json::{Map, Value};
use sikemux_process::sync::Locked;

use super::kept::Kept;
use super::{CoreError, CoreResult};
use crate::protocol::Preferences;

const FILE: &str = "preferences.json";

pub(crate) struct Prefs {
    held: Mutex<Map<String, Value>>,
    kept: Kept,
}

impl Prefs {
    pub(crate) fn new(data_dir: Option<&Path>) -> Self {
        let kept = Kept::new(data_dir, FILE);
        let mut held = defaults();
        let saved: Value = kept.load();
        for (key, value) in saved.as_object().into_iter().flatten() {
            let _ = put(&mut held, key, value.clone());
        }
        Self {
            held: Mutex::new(held),
            kept,
        }
    }

    pub(crate) fn get(&self) -> Preferences {
        read(&self.held.locked())
    }

    /// Sets the preference `key`, refusing a key there is none of or a value
    /// it cannot hold.
    pub(crate) fn set(&self, key: &str, value: Value) -> CoreResult<Preferences> {
        let mut held = self.held.locked();
        let preferences = put(&mut held, key, value)?;
        self.kept.save(&*held);
        Ok(preferences)
    }
}

/// Every preference's name.
pub(crate) fn keys() -> Vec<String> {
    defaults().keys().cloned().collect()
}

fn defaults() -> Map<String, Value> {
    match serde_json::to_value(Preferences::default()) {
        Ok(Value::Object(fields)) => fields,
        _ => Map::new(),
    }
}

fn read(held: &Map<String, Value>) -> Preferences {
    serde_json::from_value(Value::Object(held.clone())).unwrap_or_default()
}

fn put(held: &mut Map<String, Value>, key: &str, value: Value) -> CoreResult<Preferences> {
    if !held.contains_key(key) {
        return Err(CoreError::from(format!("there is no preference named {key}")));
    }
    let mut next = held.clone();
    next.insert(key.to_owned(), value);
    let preferences = serde_json::from_value(Value::Object(next.clone()))
        .map_err(|_| CoreError::from(format!("that is not a value {key} can have")))?;
    *held = next;
    Ok(preferences)
}

