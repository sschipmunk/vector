use super::stream;
use futures::stream::StreamExt;
use http02::StatusCode;
use k8s_openapi::{api::core::v1::Pod, WatchOptional, WatchResponse};

pub struct WatchedState {
    field_selector: Option<String>,
    label_selector: Option<String>,
    resource_version: Option<String>,
}

impl WatchedState {
    pub fn new(field_selector: Option<String>, label_selector: Option<String>) -> Self {
        Self {
            label_selector,
            field_selector,
            resource_version: None,
        }
    }
}

impl WatchedState {
    pub async fn watch(&mut self) -> crate::Result<()> {
        loop {
            let watch_options = WatchOptional {
                field_selector: Some(&field_selector),
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
            let watch_stream = stream::body::<_, WatchResponse<T>>(body);

            while let Some(item) = watch_stream.next().await {
                info!("Got an item: {:?}", item);
            }
        }
    }
}
