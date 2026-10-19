// Kubernetes: the clusters in the person's kubeconfig, their workloads, pods,
// logs and events, in a pane and as agent tools. It runs the person's own kubectl,
// so every way their clusters sign in keeps working.
//
//   kubectl  — running kubectl in a context and namespace, and its errors in words
//   contexts — the kubeconfig's clusters, the current one, and each one's namespaces
//   pods     — pods with their status as kubectl words it, restarts and owning workload

mod contexts;
mod error;
mod kubectl;
mod pods;

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use sikemux_plugin_api::{
    params, reply, Manifest, Plugin, PluginContext, PluginError, PluginFuture,
};

use crate::error::KubeResult;

pub fn plugin() -> Result<Arc<dyn Plugin>, PluginError> {
    Ok(Arc::new(Kubernetes {
        manifest: Manifest::from_json(include_str!("../manifest.json"))?,
    }))
}

struct Kubernetes {
    manifest: Manifest,
}

async fn answer<T: Serialize>(
    result: impl std::future::Future<Output = KubeResult<T>>,
) -> Result<Value, PluginError> {
    reply(result.await?)
}

impl Plugin for Kubernetes {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn call<'a>(
        &'a self,
        _ctx: &'a PluginContext,
        method: &'a str,
        input: Value,
    ) -> PluginFuture<'a, Value> {
        Box::pin(async move {
            match method {
                "status" => reply(contexts::status().await),
                "contexts" => answer(contexts::contexts()).await,
                "namespaces" => answer(contexts::namespaces(params(input)?)).await,
                "pods" => answer(pods::list(params(input)?)).await,
                _ => Err(PluginError::unknown_method(method)),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads the clusters in this Mac's kubeconfig: `cargo test -p sikemux-plugin-kubernetes -- --ignored`.
    #[tokio::test]
    #[ignore = "needs kubectl and a reachable cluster"]
    async fn against_the_local_cluster() {
        let plugin = plugin().expect("plugin loads");
        let ctx = PluginContext::new(std::env::temp_dir());
        let status = plugin
            .call(&ctx, "status", Value::Null)
            .await
            .expect("status");
        println!("{status:#}");
        assert_eq!(status["ready"], true);
        let namespaces = plugin
            .call(&ctx, "namespaces", serde_json::json!({}))
            .await
            .expect("namespaces");
        println!("{namespaces:#}");
        let pods = plugin
            .call(&ctx, "pods", serde_json::json!({ "allNamespaces": true }))
            .await
            .expect("pods");
        for pod in pods.as_array().into_iter().flatten() {
            println!(
                "{} {} {} {}/{} restarts {} {}",
                pod["namespace"],
                pod["name"],
                pod["status"],
                pod["ready"],
                pod["containers"],
                pod["restarts"],
                pod["owner"]
            );
        }
    }

    #[test]
    fn its_manifest_parses() {
        let plugin = plugin().expect("manifest parses");
        assert_eq!(plugin.manifest().id, "sikemux.kubernetes");
    }
}
