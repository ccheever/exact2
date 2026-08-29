//! The runner wrapped for a presenter: receipts → batches, with layout and
//! motion.
//!
//! @ref LLP 1008 §1
//!
//! After every commit the host walks the receipt (destroy, create, props,
//! style, children — the web host's rule: the view tree mirrors the kernel
//! tree), lays the roots out with the kernel's layout under the viewport,
//! emits every parent-relative frame that changed and every scroll
//! container's content size that changed, then feeds the motion engine the
//! commit (LLP 1003 §4), seeks it to the app's clock, and emits each
//! presentation value that changed. The kernel is the single source of
//! truth; the mirror is a memo of what the presenter has been told.

use crate::batch::Batch;
use crate::style;
use exact_kernel::motion::{motion_node, targets, MotionSync};
use exact_kernel::{
    Frame, Kernel, NodeKey, NodeRef, NodeType, Offer, Overflow, PropId, PropValue, TextMeasurer,
    ViewId,
};
use exact_motion::{Change, Engine, Property};
use exact_plan::{EventKind, Plan};
use exact_runner::{Carried, DataSource, Event, Runner, RunnerError, Timed};
use std::collections::BTreeMap;

/// Why the host refused.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum HostError {
    Plan(exact_plan::PlanError),
    Runner(RunnerError),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Mirror {
    props: BTreeMap<String, String>,
    style: String,
    children: Vec<ViewId>,
    frame: Option<(f32, f32, f32, f32)>,
    content: Option<(f32, f32)>,
}

/// One runner, one presenter.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    mirror: BTreeMap<ViewId, Mirror>,
    keys: BTreeMap<NodeKey, ViewId>,
    roots: Vec<ViewId>,
    engine: Engine,
    viewport: (f32, f32),
    now_ms: f64,
}

impl<D: DataSource> Host<D> {
    /// Boot from plan bytes with the app's text measurer and viewport (points):
    /// decode (a validation pass), boot the runner, lay out, and produce the
    /// first batch, which creates and places the whole tree.
    pub fn boot(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
    ) -> Result<(Host<D>, String), HostError> {
        Host::boot_with(plan_bytes, data, measurer, width, height, None)
    }

    /// Boot carrying an earlier host's state (the dev reload, LLP 1007 §6):
    /// slots by name where their types still fit, settled resources where
    /// their arguments still match, the clock.
    pub fn boot_with(
        plan_bytes: &[u8],
        data: D,
        measurer: Box<dyn TextMeasurer>,
        width: f32,
        height: f32,
        carried: Option<&Carried>,
    ) -> Result<(Host<D>, String), HostError> {
        let plan = Plan::decode(plan_bytes).map_err(HostError::Plan)?;
        let kernel = Kernel::new(measurer);
        let runner = match carried {
            Some(c) => Runner::boot_carrying(plan, data, kernel, c),
            None => Runner::boot(plan, data, kernel),
        }
        .map_err(HostError::Runner)?;
        let mut host = Host {
            runner,
            mirror: BTreeMap::new(),
            keys: BTreeMap::new(),
            roots: Vec::new(),
            engine: Engine::new(),
            viewport: (width, height),
            now_ms: 0.0,
        };
        let mut batch = Batch::new();
        let order = host.preorder();
        for id in &order {
            host.create(*id, &mut batch);
        }
        for id in &order {
            host.emit_children(*id, &mut batch);
        }
        host.roots = host.runner.roots();
        batch.roots(&host.roots.clone());
        for s in host.runner.take_surface_updates() {
            batch.surface(s.view, &s.name, &s.values);
        }
        // The engine hears the whole tree once: values, no transitions.
        let mut sync = MotionSync::default();
        for id in &order {
            if let Some(node) = host.runner.kernel().node(*id) {
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
        let error = host.layout(&mut batch).err();
        host.present(&mut batch, true);
        let timers = host.runner.has_timers();
        let motion = !host.engine.quiescent();
        let clock = host.runner.now_ms();
        Ok((host, batch.finish(timers, motion, clock, error.as_deref())))
    }

    /// The runner.
    pub fn runner(&self) -> &Runner<D> {
        &self.runner
    }

    /// What a reload keeps (`Runner::carry`).
    pub fn carry(&self) -> Carried {
        self.runner.carry()
    }

    /// The motion engine: presentation values as the presenter shows them.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, and
    /// `logs` from the runner; `settle` — the clock at which the last
    /// transition in flight ends, milliseconds, `null` when quiescent — from
    /// the engine, which is what the presenter's `clock` advances to.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self.engine.settle_time() {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t * 1000.0)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        exact_runner::agent::handle(&self.runner, request)
    }

    /// Deliver an event at the app's clock (milliseconds); the batch makes
    /// the presenter equal to the tree after the commit, laid out, with any
    /// motion the change started. A refusal is reported in the batch's
    /// `error`, and the presenter is untouched (as the kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.dispatch(view, event) {
            Ok(receipt) => {
                let at_ms = self.now_ms;
                self.commit(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => self.commit(&[], Some(format!("{e:?}"))),
        }
    }

    /// [`Host::dispatch_at`] at the clock's last value.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> String {
        self.dispatch_at(view, event, self.now_ms)
    }

