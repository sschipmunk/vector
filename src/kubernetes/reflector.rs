//! Watch and cache the remote Kubernetes API resources.

use super::{
    hash_value::HashValue,
    resource_version,
    watcher::{self, Watcher},
};
use evmap10::WriteHandle;
use futures::{pin_mut, stream::StreamExt};
use k8s_openapi::{
    apimachinery::pkg::apis::meta::v1::{ObjectMeta, WatchEvent},
    Metadata, WatchOptional, WatchResponse,
};
use snafu::Snafu;
use std::time::Duration;
use tokio::time::delay_for;

/// Watches remote Kubernetes resources and maintains a local representation of
/// the remote state. "Reflects" the remote state locally.
///
/// Does not expose evented API, but keeps track of the resource versions and
/// will automatically resume on desync.
pub struct Reflector<W>
where
    W: Watcher,
    <W as Watcher>::Object: Metadata<Ty = ObjectMeta>,
{
    watcher: W,
    state_writer: WriteHandle<String, Value<<W as Watcher>::Object>>,
    field_selector: Option<String>,
    label_selector: Option<String>,
    resource_version: resource_version::State,
    pause_between_requests: Duration,
}

impl<W> Reflector<W>
where
    W: Watcher,
    <W as Watcher>::Object: Metadata<Ty = ObjectMeta>,
{
    /// Create a new [`Cache`].
    pub fn new(
        watcher: W,
        state_writer: WriteHandle<String, Value<<W as Watcher>::Object>>,
        field_selector: Option<String>,
        label_selector: Option<String>,
        pause_between_requests: Duration,
    ) -> Self {
        let resource_version = resource_version::State::new();
        Self {
            watcher,
            state_writer,
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
    <W as Watcher>::Object: Metadata<Ty = ObjectMeta> + Unpin + std::fmt::Debug,
    <W as Watcher>::InvocationError: Unpin,
    <W as Watcher>::StreamError: Unpin,
{
    /// Run the watch loop.
    pub async fn run(&mut self) -> Result<WatchEvent<<W as Watcher>::Object>, Error<W>> {
        self.state_writer.purge();
        loop {
            let invocation_result = self.issue_request().await;
            let stream = match invocation_result {
                Ok(val) => val,
                Err(watcher::invocation::Error::Desync { source }) => {
                    // TODO: handle desync.
                    return Err(Error::Invocation { source });
                }
                Err(watcher::invocation::Error::Other { source }) => {
                    // Not a desync, fail everything.
                    return Err(Error::Invocation { source });
                }
            };

            pin_mut!(stream);
            while let Some(response) = stream.next().await {
                // Any streaming error means the protocol is in an unxpected
                // state. This is considered a fatal error, do not attempt
                // to retry and just quit.
                let response = response.map_err(|source| Error::Streaming { source })?;

                // Unpack the event.
                let event = match response {
                    WatchResponse::Ok(event) => event,
                    WatchResponse::Other(_) => {
                        // Even though we could parse the response, we didn't
                        // get the data we expected on the wire.
                        // According to the rules, we just ignore the unknown
                        // responses. This may be a newly added piece of data
                        // our code doesn't know of.
                        warn!("Got unexpected data in the watch response");
                        continue;
                    }
                };

                // Prepare a resource version cvandidate so we can update (aka
                // commit) it later.
                let resource_version_candidate =
                    match resource_version::Candidate::from_watch_event(&event) {
                        Some(val) => val,
                        None => {
                            // This event doesn't have a resource version, this means
                            // it's not something we care about.
                            continue;
                        }
                    };

                // Process the event.
                self.process_event(event);

                // Record the resourse version for this event, so when we resume
                // it won't be redelivered.
                self.resource_version.update(resource_version_candidate);

                // Flush the changes to the state.
                self.state_writer.flush();
            }

            // For the next pause duration we won't get any updates.
            // This is better than flooding k8s api server with requests.
            delay_for(self.pause_between_requests).await;
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

    fn process_event(&mut self, event: WatchEvent<<W as Watcher>::Object>) {
        match event {
            WatchEvent::Added(object) => {
                trace!(message = "Got an object for processing (added)", ?object);
                if let Some((key, value)) = kv(object) {
                    self.state_writer.insert(key, value);
                }
            }
            WatchEvent::Deleted(object) => {
                trace!(message = "Got an object for processing (deleted)", ?object);
                if let Some((key, _value)) = kv(object) {
                    self.state_writer.clear(key);
                }
            }
            WatchEvent::Modified(object) => {
                trace!(message = "Got an object for processing (modified)", ?object);
                if let Some((key, value)) = kv(object) {
                    self.state_writer.update(key, value);
                }
            }
            WatchEvent::Bookmark(object) => {
                // noop
                trace!(message = "Got an object for processing (bookmark)", ?object);
            }
            _ => unreachable!("other event types should never reach this code"),
        }
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

/// An alias to the value used at [`evmap`].
pub type Value<T> = Box<HashValue<T>>;

/// Build a key value pair for using in [`evmap`].
fn kv<T: Metadata<Ty = ObjectMeta>>(object: T) -> Option<(String, Value<T>)> {
    let value = Box::new(HashValue::new(object));
    let key = value.uid()?.to_owned();
    Some((key, value))
}
