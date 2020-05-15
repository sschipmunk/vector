//! This mod implements `kubernetes_logs` source.
//! The scope of this source is to consume the log files that `kubelet` keeps
//! at `/var/logs/pods` at the host of the k8s node.

#![deny(missing_docs)]

mod k8s_paths_provider;
mod parser;

use crate::event::{self, Event};
use crate::kubernetes as k8s;
use crate::{
    dns::Resolver,
    shutdown::ShutdownSignal,
    sources,
    topology::config::{DataType, GlobalOptions, SourceConfig, SourceDescription},
};
use bytes05::Bytes;
use evmap10::{self as evmap};
use file_source::{FileServer, Fingerprinter};
use futures::{future::FutureExt, SinkExt};
use futures01::sync::mpsc;
use k8s_paths_provider::K8sPathsProvider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::task::spawn_blocking;

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
        name: &str,
        globals: &GlobalOptions,
        shutdown: ShutdownSignal,
        out: mpsc::Sender<Event>,
    ) -> crate::Result<sources::Source> {
        // TODO: switch to the ones provided by the topology once PRs for that
        // pass.
        let rt = crate::runtime::Runtime::new()?;
        let exec = rt.executor();
        let resolver = Resolver::new(globals.dns_servers.clone(), exec)?;

        let source = Source::init(self, resolver, globals, name)?;

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
    data_dir: PathBuf,
}

impl Source {
    fn init(
        config: &Config,
        resolver: Resolver,
        globals: &GlobalOptions,
        name: &str,
    ) -> crate::Result<Self> {
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

        let data_dir = globals.resolve_and_make_data_subdir(None, name)?;

        Ok(Self {
            client,
            self_node_name,
            data_dir,
        })
    }

    async fn run(self, out: mpsc::Sender<Event>, shutdown: ShutdownSignal) -> crate::Result<()> {
        // Poll k8s metadata;
        // Read files based on k8s metadata;
        // Enhance events with k8s metadata;

        let Self {
            client,
            self_node_name,
            data_dir,
        } = self;

        let field_selector = format!("spec.nodeName={}", self_node_name);

        let watcher = k8s::api_watcher::ApiWatcher::new(client);
        let (state_reader, state_writer) = evmap::new();

        let mut reflector = k8s::reflector::Reflector::new(
            watcher,
            state_writer,
            Some(field_selector),
            None,
            std::time::Duration::from_secs(1),
        );

        let paths_provider = K8sPathsProvider::new(state_reader);

        // TODO: maybe some of the parameters have to be configurable.
        let file_server = FileServer {
            paths_provider,
            max_read_bytes: 2048,
            start_at_beginning: true,
            ignore_before: None,
            max_line_bytes: bytesize::kib(100u64) as usize,
            data_dir,
            glob_minimum_cooldown: Duration::from_secs(1),
            fingerprinter: Fingerprinter::Checksum {
                fingerprint_bytes: 256,
                ignored_header_bytes: 0,
            },
            oldest_first: false,
        };

        let _ = reflector.run().await;

        let (tx, rx) = futures::channel::mpsc::channel(0);

        let span = info_span!("file_server");
        let file_server_fut = spawn_blocking(move || {
            let _enter = span.enter();
            file_server.run(
                tx.sink_map_err(drop),
                futures::compat::Compat01As03::new(shutdown),
            );
        });

        file_server_fut.await.map_err(|error| {
            error!(message = "File server unexpectedly stopped.", %error);
        });

        info!("Done");
        drop(out);
        Ok(())
    }
}

/// Here we have `bytes05::Bytes`, but the `Event` has `bytes::Bytes`.
/// This is a really silly situation, but we have to work around it.
/// And this is *not* fast.
fn convert_bytes(from: Bytes) -> bytes::Bytes {
    bytes::Bytes::from(from.as_ref())
}

fn create_event(line: Bytes) -> Event {
    let mut event = Event::from(convert_bytes(line));

    // Add source type.
    event
        .as_mut_log()
        .insert(event::log_schema().source_type_key(), COMPONENT_NAME);

    event
}
