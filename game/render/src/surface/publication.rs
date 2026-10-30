//! The host's submitted-frame fact, separate from saved simulation publications.

use exact_game::{World, WorldId};
use serde_json::value::RawValue;
use std::collections::BTreeMap;

type Generation = (WorldId, u64);

pub(super) struct Publication {
    presented: Option<Generation>,
    encoded: Option<(Generation, bool)>,
    record: String,
    sent_ready: Option<bool>,
}

impl Default for Publication {
    fn default() -> Self {
        Self {
            presented: None,
            encoded: None,
            record: "{}".into(),
            sent_ready: None,
        }
    }
}

fn generation(world: &World) -> Generation {
    (world.id(), world.presentation_generation())
}

impl Publication {
    pub(super) fn ready(&self, world: &World) -> bool {
        self.presented.as_ref() == Some(&generation(world))
    }

    pub(super) fn discard_encoded(&mut self) {
        self.encoded = None;
    }

    pub(super) fn invalidate(&mut self) {
        self.presented = None;
        self.encoded = None;
    }

    pub(super) fn encode(&mut self, world: &World, drawable: bool, complete: bool) {
        let available = complete || (drawable && self.ready(world));
        self.encoded = Some((generation(world), available));
    }

    pub(super) fn submitted(&mut self, world: &World) {
        if let Some((encoded, available)) = self.encoded.take() {
            if encoded == generation(world) {
                self.presented = available.then_some(encoded);
            }
        }
    }

    pub(super) fn record(&mut self, record: Option<String>, ready: bool) -> Option<String> {
        if record.is_none() && self.sent_ready == Some(ready) {
            return None;
        }
        if let Some(record) = record {
            self.record = record;
        }
        // Raw values preserve the game's numeric text and named nested records.
        let mut fields: BTreeMap<String, &RawValue> =
            serde_json::from_str(&self.record).expect("simulation publication is a JSON record");
        let value = RawValue::from_string(ready.to_string()).expect("boolean JSON");
        fields.insert("frame_ready".into(), &value);
        let record = serde_json::to_string(&fields).expect("raw publication fields are valid JSON");
        self.sent_ready = Some(ready);
        Some(record)
    }
}

#[cfg(test)]
#[path = "publication_tests.rs"]
mod tests;
