//! What waits on the person, as the core ranks it for every device. The app
//! watches the core's device view and hands the page its inbox whenever it
//! changes.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use sikemux_core::client::CoreClient;
use sikemux_core::protocol::Request;
use tauri::{AppHandle, Emitter};

use crate::pty::core_error;

const CHANGED_EVENT: &str = "inbox_changed";

/// Asks a core the app just connected to for its device view as it changes.
pub(crate) async fn connected(client: &Arc<CoreClient>) {
    let watched = async {
        client
            .submit(Request::WatchView, |reply| reply)
            .map_err(core_error)?
            .await
            .map_err(core_error)?
            .map_err(core_error)
    };
    if let Err(error) = watched.await {
        eprintln!("Sikemux could not watch its core's inbox: {error}");
    }
}

/// The inbox last handed to the page, so an unchanged one is not sent again.
#[derive(Default)]
pub(crate) struct LastInbox(Mutex<Value>);

impl LastInbox {
    pub(crate) fn changed(&self, app: &AppHandle, inbox: Value) {
        let Ok(mut last) = self.0.lock() else {
            return;
        };
        if *last == inbox {
            return;
        }
        if app.emit_to("main", CHANGED_EVENT, &inbox).is_ok() {
            *last = inbox;
        }
    }
}
