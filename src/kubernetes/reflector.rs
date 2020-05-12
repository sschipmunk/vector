//! Watch and cache the remote Kubernetes API resources.

use super::{
    watch_state::ResourceVersionState,
    watcher::{self, Watcher},
};
use async_stream::try_stream;
use futures::{
    pin_mut,
    stream::{Stream, StreamExt},
};
use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::WatchEvent, Metadata, WatchOptional, WatchResponse,
};
use kube::api::ObjectMeta;
use snafu::Snafu;
use std::time::Duration;
use tokio::time::delay_for;

/// Watches remote Kubernetes resources and maintains a local representation of
/// the remote state. "Reflects" the remote state locally.
///
/// Does not expose evented API, but keeps track of the resource versions and
/// will automatically resume on desync.
pub struct Reflector<W> {
    watcher: W,
    field_selector: Option<String>,
    label_selector: Option<String>,
    resource_version: ResourceVersionState,
    pause_between_requests: Duration,
}

impl<W> Reflector<W> {
    /// Create a new [`Cache`].
    pub fn new(
        watcher: W,
        field_selector: Option<String>,
        label_selector: Option<String>,
        pause_between_requests: Duration,
    ) -> Self {
        let resource_version = ResourceVersionState::new();
        Self {
            watcher,
            label_selector,
            field_selector,
            resource_version,
            pause_between_requests,
        }
    }
}

impl<W> Reflector<W>
where
    W: Watcher,
    <W as Watcher>::Object: Metadata<Ty = ObjectMeta> + Unpin,
    <W as Watcher>::InvocationError: Unpin,
    <W as Watcher>::StreamError: Unpin,
{
    /// Run the watch loop.
    pub fn run(
        &mut self,
    ) -> impl Stream<Item = Result<WatchEvent<<W as Watcher>::Object>, Error<W>>> + '_ {
        try_stream! {
            loop {
                let invocation_result = self.issue_request().await;
                let stream = match invocation_result {
                    Ok(val) => val,
                    Err(watcher::invocation::Error::Desync{ source }) => {
                        /// TODO: handle desync.
                        Err(Error::Invocation { source })?;
                        return;
                    }
                    Err(watcher::invocation::Error::Other{ source }) => {
                        /// Not a desync, fail everything.
                        Err(Error::Invocation { source })?;
                        return;
                    }
                };

                pin_mut!(stream);
                while let Some(item) = stream.next().await {
                    // Any error here is considered critical, do not attempt
                    // to retry and just quit.
                    let item = item.map_err(|source| Error::Streaming { source })?;

                    let item = match item {
                        WatchResponse::Ok(item) => item,
                        WatchResponse::Other(item) => Err(Error::InvalidDataOnStream)?,
                    };

                    self.resource_version.update(&item);

                    yield item;
                }

                // For the next pause duration we won't get any updates.
                // This is better than flooding k8s api server with requests.
                delay_for(self.pause_between_requests).await;
            }
        }
    }

    async fn issue_request(
        &mut self,
    ) -> Result<<W as Watcher>::Stream, watcher::invocation::Error<<W as Watcher>::InvocationError>>
    {
        let watch_options = WatchOptional {
            field_selector: self.field_selector.as_ref().map(|s| s.as_str()),
            label_selector: self.label_selector.as_ref().map(|s| s.as_str()),
            pretty: None,
            resource_version: self.resource_version.get(),
            timeout_seconds: None,
            allow_watch_bookmarks: Some(true),
        };
        let stream = self.watcher.watch(watch_options).await?;
        Ok(stream)
    }
}

/// Errors that can occur while watching.
#[derive(Debug, Snafu)]
pub enum Error<W>
where
    W: Watcher,
{
    ///
    #[snafu(display("..."))]
    Invocation {
        /// The underlying invocation error.
        source: <W as Watcher>::InvocationError,
    },
    ///
    #[snafu(display("..."))]
    InvalidDataOnStream,
    ///
    #[snafu(display("..."))]
    Streaming {
        /// The underlying stream error.
        source: <W as Watcher>::StreamError,
    },
}
