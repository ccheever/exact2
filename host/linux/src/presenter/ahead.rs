//! A press's screen, laid out ahead of its release.
//!
//! The first time a press opens a screen, the release shapes and breaks all
//! of that screen's text before it can answer (on a Pixel 10 Pro XL, 2 ms of
//! the 9 ms the daily benchmark's first mail takes to be ready, which costs
//! it a frame). A touch comes down some 60 ms before it lifts. The runner
//! says what the press would mount ([`exact_runner::Runner::foresee`]); here
//! that is laid out in a scratch kernel over this presenter's text engine,
//! and the release's own layout finds its paragraphs in the engine's cache.
//!
//! Nothing of it is seen. The scratch kernel holds copies of the live nodes
//! the new views would hang from (each at the size the last layout gave it,
//! so the new views are offered the widths they will be) and the new views,
//! under ids the runner never gives out; it is dropped when its layout ends.
//! The live kernel, the runner and the painter are not touched, nothing is
//! committed, and a guess that turns out wrong costs only its own time.
//!
//! The work is cut in slices ([`Presenter::foresee_slice`]): a slice stops
//! shaping when its budget is spent (the texts it shaped stay in the cache)
//! and the next lays the scratch out again. The host runs a slice only when
//! it has nothing else to do, and forgets the rest when the touch lifts or
//! is cancelled, and when anything commits.

use super::*;
use exact_kernel::{
    BoxSizing, ButtonMeasure, ButtonMeasureRequest, Dimension, Direction, FieldChrome,
    FieldChromeRequest, Kernel, Offer, Op, StyleId, StyleMask, TextMeasureRequest, TextMeasurer,
    TextMetrics,
};
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// A pass that never finishes within its slices is given up.
const PASSES: u8 = 12;

/// What a touch down left to do.
pub(crate) struct Ahead {
    target: ViewId,
    /// When the touch came down, the host's clock.
    at: f64,
    /// The kernel's epoch at the touch: a commit since makes the guess stale.
    epoch: u64,
    /// The scratch kernel's ops and roots, once the runner has been asked.
    scratch: Option<(Vec<Op>, Vec<ViewId>)>,
    passes: u8,
}

/// The engine's measurer until `until`; then nothing is shaped, and the pass
/// (whose answers no longer mean anything) only runs out.
struct Budgeted {
    inner: Measurer,
    until: Instant,
    cut: Rc<Cell<bool>>,
}

impl Budgeted {
    fn spent(&self) -> bool {
        if !self.cut.get() && Instant::now() >= self.until {
            self.cut.set(true);
        }
        self.cut.get()
    }
}

impl TextMeasurer for Budgeted {
    fn measure_revision(&self) -> u64 {
        self.inner.measure_revision()
    }
    fn field_chrome(&mut self, request: &FieldChromeRequest) -> FieldChrome {
        self.inner.field_chrome(request)
    }
    fn button_measure(&mut self, request: &ButtonMeasureRequest) -> Option<ButtonMeasure> {
        if self.spent() {
            return None;
        }
        self.inner.button_measure(request)
    }
    // The engine's language is the live document's: a scratch never sets it.
    fn set_language(&mut self, _language: &str) {}
    // By content alone (the default `measure_identified`): a scratch node's
    // stamp names nothing the engine should remember.
    fn measure(&mut self, request: &TextMeasureRequest<'_>) -> TextMetrics {
        if self.spent() {
            return TextMetrics::default();
        }
        self.inner.measure(request)
    }
    fn height_free(&self) -> bool {
        true
    }
}

impl<D: DataSource> Presenter<D> {
    /// A touch came down at (`x`, `y`), points: what its press would mount
    /// is to be laid out ahead, in [`Presenter::foresee_slice`]s.
    pub fn foresee_press(&mut self, x: f32, y: f32, now_ms: f64) {
        self.ahead = self
            .hit(x, y)
            .and_then(|hit| self.handler_target(hit, EventKind::Press))
            .map(|target| Ahead {
                target,
                at: now_ms,
                epoch: self.host.kernel().epoch(),
                scratch: None,
                passes: 0,
            });
    }

    /// When the touch whose press is still to lay out came down.
    pub fn foreseen_at(&self) -> Option<f64> {
        self.ahead.as_ref().map(|a| a.at)
    }