    /// Move the clock; every timer due fires at its own time; one batch for
    /// all of them, the engine hearing each commit at the time it was made.
    /// A timer's refusal stops the clock there: the commits before it are in
    /// the batch, the refusal in `error`, and `clock` says where the runner
    /// stands.
    pub fn advance(&mut self, now_ms: f64) -> String {
        let a = self.runner.advance_timed(now_ms);
        self.now_ms = a.now_ms.max(self.now_ms);
        let error = a.error.map(|e| format!("{e:?}"));
        self.commit(&a.receipts, error)
    }

    /// An image loaded: its intrinsic size, in points (`None` when it failed
    /// or was cleared). Lays out again; the batch carries the frames that
    /// moved — the image's, and everything its size pushed.
    pub fn set_intrinsic(&mut self, view: ViewId, size: Option<(f32, f32)>) -> String {
        let mut batch = Batch::new();
        let error = match self.runner.kernel_mut().set_intrinsic_size(view, size) {
            Ok(()) => self.layout(&mut batch).err(),
            Err(e) => Some(format!("intrinsic: {e:?}")),
        };
        self.finish(batch, error)
    }

    /// The viewport changed: lay out again; the batch carries the frames
    /// that moved.
    pub fn resize(&mut self, width: f32, height: f32) -> String {
        self.viewport = (width, height);
        let mut batch = Batch::new();
        let error = self.layout(&mut batch).err();
        self.finish(batch, error)
    }

    /// A motion frame: seek the engine to `now_ms` and report every
    /// presentation value that changed. Nothing else moves.
    pub fn tick(&mut self, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        let mut batch = Batch::new();
        let seek = self.engine.advance(self.now_ms / 1000.0);
        debug_assert!(seek.is_ok(), "the clock never runs backwards here");
        self.present(&mut batch, false);
        self.finish(batch, None)
    }

    fn finish(&self, batch: Batch, error: Option<String>) -> String {
        batch.finish(
            self.runner.has_timers(),
            !self.engine.quiescent(),
            self.runner.now_ms(),
            error.as_deref(),
        )
    }

    fn commit(&mut self, receipts: &[Timed], error: Option<String>) -> String {
        let mut batch = Batch::new();
        for t in receipts {
            let r = &t.receipt;
            for key in &r.destroyed {
                if let Some(id) = self.keys.remove(key) {
                    self.mirror.remove(&id);
                    batch.destroy(id);
                }
            }
            for key in &r.created {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    self.create(id, &mut batch);
                }
            }
            for key in r.created.iter().chain(r.touched.iter()) {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let id = node.id;
                    if r.touched.contains(key) {
                        self.update(id, &mut batch);
                    }
                    self.emit_children(id, &mut batch);
                }
            }
        }
        let roots = self.runner.roots();
        if roots != self.roots {
            self.roots = roots.clone();
            batch.roots(&roots);
        }
        let layout_error = if receipts.is_empty() {
            None
        } else {
            self.layout(&mut batch).err()
        };
        for s in self.runner.take_surface_updates() {
            batch.surface(s.view, &s.name, &s.values);
        }
        // Motion last, each commit at its own time: targets are in place
        // before the engine hears them, and a transition a timer started is
        // born at that timer's due time — so one seek and sixty give the same
        // bits (LLP 1002 D3; LLP 1012).
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
        self.present(&mut batch, false);
        self.finish(batch, error.or(layout_error))
    }

    /// Lay every root out under the viewport and emit the parent-relative
    /// frames and scroll content sizes that changed.
    fn layout(&mut self, batch: &mut Batch) -> Result<(), String> {
        let (w, h) = self.viewport;
        for root in self.runner.roots() {
            self.runner
                .kernel_mut()
                .compute_layout(root, Offer::definite(w, h))
                .map_err(|e| format!("layout: {e:?}"))?;
        }
        for id in self.preorder() {
            let kernel = self.runner.kernel();
            let Some(node) = kernel.node(id) else {
                continue;
            };
            let parent = node.parent.and_then(|p| kernel.node(p)).map(|p| p.frame);
            let rel = relative(node.frame, parent);
            let content = (style::effective_overflow(&node)
                != (Overflow::Visible, Overflow::Visible))
                .then(|| content_size(&node, kernel));
            let m = self.mirror.entry(id).or_default();
            if m.frame != Some(rel) {
                m.frame = Some(rel);
                batch.frame(id, rel.0, rel.1, rel.2, rel.3);
            }
            if let Some(c) = content {
                if m.content != Some(c) {
                    m.content = Some(c);
                    batch.content(id, c.0, c.1);
                }
            }
        }
        Ok(())
    }

    /// Every presentation value the engine changed, as `present` ops. At
    /// boot only values that are not the property's identity: the presenter
    /// starts every view at identity, and the four motion rows are never in
    /// the style dictionary.
    fn present(&mut self, batch: &mut Batch, boot: bool) {
        for p in self.engine.frame() {
            if boot && p.value == p.property.identity() {
                continue;
            }
            let key = NodeKey {
                index: p.node as u32,
                generation: (p.node >> 32) as u32,
            };
            let Some(view) = self.keys.get(&key).copied() else {
                continue;
            };
            let (x, y) = match p.property {
                Property::Translate => (p.value.x, p.value.y),
                _ => (p.value.x, 0.0),
            };
            batch.present(view, p.property.name(), x, y);
        }
    }

    fn preorder(&self) -> Vec<ViewId> {
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

    fn create(&mut self, id: ViewId, batch: &mut Batch) {
        let node = self.runner.kernel().node(id).expect("live");
        let key = node.key;
        let kind = kind_for(&node);
        let props = props_for(&node);
        let (style, _skipped) = style::style_json_for(&node);
        let handlers: Vec<&str> = self
            .runner
            .handlers_of(id)
            .into_iter()
            .map(|e| match e {
                EventKind::Press => "press",
                EventKind::Change => "change",
            })
            .collect();
        let pairs: Vec<(&str, String)> =
            props.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        batch.create(id, kind, &pairs, &style, &handlers);
        self.mirror.insert(
            id,
            Mirror {
                props,
                style,
                ..Mirror::default()
            },
        );
        self.keys.insert(key, id);
    }

    fn update(&mut self, id: ViewId, batch: &mut Batch) {
        let node = self.runner.kernel().node(id).expect("live");
        let props = props_for(&node);
        let (style, _skipped) = style::style_json_for(&node);
        let m = self.mirror.entry(id).or_default();
        if props != m.props {
            let set: Vec<(&str, String)> = props
                .iter()
                .filter(|(k, v)| m.props.get(*k) != Some(*v))
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            let clear: Vec<&str> = m
                .props
                .keys()
                .filter(|k| !props.contains_key(*k))
                .map(String::as_str)
                .collect();
            batch.props(id, &set, &clear);
            m.props = props;
        }
        if style != m.style {
            batch.style(id, &style);
            m.style = style;
        }
    }

    fn emit_children(&mut self, id: ViewId, batch: &mut Batch) {
        let children = self.runner.kernel().node(id).expect("live").children();
        let m = self.mirror.entry(id).or_default();
        if children != m.children {
            batch.children(id, &children);
            m.children = children;
        }
    }
}

