//! Layout transition on Linux, and its refusal of exit animation (LLP 1063).
//!
//! A node with a `-exact-layout-transition` has its laid-out box in its parent
//! (`Kernel::layout_box`) observed as `Property::Layout` after every layout;
//! the engine's transition rules apply. What it shows against the laid-out
//! box is an offset, which the painter applies outermost, and a size
//! difference, which only its surface and the clip on its children take, from
//! the box's top-left corner (`Presented::layout`), so a moving parent carries
//! its children and nothing is laid out per frame. A node that gains the row is
//! seeded with the box it had before the commit's layout, so its first move
//! after gaining it animates.
//!
//! Exit animation is refused here: this painter reads the live kernel tree
//! every frame, and a destroyed node is not in it, so there is nothing to
//! keep painting. A removed node leaves at once and the journal says so.
//!
//! Shared-element flights (LLP 1013.000) are refused for a like reason in
//! stage 1: an arriver appears in place, and the journal says so once.

use super::*;
use exact_kernel::motion::LayoutMotion;

/// What the host keeps for layout transitions.
#[derive(Debug, Default)]
pub(super) struct Presence {
    /// Nodes declaring a layout transition.
    pub(super) layout: LayoutMotion,
    /// Whether the refusal of exit animation is in the journal.
    refused: bool,
    /// Whether the refusal of shared-element flights is.
    refused_flights: bool,
    /// A resize lays out next: positions are taken, not animated.
    pub(super) snap: bool,
}

impl<D: DataSource> Host<D> {
    /// Which nodes declare a layout transition, after a commit and before
    /// its layout; and the refusal, the first time a node leaves with an exit.
    pub(super) fn track_presence(&mut self, receipts: &[Timed]) {
        for t in receipts {
            if !t.receipt.exits.is_empty() && !self.presence.refused {
                self.presence.refused = true;
                self.log("-exact-exit-animation: refused on Linux (LLP 1063): the painter reads the live tree, so a removed node leaves at once");
            }
            if !t.receipt.handoffs.is_empty() && !self.presence.refused_flights {
                self.presence.refused_flights = true;
                self.log("shared element: flights are not drawn on Linux (LLP 1013.000); an arriver appears in place");
            }
            let retired =
                self.presence
                    .layout
                    .seed(self.runner.kernel(), &t.receipt, &mut self.engine);
            self.retire_layout(retired);
        }
    }

    /// An agent's jump to `to`, stopping after a timer that sends
    /// ([`exact_runner::Runner::advance_until_request`]). The engine hears
    /// each commit of a jump at its timer's due time (LLP 1002 D3), but the
    /// kernel holds only the jump's last tree, laid out at its end: a box a
    /// timer moved inside the jump would start its layout transition there.
    /// While a node declares a layout transition, the jump lays out at each
    /// due time, as real frames would; only a request stops it short of `to`,
    /// so these stops spend none of the agent's request stops.
    pub(super) fn advance_in_layout_steps(&mut self, to: f64) -> Option<String> {
        if self.presence.layout.is_empty() {
            let a = self.runner.advance_until_request(to);
            return self.advanced(a).0;
        }
        let mut error = None;
        for _ in 0..LAYOUT_STOPS {
            let due = self.runner.timer_due_ms().unwrap_or(to);
            let step = due.max(self.runner.now_ms()).min(to);
            let a = self.runner.advance_until_request(step);
            // A commit that sent stops the jump: the next send would drop
            // its reply (LLP 1016 D5).
            let stopped = a.error.is_some() || a.stopped_on_request || step >= to;
            error = error.or(self.advanced(a).0);
            if stopped || self.presence.layout.is_empty() {
                break;
            }
        }
        error
    }

    /// Observe every tracked node's box, after a layout.
    pub(super) fn observe_layout(&mut self) {
        let snap = std::mem::take(&mut self.presence.snap);
        let retired =
            self.presence
                .layout
                .observe_all(self.runner.kernel(), &mut self.engine, snap);
        self.retire_layout(retired);
    }

    fn retire_layout(&mut self, retired: Vec<NodeKey>) {
        for key in retired {
            if let Some(p) = self.keys.get(&key).and_then(|v| self.presented.get_mut(v)) {
                p.layout = Presented::IDENTITY.layout;
            }
        }
    }
}

/// The most layout stops one agent call makes: a frame task's every frame
/// across ten minutes. The agent's loop calls again past it.
const LAYOUT_STOPS: usize = 60 * 60 * 10;
