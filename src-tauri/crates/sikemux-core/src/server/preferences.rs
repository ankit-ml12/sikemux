//! How the person wants Sikemux to behave, kept here so every app follows
//! the same choices. Each one's default is `Preferences::default()`.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value};
use sikemux_process::sync::Locked;

use super::kept::Kept;
use super::{CoreError, CoreResult};
use crate::protocol::{
    Preferences, MAX_TERMINAL_FONT_SIZE, MAX_TEXT_SCALE, MIN_TERMINAL_FONT_SIZE, MIN_TEXT_SCALE,
    UI_TEXT_SCALES,
};

const FILE: &str = "preferences.json";

pub(crate) struct Prefs(Arc<Held>);

struct Held {
    values: Mutex<Map<String, Value>>,
    kept: Kept,
    /// A save is on its way and will write what `values` holds by then.
    saving: AtomicBool,
    /// Taken for each save, so a later one never writes before an earlier.
    writing: Mutex<()>,
}

impl Prefs {
    pub(crate) fn new(data_dir: Option<&Path>) -> Self {
        let kept = Kept::new(data_dir, FILE);
        let mut values = defaults();
        let saved: Value = kept.load();
        for (key, value) in saved.as_object().into_iter().flatten() {
            let _ = put(&mut values, key, value.clone());
        }
        Self(Arc::new(Held {
            values: Mutex::new(values),
            kept,
            saving: AtomicBool::new(false),
            writing: Mutex::new(()),
        }))
    }

    pub(crate) fn get(&self) -> Preferences {
        read(&self.0.values.locked())
    }

    /// Sets the preference `key`, refusing a key there is none of or a value
    /// it cannot hold.
    pub(crate) fn set(&self, key: &str, value: Value) -> CoreResult<Preferences> {
        let preferences = put(&mut self.0.values.locked(), key, value)?;
        self.save_soon();
        Ok(preferences)
    }

    /// Saves off the caller's thread; a burst of changes, such as a pinch's,
    /// is written once each save has finished.
    fn save_soon(&self) {
        if self.0.saving.swap(true, Ordering::AcqRel) {
            return;
        }
        let held = self.0.clone();
        let save = move || {
            let _writing = held.writing.locked();
            held.saving.store(false, Ordering::Release);
            let values = held.values.locked().clone();
            held.kept.save(&values);
        };
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => drop(runtime.spawn_blocking(save)),
            Err(_) => save(),
        }
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

fn read(values: &Map<String, Value>) -> Preferences {
    serde_json::from_value(Value::Object(values.clone())).unwrap_or_default()
}

fn put(values: &mut Map<String, Value>, key: &str, value: Value) -> CoreResult<Preferences> {
    if !values.contains_key(key) {
        return Err(CoreError::from(format!("there is no preference named {key}")));
    }
    let mut next = values.clone();
    next.insert(key.to_owned(), value);
    let preferences: Preferences = serde_json::from_value(Value::Object(next))
        .map_err(|_| CoreError::from(format!("that is not a value {key} can have")))?;
    let preferences = within_limits(preferences);
    if let Ok(Value::Object(fields)) = serde_json::to_value(&preferences) {
        *values = fields;
    }
    Ok(preferences)
}

fn within_limits(mut preferences: Preferences) -> Preferences {
    let scale = |value: f64| ((value * 100.0).round() / 100.0).clamp(MIN_TEXT_SCALE, MAX_TEXT_SCALE);
    preferences.chat_text_scale = scale(preferences.chat_text_scale);
    preferences.editor_text_scale = scale(preferences.editor_text_scale);
    preferences.terminal_font_size = preferences
        .terminal_font_size
        .clamp(MIN_TERMINAL_FONT_SIZE, MAX_TERMINAL_FONT_SIZE);
    preferences.ui_text_scale = UI_TEXT_SCALES
        .into_iter()
        .min_by(|a, b| {
            (a - preferences.ui_text_scale)
                .abs()
                .total_cmp(&(b - preferences.ui_text_scale).abs())
        })
        .unwrap_or(UI_TEXT_SCALES[0]);
    preferences.last_agent_type = preferences
        .last_agent_type
        .filter(|agent| crate::agent_catalog::agent(agent).is_some());
    preferences
}

