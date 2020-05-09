use evmap10::{ReadHandle, WriteHandle};
use k8s_openapi::Metadata;
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug)]
pub struct WatchedState<T: Eq + Hash> {
    items: WriteHandle<String, Box<T>>,
    resource_version: Option<String>,
}

impl<T: Eq> WatchedState<T> {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            resource_version: None,
        }
    }

    pub fn resource_version(&self) -> Option<&str> {
        self.resource_version.map(|val| val.as_str())
    }

    pub fn state(&self) -> impl Iterator<Item = &T> + '_ {
        self.items.values()
    }
}

impl<T> WatchedState<T>
where
    T: Metadata,
{
    pub fn add(&mut self, item: T) {
        // self.items.insert(k, v)
    }

    pub fn delete(&mut self, item: T) {}

    pub fn modify(&mut self, item: T) {}

    pub fn bookmark(&mut self, item: T) {}

    fn name(item: &T) -> Option<String> {
        item.metadata().name
    }
}
