//! This mod implements `kubernetes_logs` source.
//! The scope of this source is to consume the log files that `kubelet` keeps
//! at `/var/logs/pods` at the host of the k8s node.

#![deny(missing_docs)]

use crate::kubernetes as k8s;
use crate::{
    dns::Resolver,
    event::Event,
    shutdown::ShutdownSignal,
    sources,
    topology::config::{DataType, GlobalOptions, SourceConfig, SourceDescription},
};
use futures::{future::FutureExt, stream::StreamExt};
use futures01::sync::mpsc;
use k8s_openapi::{ WatchOptional, WatchResponse};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// # TODO List
//
// - Connect to the k8s API and poll for `Pod`s.
// - Filter the files to read from the `Pod`s list.
// - Read files.
//   - JsonLines format.
//   - CRI format.
//   - Automatic partial merge.

/// Configuration for the `kubernetes_logs` source.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    self_node_name: String,
}

inventory::submit! {
    SourceDescription::new_without_default::<Config>(COMPONENT_NAME)
}

const COMPONENT_NAME: &str = "kubernetes_logs";

#[typetag::serde(name = "kubernetes_logs")]
impl SourceConfig for Config {
    fn build(
        &self,
        _name: &str,
        globals: &GlobalOptions,
        shutdown: ShutdownSignal,
        out: mpsc::Sender<Event>,
    ) -> crate::Result<sources::Source> {
        // TODO: switch to the ones provided by the topology once PRs for that
        // pass.
        let rt = crate::runtime::Runtime::new()?;
        let exec = rt.executor();
        let resolver = Resolver::new(globals.dns_servers.clone(), exec)?;

        let source = Source::init(self, resolver)?;

        // TODO: this is a workaround for the legacy futures 0.1.
        // When the core is updated to futures 0.3 this should be simplied
        // significantly.
        let fut = source.run(out, shutdown);
        let fut = fut.map(|result| {
            result.map_err(|error| {
                error!(message = "source future failed", ?error);
            })
        });
        let fut = Box::pin(fut);
        let fut = futures::compat::Compat::new(fut);
        let fut: sources::Source = Box::new(fut);
        Ok(fut)
    }

    fn output_type(&self) -> DataType {
        DataType::Log
    }

    fn source_type(&self) -> &'static str {
        COMPONENT_NAME
    }
}

#[derive(Clone)]
struct Source {
    client: k8s::client::Client,
    self_node_name: String,
}

impl Source {
    fn init(config: &Config, resolver: Resolver) -> crate::Result<Self> {
        let self_node_name = if config.self_node_name.is_empty() {
            std::env::var("VECTOR_SELF_NODE_NAME")
                .map_err(|_| "VECTOR_SELF_NODE_NAME is not set")?
        } else {
            config.self_node_name.clone()
        };
        info!(
            message = "Obtained Kubernetes node name to collect logs for (self).",
            ?self_node_name
        );

        let k8s_config = k8s::client::config::Config::in_cluster()?;
        let client = k8s::client::Client::new(k8s_config, resolver)?;

        Ok(Self {
            client,
            self_node_name,
        })
    }

    async fn run(self, out: mpsc::Sender<Event>, shutdown: ShutdownSignal) -> crate::Result<()> {
        // Poll k8s metadata;
        // Read files based on k8s metadata;
        // Enhance events with k8s metadata;

        let Self {
            mut client,
            self_node_name,
        } = self;

        let field_selector = format!("spec.nodeName={}", self_node_name);

        let mut watcher = k8s::pods_watcher::PodsWatcher::new(self.client, Some(field_selector), None);
        watcher.watch().await;

        let _ = futures::compat::Compat01As03::new(shutdown).await;

        info!("Done");
        drop(out);
        Ok(())
    }
}

struct K8sPathsProvider {
    // reflector: Reflector<Api<Pod>>,
}

impl K8sPathsProvider {
    fn paths_provider(&self) -> &[PathBuf] {
        // self.reflector
        todo!()
    }
}
