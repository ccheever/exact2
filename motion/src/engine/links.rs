//! What an engine plays, chosen once (LLP 1047.001 D3): every animation, or
//! only settled values. A plan that animates nothing (no transition, layout
//! transition, shared element, keyframes, timeline, spring or hold) still
//! needs the engine as the store of what the host presents, and nothing
//! else of it. [`EngineLinks::SETTLE`] keeps that store and names none of the
//! machinery — curves, springs, keyframes, holds, clocks, timelines, paths
//! — so an artifact that makes its engine from it in a `const` links none
//! of them. [`Engine::new`] plays everything.

use super::*;
use crate::animation::Animations;
use crate::path::PathValue;

/// An entry point's answer.
type Answer<T> = Result<T, EngineError>;
/// How a hold begins: on a transform's two values, or on one property.
type BeginTransformHold =
    fn(&mut Engine, u64, f64, Option<[Value; 2]>) -> Answer<Option<TransformHold>>;
type BeginHold = fn(&mut Engine, u64, Property, f64, Option<Value>) -> Answer<Option<HoldStart>>;
/// A node's `animation-timeline` and range.
type TimelineBinding = Option<(NamedTimeline, [f64; 2])>;

/// The engine's animation entry points.
pub struct EngineLinks {
    name: &'static str,
    pub(super) observe: fn(&mut Engine, Change) -> Answer<()>,
    pub(super) advance: fn(&mut Engine, f64) -> Answer<()>,
    pub(super) set_transitions: fn(&mut Engine, u64, Transitions) -> Answer<()>,
    pub(super) set_layout_transition: fn(&mut Engine, u64, &Transitions) -> Answer<()>,
    pub(super) spring_descriptor: fn(&Engine, u64, Property) -> Option<SpringDescriptor>,
    pub(super) spring_frames: fn(&Engine, u64, Property) -> Option<SpringFrames>,
    pub(super) set_animations: fn(&mut Engine, u64, &Animations) -> Answer<()>,
    pub(super) animated: fn(&Engine, u64, Property, Value) -> Option<Value>,
    pub(super) play_exit: fn(&mut Engine, u64, &Animations) -> Answer<f64>,
    pub(super) set_animation_clock: fn(&mut Engine, u64, Option<&str>),
    pub(super) hold_clock_joins: fn(&mut Engine),
    pub(super) join_clocks: fn(&mut Engine),
    pub(super) set_drag_timeline: fn(&mut Engine, u64, Option<bool>),
    pub(super) set_animation_timeline: fn(&mut Engine, u64, TimelineBinding),
    pub(super) seek_timelines: fn(&mut Engine),
    pub(super) play_transition: fn(&mut Engine, u64, Property) -> Option<PlayedTransition>,
    pub(super) observe_path: fn(&mut Engine, u64, Option<PathValue>),
    pub(super) begin_transform_hold: BeginTransformHold,
    pub(super) update_transform_hold:
        fn(&mut Engine, TransformHold, f64, [Value; 2]) -> Answer<bool>,
    pub(super) begin_hold: BeginHold,
    pub(super) update_hold: fn(&mut Engine, HoldToken, f64, Value) -> Answer<bool>,
    pub(super) hold_velocity: fn(&Engine, HoldToken, f64) -> Option<Value>,
    pub(super) track_hold: fn(&mut Engine, HoldToken, f64, Value) -> bool,
    pub(super) end_hold: fn(&mut Engine, HoldToken, f64, HoldEnd) -> Answer<bool>,
}

impl EngineLinks {
    /// Every animation: transitions and springs, keyframes, holds, clocks,
    /// timelines and paths.
    pub const ALL: EngineLinks = EngineLinks {
        name: "EngineLinks::ALL",
        observe: Engine::observe_full,
        advance: Engine::advance_full,
        set_transitions: Engine::set_transitions_full,
        set_layout_transition: Engine::set_layout_transition_full,
        spring_descriptor: Engine::spring_descriptor_full,
        spring_frames: Engine::spring_frames_full,
        set_animations: Engine::set_animations_full,
        animated: Engine::animated_full,
        play_exit: Engine::play_exit_full,
        set_animation_clock: Engine::set_animation_clock_full,
        hold_clock_joins: Engine::hold_clock_joins_full,
        join_clocks: Engine::join_clocks_full,
        set_drag_timeline: Engine::set_drag_timeline_full,
        set_animation_timeline: Engine::set_animation_timeline_full,
        seek_timelines: Engine::seek_timelines_full,
        play_transition: Engine::play_transition_full,
        observe_path: Engine::observe_path_full,
        begin_transform_hold: Engine::begin_transform_hold_full,
        update_transform_hold: Engine::update_transform_hold_full,
        begin_hold: Engine::begin_hold_full,
        update_hold: Engine::update_hold_full,
        hold_velocity: Engine::hold_velocity_full,
        track_hold: Engine::track_hold_full,
        end_hold: Engine::end_hold_full,
    };

