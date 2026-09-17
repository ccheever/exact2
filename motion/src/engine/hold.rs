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

/// One atomic Translate + Scale takeover of the same node. Only the engine
/// constructs this pair; it retains two ordinary tokens and their origins,
/// with no registry or additional lifetime. A rebegin of either property
/// makes subsequent paired updates stale.
///
/// Hosts must validate the ENTIRE terminal payload (both samples/velocities
/// and time) before dispatching an authored action or ending the first token.
/// Dispatch the action only while both tokens are live. Afterwards, use
/// [`Engine::has_hold`] / [`Engine::end_hold`] independently on each original
/// token: an action may delete or replace one property. Cancellation uses
/// [`HoldEnd::Cancel`] (zero velocity) for each surviving token. Never abandon
/// a survivor merely because the pair is stale, or end its replacement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformHold {
    translate: HoldStart,
    scale: HoldStart,
}

impl TransformHold {
    /// The original Translate token and captured origin, not the current value.
    pub fn translate(self) -> HoldStart {
        self.translate
    }

    /// The original Scale token and captured origin, not the current value.
    pub fn scale(self) -> HoldStart {
        self.scale
    }
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
    /// Atomically take presentation ownership of this node's Translate and
    /// Scale. Arrays are always `[translate, scale]`. `None` captures both
    /// engine presentations at `now_s`; browser hosts supply both computed
    /// samples from one recognition read phase, before cancelling playback.
    ///
    /// Either missing adopted slot returns `None` before input validation.
    /// Time and BOTH resulting values are validated, then TWO process-unique
    /// serials reserved together, before any clock, curve or hold changes.
    /// Failure leaves the entire engine unchanged. Authored targets survive.
    /// Scale retains its generic scalar semantics; positive-scale eligibility
    /// is host policy. This pair alone creates no idle animation work.
    pub fn begin_transform_hold(
        &mut self,
        node: u64,
        now_s: f64,
        presented: Option<[Value; 2]>,
    ) -> Result<Option<TransformHold>, EngineError> {
        self.begin_transform_hold_with_counter(node, now_s, presented, &NEXT_SERIAL)
    }

    fn begin_transform_hold_with_counter(
        &mut self,
        node: u64,
        now_s: f64,
        presented: Option<[Value; 2]>,
        counter: &AtomicU64,
    ) -> Result<Option<TransformHold>, EngineError> {
        let keys = [(node, Property::Translate), (node, Property::Scale)];
        if keys.iter().any(|key| !self.slots.contains_key(key)) {
            return Ok(None);
        }
        self.validate_time(now_s)?;
        let values = presented.unwrap_or_else(|| {
            keys.map(|key| {
                let slot = &self.slots[&key];
                slot.running
                    .as_ref()
                    .map_or(slot.presented, |running| running.sample(now_s).value)
            })
        });
        for (key, value) in keys.into_iter().zip(values) {
            validate_value(key.1, value)?;
        }
        let serial = allocate_serials(counter, 2)?;
        self.advance(now_s)?;
        let [translate, scale] = std::array::from_fn(|i| {
            let key = keys[i];
            let slot = self.slots.get_mut(&key).expect("both adopted properties");
            slot.presented = values[i];
            slot.running = None;
            slot.hold = Some(serial + i as u64);
            self.dirty.insert(key);
            HoldStart {
                token: HoldToken {
                    node,
                    property: key.1,
                    serial: serial + i as u64,
                },
                value: values[i],
            }
        });
        Ok(Some(TransformHold { translate, scale }))
    }

    /// Change BOTH held presentations after validating the complete pair.
    /// Values are `[translate, scale]`; authored targets are untouched. If
    /// either original token is stale, return `false` BEFORE checking values
    /// or time, without affecting a successor or the surviving old token.
    /// Invalid live input leaves both presentations, curves and clock intact.
    /// See [`TransformHold`] for independent terminal cleanup obligations.
    pub fn update_transform_hold(
        &mut self,
        held: TransformHold,
        now_s: f64,
        values: [Value; 2],
    ) -> Result<bool, EngineError> {
        let starts = [held.translate, held.scale];
        if starts.iter().any(|start| !self.has_hold(start.token)) {
            return Ok(false);
        }
        for (start, value) in starts.into_iter().zip(values) {
            validate_value(start.token.property, value)?;
        }
        self.advance(now_s)?;
        for (start, value) in starts.into_iter().zip(values) {
            let key = (start.token.node, start.token.property);
            self.slots.get_mut(&key).expect("both live holds").presented = value;
            self.dirty.insert(key);
        }
        Ok(true)
    }

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
    allocate_serials(counter, 1)
}

