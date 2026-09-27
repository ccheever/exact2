//! Springs on the web: the evaluator runs once per release, as a compiler.
//!
//! @ref LLP 1002 D2 (on the web a spring is lowered, not evaluated per frame)
//! @ref LLP 1003 §4 (the seam: `Kernel::motion_sync`)
//!
//! CSS plays every easing transition itself. A `spring()` it cannot, so the
//! host keeps the same [`Engine`] every native host runs, feeds it each
//! commit through the kernel's seam, seeks it to the commit's clock, and asks
//! it for the frames of any spring that just started — including one that
//! interrupts a transition in flight, whose release value and velocity the
//! engine takes from the curve it was on (CSS Transitions §3, the same rule
//! natively). Nothing here runs per frame: the engine is sampled once per
//! commit and the browser interpolates the frames.
//!
//! Motion is a linked capability (LLP 1047 D3): the host holds a
//! [`Motion`], which is [`Still`] unless the app's entry registered
//! [`springs`], so an app that uses no spring or hold carries no engine.

use exact_kernel::motion::{motion_node, targets, MotionSync};
use exact_kernel::{CommitReceipt, Kernel, NodeKey, ViewId};
use exact_motion::{
    Change, Engine, EngineError, HoldEnd, HoldStart, HoldToken, Property, SpringDescriptor,
    TransformHold, Value,
};
use std::collections::BTreeMap;

/// What the page must do about one property's spring after a commit.
#[derive(Debug, Clone, PartialEq)]
pub enum Lowered {
    /// Play these frames, evenly spaced over `duration` seconds after
    /// `delay` seconds, replacing whatever was playing on the property.
    Start {
        /// The node.
        view: ViewId,
        /// The property.
        property: Property,
        /// Seconds before the first frame.
        delay: f64,
        /// Seconds from the first frame to the last.
        duration: f64,
        /// The frames; the last is the target.
        values: Vec<Value>,
    },
    /// Stop playing: the property is no longer under a spring (the style is
    /// its value, or a CSS transition is).
    Cancel {
        /// The node.
        view: ViewId,
        /// The property.
        property: Property,
    },
    /// Eligibility or explicit ownership ended. Remove held DOM overrides as
    /// well as playback, even when removing the Engine slot emitted no frame.
    Retire {
        /// The node's last live view id.
        view: ViewId,
        /// Only this property is retired.
        property: Property,
    },
}

/// Springs and holds: the motion capability's seam (LLP 1047 D3). The host
/// calls it at every commit and for every hold; [`Still`] answers when the
/// artifact doesn't link motion, and [`springs`] is the engine an entry
/// registers when its plan uses it.
pub trait Motion {
    /// The engine's clock, seconds.
    fn now(&self) -> f64;
    /// When the last spring in flight ends, seconds; `None` when none is.
    fn settle_time(&self) -> Option<f64>;
    /// The spring engine, when the artifact links one.
    fn linked_engine(&self) -> Option<&Engine>;
    /// Keyframe compilations so far.
    #[cfg(test)]
    fn frame_compilations(&self) -> usize;
    /// Number of property springs retained for the current mounted tree.
    fn playing_count(&self) -> usize;
    /// The registered numeric-height owner.
    fn height_owner(&self) -> Option<(NodeKey, ViewId)>;
    /// Register the numeric-height owner, or clear it.
    fn set_height_owner(
        &mut self,
        kernel: &Kernel,
        view: Option<ViewId>,
    ) -> Result<Vec<Lowered>, &'static str>;
    /// A live hold's token.
    fn token(&self, serial: u64) -> Option<HoldToken>;
    /// Capture a presented value.
    fn begin_hold(
        &mut self,
        kernel: &Kernel,
        view: ViewId,
        property: Property,
        presented: Value,
        now: f64,
    ) -> Result<Option<HoldStart>, EngineError>;
    /// Move a hold.
    fn update_hold(&mut self, serial: u64, value: Value, now: f64) -> Result<bool, EngineError>;
    /// Capture a translate and scale pair.
    fn begin_transform_hold(
        &mut self,
        node: u64,
        values: [Value; 2],
        now: f64,
    ) -> Result<Option<TransformHold>, EngineError>;
    /// Move a translate and scale pair.
    fn update_transform_hold(
        &mut self,
        pair: TransformHold,
        values: [Value; 2],
        now: f64,
    ) -> Result<bool, EngineError>;
    /// Synchronize, then take a transform target's newest authored values.
    fn synchronize_transform(&mut self, kernel: &Kernel, target: NodeKey, now: f64)
        -> Vec<Lowered>;
    /// Release or cancel a hold.
    fn end_hold(&mut self, serial: u64, end: HoldEnd, now: f64) -> Result<bool, EngineError>;
    /// Take the tree at boot as it is.
    fn adopt(&mut self, kernel: &Kernel, views: &[ViewId]);
    /// Synchronize accepted receipts while holds still own presentation.
    fn synchronize(
        &mut self,
        kernel: &Kernel,
        receipts: &[CommitReceipt],
        now: f64,
    ) -> Vec<Lowered>;
    /// Synchronize and lower changed properties once.
    fn commit(&mut self, kernel: &Kernel, receipts: &[CommitReceipt], now: f64) -> Vec<Lowered>;
    /// Lower what changed at the current clock.
    fn lower_current(&mut self, kernel: &Kernel) -> Vec<Lowered>;
}