    /// Give up what [`Presenter::foresee_press`] left to do.
    pub fn forget_press(&mut self) {
        self.ahead = None;
    }

    /// One slice of it, shaping for about `budget`; whether more is left.
    pub fn foresee_slice(&mut self, budget: Duration) -> bool {
        let Some(mut ahead) = self.ahead.take() else {
            return false;
        };
        if ahead.epoch != self.host.kernel().epoch() || ahead.passes >= PASSES {
            return false;
        }
        if ahead.scratch.is_none() {
            // The first slice asks the runner, and lays nothing out.
            ahead.scratch = self.scratch(ahead.target);
            let more = ahead.scratch.is_some();
            if more {
                self.ahead = Some(ahead);
            }
            return more;
        }
        ahead.passes += 1;
        let (ops, roots) = ahead.scratch.as_ref().expect("asked");
        let cut = Rc::new(Cell::new(false));
        let mut kernel = Kernel::new(Box::new(Budgeted {
            inner: Measurer(self.text.clone()),
            until: Instant::now() + budget,
            cut: cut.clone(),
        }));
        let runner = self.host.runner();
        let direction = if runner.direction() == "rtl" {
            Direction::Rtl
        } else {
            Direction::Ltr
        };
        let language = runner.resolved_locale();
        if kernel
            .apply_document(0, 1, ops, Some((language, direction)))
            .is_err()
        {
            return false;
        }
        let (w, h) = self.host.viewport();
        crate::text::cache::deferring_eviction(|| {
            for root in roots {
                if kernel.compute_layout(*root, Offer::definite(w, h)).is_err() {
                    break;
                }
            }
        });
        let more = cut.get();
        if more {
            self.ahead = Some(ahead);
        }
        more
    }

    /// The scratch kernel for what a press on `target` would mount: its ops
    /// and its roots.
    fn scratch(&self, target: ViewId) -> Option<(Vec<Op>, Vec<ViewId>)> {
        let seen = self.host.runner().foresee(target)?;
        let kernel = self.host.kernel();
        // The live nodes the new views hang from, up to their roots: each
        // with the children the scratch gives it.
        let mut held: BTreeMap<ViewId, Vec<ViewId>> = BTreeMap::new();
        let mut roots = Vec::new();
        for mount in &seen.mounts {
            let frame = kernel.node(mount.parent)?.frame;
            if frame.width <= 0.0 || frame.height <= 0.0 {
                continue;
            }
            held.entry(mount.parent)
                .or_default()
                .extend(mount.roots.iter().copied());
            let mut at = mount.parent;
            while let Some(parent) = kernel.node(at)?.parent {
                let children = held.entry(parent).or_default();
                if children.contains(&at) {
                    break;
                }
                children.push(at);
                at = parent;
            }
            if kernel.node(at)?.parent.is_none() && !roots.contains(&at) {
                roots.push(at);
            }
        }
        if roots.is_empty() {
            return None;
        }
        // Every view is made before a child list names it.
        let mut ops = Vec::with_capacity(seen.ops.len() + 3 * held.len() + roots.len());
        for &id in held.keys() {
            let node = kernel.node(id)?;
            // Where the new views hang, the node's own style lays them out;
            // above it only what text inherits matters. Each is as large as
            // it was last laid out.
            let mut patch = Box::new(node.style.clone());
            if !seen.mounts.iter().any(|m| m.parent == id) {
                patch.mask = patch.mask.intersect(StyleMask::INHERITED);
            }
            patch.width = Dimension::Points(node.frame.width);
            patch.height = Dimension::Points(node.frame.height);
            patch.box_sizing = BoxSizing::BorderBox;
            for row in [StyleId::Width, StyleId::Height, StyleId::BoxSizing] {
                patch.mask.set(row);
            }
            ops.push(Op::CreateView {
                id,
                node_type: node.node_type,
            });
            ops.push(Op::SetStyle { id, patch });
        }
        ops.extend(seen.ops);
        ops.extend(
            held.into_iter()
                .map(|(id, children)| Op::SetChildren { id, children }),
        );
        ops.extend(roots.iter().map(|&id| Op::AttachRoot { id }));
        Some((ops, roots))
    }
}
