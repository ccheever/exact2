//! Temporary presentation ownership; committed style remains the target.

use super::{validate_value, Engine, EngineError, Property, Running, Value};
use crate::transition::TimingFunction;
use std::sync::atomic::{AtomicU64, Ordering};

// Shared across Engines, so a token cannot address another native session's
// same-numbered node. A Wasm/module reload still needs host incarnation checks.
static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// Opaque ownership of one live node/property presentation. A new hold or
/// removal invalidates it; hosts must also discard tokens on runtime teardown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldToken {
    node: u64,
    property: Property,
    serial: u64,
}

impl HoldToken {
    /// The node, in the host's numbering.
    pub fn node(self) -> u64 {
        self.node
    }

    /// The property held.
    pub fn property(self) -> Property {
        self.property
    }

    /// Process-local serial. Carry all 64 bits across a host bridge.
    pub fn serial(self) -> u64 {
        self.serial
    }
}

/// A hold's identity and its presentation at takeover. Pointer displacement
/// is relative to this value, not the authored target or a pointer-down sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HoldStart {
    /// Identity required by subsequent moves and release.
    pub token: HoldToken,
    /// The presentation captured at takeover.
    pub value: Value,
}

/// How ownership returns to the latest committed target and transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoldEnd {
    /// A spring inherits presentation velocity; an easing ignores it.
    Release {
        /// Property units per second, measured after any drag resistance.
        velocity: Value,
    },
    /// Return to the latest target with zero release velocity.
    Cancel,
}

impl Engine {
    /// Seek to `now_s` and take presentation ownership of an adopted property.
    /// `None` samples this engine's curve; browser hosts supply their computed
    /// presentation at recognition. The authored target is preserved.
    ///
    /// Unknown properties return `None` without changing time. All live input
    /// validation and serial allocation precede mutation; failure leaves any
    /// previous hold valid. A successful rebegin invalidates the previous token.
    pub fn begin_hold(
        &mut self,
        node: u64,
        property: Property,
        now_s: f64,
        presented: Option<Value>,
    ) -> Result<Option<HoldStart>, EngineError> {
        let key = (node, property);
        if !self.slots.contains_key(&key) {
            return Ok(None);
        }
        if let Some(value) = presented {
            validate_value(property, value)?;
        }
        self.validate_time(now_s)?;
        let serial = allocate_serial(&NEXT_SERIAL)?;
        self.advance(now_s)?;
        let slot = self.slots.get_mut(&key).expect("adopted property");
        let value = presented.unwrap_or(slot.presented);
        slot.presented = value;
        slot.running = None;
        slot.hold = Some(serial);
        self.dirty.insert(key);
        Ok(Some(HoldStart {
            token: HoldToken {
                node,
                property,
                serial,
            },
            value,
        }))
    }

    /// Whether this exact token still owns presentation. Hosts can reject
    /// stale callbacks before advancing their own clock or opening a batch.
    pub fn has_hold(&self, token: HoldToken) -> bool {
        self.slots
            .get(&(token.node, token.property))
            .is_some_and(|slot| slot.hold == Some(token.serial))
    }

    /// Whether a property has a live hold. Browser lowering must preserve its
    /// held presentation across authored style commits until release.
    pub fn is_held(&self, node: u64, property: Property) -> bool {
        self.slots
            .get(&(node, property))
            .is_some_and(|slot| slot.hold.is_some())
    }

    /// Seek and change only the held presentation. Stale tokens return `false`
    /// before checking time or value; invalid live input changes nothing.
    pub fn update_hold(
        &mut self,
        token: HoldToken,
        now_s: f64,
        value: Value,
    ) -> Result<bool, EngineError> {
        if !self.has_hold(token) {
            return Ok(false);
        }
        validate_value(token.property, value)?;
        self.advance(now_s)?;
        let key = (token.node, token.property);
        self.slots.get_mut(&key).expect("live hold").presented = value;
        self.dirty.insert(key);
        Ok(true)
    }

    /// Release to the newest authored target using the newest transition.
    /// Cancel uses zero velocity. Without a matching transition this snaps.
    /// A zero-distance spring still inherits nonzero release velocity.
    ///
    /// Stale tokens return `false` before checking time or velocity; invalid
    /// live input leaves the hold and clock unchanged. Holds alone are quiescent.
    pub fn end_hold(
        &mut self,
        token: HoldToken,
        now_s: f64,
        end: HoldEnd,
    ) -> Result<bool, EngineError> {
        if !self.has_hold(token) {
            return Ok(false);
        }
        let velocity = match end {
            HoldEnd::Release { velocity } => velocity,
            HoldEnd::Cancel => Value::ZERO,
        };
        validate_value(token.property, velocity)?;
        self.advance(now_s)?;
        let key = (token.node, token.property);
        let slot = self.slots.get_mut(&key).expect("live hold");
        let from = slot.presented;
        let declaration = self
            .transitions
            .get(&token.node)
            .and_then(|transitions| transitions.matching(token.property))
            .filter(|transition| transition.starts())
            .filter(|transition| {
                from != slot.target
                    || (velocity != Value::ZERO
                        && matches!(transition.timing, TimingFunction::Spring(_)))
            });
        // Do not use observe: its unchanged-target fast path deliberately
        // suppresses redundant commits, whereas this is a presentation release.
        slot.running = declaration.map(|declaration| {
            Running::start(declaration, from, slot.target, velocity, now_s, from, 1.0)
        });
        slot.hold = None;
        slot.presented = if let Some(running) = &slot.running {
            let sample = running.sample(now_s);
            if sample.done {
                slot.running = None;
            }
            sample.value
        } else {
            slot.target
        };
        self.dirty.insert(key);
        Ok(true)
    }
}

fn allocate_serial(counter: &AtomicU64) -> Result<u64, EngineError> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |serial| {
            serial.checked_add(1)
        })
        .map_err(|_| EngineError::HoldSerialExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_exhaustion_never_wraps_or_reuses_a_token() {
        // A local counter exercises the production allocator without corrupting
        // process-global state used by parallel engine tests.
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(allocate_serial(&counter), Ok(u64::MAX - 1));
        for _ in 0..2 {
            assert_eq!(
                allocate_serial(&counter),
                Err(EngineError::HoldSerialExhausted)
            );
            assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        }
    }
}