impl dyn Motion + '_ {
    /// The spring engine, for inspection: presentation values as the page
    /// shows them. Only an artifact that links motion has one.
    pub fn engine(&self) -> &Engine {
        self.linked_engine().expect("this artifact links motion")
    }
}

/// Motion in an artifact that doesn't link it: no springs and no holds.
/// Admission (LLP 1047 D6) keeps a plan that needs them off such an artifact;
/// the clock still follows the commits.
#[derive(Debug, Default)]
pub struct Still {
    now: f64,
}

impl Motion for Still {
    fn now(&self) -> f64 {
        self.now
    }

    fn settle_time(&self) -> Option<f64> {
        None
    }

    fn linked_engine(&self) -> Option<&Engine> {
        None
    }

    #[cfg(test)]
    fn frame_compilations(&self) -> usize {
        0
    }

    fn playing_count(&self) -> usize {
        0
    }

    fn height_owner(&self) -> Option<(NodeKey, ViewId)> {
        None
    }

    fn set_height_owner(
        &mut self,
        _: &Kernel,
        view: Option<ViewId>,
    ) -> Result<Vec<Lowered>, &'static str> {
        view.map_or(Ok(Vec::new()), |_| Err("motion is not linked"))
    }

    fn token(&self, _: u64) -> Option<HoldToken> {
        None
    }

    fn begin_hold(
        &mut self,
        _: &Kernel,
        _: ViewId,
        _: Property,
        _: Value,
        _: f64,
    ) -> Result<Option<HoldStart>, EngineError> {
        Ok(None)
    }

    fn update_hold(&mut self, _: u64, _: Value, _: f64) -> Result<bool, EngineError> {
        Ok(false)
    }

    fn begin_transform_hold(
        &mut self,
        _: u64,
        _: [Value; 2],
        _: f64,
    ) -> Result<Option<TransformHold>, EngineError> {
        Ok(None)
    }

    fn update_transform_hold(
        &mut self,
        _: TransformHold,
        _: [Value; 2],
        _: f64,
    ) -> Result<bool, EngineError> {
        Ok(false)
    }

    fn synchronize_transform(&mut self, _: &Kernel, _: NodeKey, now: f64) -> Vec<Lowered> {
        self.now = self.now.max(now);
        Vec::new()
    }

    fn end_hold(&mut self, _: u64, _: HoldEnd, _: f64) -> Result<bool, EngineError> {
        Ok(false)
    }

    fn adopt(&mut self, _: &Kernel, _: &[ViewId]) {}

    fn synchronize(&mut self, _: &Kernel, _: &[CommitReceipt], now: f64) -> Vec<Lowered> {
        self.now = self.now.max(now);
        Vec::new()
    }

    fn commit(&mut self, _: &Kernel, _: &[CommitReceipt], now: f64) -> Vec<Lowered> {
        self.now = self.now.max(now);
        Vec::new()
    }

    fn lower_current(&mut self, _: &Kernel) -> Vec<Lowered> {
        Vec::new()
    }
}

/// The spring engine, for an entry that links motion (`exact-web-capabilities`
/// registers it). Nothing else names it, so an artifact without motion
/// carries none of it.
pub fn springs() -> Box<dyn Motion> {
    Box::new(Springs::new())
}

/// The web host's spring evaluator: one engine, sampled at commits.
#[derive(Debug, Default)]
pub struct Springs {
    engine: Engine,
    holds: BTreeMap<u64, HoldToken>,
    #[cfg(test)]
    frame_compilations: usize,
    /// Compare fixed-size curve identity before compiling browser keyframes.
    playing: BTreeMap<(u64, Property), SpringDescriptor>,
    /// One explicit trial owner, bound to its arena generation. Unsupported
    /// styles retire its slot; a later eligible style can adopt it again.
    height_owner: Option<(NodeKey, ViewId)>,
}

