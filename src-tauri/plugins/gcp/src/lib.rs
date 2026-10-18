// Google Cloud: a console over the projects the person's gcloud accounts can
// see, and gcp_* tools for agents.
//
//   gcloud   — the gcloud CLI, which holds the sign-in; nothing here stores a credential
//   auth     — accounts, configurations, and the access tokens gcloud hands out
//   client   — Google's REST APIs, pages and errors
//   projects — the projects an account can see
//   run      — Cloud Run services, revisions, traffic, jobs and executions

mod auth;
mod client;
mod error;
mod gcloud;
mod projects;
mod run;

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;
use sikemux_plugin_api::{
    params, reply, Manifest, Plugin, PluginContext, PluginError, PluginFuture, StreamSink,
};

use crate::error::GcpResult;

pub fn plugin() -> Result<Arc<dyn Plugin>, PluginError> {
    Ok(Arc::new(Gcp {
        manifest: Manifest::from_json(include_str!("../manifest.json"))?,
    }))
}

struct Gcp {
    manifest: Manifest,
}

async fn answer<T: Serialize>(
    result: impl std::future::Future<Output = GcpResult<T>>,
) -> Result<Value, PluginError> {
    reply(result.await?)
}

impl Plugin for Gcp {
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
                "status" => reply(auth::status().await),
                "projects" => answer(projects::projects(params(input)?)).await,
                "runServices" => answer(run::services(params(input)?)).await,
                "runService" => answer(run::service_detail(params(input)?)).await,
                "runSetTraffic" => answer(run::set_traffic(params(input)?)).await,
                "runJobs" => answer(run::jobs(params(input)?)).await,
                "runJob" => answer(run::job_detail(params(input)?)).await,
                "runJobExecute" => answer(run::run_job(params(input)?)).await,
                _ => Err(PluginError::unknown_method(method)),
            }
        })
    }

    fn stream<'a>(
        &'a self,
        _ctx: &'a PluginContext,
        method: &'a str,
        _input: Value,
        sink: StreamSink,
    ) -> PluginFuture<'a, ()> {
        Box::pin(async move {
            match method {
                "signIn" => {
                    auth::sign_in().await?;
                    sink.send(reply(auth::status().await)?)
                }
                "signInApplicationDefault" => {
                    auth::sign_in_application_default().await?;
                    sink.send(reply(auth::status().await)?)
                }
                _ => Err(PluginError::unknown_method(method)),
            }
        })
    }

    /// Google Cloud is not tied to a repository, so its tools are offered wherever gcloud has signed someone in.
    fn offers_agent_tools<'a>(
        &'a self,
        _ctx: &'a PluginContext,
        _remotes: &'a [String],
    ) -> PluginFuture<'a, bool> {
        Box::pin(async { Ok(auth::signed_in_here()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_manifest_parses() {
        let plugin = plugin().expect("manifest parses");
        assert_eq!(plugin.manifest().id, "sikemux.gcp");
    }
}
