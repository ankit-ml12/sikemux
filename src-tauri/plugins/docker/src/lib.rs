// Docker: the engine running on this Mac, its containers, Compose stacks,
// images and volumes, in a pane and as agent tools.
//
//   engine — finding the engine's socket, and HTTP over it
//   status — whether an engine is running, which one, and how much it holds

mod engine;
mod error;
mod status;

use std::sync::Arc;

use serde_json::Value;
use sikemux_plugin_api::{reply, Manifest, Plugin, PluginContext, PluginError, PluginFuture};

pub fn plugin() -> Result<Arc<dyn Plugin>, PluginError> {
    Ok(Arc::new(Docker {
        manifest: Manifest::from_json(include_str!("../manifest.json"))?,
    }))
}

struct Docker {
    manifest: Manifest,
}

impl Plugin for Docker {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call<'a>(
        &'a self,
        _ctx: &'a PluginContext,
        method: &'a str,
        _input: Value,
    ) -> PluginFuture<'a, Value> {
        Box::pin(async move {
            match method {
                "status" => reply(status::status().await),
                _ => Err(PluginError::unknown_method(method)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the engine running on this Mac: `cargo test -p sikemux-plugin-docker -- --ignored`.
    #[tokio::test]
    #[ignore = "needs a running Docker engine"]
    async fn against_the_local_engine() {
        let plugin = plugin().expect("plugin loads");
        let ctx = PluginContext::new(std::env::temp_dir());
        let status = plugin
            .call(&ctx, "status", Value::Null)
            .await
            .expect("status");
        println!("{status:#}");
        assert_eq!(status["running"], true);
    }

    #[test]
    fn its_manifest_parses() {
        let plugin = plugin().expect("manifest parses");
        assert_eq!(plugin.manifest().id, "sikemux.docker");
    }
}
