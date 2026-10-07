//! Shared-element flights (LLP 1013.000 D4).
//!
//! A receipt's handoff names a destroyed node and the created node its name
//! moved to. The host tells the presenter `flight` before the batch's
//! destroys, so it can capture where the leaver is shown while it still
//! exists, and starts the flight's curve: the engine's `Layout` property on
//! a host-owned motion node (the arriver's, with [`FLIGHT`] set), first seen
//! at 0 and then observed at [`END`] under the handoff's declaration. So
//! springs, easings, interruption, the agent's seekable clock and
//! `settle_time` are the engine's, and no property is added. The presenter
//! gets the curve's progress as `present … "flight"` each frame, owns every
//! rectangle (window geometry, scroll offsets and crops are its state, not
//! the kernel's), and `land` when the curve settles.

use super::*;
use exact_kernel::motion::motion_node;
use exact_kernel::CommitReceipt;

/// Marks a flight's motion node; no kernel node's generation reaches it.
pub(super) const FLIGHT: u64 = 1 << 63;
/// The progress a flight ends at. Points-scaled, so the engine's rest
/// threshold (in points for `Layout`) settles it as finely as a box.
const END: f64 = 1000.0;

/// One flight in progress.
#[derive(Debug)]
struct Flight {
    view: ViewId,
    node: u64,
    /// When a spring flight lands (engine seconds): as UIKit's spring
    /// animators finish, not at the engine's rest.
    lands_at: Option<f64>,
}

/// Where a spring flight is done: within 1/1000 of its travel and moving
/// under 1/20 of it a second, about where UIKit's spring animators finish
/// (`CASpringAnimation.settlingDuration`: 0.40 s for a critically damped
/// response-0.25 spring; this gives 0.37 s). The engine's own rest
/// (`REST_THRESHOLD`, absolute, in the progress's 1000ths) ran a flight to
/// 1e-6 of its travel, 0.78 s for that spring, and until it landed the
/// flying view covered what lies above its slot (Signal Clone: a photo's
/// footer came back 0.4 s late).
const LAND_DISPLACEMENT: f64 = 1e-3;
const LAND_SPEED: f64 = 0.05;

/// Seconds from the start until a unit spring flight lands: the first time
/// on a 240 Hz grid from which it stays within those bounds (so a slow
/// crossing of an underdamped spring is not taken for its end), up to the
/// engine's 10 s cap; `None` past it (the engine's rest decides).
fn land_after(config: &exact_motion::SpringConfig) -> Option<f64> {
    let rate = 240.0;
    let samples = rate as usize * 10;
    let within = |n: usize| {
        let s = config.sample(1.0, 0.0, n as f64 / rate);
        s.displacement.abs() < LAND_DISPLACEMENT && s.velocity.abs() < LAND_SPEED
    };
    if !within(samples) {
        return None;
    }
    let last_out = (0..samples).rev().find(|&n| !within(n));
    Some(last_out.map_or(0.0, |n| (n + 1) as f64 / rate))
}

/// The flights the host is running.
#[derive(Debug, Default)]
pub(crate) struct Flights {
    running: Vec<Flight>,
}

impl<D: DataSource> Host<D> {
    /// A receipt's handoffs, before its destroys reach the batch: the
    /// presenter captures each leaver while it is still there.
    pub(super) fn begin_flights(&mut self, receipt: &CommitReceipt, batch: &mut Batch) {
        for h in &receipt.handoffs {
            let Some(&from) = self.keys.get(&h.from) else {
                continue;
            };
            let Some(to) = self.runner.kernel().node_by_key(h.to).map(|n| n.id) else {
                continue;
            };
            batch.flight(to, from);
        }
    }

    /// Start a receipt's flights at its time, once the engine is there.
    pub(super) fn start_flights(&mut self, receipt: &CommitReceipt) {
        for h in &receipt.handoffs {
            let Some(to) = self.runner.kernel().node_by_key(h.to).map(|n| n.id) else {
                continue;
            };
            let node = motion_node(h.to) | FLIGHT;
            self.end_flight_of(to);
            let declared = self.engine.set_layout_transition(
                node,
                &exact_motion::Transitions(vec![h.transition.clone()]),
            );
            debug_assert!(declared.is_ok(), "the kernel validated the row");
            for x in [0.0, END] {
                let observed = self.engine.observe(exact_motion::Change {
                    node,
                    property: Property::Layout,
                    value: exact_motion::Value::four(x, 0.0, 0.0, 0.0),
                    velocity: None,
                });
                debug_assert!(observed.is_ok(), "progress is finite");
            }
            let lands_at = match &h.transition.timing {
                exact_motion::TimingFunction::Spring(config) => {
                    land_after(config).map(|t| self.engine.now() + h.transition.delay.max(0.0) + t)
                }
                _ => None,
            };
            self.flights.running.push(Flight {
                view: to,
                node,
                lands_at,
            });
        }
    }

    /// A destroyed view's flight ends with it (an interruption's new flight
    /// is already captured from where it was shown).
    pub(super) fn end_flight_of(&mut self, view: ViewId) {
        let engine = &mut self.engine;
        self.flights.running.retain(|f| {
            let keep = f.view != view;
            if !keep {
                engine.remove(f.node);
            }
            keep
        });
    }

    /// A flight's frame, if this presentation is one.
    pub(super) fn present_flight(&self, p: &exact_motion::Presentation, batch: &mut Batch) -> bool {
        if p.node & FLIGHT == 0 {
            return false;
        }
        if let Some(f) = self.flights.running.iter().find(|f| f.node == p.node) {
            batch.present(f.view, "flight", p.value.x / END, 0.0);
        }
        true
    }

    /// Flights whose curve has settled land.
    pub(super) fn land_flights(&mut self, batch: &mut Batch) {
        let engine = &mut self.engine;
        let now = engine.now();
        self.flights.running.retain(|f| {
            let flying =
                engine.is_active(f.node, Property::Layout) && f.lands_at.is_none_or(|at| now < at);
            if !flying {
                engine.remove(f.node);
                batch.land(f.view);
            }
            flying
        });
    }
}
