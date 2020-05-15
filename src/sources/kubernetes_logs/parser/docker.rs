use crate::{
    event::{self, Event, Value},
    transforms::{
        json_parser::{JsonParser, JsonParserConfig},
        Transform,
    },
};
use chrono::{DateTime, Utc};
use string_cache::DefaultAtom as Atom;

pub fn build() -> Box<dyn Transform> {
    Box::new(Docker::new())
}

/// Parser for docker format.
// TODO: ensure partial event indicator is correctly set.
// TODO: add tests.
#[derive(Debug)]
struct Docker {
    json_parser: JsonParser,
    atom_time: Atom,
    atom_log: Atom,
}

impl Docker {
    fn new() -> Self {
        let mut config = JsonParserConfig::default();

        // Drop so that it's possible to detect if message is in json format
        config.drop_invalid = true;

        config.drop_field = true;

        Docker {
            json_parser: config.into(),
            atom_time: Atom::from("time"),
            atom_log: Atom::from("log"),
        }
    }
}

impl Transform for Docker {
    fn transform(&mut self, event: Event) -> Option<Event> {
        let mut event = self.json_parser.transform(event)?;

        // Rename fields
        let log = event.as_mut_log();

        // time -> timestamp
        if let Some(Value::Bytes(timestamp_bytes)) = log.remove(&self.atom_time) {
            match DateTime::parse_from_rfc3339(
                String::from_utf8_lossy(timestamp_bytes.as_ref()).as_ref(),
            ) {
                Ok(timestamp) => {
                    log.insert(
                        event::log_schema().timestamp_key(),
                        timestamp.with_timezone(&Utc),
                    );
                }
                Err(error) => {
                    debug!(message = "Non rfc3339 timestamp.", %error, rate_limit_secs = 10);
                    return None;
                }
            }
        } else {
            debug!(message = "Missing field.", field = %self.atom_time, rate_limit_secs = 10);
            return None;
        }

        // log -> message
        if let Some(message) = log.remove(&self.atom_log) {
            log.insert(event::log_schema().message_key(), message);
        } else {
            debug!(message = "Missing field.", field = %self.atom_log, rate_limit_secs = 10);
            return None;
        }

        Some(event)
    }
}
