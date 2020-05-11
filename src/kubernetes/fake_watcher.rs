use futures::stream::{self, Stream};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::WatchEvent;

pub struct FakeWatcher<I> {
    iter: I,
}

impl<I, T> FakeWatcher<I>
where
    I: Iterator<Item = Result<WatchEvent<T>, crate::Error>> + Clone,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }

    pub fn watch(&mut self) -> impl Stream<Item = <I as Iterator>::Item> + '_ {
        stream::iter(self.iter.clone())
    }
}
