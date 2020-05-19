//! Simple helpers for building and parsing k8s paths.
//!
//! Loosely based on https://github.com/kubernetes/kubernetes/blob/31305966789525fca49ec26c289e565467d1f1c4/pkg/kubelet/kuberuntime/helpers.go

use std::path::PathBuf;

/// The root directory for pod logs.
const K8S_LOGS_DIR: &str = "/var/log/pods";

/// The delimiter used in the log path.
const LOG_PATH_DELIMITER: &str = "_";

/// Builds absolute log directory path for a pod sandbox.
///
/// Based on https://github.com/kubernetes/kubernetes/blob/31305966789525fca49ec26c289e565467d1f1c4/pkg/kubelet/kuberuntime/helpers.go#L178
pub(super) fn build_pod_logs_directory(
    pod_namespace: &str,
    pod_name: &str,
    pod_uid: &str,
) -> PathBuf {
    [
        K8S_LOGS_DIR,
        &[pod_namespace, pod_name, pod_uid].join(LOG_PATH_DELIMITER),
    ]
    .iter()
    .collect()
}

/// Parses pod logs directory name and returns the pod UID.
///
/// Assumes the input is a valid pod directory name.
///
/// Based on https://github.com/kubernetes/kubernetes/blob/31305966789525fca49ec26c289e565467d1f1c4/pkg/kubelet/kuberuntime/helpers.go#L186
pub(super) fn parse_pod_uid_from_logs_directory(path: &str) -> String {
    path.rsplit(LOG_PATH_DELIMITER)
        .next()
        .map(ToOwned::to_owned)
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pod_logs_directory() {
        let cases = [
            (("", "", ""), "/var/log/pods/__"),
            (
                ("sandbox0-ns", "sandbox0-name", "sandbox0-uid"),
                "/var/log/pods/sandbox0-ns_sandbox0-name_sandbox0-uid",
            ),
        ];

        for ((in_namespace, in_name, in_uid), expected) in cases {
            assert_eq!(
                build_pod_logs_directory(in_namespace, in_name, in_uid),
                PathBuf::from(expected)
            );
        }
    }

    #[test]
    fn test_parse_pod_uid_from_logs_directory() {
        let cases = [
            (
                "/var/log/pods/sandbox0-ns_sandbox0-name_sandbox0-uid",
                "sandbox0-uid",
            ),
            ("/var/log/pods/other", "/var/log/pods/other"),
            ("qwe", ""),
            ("", ""),
        ];

        for (input, expected) in cases {
            assert_eq!(
                parse_pod_uid_from_logs_directory(input),
                expected.to_owned()
            );
        }
    }
}
