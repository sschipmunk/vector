//! A fake watcher.

use super::watcher::{self, Watcher};
use future::BoxFuture;
use futures::future;
use futures::stream::{self, BoxStream};
use k8s_openapi::{WatchOptional, WatchResponse};
use serde::de::DeserializeOwned;
use snafu::Snafu;
use std::marker::PhantomData;

/// A fake watcher, useful for tests.
pub struct FakeWatcher<I, T> {
    iter: I,
    data_type: PhantomData<T>,
}

impl<I, T> FakeWatcher<I, T> {
    /// Create a new [`FakeWatcher`] from an iterator of results to stream.
    pub fn new(iter: I) -> Self {
        let data_type = PhantomData;
        Self { iter, data_type }
    }
}

impl<I, T> FakeWatcher<I, T>
where
    I: Iterator<Item = Result<WatchResponse<T>, Error>> + Clone + Send + Sync + 'static,
    T: DeserializeOwned + Send + Sync,
{
    async fn invoke_boxed_stream(
        &self,
        _watch_optional: WatchOptional<'_>,
    ) -> Result<
        BoxStream<'static, Result<WatchResponse<T>, Error>>,
        watcher::invocation::Error<Error>,
    > {
        Ok(Box::pin(stream::iter(self.iter.clone())))
    }
}

// impl<I, T> Watcher for FakeWatcher<I, T>
// where
//     I: Iterator<Item = Result<WatchResponse<T>, watcher::Error<Error>>> + Clone + Send + 'static,
//     T: DeserializeOwned,
// {
//     type Object = T;
//     type Error = Error;
//     type Stream =
//         BoxStream<'static, Result<WatchResponse<Self::Object>, watcher::Error<Self::Error>>>;

//     fn watch<'wo>(&mut self, _watch_optional: WatchOptional<'wo>) -> Self::Stream {
//         Box::pin(stream::iter(self.iter.clone()))
//     }
// }

impl<I, T> Watcher for FakeWatcher<I, T>
where
    I: Iterator<Item = Result<WatchResponse<T>, Error>> + Clone + Send + Sync + 'static,
    T: DeserializeOwned + Send + Sync,
{
    type Object = T;

    type StreamError = Error;
    type Stream = BoxStream<'static, Result<WatchResponse<Self::Object>, Self::StreamError>>;

    type InvocationError = Error;

    fn watch<'a>(
        &'a mut self,
        watch_optional: WatchOptional<'a>,
    ) -> BoxFuture<'a, Result<Self::Stream, watcher::invocation::Error<Self::InvocationError>>>
    {
        Box::pin(self.invoke_boxed_stream(watch_optional))
    }
}

/// An error kind for the fake watcher.
#[derive(Debug, Snafu)]
pub enum Error {}
