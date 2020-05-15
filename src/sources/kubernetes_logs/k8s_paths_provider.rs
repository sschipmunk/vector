//! A paths provider for k8s logs.

use crate::kubernetes as k8s;
use evmap10::ReadHandle;
use file_source::paths_provider::PathsProvider;
use k8s_openapi::api::core::v1::Pod;
use std::path::PathBuf;

/// A paths provider implementation that uses the state obtained from the
/// the k8s API.
pub struct K8sPathsProvider {
    pods_state_reader: ReadHandle<String, k8s::reflector::Value<Pod>>,
}

impl K8sPathsProvider {
    pub fn new(pods_state_reader: ReadHandle<String, k8s::reflector::Value<Pod>>) -> Self {
        Self { pods_state_reader }
    }
}

impl PathsProvider for K8sPathsProvider {
    type IntoIter = Vec<PathBuf>;

    fn paths(&self) -> Vec<PathBuf> {
        let read_ref = match self.pods_state_reader.read() {
            Some(v) => v,
            None => return Vec::new(),
        };

        read_ref
            .into_iter()
            .flat_map(|(uid, values)| {
                let pod = values
                    .get_one()
                    .expect("we are supposed to be working with single-item values only");
                info!(message = "got pod", ?uid, ?pod);
                list_pod_log_paths(pod)
            })
            .collect()
    }
}

const K8S_LOGS_DIR: &str = "/var/log";
const LOG_PATH_DELIMITER: &str = "_";

/// Based on https://github.com/kubernetes/kubernetes/blob/31305966789525fca49ec26c289e565467d1f1c4/pkg/kubelet/kuberuntime/helpers.go#L178
fn build_pod_logs_directory(pod_namespace: &str, pod_name: &str, pod_uid: &str) -> PathBuf {
    [
        K8S_LOGS_DIR,
        &[pod_namespace, pod_name, pod_uid].join(LOG_PATH_DELIMITER),
    ]
    .iter()
    .collect()
}

fn extract_pod_logs_directory(pod: &Pod) -> Option<PathBuf> {
    let metadata = pod.metadata.as_ref()?;
    let namespace = metadata.namespace.as_ref()?;
    let name = metadata.name.as_ref()?;
    let uid = metadata.uid.as_ref()?;
    Some(build_pod_logs_directory(&namespace, &name, &uid))
}

// use std::fs::{read_dir, DirEntry};
// // This code needs generators.
// fn read_pod_dir(dir: impl AsRef<Path>) -> impl Iterator<Item = DirEntry> {
//     read_dir(dir)
//         .into_iter()
//         .flat_map(|read_dir| read_dir)
//         .flat_map(|entry_result| entry_result.into_iter())
// }
//
// fn list_pod_log_paths(pod: &Pod) -> impl Iterator<Item = PathBuf> + '_ {
//     extract_pod_logs_directory(pod)
//         .into_iter()
//         .flat_map(|dir| read_pod_dir(dir).map(|entry| entry.path()))
// }

fn list_pod_log_paths(pod: &Pod) -> impl Iterator<Item = PathBuf> + '_ {
    extract_pod_logs_directory(pod).into_iter().flat_map(|dir| {
        glob::glob(
            &[
                dir.to_str()
                    .expect("non-utf8 path to pod logs dir is not supported"),
                "/**/*.log",
            ]
            .join("/"),
        )
        .expect("the pattern is supposed to always be correct")
        .flat_map(|paths| paths.into_iter())
    })
}

// fn transforms() -> chain::Two<parser::Parser, partial_events_merger::Merger> {}
