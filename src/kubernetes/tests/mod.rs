#[cfg(test)]
mod kube_tests {
    #![cfg(feature = "kubernetes-integration-tests")]

    use super::ClientConfig;
    use crate::{
        dns::Resolver,
        sources::kubernetes::test::{echo, Kube},
        test_util::{runtime, temp_file},
        tls::{TlsOptions, TlsSettings},
    };
    use dirs;
    use futures01::{future::Future, stream::Stream};
    use http::Uri;
    use kube::config::Config;
    use serde_yaml;
    use snafu::{ResultExt, Snafu};
    use std::{
        fs::{File, OpenOptions},
        io::Write,
        path::PathBuf,
        str::FromStr,
        sync::mpsc::channel,
        time::Duration,
    };
    use uuid::Uuid;

    /// Enviorment variable that can containa path to kubernetes config file.
    const CONFIG_PATH: &str = "KUBECONFIG";

    fn store_to_file(data: &[u8]) -> Result<PathBuf, std::io::Error> {
        let path = temp_file();

        let mut file = OpenOptions::new().write(true).open(path.clone())?;
        file.write_all(data)?;
        file.sync_all()?;

        Ok(path)
    }

    /// Loads configuration from local kubeconfig file, the same
    /// one that kubectl uses.
    /// None if such file doesn't exist.
    fn load_kube_config() -> Option<Result<Config, KubeConfigLoadError>> {
        let path = std::env::var(CONFIG_PATH)
            .ok()
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".kube").join("config")))?;

        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
            Err(error) => {
                return Some(Err(KubeConfigLoadError::FileError { source: error }));
            }
        };

        Some(serde_yaml::from_reader(file).context(ParsingError))
    }

    #[derive(Debug, Snafu)]
    pub enum KubeConfigLoadError {
        #[snafu(display("Error opening Kubernetes config file: {}.", source))]
        FileError { source: std::io::Error },
        #[snafu(display("Error parsing Kubernetes config file: {}.", source))]
        ParsingError { source: serde_yaml::Error },
    }

    impl ClientConfig {
        // NOTE: Currently used only for tests, but can be later used in
        //       other places, but then the unsupported feature should be
        //       implemented.
        //
        /// Loads configuration from local kubeconfig file, the same
        /// one that kubectl uses.
        fn load_kube_config(resolver: Resolver) -> Option<Self> {
            let config = load_kube_config()?.unwrap();
            // Get current context
            let context = &config
                .contexts
                .iter()
                .find(|context| context.name == config.current_context)?
                .context;
            // Get current user
            let user = &config
                .auth_infos
                .iter()
                .find(|user| user.name == context.user)?
                .auth_info;
            // Get current cluster
            let cluster = &config
                .clusters
                .iter()
                .find(|cluster| cluster.name == context.cluster)?
                .cluster;
            // The not yet supported features
            assert!(user.username.is_none(), "Not yet supported");
            assert!(user.password.is_none(), "Not yet supported");
            assert!(user.token_file.is_none(), "Not yet supported");
            assert!(user.client_key_data.is_none(), "Not yet supported");

            let certificate_authority_path = cluster
                .certificate_authority
                .clone()
                .map(PathBuf::from)
                .or_else(|| {
                    cluster.certificate_authority_data.as_ref().map(|data| {
                        store_to_file(data.as_bytes())
                            .expect("Failed to store certificate authority public key.")
                    })
                });

            let client_certificate_path = user
                .client_certificate
                .clone()
                .map(PathBuf::from)
                .or_else(|| {
                    user.client_certificate_data.as_ref().map(|data| {
                        store_to_file(data.as_bytes()).expect("Failed to store clients public key.")
                    })
                });

            // Construction
            Some(ClientConfig {
                resolver,
                node_name: None,
                token: user.token.clone(),
                server: Uri::from_str(&cluster.server).unwrap(),
                tls_settings: TlsSettings::from_options(&Some(TlsOptions {
                    verify_certificate: cluster.insecure_skip_tls_verify,
                    ca_path: certificate_authority_path,
                    crt_path: client_certificate_path,
                    key_path: user.client_key.clone().map(PathBuf::from),
                    ..TlsOptions::default()
                }))
                .unwrap(),
            })
        }
    }

    #[test]
    fn watch_pod() {
        let namespace = format!("watch-pod-{}", Uuid::new_v4());
        let kube = Kube::new(namespace.as_str());

        let mut rt = runtime();

        let (sender, receiver) = channel();

        // May pickup other pods, which is fine.
        let mut client =
            ClientConfig::load_kube_config(Resolver::new(Vec::new(), rt.executor()).unwrap())
                .expect("Kubernetes configuration file not present.")
                .build()
                .unwrap();

        let stream = client.watch_metadata(None, None).unwrap();

        rt.spawn(
            stream
                .map(move |_| sender.send(()))
                .into_future()
                .map(|_| ())
                .map_err(|(error, _)| error!(?error)),
        );

        // Start echo
        let _echo = echo(&kube, "echo", "210");

        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("Client did not see a Pod change.");
    }
}
