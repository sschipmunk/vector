use super::{client::Client, stream as k8s_stream};
use async_stream::try_stream;
use futures::stream::{Stream, StreamExt};
use http02::StatusCode;
use hyper13::Error as BodyError;
use k8s_openapi::{api::core::v1::Pod, WatchOptional, WatchResponse};
// use snafu::Snafu;

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
    pub fn watch(&mut self) -> impl Stream<Item = Result<WatchResponse<Pod>, crate::Error>> {
        try_stream! {
            loop {
                let stream = self.watch_once().await?;
                while let Some(item) = stream.next().await {
                    let item = item?;
                    yield item;
                }
            }
        }
    }

    async fn watch_once(
        &mut self,
    ) -> crate::Result<impl Stream<Item = Result<WatchResponse<Pod>, k8s_stream::Error<BodyError>>>>
    {
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
        Ok(stream::body::<_, WatchResponse<Pod>>(body))
    }
}

// /// Errors that can occur while watching.
// #[derive(Debug, Snafu)]
// pub enum Error
// {
//     /// Server .
//     #[snafu(display("reading the data chunk failed"))]
//     WatchRequest {
//         /// The error we got while reading.
//         source: ReadError,
//     },
// }