impl Springs {
    /// Empty, at clock zero. The browser runs every CSS animation from the
    /// page's `@keyframes` (LLP 1055 D7), so the engine only tracks them.
    pub fn new() -> Springs {
        let mut springs = Springs::default();
        springs.engine.set_lowered(true);
        springs
    }

    fn retire_height(&mut self, key: NodeKey, view: ViewId, out: &mut Vec<Lowered>) {
        let node = motion_node(key);
        let removed = self.engine.remove_property(node, Property::Height);
        self.playing.remove(&(node, Property::Height));
        if removed {
            out.push(Lowered::Retire {
                view,
                property: Property::Height,
            });
        }
    }

    fn reconcile_height(&mut self, kernel: &Kernel, out: &mut Vec<Lowered>) {
        let Some((key, view)) = self.height_owner else {
            return;
        };
        let sync = kernel.height_motion_sync(key);
        if !sync.retired.is_empty() {
            self.retire_height(key, view, out);
        }
        let applied = sync.apply(&mut self.engine);
        debug_assert!(applied.is_ok(), "height target is validated kernel input");
        if kernel.node_by_key(key).is_none() {
            self.height_owner = None;
        }
    }

    fn view_of(&self, kernel: &Kernel, node: u64) -> Option<ViewId> {
        let key = NodeKey {
            index: node as u32,
            generation: (node >> 32) as u32,
        };
        kernel.node_by_key(key).map(|n| n.id)
    }
}

impl Motion for Springs {
    fn now(&self) -> f64 {
        self.engine.now()
    }

    fn settle_time(&self) -> Option<f64> {
        self.engine.settle_time()
    }

    fn linked_engine(&self) -> Option<&Engine> {
        Some(&self.engine)
    }

    #[cfg(test)]
    fn frame_compilations(&self) -> usize {
        self.frame_compilations
    }

    /// Number of property springs retained for the current mounted tree.
    fn playing_count(&self) -> usize {
        self.playing.len()
    }

    fn height_owner(&self) -> Option<(NodeKey, ViewId)> {
        self.height_owner
    }

    fn set_height_owner(
        &mut self,
        kernel: &Kernel,
        view: Option<ViewId>,
    ) -> Result<Vec<Lowered>, &'static str> {
        // Refuse before retiring the old owner or touching its clock/hold.
        let next = view
            .map(|view| {
                let node = kernel.node(view).ok_or("unknown height owner")?;
                // Registration is generational intent across temporary hidden
                // or unsupported styles; repeating it is still idempotent.
                if self.height_owner == Some((node.key, view)) {
                    return Ok((node.key, view));
                }
                kernel
                    .height_target(node.key)
                    .ok_or("height owner is not an attached numeric box")?;
                Ok((node.key, view))
            })
            .transpose()?;
        if self.height_owner == next {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        if let Some((key, view)) = self.height_owner.take() {
            self.retire_height(key, view, &mut out);
        }
        self.height_owner = next;
        self.reconcile_height(kernel, &mut out);
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        Ok(out)
    }

    fn token(&self, serial: u64) -> Option<HoldToken> {
        self.holds
            .get(&serial)
            .copied()
            .filter(|t| self.engine.has_hold(*t))
    }

    fn begin_hold(
        &mut self,
        kernel: &Kernel,
        view: ViewId,
        property: Property,
        presented: Value,
        now: f64,
    ) -> Result<Option<HoldStart>, EngineError> {
        let Some(node) = kernel.node(view) else {
            return Ok(None);
        };
        if self.engine.value(motion_node(node.key), property).is_none() {
            return Ok(None);
        }
        validate_height_position(property, presented)?;
        let Some(start) =
            self.engine
                .begin_hold(motion_node(node.key), property, now, Some(presented))?
        else {
            return Ok(None);
        };
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        self.holds.insert(start.token.serial(), start.token);
        // Even a curve crossing its target must be cancelled on takeover; a
        // same-clock release with new velocity must not hit the old dedup key.
        self.playing.remove(&(start.token.node(), property));
        Ok(Some(start))
    }

    fn update_hold(&mut self, serial: u64, value: Value, now: f64) -> Result<bool, EngineError> {
        let Some(token) = self.token(serial) else {
            return Ok(false);
        };
        validate_height_position(token.property(), value)?;
        self.engine.update_hold(token, now, value)
    }