/// A frame in its parent's space (a root's is absolute).
fn relative(frame: Frame, parent: Option<Frame>) -> (f32, f32, f32, f32) {
    match parent {
        Some(p) => (frame.x - p.x, frame.y - p.y, frame.width, frame.height),
        None => (frame.x, frame.y, frame.width, frame.height),
    }
}

/// A scroll container's content extent: the kernel's scrollable overflow
/// (Taffy's `content_size`, padding and every descendant included), never
/// less than the box itself.
fn content_size(node: &NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    // Taffy's block containers do not always count end-edge padding in
    // `content_size` (its flex containers do); CSS's `scrollHeight` does.
    // Floor with the direct children's extent plus the end padding.
    let pad = |d: exact_kernel::Dimension, against: f32| match d {
        exact_kernel::Dimension::Points(p) => p,
        exact_kernel::Dimension::Percent(p) => against * p / 100.0,
        exact_kernel::Dimension::Auto => 0.0,
    };
    let pad_right = pad(node.style.padding_right, node.frame.width);
    let pad_bottom = pad(node.style.padding_bottom, node.frame.width);
    let mut w = node.frame.width.max(node.content.0);
    let mut h = node.frame.height.max(node.content.1);
    for child in node.children() {
        if let Some(c) = kernel.node(child) {
            w = w.max(c.frame.x - node.frame.x + c.frame.width + pad_right);
            h = h.max(c.frame.y - node.frame.y + c.frame.height + pad_bottom);
        }
    }
    (w, h)
}

/// The presenter's kind for a node: its type, in the schema's names.
fn kind_for(node: &NodeRef<'_>) -> &'static str {
    match node.node_type {
        NodeType::View => "view",
        NodeType::List => "list",
        NodeType::NativeView => "native",
        NodeType::Svg => "svg",
        NodeType::ScrollView => "scroll",
        NodeType::Text => "text",
        NodeType::Image => "image",
        NodeType::TextInput => "input",
        NodeType::Pressable => "button",
        NodeType::Toggle => "toggle",
        NodeType::Canvas => "canvas",
    }
}

/// Props by their own names, as strings.
fn props_for(node: &NodeRef<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (id, value) in node.props.iter() {
        let text = match value {
            PropValue::Str(s) => s.clone(),
            PropValue::Bool(b) => b.to_string(),
            PropValue::Int(i) => i.to_string(),
            PropValue::Float(f) => style::num(*f as f32),
        };
        let _: PropId = id;
        out.insert(id.name().to_string(), text);
    }
    out
}
