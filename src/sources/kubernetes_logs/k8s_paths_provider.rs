//! A paths provider for k8s logs.

use crate::kubernetes as k8s;
use evmap10::ReadHandle;
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

    pub fn paths(&self) -> Vec<PathBuf> {
        let read_ref = match self.pods_state_reader.read() {
            Some(v) => v,
            None => return Vec::new(),
        };

        read_ref
            .into_iter()
            .map(|(uid, pod)| {
                info!(message = "got pod", ?uid, ?pod);
                "".into()
            })
            .collect()
    }
}