fn allocate_serials(counter: &AtomicU64, count: u64) -> Result<u64, EngineError> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |serial| {
            serial.checked_add(count)
        })
        .map_err(|_| EngineError::HoldSerialExhausted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moving_pair(held: [bool; 2]) -> (Engine, Vec<HoldToken>) {
        use crate::{Change, SpringConfig, Transition, TransitionProperty, Transitions};
        let mut e = Engine::new();
        e.set_transitions(
            7,
            Transitions(vec![Transition::new(
                TransitionProperty::All,
                0.0,
                TimingFunction::Spring(SpringConfig::default()),
            )]),
        )
        .unwrap();
        let mut tokens = Vec::new();
        for (property, should_hold) in [Property::Translate, Property::Scale].into_iter().zip(held)
        {
            for value in [Value::scalar(-0.0), Value::scalar(3.0)] {
                e.observe(Change {
                    node: 7,
                    property,
                    value,
                    velocity: None,
                })
                .unwrap();
            }
            if should_hold {
                tokens.push(e.begin_hold(7, property, 0.0, None).unwrap().unwrap().token);
            }
        }
        e.advance(0.02).unwrap();
        e.frame();
        (e, tokens)
    }

    #[test]
    fn pair_serial_exhaustion_through_begin_preserves_all_engine_state() {
        for held in [[false, false], [true, false], [false, true], [true, true]] {
            for next in [u64::MAX - 1, u64::MAX] {
                let counter = AtomicU64::new(next);
                let (mut e, tokens) = moving_pair(held);
                let before = format!("{e:?}");
                assert_eq!(
                    e.begin_transform_hold_with_counter(7, 0.4, None, &counter),
                    Err(EngineError::HoldSerialExhausted)
                );
                assert_eq!(format!("{e:?}"), before);
                assert_eq!(counter.load(Ordering::Relaxed), next);
                assert!(tokens.into_iter().all(|token| e.has_hold(token)));
                assert!(e.frame().is_empty());
            }
        }
    }

    #[test]
    fn pair_reserves_last_two_serials_together_without_wrap_or_reuse() {
        let counter = AtomicU64::new(u64::MAX - 2);
        let (mut e, _) = moving_pair([false, false]);
        let pair = e
            .begin_transform_hold_with_counter(7, 0.1, None, &counter)
            .unwrap()
            .unwrap();
        assert_eq!(pair.translate().token.serial(), u64::MAX - 2);
        assert_eq!(pair.scale().token.serial(), u64::MAX - 1);
        assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
        let before = format!("{e:?}");
        assert_eq!(
            e.begin_transform_hold_with_counter(7, 0.2, None, &counter),
            Err(EngineError::HoldSerialExhausted)
        );
        assert_eq!(format!("{e:?}"), before);
        assert_eq!(
            allocate_serial(&counter),
            Err(EngineError::HoldSerialExhausted)
        );
    }

    #[test]
    fn pair_preflight_errors_never_reserve_serials() {
        let counter = AtomicU64::new(100);
        let (mut e, _) = moving_pair([true, true]);
        let before = format!("{e:?}");
        assert_eq!(
            e.begin_transform_hold_with_counter(
                7,
                0.1,
                Some([Value::ZERO, Value::new(2.0, 1.0)]),
                &counter
            ),
            Err(EngineError::InvalidValueShape)
        );
        assert_eq!(
            e.begin_transform_hold_with_counter(7, f64::NAN, None, &counter),
            Err(EngineError::NonFinite)
        );
        assert_eq!(format!("{e:?}"), before);
        e.remove_property(7, Property::Scale);
        let before = format!("{e:?}");
        assert_eq!(
            e.begin_transform_hold_with_counter(
                7,
                f64::NAN,
                Some([Value::scalar(f64::NAN); 2]),
                &counter
            ),
            Ok(None)
        );
        assert_eq!(format!("{e:?}"), before);
        assert_eq!(counter.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn invalid_computed_second_sample_refuses_before_advance_or_serial_reservation() {
        use crate::{Change, Easing, Transition, TransitionProperty, Transitions};
        let counter = AtomicU64::new(100);
        let mut e = Engine::new();
        for (property, value) in [
            (Property::Translate, Value::ZERO),
            (Property::Scale, Value::scalar(-f64::MAX)),
        ] {
            e.observe(Change {
                node: 7,
                property,
                value,
                velocity: None,
            })
            .unwrap();
        }
        let mut declaration = Transition::new(
            TransitionProperty::All,
            1.0,
            TimingFunction::Easing(Easing::Linear),
        );
        declaration.delay = 0.25;
        e.set_transitions(7, Transitions(vec![declaration]))
            .unwrap();
        for (property, value) in [
            (Property::Translate, Value::new(40.0, 10.0)),
            (Property::Scale, Value::scalar(f64::MAX)),
        ] {
            e.observe(Change {
                node: 7,
                property,
                value,
                velocity: None,
            })
            .unwrap();
        }
        e.frame();
        assert!(e.value(7, Property::Scale).unwrap().is_finite());
        let before = format!("{e:?}");
        // The finite endpoints overflow during interpolation after the delay.
        assert_eq!(
            e.begin_transform_hold_with_counter(7, 0.3, None, &counter),
            Err(EngineError::NonFinite)
        );
        assert_eq!(format!("{e:?}"), before);
        assert_eq!(counter.load(Ordering::Relaxed), 100);
        assert!(e.frame().is_empty());
    }

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
