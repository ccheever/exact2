//! The runner wrapped for a painter: commits → layout → motion, with the
//! kernel read directly — no batch, no mirror.
//!
//! @ref LLP 1015 §1; LLP 1008 §1 (the same orchestration, whose batch
//! exists because a foreign view tree consumes it — here nothing does)
//!
//! After every commit the host lays the roots out with the kernel's layout
//! under the viewport, feeds the motion engine the commit (LLP 1003 §4),
//! seeks it to the app's clock, and keeps each node's presentation values.
//! The painter then reads frames, styles, and props from the kernel and the
//! presentation values from here. The kernel is the single source of truth
//! and the only copy.

use crate::paint::Presented;
use exact_kernel::motion::{motion_node, targets, MotionSync};
use exact_kernel::{Kernel, NodeKey, Offer, TextMeasurer, ViewId};
use exact_motion::{Change, Engine, Property};
use exact_plan::Plan;
use exact_runner::{Carried, DataSource, Event, Outcome, RequestOut, Runner, RunnerError, Timed};
use std::collections::BTreeMap;

/// Why the host refused to boot.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
    Painter(String),
    Asset(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// One runner, one painter.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    engine: Engine,
    keys: BTreeMap<NodeKey, ViewId>,
    presented: BTreeMap<ViewId, Presented>,
    viewport: (f32, f32),
    now_ms: f64,
}

impl<D: DataSource> Host<D> {
    /// Boot from plan bytes with a text measurer under a viewport (points):
    /// decode, boot the runner, lay out, hear the whole tree in the engine.
    pub fn boot(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        Host::boot_with(plan_bytes, data, measurer, width, height, None)
    }

    /// Boot carrying an earlier host's state (the dev reload, LLP 1007 §6).
    pub fn boot_with(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
    ) -> Result<(Host<D>, Option<String>), HostError> {
        let plan = Plan::decode(plan_bytes).map_err(HostError::Plan)?;
        let kernel = Kernel::new(measurer);
        let runner = match carried {
            Some(c) => Runner::boot_carrying(plan, data, kernel, c),
            None => Runner::boot(plan, data, kernel),
        }
        .map_err(HostError::Runner)?;
        let mut host = Host {
            runner,
            engine: Engine::new(),
            keys: BTreeMap::new(),
            presented: BTreeMap::new(),
            viewport: (width, height),
            now_ms: 0.0,
        };
        // The engine hears the whole tree once: values, no transitions.
        let mut sync = MotionSync::default();
        for id in host.preorder() {
            if let Some(node) = host.runner.kernel().node(id) {
                host.keys.insert(node.key, id);
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
        }
        let applied = sync.apply(&mut host.engine);
        debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        let error = host.layout().err();
        host.present();
        Ok((host, error))
    }

    /// Tell the runner what this binary knows about its delivery (LLP 1030
    /// D7), from the archive's `compat.json`: the compatibility id, whether
    /// an update store is linked, and the executors. A `delivery` resource
    /// is answered again in that one commit, which the painter picks up
    /// like any other — the kernel is the display list here.
    pub fn set_delivery_from_compat(&mut self, json: &str) -> Option<String> {
        match self.runner.set_delivery_from_compat(json) {
            Ok(None) => None,
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => Some(format!("delivery: {e:?}")),
        }
    }

    /// The delivery facts whole (LLP 1030 D7) — what the update store has to
    /// say after a check or an activation, on top of the binary's own — into
    /// the runner, as one commit when they changed.
    pub fn set_delivery(&mut self, delivery: exact_runner::Delivery) -> Option<String> {
        match self.runner.set_delivery(delivery) {
            Ok(None) => None,
            Ok(Some(receipt)) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => Some(format!("delivery: {e:?}")),
        }
    }

    /// The commands the last commits' actions asked for, in order (LLP 1005
    /// §3): `deliveryCheck`, `deliveryActivate`, `setScheme`.
    pub fn take_commands(&mut self) -> Vec<exact_runner::Command> {
        self.runner.take_commands()
    }

    /// The runner.
    pub fn runner(&self) -> &Runner<D> {
        &self.runner
    }

    /// The kernel.
    pub fn kernel(&self) -> &Kernel {
        self.runner.kernel()
    }

    /// The motion engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// What a reload keeps (`Runner::carry`).
    pub fn carry(&self) -> Carried {
        self.runner.carry()
    }

    /// The viewport, points.
    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    /// The clock's last value, milliseconds.
    pub fn now(&self) -> f64 {
        self.now_ms
    }

    /// Whether the runner has timers (the presenter runs its clock).
    pub fn has_timers(&self) -> bool {
        self.runner.has_timers()
    }

    /// Whether motion is running (the presenter runs frames).
    pub fn motion(&self) -> bool {
        !self.engine.quiescent()
    }

    /// The roots, in order.
    pub fn roots(&self) -> Vec<ViewId> {
        self.runner.roots()
    }

    /// A node's presentation values: the engine's, else the committed
    /// style's.
    pub fn presented(&self, id: ViewId) -> Presented {
        if let Some(p) = self.presented.get(&id) {
            return *p;
        }
        self.runner
            .kernel()
            .node(id)
            .map_or(Presented::IDENTITY, |n| Presented::from_style(n.style))
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, `logs`
    /// from the runner; `settle` from the engine.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self.engine.settle_time() {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t * 1000.0)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        exact_runner::agent::handle(&self.runner, request)
    }

    /// A line into the runner's journal (the agent's `logs`): a host fact
    /// worth reading beside the app's own lines.
    pub fn log(&mut self, line: impl Into<String>) {
        self.runner.log(line);
    }

    /// The hosts the app may reach (LLP 1016 D6), as the data crate declares them.
    pub fn grants(&mut self) -> String {
        self.runner.data().grants().to_string()
    }

    /// The requests the runner handed out since the last take (LLP 1016 D2).
    pub fn take_requests(&mut self) -> Vec<RequestOut> {
        self.runner.take_requests()
    }

    /// The executor's replies, oldest first, each a commit at `now_ms` (a
    /// ticket no longer held commits nothing); a reply the source cannot
    /// shape is the error, and the ones before it stand.
    pub fn fulfill_all(&mut self, outcomes: Vec<(u64, Outcome)>, now_ms: f64) -> Option<String> {
        self.now_ms = now_ms.max(self.now_ms);
        let mut receipts = Vec::new();
        let mut error = None;
        for (ticket, outcome) in outcomes {
            match self.runner.fulfill(ticket, outcome) {
                Ok(Some(receipt)) => receipts.push(Timed {
                    at_ms: self.now_ms,
                    receipt,
                }),
                Ok(None) => {}
                Err(e) => {
                    error = Some(format!("{e:?}"));
                    break;
                }
            }
        }
        self.commit(&receipts, error)
    }

    /// Deliver an event at the app's clock (milliseconds). A refusal is the
    /// error; the tree is untouched (as the kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> Option<String> {
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.dispatch(view, event) {
            Ok(receipt) => self.commit(
                &[Timed {
                    at_ms: self.now_ms,
                    receipt,
                }],
                None,
            ),
            Err(e) => self.commit(&[], Some(format!("{e:?}"))),
        }
    }

