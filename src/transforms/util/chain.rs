//! Chain transforms.

#![deny(missing_docs)]

use crate::{event::Event, transforms::Transform};

/// Chains two transforms, passing the event through the `first` one, and then,
/// if there's still an event to pass, through the second one.
pub struct Two<A: Transform, B: Transform> {
    pub first: A,
    pub second: B,
}

impl<A: Transform, B: Transform> Transform for Two<A, B> {
    fn transform(&mut self, event: Event) -> Option<Event> {
        let event = self.first.transform(event)?;
        self.second.transform(event)
    }
}
