//! The async runtime the engine's own tasks run on. The app that hosts the
//! engine hands its runtime over once, since a tab's work can start on any
//! thread, the main one included.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::Handle;

static RUNTIME: OnceLock<Handle> = OnceLock::new();

pub fn set_runtime(handle: Handle) {
    let _ = RUNTIME.set(handle);
}

pub(crate) fn spawn<F>(work: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    match RUNTIME.get() {
        Some(runtime) => {
            runtime.spawn(work);
        }
        None => {
            tokio::spawn(work);
        }
    }
}

/// Waits on `work` from a thread that is not one of the runtime's own.
pub(crate) fn block_on<F: Future>(work: F) -> F::Output {
    match RUNTIME.get() {
        Some(runtime) => runtime.block_on(work),
        None => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime for one call")
            .block_on(work),
    }
}