    /// Move the clock: every timer due fires at its own due time (LLP 1012
    /// §2). The clock lands where the runner says — a timer's refusal stops
    /// it at that timer's due time and is the error; the commits before it
    /// are shown.
    pub fn advance(&mut self, now_ms: f64) -> Option<String> {
        let a = self.runner.advance_timed(now_ms);
        self.now_ms = a.now_ms.max(self.now_ms);
        let error = a.error.map(|e| format!("{e:?}"));
        self.commit(&a.receipts, error)
    }

    /// An image loaded: its intrinsic size in points (`None` when it failed
    /// or was cleared). Lays out again.
    pub fn set_intrinsic(&mut self, view: ViewId, size: Option<(f32, f32)>) -> Option<String> {
        match self.runner.kernel_mut().set_intrinsic_size(view, size) {
            Ok(()) => self.layout().err(),
            Err(e) => Some(format!("intrinsic: {e:?}")),
        }
    }

    /// The viewport changed: lay out again.
    pub fn resize(&mut self, width: f32, height: f32) -> Option<String> {
        self.viewport = (width, height);
        self.layout().err()
    }

    /// A motion frame: seek the engine to `now_ms`; presentation values
    /// follow. Nothing else moves.
    pub fn tick(&mut self, now_ms: f64) {
        self.now_ms = now_ms.max(self.now_ms);
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        self.present();
    }

    fn commit(&mut self, receipts: &[Timed], error: Option<String>) -> Option<String> {
        for t in receipts {
            let r = &t.receipt;
            for key in &r.destroyed {
                if let Some(id) = self.keys.remove(key) {
                    self.presented.remove(&id);
                }
            }
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    self.keys.insert(*key, node.id);
                }
            }
        }
        let layout_error = if receipts.is_empty() {
            None
        } else {
            self.layout().err()
        };
        // Motion last, each commit at its own time: targets are in place
        // before the engine hears them, and a transition a timer started is
        // born at that timer's due time — one seek and sixty give the same
        // bits (LLP 1002 D3; LLP 1012 §2).
        for t in receipts {
            let seek = self.engine.advance(t.at_ms / 1000.0);
            debug_assert!(seek.is_ok(), "the clock never runs backwards here");
            let applied = self
                .runner
                .kernel()
                .motion_sync(&t.receipt)
                .apply(&mut self.engine);
            debug_assert!(applied.is_ok(), "kernel rows are always valid engine input");
        }
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        self.present();
        error.or(layout_error)
    }

    fn layout(&mut self) -> Result<(), String> {
        let (w, h) = self.viewport;
        for root in self.runner.roots() {
            self.runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(w, h))
                .map_err(|e| format!("layout: {e:?}"))?;
        }
        Ok(())
    }

    /// Every presentation value the engine changed, kept by node.
    fn present(&mut self) {
        for p in self.engine.frame() {
            let key = NodeKey {
                index: p.node as u32,
                generation: (p.node >> 32) as u32,
            };
            let Some(view) = self.keys.get(&key).copied() else {
                continue;
            };
            let base = self.presented(view);
            let entry = self.presented.entry(view).or_insert(base);
            match p.property {
                Property::Translate => entry.translate = (p.value.x as f32, p.value.y as f32),
                Property::Scale => entry.scale = p.value.x as f32,
                Property::Rotate => entry.rotate = p.value.x as f32,
                Property::Opacity => entry.opacity = p.value.x as f32,
            }
        }
    }

    /// Every live node in preorder.
    pub fn preorder(&self) -> Vec<ViewId> {
        let kernel = self.runner.kernel();
        let mut stack: Vec<ViewId> = self.runner.roots().into_iter().rev().collect();
        let mut order = Vec::new();
        while let Some(id) = stack.pop() {
            order.push(id);
            if let Some(node) = kernel.node(id) {
                let mut children = node.children();
                children.reverse();
                stack.extend(children);
            }
        }
        order
    }
}
