use crate::transforms::util::optional::Optional;
use crate::transforms::Merge;

/// Partial event merger.
pub type PartialEventsMerger = Optional<Merge>;

pub fn build(enabled: bool) -> PartialEventsMerger {
    Optional(if enabled {
        Some(Merge {
            partial_event_marker_field: event::PARTIAL.clone(),
            merge_fields: vec![event::log_schema().message_key().clone()],
            // TODO: ensure we split the streams by pod uids.
            stream_discriminant_fields: vec![],
        })
    } else {
        None
    })
}