    /// Settled values only: every change is presented at once, and nothing
    /// starts. A plan that would start something is refused before its
    /// engine is made (LLP 1047.001 D5), so these answers are never a
    /// silent loss: a transition declared here moves nothing, a hold never
    /// begins, an exit ends where it starts.
    pub const SETTLE: EngineLinks = EngineLinks {
        name: "EngineLinks::SETTLE",
        observe: settle_observe,
        advance: settle_advance,
        set_transitions: |_, _, _| Ok(()),
        set_layout_transition: |_, _, _| Ok(()),
        spring_descriptor: |_, _, _| None,
        spring_frames: |_, _, _| None,
        set_animations: |_, _, _| Ok(()),
        animated: |_, _, _, _| None,
        play_exit: |engine, _, _| Ok(engine.now),
        set_animation_clock: |_, _, _| {},
        hold_clock_joins: |_| {},
        join_clocks: |_| {},
        set_drag_timeline: |_, _, _| {},
        set_animation_timeline: |_, _, _| {},
        seek_timelines: |_| {},
        play_transition: |_, _, _| None,
        observe_path: |_, _, _| {},
        begin_transform_hold: |_, _, _, _| Ok(None),
        update_transform_hold: |_, _, _, _| Ok(false),
        begin_hold: |_, _, _, _, _| Ok(None),
        update_hold: |_, _, _, _| Ok(false),
        hold_velocity: |_, _, _| None,
        track_hold: |_, _, _, _| false,
        end_hold: |_, _, _, _| Ok(false),
    };
}

impl std::fmt::Debug for EngineLinks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

impl Engine {
    /// An engine that plays what `links` names: [`EngineLinks::ALL`] is
    /// [`Engine::new`]; [`EngineLinks::SETTLE`] presents settled values only.
    pub fn linked(links: &'static EngineLinks) -> Engine {
        // Field by field, never through `Default`: that names `ALL`, and an
        // artifact that settles must not link what `ALL` names.
        Engine {
            now: Default::default(),
            shown: Default::default(),
            start_on_frame: Default::default(),
            pending: Default::default(),
            transitions: Default::default(),
            slots: Default::default(),
            running: Default::default(),
            played: Default::default(),
            dirty: Default::default(),
            animations: Default::default(),
            animating: Default::default(),
            lowered: Default::default(),
            forced: Default::default(),
            held: Default::default(),
            layout: Default::default(),
            dark: Default::default(),
            node_dark: Default::default(),
            timelines: Default::default(),
            clocks: Default::default(),
            paths: Default::default(),
            properties: Default::default(),
            links,
        }
    }
}

impl Default for Engine {
    fn default() -> Engine {
        Engine::linked(&EngineLinks::ALL)
    }
}

/// [`Engine::observe`] with nothing declared: the value is presented now, as
/// an idle property with no transition takes it.
fn settle_observe(engine: &mut Engine, change: Change) -> Answer<()> {
    validate_value(change.property, change.value)?;
    if let Some(velocity) = change.velocity {
        validate_value(change.property, velocity)?;
    }
    let key = (change.node, change.property);
    match engine.slots.get_mut(&key) {
        Some(slot) if slot.target == change.value => return Ok(()),
        Some(slot) => {
            slot.set_target(change.value);
            slot.set_presented(change.value);
        }
        None => {
            engine.keep(key, Slot::settled(change.value));
        }
    }
    engine.dirty.insert(key);
    Ok(())
}

/// [`Engine::advance`] with nothing running: the clock moves.
fn settle_advance(engine: &mut Engine, now: f64) -> Answer<()> {
    engine.validate_time(now)?;
    engine.now = now;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Easing, TimingFunction, Transition, TransitionProperty};

    fn change(node: u64, value: f64) -> Change {
        Change {
            node,
            property: Property::Opacity,
            value: Value::scalar(value),
            velocity: None,
        }
    }

    #[test]
    fn a_settling_engine_presents_every_change_at_once_and_starts_nothing() {
        let mut engine = Engine::linked(&EngineLinks::SETTLE);
        engine.observe(change(7, 1.0)).unwrap();
        assert_eq!(engine.frame().len(), 1);
        // A declared transition moves nothing: the change is presented now.
        engine
            .set_transitions(
                7,
                Transitions(vec![Transition::new(
                    TransitionProperty::All,
                    1.0,
                    TimingFunction::Easing(Easing::Linear),
                )]),
            )
            .unwrap();
        engine.observe(change(7, 0.0)).unwrap();
        assert_eq!(engine.value(7, Property::Opacity), Some(Value::scalar(0.0)));
        assert!(engine.quiescent());
        let shown = engine.frame();
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].value, Value::scalar(0.0));
        // An unchanged value is not presented again; the clock moves.
        engine.observe(change(7, 0.0)).unwrap();
        assert!(engine.frame().is_empty());
        engine.advance(2.0).unwrap();
        assert_eq!(engine.now(), 2.0);
        assert!(
            engine.advance(1.0).is_err(),
            "the clock never runs backwards"
        );
        // Nothing holds, nothing plays.
        assert!(engine
            .begin_hold(7, Property::Opacity, 2.0, None)
            .unwrap()
            .is_none());
        assert_eq!(engine.settle_time(), None);
        assert!(engine.spring_descriptor(7, Property::Opacity).is_none());
    }

    #[test]
    fn a_default_engine_plays_everything() {
        for mut engine in [Engine::new(), Engine::default()] {
            assert_eq!(engine.links.name, "EngineLinks::ALL");
            engine.observe(change(7, 1.0)).unwrap();
            engine
                .set_transitions(
                    7,
                    Transitions(vec![Transition::new(
                        TransitionProperty::All,
                        1.0,
                        TimingFunction::Easing(Easing::Linear),
                    )]),
                )
                .unwrap();
            engine.observe(change(7, 0.0)).unwrap();
            assert!(!engine.quiescent(), "the transition runs");
        }
    }
}
