//! Temporary presentation ownership; committed style remains the target.

use super::{validate_value, Engine, EngineError, Owner, Property, Running, Value};
use crate::transition::TimingFunction;
use std::sync::atomic::{AtomicU64, Ordering};

// Shared across Engines, so a token cannot address another native session's
// same-numbered node. A Wasm/module reload still needs host incarnation checks.
static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// Opaque ownership of one live node/property presentation. A new hold or
/// removal invalidates it; hosts must also discard tokens on runtime teardown.
/// After ending, it can identify only its own running return via `owns_return`;
/// all hold mutations still require `has_hold` and refuse a returning token.
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
            self.running.remove(&key);
            slot.owner = Some(Owner::Held(serial + i as u64));
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
        for start in [translate, scale] {
            self.track(start.token, now_s, start.value);
        }
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
            self.track(start.token, now_s, value);
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
        self.running.remove(&key);
        slot.owner = Some(Owner::Held(serial));
        self.dirty.insert(key);
        let token = HoldToken {
            node,
            property,
            serial,
        };
        self.track(token, now_s, value);
        Ok(Some(HoldStart { token, value }))
    }

    /// Whether this exact token still owns presentation. Hosts can reject
    /// stale callbacks before advancing their own clock or opening a batch.
    pub fn has_hold(&self, token: HoldToken) -> bool {
        self.slots
            .get(&(token.node, token.property))
            .is_some_and(|slot| slot.owner == Some(Owner::Held(token.serial)))
    }

    /// Whether this exact token's return still runs in this engine. This is
    /// read-only and does not grant hold mutation authority. Retarget, rebegin,
    /// removal and natural completion revoke it, even if a new curve has the
    /// same numeric descriptor. Unchanged-target observations preserve it.
    pub fn owns_return(&self, token: HoldToken) -> bool {
        self.slots
            .get(&(token.node, token.property))
            .is_some_and(|slot| {
                slot.owner == Some(Owner::Returning(token.serial)) && slot.running.is_some()
            })
    }

    /// Whether a property has a live hold. Browser lowering must preserve its
    /// held presentation across authored style commits until release.
    pub fn is_held(&self, node: u64, property: Property) -> bool {
        self.slots
            .get(&(node, property))
            .is_some_and(|slot| matches!(slot.owner, Some(Owner::Held(_))))
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
        self.track(token, now_s, value);
        Ok(true)
    }

    /// The held presentation's velocity at `now_s`, in property units per
    /// second: `VelocityTracker` over every value this hold was given. For a
    /// host whose platform measures none (LLP 1057.001 §3). `None` for a stale
    /// token or a non-finite time.
    pub fn hold_velocity(&self, token: HoldToken, now_s: f64) -> Option<Value> {
        if !self.has_hold(token) || !now_s.is_finite() {
            return None;
        }
        Some(
            self.held
                .get(&token.serial)
                .map_or(Value::ZERO, |t| t.estimate(now_s)),
        )
    }

    fn track(&mut self, token: HoldToken, now_s: f64, value: Value) {
        // A replaced or removed hold leaves its tracker; forget those here,
        // among the few live holds, rather than on every removal path.
        if !self.held.contains_key(&token.serial) {
            let slots = &self.slots;
            self.held.retain(|serial, _| {
                slots
                    .values()
                    .any(|slot| slot.owner == Some(Owner::Held(*serial)))
            });
        }
        self.held
            .entry(token.serial)
            .or_default()
            .push(now_s, value);
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
        self.held.remove(&token.serial);
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
        slot.presented = if let Some(running) = &slot.running {
            let sample = running.sample(now_s);
            if sample.done {
                slot.running = None;
            }
            sample.value
        } else {
            slot.target
        };
        slot.owner = slot
            .running
            .as_ref()
            .map(|_| Owner::Returning(token.serial));
        if slot.running.is_some() {
            self.running.insert(key);
        }
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

    fn returning(end: HoldEnd) -> (Engine, HoldToken) {
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
        e.observe(Change {
            node: 7,
            property: Property::Translate,
            value: Value::ZERO,
            velocity: None,
        })
        .unwrap();
        let token = e
            .begin_hold(7, Property::Translate, 1.0, Some(Value::new(80.0, 4.0)))
            .unwrap()
            .unwrap()
            .token;
        assert!(e.has_hold(token));
        assert!(!e.owns_return(token));
        assert!(e.end_hold(token, 1.0, end).unwrap());
        e.frame();
        (e, token)
    }

    #[test]
    fn return_owner_is_not_held_and_unchanged_observation_preserves_it() {
        use crate::Change;
        eprintln!("return-owner layout bytes: old_optional_serial={} tagged_optional_owner={} slot={} token={}",
            std::mem::size_of::<Option<u64>>(), std::mem::size_of::<Option<super::super::Owner>>(),
            std::mem::size_of::<super::super::Slot>(), std::mem::size_of::<HoldToken>());
        for end in [
            HoldEnd::Cancel,
            HoldEnd::Release {
                velocity: Value::new(12.0, 0.0),
            },
        ] {
            let (mut e, token) = returning(end);
            assert!(e.owns_return(token));
            assert!(!e.has_hold(token));
            assert!(!e.is_held(7, Property::Translate));
            assert!(e.is_active(7, Property::Translate));
            let descriptor = e.spring_descriptor(7, Property::Translate).unwrap();
            let before = format!("{e:?}");
            assert!(e.owns_return(token));
            assert_eq!(format!("{e:?}"), before, "qualification is read-only");
            assert!(!e
                .update_hold(token, f64::NAN, Value::scalar(f64::NAN))
                .unwrap());
            assert!(!e.end_hold(token, f64::NAN, HoldEnd::Cancel).unwrap());
            assert_eq!(
                format!("{e:?}"),
                before,
                "returned token is stale for hold mutation"
            );
            e.observe(Change {
                node: 7,
                property: Property::Translate,
                value: Value::ZERO,
                velocity: None,
            })
            .unwrap();
            assert!(e.owns_return(token));
            assert_eq!(
                e.spring_descriptor(7, Property::Translate),
                Some(descriptor)
            );
            assert_eq!(e.now(), 1.0);
            assert!(e.frame().is_empty());
        }
    }

    #[test]
    fn identical_descriptor_replacement_return_has_a_different_owner() {
        let (mut e, old) = returning(HoldEnd::Cancel);
        let descriptor = e.spring_descriptor(7, Property::Translate).unwrap();
        let new = e
            .begin_hold(7, Property::Translate, 1.0, None)
            .unwrap()
            .unwrap()
            .token;
        assert_ne!(old, new);
        assert!(!e.owns_return(old));
        assert!(e.has_hold(new));
        assert!(e.is_held(7, Property::Translate));
        assert!(!e.owns_return(new));
        assert!(e.end_hold(new, 1.0, HoldEnd::Cancel).unwrap());
        assert_eq!(
            e.spring_descriptor(7, Property::Translate),
            Some(descriptor)
        );
        assert!(
            !e.owns_return(old),
            "identical numbers must not revive an old owner"
        );
        assert!(e.owns_return(new));
        assert!(!e.has_hold(new));
        assert!(!e.is_held(7, Property::Translate));
        let before = format!("{e:?}");
        assert!(!e.end_hold(old, 1.0, HoldEnd::Cancel).unwrap());
        assert_eq!(format!("{e:?}"), before);
    }

    #[test]
    fn changed_target_replaces_return_authority_even_without_a_new_curve() {
        use crate::{Change, Transitions};
        for mode in 0..3 {
            let (mut e, token) = returning(HoldEnd::Cancel);
            let target = if mode == 1 {
                e.value(7, Property::Translate).unwrap()
            } else {
                Value::new(20.0, 0.0)
            };
            if mode == 2 {
                e.set_transitions(7, Transitions::default()).unwrap();
            }
            e.observe(Change {
                node: 7,
                property: Property::Translate,
                value: target,
                velocity: None,
            })
            .unwrap();
            assert!(!e.owns_return(token), "retarget branch {mode}");
            assert!(!e.has_hold(token));
            assert_eq!(e.target(7, Property::Translate), Some(target));
            assert_eq!(e.is_active(7, Property::Translate), mode == 0);
        }
    }

    #[test]
    fn return_owner_survives_sampling_and_declaration_but_not_completion_or_removal() {
        use crate::{Change, Transitions};
        let (mut e, token) = returning(HoldEnd::Cancel);
        let descriptor = e.spring_descriptor(7, Property::Translate).unwrap();
        e.set_transitions(7, Transitions::default()).unwrap();
        e.advance(1.02).unwrap();
        assert!(e.owns_return(token));
        assert_eq!(
            e.spring_descriptor(7, Property::Translate),
            Some(descriptor)
        );
        e.advance(e.settle_time().unwrap() + 1.0).unwrap();
        assert!(!e.owns_return(token));
        assert!(!e.has_hold(token));
        assert!(!e.is_active(7, Property::Translate));
        for whole_node in [false, true] {
            let (mut e, token) = returning(HoldEnd::Cancel);
            if whole_node {
                e.remove(7);
            } else {
                assert!(e.remove_property(7, Property::Translate));
            }
            assert!(!e.owns_return(token));
            e.observe(Change {
                node: 7,
                property: Property::Translate,
                value: Value::ZERO,
                velocity: None,
            })
            .unwrap();
            assert!(!e.owns_return(token));
            assert!(!e.has_hold(token));
        }
    }

    #[test]
    fn paired_takeover_replaces_both_returns_and_keeps_hold_predicates_strict() {
        use crate::Change;
        let (mut e, translate) = returning(HoldEnd::Cancel);
        e.observe(Change {
            node: 7,
            property: Property::Scale,
            value: Value::scalar(1.0),
            velocity: None,
        })
        .unwrap();
        let scale = e
            .begin_hold(7, Property::Scale, 1.0, Some(Value::scalar(1.5)))
            .unwrap()
            .unwrap()
            .token;
        assert!(e.end_hold(scale, 1.0, HoldEnd::Cancel).unwrap());
        assert!(e.owns_return(translate) && e.owns_return(scale));
        let pair = e.begin_transform_hold(7, 1.0, None).unwrap().unwrap();
        assert!(!e.owns_return(translate) && !e.owns_return(scale));
        for token in [pair.translate().token, pair.scale().token] {
            assert!(e.has_hold(token));
            assert!(e.is_held(token.node(), token.property()));
            assert!(!e.owns_return(token));
        }
        assert!(e
            .update_transform_hold(pair, 1.1, [Value::new(30.0, 0.0), Value::scalar(1.2)])
            .unwrap());
        for token in [pair.translate().token, pair.scale().token] {
            assert!(e.end_hold(token, 1.1, HoldEnd::Cancel).unwrap());
            assert!(e.owns_return(token));
            assert!(!e.has_hold(token));
        }
        assert!(!e
            .update_transform_hold(pair, f64::NAN, [Value::scalar(f64::NAN); 2])
            .unwrap());
    }

    #[test]
    fn process_serials_refuse_other_engine_return_despite_identical_node_and_curve() {
        let (a, at) = returning(HoldEnd::Cancel);
        let (b, bt) = returning(HoldEnd::Cancel);
        assert_ne!(at.serial(), bt.serial());
        assert_eq!(
            a.spring_descriptor(7, Property::Translate),
            b.spring_descriptor(7, Property::Translate)
        );
        assert!(a.owns_return(at) && b.owns_return(bt));
        assert!(!a.owns_return(bt) && !b.owns_return(at));
        assert!(!a.has_hold(bt) && !b.has_hold(at));
    }

    #[test]
    fn an_end_without_a_running_curve_does_not_leave_return_authority() {
        use crate::{Change, Transitions};
        for zero_distance in [false, true] {
            let (mut e, old) = returning(HoldEnd::Cancel);
            if !zero_distance {
                e.set_transitions(7, Transitions::default()).unwrap();
            }
            let value = if zero_distance {
                Value::ZERO
            } else {
                Value::new(30.0, 0.0)
            };
            let token = e
                .begin_hold(7, Property::Translate, 1.0, Some(value))
                .unwrap()
                .unwrap()
                .token;
            assert!(!e.owns_return(old));
            assert!(e.end_hold(token, 1.0, HoldEnd::Cancel).unwrap());
            assert!(!e.owns_return(token));
            assert!(!e.has_hold(token));
            assert!(!e.is_active(7, Property::Translate));
            e.observe(Change {
                node: 7,
                property: Property::Translate,
                value: Value::new(5.0, 0.0),
                velocity: None,
            })
            .unwrap();
            assert!(
                !e.owns_return(token),
                "a later authored curve cannot inherit the token"
            );
        }
    }
}
