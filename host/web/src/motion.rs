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

use exact_kernel::motion::{motion_node, targets, MotionSync};
use exact_kernel::{CommitReceipt, Kernel, NodeKey, ViewId};
use exact_motion::{
    Change, Engine, EngineError, HoldEnd, HoldStart, HoldToken, Property, SpringDescriptor, Value,
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
}

impl Springs {
    /// Empty, at clock zero.
    pub fn new() -> Springs {
        Springs::default()
    }

    /// The engine (for tests and hosts that want to read presentation values).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Number of property springs retained for the current mounted tree.
    pub fn playing_count(&self) -> usize {
        self.playing.len()
    }

    pub(crate) fn token(&self, serial: u64) -> Option<HoldToken> {
        self.holds
            .get(&serial)
            .copied()
            .filter(|t| self.engine.has_hold(*t))
    }

    pub(crate) fn begin_hold(
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

    pub(crate) fn update_hold(
        &mut self,
        serial: u64,
        value: Value,
        now: f64,
    ) -> Result<bool, EngineError> {
        let Some(token) = self.token(serial) else {
            return Ok(false);
        };
        self.engine.update_hold(token, now, value)
    }

    pub(crate) fn end_hold(
        &mut self,
        serial: u64,
        end: HoldEnd,
        now: f64,
    ) -> Result<bool, EngineError> {
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
    pub fn adopt(&mut self, kernel: &Kernel, views: &[ViewId]) {
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

    /// Feed the receipts of one commit at `now` seconds and return what the
    /// page must do: springs to start (or restart from their current value)
    /// and springs that stopped being the truth for their property.
    pub fn commit(
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
        self.holds.retain(|_, token| self.engine.has_hold(*token));
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

    fn view_of(&self, kernel: &Kernel, node: u64) -> Option<ViewId> {
        let key = NodeKey {
            index: node as u32,
            generation: (node >> 32) as u32,
        };
        kernel.node_by_key(key).map(|n| n.id)
    }
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
        assert_eq!(host.springs().frame_compilations, 64);
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
            host.springs().frame_compilations,
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
        assert_eq!(host.springs().frame_compilations, 65);
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
            host.springs().frame_compilations,
            66,
            "same-clock rebegin/release with a new velocity compiles once"
        );
    }
}
