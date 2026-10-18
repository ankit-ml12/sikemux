//! Tells every client that something it reads through a `Call` changed, so
//! it reads that again. A burst of changes to one topic and project goes out
//! as one notice.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::Notify;

use crate::protocol::Event;

use super::Core;

const GATHER: Duration = Duration::from_millis(50);
/// Past this many paths a notice says anything may have changed.
const MAX_PATHS: usize = 512;

#[derive(Default)]
pub(crate) struct Changes {
    pending: Mutex<BTreeMap<(String, Option<String>), Option<BTreeSet<String>>>>,
    notify: Notify,
}

impl Changes {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<(String, Option<String>), Option<BTreeSet<String>>>> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// `topic` changed for `project`, or for every project when it is none.
    /// `paths` names what moved inside it; none means anything may have.
    pub(crate) fn changed(&self, topic: &str, project: Option<&str>, paths: Option<Vec<String>>) {
        let mut pending = self.lock();
        let key = (topic.to_owned(), project.map(str::to_owned));
        let known = pending.get(&key);
        let merged = match (known, paths) {
            (Some(None), _) | (_, None) => None,
            (Some(Some(known)), Some(paths)) => {
                let mut known = known.clone();
                known.extend(paths);
                Some(known)
            }
            (None, Some(paths)) => Some(paths.into_iter().collect()),
        };
        pending.insert(key, merged.filter(|paths| paths.len() <= MAX_PATHS));
        drop(pending);
        self.notify.notify_one();
    }
}

pub(crate) async fn serve(core: Arc<Core>) {
    loop {
        core.changes.notify.notified().await;
        tokio::time::sleep(GATHER).await;
        let pending = std::mem::take(&mut *core.changes.lock());
        for ((topic, project), paths) in pending {
            core.broadcast_event(&Event::Changed {
                topic,
                project,
                paths: paths.map(|paths| paths.into_iter().collect()),
            });
        }
    }
}
