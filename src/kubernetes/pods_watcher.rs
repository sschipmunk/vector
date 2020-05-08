use super::{client::Client, stream};
use futures::{pin_mut, stream::StreamExt};
use http02::StatusCode;
use k8s_openapi::{api::core::v1::Pod, WatchOptional, WatchResponse};

pub struct PodsWatcher {
    client: Client,
    field_selector: Option<String>,
    label_selector: Option<String>,
    resource_version: Option<String>,
}

impl PodsWatcher {
    pub fn new(
        client: Client,
        field_selector: Option<String>,
        label_selector: Option<String>,
    ) -> Self {
        Self {
            client,
            label_selector,
            field_selector,
            resource_version: None,
        }
    }
}

impl PodsWatcher {
    pub async fn watch(&mut self) -> crate::Result<()> {
        loop {
            let watch_options = WatchOptional {
                field_selector: self.field_selector.map(|s| s.as_str()),
                label_selector: self.label_selector.map(|s| s.as_str()),
                ..WatchOptional::default()
            };
            let (request, _) = Pod::watch_pod_for_all_namespaces(watch_options)?;
            trace!(message = "Request prepared", ?request);

            let response = self.client.send(request).await?;
            trace!(message = "Got response", ?response);
            if response.status() != StatusCode::OK {
                Err("watch request failed")?;
            }

            let body = response.into_body();
            let watch_stream = stream::body::<_, WatchResponse<Pod>>(body);

            pin_mut!(watch_stream);
            while let Some(item) = watch_stream.next().await {
                let item = item?;
                match item {
                    WatchResponse::Ok(event) => {
                        info!("Got an event: {:?}", event);
                    }
                    WatchResponse::Other(_) => panic!("qwe"),
                }
            }
        }
    }
}
