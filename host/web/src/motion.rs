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
use exact_motion::{Change, Engine, Property, Value};
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
#[derive(Debug, Default, Clone)]
pub struct Springs {
    engine: Engine,
    /// Springs the page is playing: `(start, target)` per property, so a
    /// commit that leaves a spring untouched says nothing about it.
    playing: BTreeMap<(u64, Property), (f64, Value)>,
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
            let applied = kernel.motion_sync(receipt).apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        }
        let mut out = Vec::new();
        for p in self.engine.frame() {
            let key = (p.node, p.property);
            let Some(view) = self.view_of(kernel, p.node) else {
                self.playing.remove(&key);
                continue;
            };
            match self.engine.spring_frames(p.node, p.property) {
                Some(frames) => {
                    let target = *frames.values.last().expect("at least two frames");
                    if self.playing.get(&key) == Some(&(frames.start, target)) {
                        continue;
                    }
                    self.playing.insert(key, (frames.start, target));
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
                    if let Some((_, target)) = self.playing.remove(&key) {
                        if p.value != target {
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