    fn begin_transform_hold(
        &mut self,
        node: u64,
        values: [Value; 2],
        now: f64,
    ) -> Result<Option<TransformHold>, EngineError> {
        let Some(pair) = self.engine.begin_transform_hold(node, now, Some(values))? else {
            return Ok(None);
        };
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        for start in [pair.translate(), pair.scale()] {
            self.holds.insert(start.token.serial(), start.token);
            self.playing.remove(&(node, start.token.property()));
        }
        Ok(Some(pair))
    }

    fn update_transform_hold(
        &mut self,
        pair: TransformHold,
        values: [Value; 2],
        now: f64,
    ) -> Result<bool, EngineError> {
        self.engine.update_transform_hold(pair, now, values)
    }

    fn synchronize_transform(
        &mut self,
        kernel: &Kernel,
        target: NodeKey,
        now: f64,
    ) -> Vec<Lowered> {
        let out = self.synchronize(kernel, &[], now);
        if let Some(node) = kernel.node_by_key(target) {
            let key = motion_node(target);
            let sync = MotionSync {
                transitions: vec![(key, node.style.transition.clone())],
                changes: targets(node.style)
                    .into_iter()
                    .filter(|(property, _)| {
                        matches!(property, Property::Translate | Property::Scale)
                    })
                    .map(|(property, value)| Change {
                        node: key,
                        property,
                        value,
                        velocity: None,
                    })
                    .collect(),
                ..Default::default()
            };
            let applied = sync.apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "validated transform authoring");
        }
        out
    }

    fn end_hold(&mut self, serial: u64, end: HoldEnd, now: f64) -> Result<bool, EngineError> {
        let Some(token) = self.token(serial) else {
            return Ok(false);
        };
        let accepted = self.engine.end_hold(token, now, end)?;
        if accepted {
            self.holds.remove(&serial);
        }
        Ok(accepted)
    }

    /// Tell the engine about nodes that exist before any commit it saw —
    /// the tree at boot. Their values are taken as-is (there is no
    /// before-change style, so nothing transitions).
    fn adopt(&mut self, kernel: &Kernel, views: &[ViewId]) {
        let mut sync = MotionSync::default();
        for id in views {
            let Some(node) = kernel.node(*id) else {
                continue;
            };
            let n = motion_node(node.key);
            sync.transitions.push((n, node.style.transition.clone()));
            for (property, value) in targets(node.style) {
                sync.changes.push(Change {
                    node: n,
                    property,
                    value,
                    velocity: None,
                });
            }
        }
        let applied = sync.apply(&mut self.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        let _ = self.engine.frame();
    }

    /// Synchronize an accepted receipt while existing holds still own presentation.
    /// Retirement ops are returned, but dirty frames remain for one common lowering.
    fn synchronize(
        &mut self,
        kernel: &Kernel,
        receipts: &[CommitReceipt],
        now: f64,
    ) -> Vec<Lowered> {
        let now = now.max(self.engine.now());
        let seek = self.engine.advance(now);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        for receipt in receipts {
            let sync = kernel.motion_sync(receipt);
            // Engine::remove also erases dirty entries, so frame() will never
            // mention these nodes again. Retire ownership directly, without
            // scanning springs belonging to other mounted rows.
            for node in &sync.removed {
                for property in Property::ALL {
                    self.playing.remove(&(*node, property));
                }
            }
            let applied = sync.apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        }
        let mut out = Vec::new();
        // Ancestor hide/detach does not touch the owner's receipt key. This
        // checks only its ancestor path, never all mounted numeric heights.
        self.reconcile_height(kernel, &mut out);
        self.holds.retain(|_, token| self.engine.has_hold(*token));
        out
    }

    /// Synchronize and lower changed properties once at a commit/input boundary.
    fn commit(&mut self, kernel: &Kernel, receipts: &[CommitReceipt], now: f64) -> Vec<Lowered> {
        let now = now.max(self.engine.now());
        let mut out = self.synchronize(kernel, receipts, now);
        out.extend(self.lower_current(kernel));
        out
    }

    // Stale photo cleanup may lower surviving old tokens at the existing clock,
    // but must neither seek time nor import unrelated targets/owner changes.
    fn lower_current(&mut self, kernel: &Kernel) -> Vec<Lowered> {
        let now = self.engine.now();
        let mut out = Vec::new();
        for p in self.engine.frame() {
            let key = (p.node, p.property);
            let Some(view) = self.view_of(kernel, p.node) else {
                self.playing.remove(&key);
                continue;
            };
            if self.engine.is_held(p.node, p.property) {
                self.playing.remove(&key);
                continue;
            }
            match self.engine.spring_descriptor(p.node, p.property) {
                Some(descriptor) => {
                    if self.playing.get(&key) == Some(&descriptor) {
                        continue;
                    }
                    #[cfg(test)]
                    {
                        self.frame_compilations += 1;
                    }
                    let frames = self
                        .engine
                        .spring_frames(p.node, p.property)
                        .expect("a running spring descriptor has frames");
                    self.playing.insert(key, descriptor);
                    out.push(Lowered::Start {
                        view,
                        property: p.property,
                        delay: (frames.start - now).max(0.0),
                        duration: frames.duration,
                        values: frames.values,
                    });
                }
                None => {
                    // A spring that reached its target finished on the page
                    // too; one whose property moved on without a spring must
                    // stop, or its frames would keep overriding the style.
                    if let Some(previous) = self.playing.remove(&key) {
                        if p.value != previous.target {
                            out.push(Lowered::Cancel {
                                view,
                                property: p.property,
                            });
                        }
                    }
                }
            }
        }
        out
    }
}

