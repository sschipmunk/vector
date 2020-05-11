//! Shared state bits for watch implementations.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, WatchEvent};
use k8s_openapi::Metadata;
use std::ops::Deref;

/// Resource version state in the context of a watch request.
pub struct ResourceVersionState(Option<String>);

impl ResourceVersionState {
    /// Create a new [`ResourceVersionState`].
    pub fn new() -> Self {
        Self(None)
    }

    /// Update the resource version from an event.
    pub fn update<T>(&mut self, event: &WatchEvent<T>)
    where
        T: Metadata<Ty = ObjectMeta>,
    {
        let object = match event {
            WatchEvent::Added(item)
            | WatchEvent::Modified(item)
            | WatchEvent::Deleted(item)
            | WatchEvent::Bookmark(item) => item,
            WatchEvent::ErrorStatus(_) | WatchEvent::ErrorOther(_) => return, // noop
        };

        let metadata = match object.metadata() {
            Some(val) => val,
            None => {
                warn!(message = "Got k8s object without metadata");
                return;
            }
        };

        let new_resource_version = match metadata.resource_version {
            Some(ref val) => val,
            None => {
                warn!(message = "Got empty resource version at object metadata");
                return;
            }
        };

        self.0 = Some(new_resource_version.clone());
    }

    pub fn get(&self) -> Option<&str> {
        self.into()
    }
}

impl Deref for ResourceVersionState {
    type Target = Option<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> From<&'a ResourceVersionState> for Option<&'a str> {
    fn from(val: &'a ResourceVersionState) -> Self {
        match val.0 {
            Some(ref val) => Some(val.as_str()),
            None => None,
        }
    }
}