// Native projection uses f32 CSS lengths. Reject malformed external positions
// before advancing the clock; signed velocities/internal curve samples remain
// unrestricted, and the DOM alone clamps a negative displayed height to zero.
fn validate_height_position(property: Property, value: Value) -> Result<(), EngineError> {
    if property != Property::Height {
        return Ok(());
    }
    if !value.x.is_finite() || !value.y.is_finite() {
        return Err(EngineError::NonFinite);
    }
    if value.y != 0.0 || value.x < 0.0 || value.x > f32::MAX as f64 {
        return Err(EngineError::InvalidValueShape);
    }
    Ok(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use exact_runner::{DataError, DataSource, Event, Value as DataValue};

    struct NoData;
    impl DataSource for NoData {
        fn query(&mut self, name: &str, _: &[DataValue]) -> Result<DataValue, DataError> {
            Err(DataError::UnknownSource(name.into()))
        }
    }

    #[test]
    fn hold_moves_do_not_recompile_unchanged_springs() {
        crate::link::link_for_tests();
        let mut source = String::from("component App\n  state big = false\n  action toggle writes big\n    big = not big\n  view\n    column\n      button press=toggle testId=\"toggle\"\n        text \"Toggle\"\n      text \"Held\" testId=\"held\" transition=\"translate spring(180, 12, 1)\"\n");
        for _ in 0..32 {
            source.push_str("      text \"Moving\" scale=(big ? 1.5 : 1) opacity=(big ? 0.5 : 1) transition=\"scale spring(180, 12, 1), opacity spring(180, 12, 1)\"\n");
        }
        let (mut host, _) = crate::Host::boot(
            &contract::compile(&source).unwrap().encode(),
            NoData,
            Default::default(),
            "/",
        )
        .unwrap();
        let id = |host: &crate::Host<NoData>, name| {
            let kernel = host.runner().kernel();
            kernel
                .node_by_key(kernel.find_by_test_id(name)[0])
                .unwrap()
                .id
        };
        let toggle = id(&host, "toggle");
        let row = id(&host, "held");
        host.dispatch_at(toggle, Event::Press, 0.0);
        assert_eq!(host.springs().frame_compilations(), 64);
        let (hold, _) = host
            .begin_hold(row, Property::Translate, Value::new(80.0, 0.0), 1.0)
            .unwrap()
            .unwrap();
        for step in 2..102 {
            let batch = host
                .update_hold(
                    hold.token.serial(),
                    Value::new(80.0 + f64::from(step), 0.0),
                    f64::from(step),
                )
                .unwrap()
                .unwrap();
            assert!(!batch.contains("\"op\":\"animate\""));
        }
        assert_eq!(
            host.springs().frame_compilations(),
            64,
            "100 input samples must not rebuild the 64 unrelated curves"
        );
        host.end_hold(
            hold.token.serial(),
            HoldEnd::Release {
                velocity: Value::new(-20.0, 0.0),
            },
            101.0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(host.springs().frame_compilations(), 65);
        let (caught, _) = host
            .begin_hold(row, Property::Translate, Value::new(181.0, 0.0), 101.0)
            .unwrap()
            .unwrap();
        host.end_hold(
            caught.token.serial(),
            HoldEnd::Release {
                velocity: Value::new(30.0, 0.0),
            },
            101.0,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            host.springs().frame_compilations(),
            66,
            "same-clock rebegin/release with a new velocity compiles once"
        );
    }
}
