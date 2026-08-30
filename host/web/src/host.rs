//! The runner wrapped for a DOM: receipts → batches.
//!
//! @ref LLP 1007 §2 (the DOM mirrors the kernel tree)
//!
//! After every commit the host walks the receipt: destroyed keys become
//! `destroy`, created keys become `create` (with the node's tag, props, CSS,
//! and handler kinds), touched keys become `props`/`style`/`children` ops
//! only where the host's per-view cache says something changed. The kernel
//! is the single source of truth; the cache is a memo of what the page has
//! already been told.

use crate::batch::Batch;
use crate::css;
use crate::motion::{Lowered, Springs};
use exact_kernel::{CommitReceipt, Kernel, NodeKey, NodeRef, NodeType, PropId, PropValue, ViewId};
use exact_motion::Property;
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
    css: String,
    children: Vec<ViewId>,
}

/// One runner, one page.
pub struct Host<D: DataSource> {
    runner: Runner<D>,
    mirror: BTreeMap<ViewId, Mirror>,
    keys: BTreeMap<NodeKey, ViewId>,
    roots: Vec<ViewId>,
    springs: Springs,
    /// The page's clock at the last call, milliseconds from script start.
    now_ms: f64,
}

impl<D: DataSource> Host<D> {
    /// Boot from plan bytes: decode (a validation pass), boot the runner, and
    /// produce the first batch, which creates the whole tree.
    pub fn boot(plan_bytes: &[u8], data: D) -> Result<(Host<D>, String), HostError> {
        Host::boot_with(plan_bytes, data, None)
    }

    /// Boot carrying an earlier host's state (the dev loop's reload, LLP
    /// 1007 §6): slots by name where their types still fit, settled
    /// resources where their arguments still match, the clock. Carried state
    /// is never why a boot fails — what no longer fits starts fresh.
    pub fn boot_with(
        plan_bytes: &[u8],
        data: D,
        carried: Option<&Carried>,
    ) -> Result<(Host<D>, String), HostError> {
        let plan = Plan::decode(plan_bytes).map_err(HostError::Plan)?;
        let kernel = Kernel::with_monospace();
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
            springs: Springs::new(),
            now_ms: 0.0,
        };
        let mut batch = Batch::new();
        // Everything live is new to the page.
        let roots = host.runner.roots();
        let mut stack: Vec<ViewId> = roots.iter().rev().copied().collect();
        let mut order = Vec::new();
        while let Some(id) = stack.pop() {
            order.push(id);
            let node = host.runner.kernel().node(id).expect("live");
            let mut children = node.children();
            children.reverse();
            stack.extend(children);
        }
        for id in &order {
            host.create(*id, &mut batch);
        }
        for id in &order {
            host.emit_children(*id, &mut batch);
        }
        host.springs.adopt(host.runner.kernel(), &order);
        host.roots = roots.clone();
        batch.roots(&roots);
        // Surfaces after roots: the canvas is in the page when its surface is made.
        for s in host.runner.take_surface_updates() {
            batch.surface(s.view, &s.name, &s.values);
        }
        for c in host.runner.take_commands() {
            batch.command(&c.name, &c.args);
        }
        let timers = host.runner.has_timers();
        let clock = host.runner.now_ms();
        Ok((host, batch.finish(timers, clock, None)))
    }

    /// The runner.
    pub fn runner(&self) -> &Runner<D> {
        &self.runner
    }

    /// The runner, mutably — for tests that drive it past the host.
    pub fn runner_mut(&mut self) -> &mut Runner<D> {
        &mut self.runner
    }

    /// Deliver an event at the page's clock (milliseconds from script
    /// start); the batch makes the page equal to the tree after the commit,
    /// and any spring the change releases is in it as frames. A refusal is
    /// reported in the batch's `error`, and the page is untouched (as the
    /// kernel was).
    pub fn dispatch_at(&mut self, view: ViewId, event: Event, now_ms: f64) -> String {
        self.now_ms = now_ms.max(self.now_ms);
        match self.runner.dispatch(view, event) {
            Ok(receipt) => {
                let at_ms = self.now_ms;
                self.batch_for(&[Timed { at_ms, receipt }], None)
            }
            Err(e) => self.batch_for(&[], Some(&format!("{e:?}"))),
        }
    }

    /// [`Host::dispatch_at`] at the clock's last value.
    pub fn dispatch(&mut self, view: ViewId, event: Event) -> String {
        self.dispatch_at(view, event, self.now_ms)
    }

    /// What a reload keeps (`Runner::carry`).
    pub fn carry(&self) -> Carried {
        self.runner.carry()
    }

    /// The springs' engine: presentation values as the page shows them.
    pub fn springs(&self) -> &Springs {
        &self.springs
    }

    /// The agent API's read operations (LLP 1012): `tree`, `state`, and
    /// `logs` from the runner; `settle` — the clock at which the last spring
    /// in flight ends, milliseconds, `null` when none — from the engine here.
    /// CSS transitions are the browser's; the glue folds their end times in.
    pub fn agent(&self, request: &str) -> String {
        if exact_runner::agent::field_str(request, "op").as_deref() == Some("settle") {
            return match self.springs.engine().settle_time() {
                Some(t) => format!("{{\"settle\":{}}}", exact_runner::agent::num(t * 1000.0)),
                None => "{\"settle\":null}".to_string(),
            };
        }
        exact_runner::agent::handle(&self.runner, request)
    }

    /// Move the clock; every timer due fires at its own time; one batch for
    /// all of them, each commit's ops behind an `at` marker carrying the
    /// time it was made, so a page that owns time attributes the transitions
    /// they start to that instant (LLP 1012; LLP 1002 D3). A timer's refusal
    /// stops the clock there: the commits before it are in the batch, the
    /// refusal in `error`, and `clock` says where the runner stands.
    pub fn advance(&mut self, now_ms: f64) -> String {
        let a = self.runner.advance_timed(now_ms);
        self.now_ms = a.now_ms.max(self.now_ms);
        let error = a.error.map(|e| format!("{e:?}"));
        self.batch_for(&a.receipts, error.as_deref())
    }

    fn batch_for(&mut self, receipts: &[Timed], error: Option<&str>) -> String {
        let mut batch = Batch::new();
        for t in receipts {
            let r = &t.receipt;
            batch.at(t.at_ms);
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
            // This commit's springs, at its own time: the style (the target)
            // is in the page before the frames that approach it start playing.
            self.emit_springs(&mut batch, std::slice::from_ref(r), t.at_ms / 1000.0);
        }
        let roots = self.runner.roots();
        if roots != self.roots {
            self.roots = roots.clone();
            batch.roots(&roots);
        }
        // A canvas's inputs (LLP 1009 D2): the runner's side-output, only
        // from commits that applied.
        for s in self.runner.take_surface_updates() {
            batch.surface(s.view, &s.name, &s.values);
        }
        for c in self.runner.take_commands() {
            batch.command(&c.name, &c.args);
        }
        let timers = self.runner.has_timers();
        batch.finish(timers, self.runner.now_ms(), error)
    }

    fn emit_springs(&mut self, batch: &mut Batch, receipts: &[CommitReceipt], now_s: f64) {
        for lowered in self.springs.commit(self.runner.kernel(), receipts, now_s) {
            match lowered {
                Lowered::Start {
                    view,
                    property,
                    delay,
                    duration,
                    values,
                } => {
                    let pairs: Vec<(f64, f64)> = values.iter().map(|v| (v.x, v.y)).collect();
                    batch.animate(
                        view,
                        property.name(),
                        delay * 1000.0,
                        duration * 1000.0,
                        &pairs,
                        property == Property::Translate,
                    );
                }
                Lowered::Cancel { view, property } => {
                    batch.animate(view, property.name(), 0.0, 0.0, &[], false);
                }
            }
        }
    }

    fn create(&mut self, id: ViewId, batch: &mut Batch) {
        let node = self.runner.kernel().node(id).expect("live");
        let key = node.key;
        let tag = tag_for(&node);
        let props = props_for(&node);
        let (css, _skipped) = css::css_text(node.style);
        let css = host_css(&node, css);
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
        batch.create(id, tag, &pairs, &css, &handlers);
        self.mirror.insert(
            id,
            Mirror {
                props,
                css,
                children: Vec::new(),
            },
        );
        self.keys.insert(key, id);
    }

    fn update(&mut self, id: ViewId, batch: &mut Batch) {
        let node = self.runner.kernel().node(id).expect("live");
        let props = props_for(&node);
        let (css, _skipped) = css::css_text(node.style);
        let css = host_css(&node, css);
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
        if css != m.css {
            batch.style(id, &css);
            m.css = css;
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

/// A canvas's element hosts its surface element under its children
/// (`glue.js`, LLP 1014 D2): a containing block for it, unless the author
/// positioned the canvas, and a stacking context of its own — the
/// `isolation: isolate` the web's `drawable` implies — so the surface paints
/// above the canvas's background and below its children.
fn host_css(node: &NodeRef<'_>, mut css: String) -> String {
    if node.node_type == NodeType::Canvas {
        if !(css.starts_with("position:") || css.contains(";position:")) {
            css.push_str("position:relative;");
        }
        css.push_str("isolation:isolate;");
    }
    css
}

/// The element for a node: its type, refined by `semanticTag`.
fn tag_for(node: &NodeRef<'_>) -> &'static str {
    if let Some(t) = node.props.str(PropId::SemanticTag) {
        match t {
            "main" => return "main",
            "header" => return "header",
            "nav" => return "nav",
            "section" => return "section",
            "footer" => return "footer",
            "article" => return "article",
            "aside" => return "aside",
            _ => {}
        }
    }
    match node.node_type {
        NodeType::View | NodeType::List | NodeType::NativeView | NodeType::Svg => "div",
        NodeType::ScrollView => "div",
        NodeType::Text => {
            if node.parent.is_some() && node.props.str(PropId::Text).is_some() {
                "span"
            } else {
                "div"
            }
        }
        NodeType::Image => "img",
        NodeType::TextInput => "input",
        NodeType::Pressable => "button",
        NodeType::Toggle => "input",
        NodeType::Canvas => "canvas",
    }
}

/// Props as DOM attributes/properties. Names are the DOM's.
fn props_for(node: &NodeRef<'_>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (id, value) in node.props.iter() {
        let text = match value {
            PropValue::Str(s) => s.clone(),
            PropValue::Bool(b) => b.to_string(),
            PropValue::Int(i) => i.to_string(),
            PropValue::Float(f) => crate::css::num(*f as f32),
        };
        let name = match id {
            PropId::Text => "text",
            PropId::TestId => "data-testid",
            // An image's label is its `alt`: the replaced element's text
            // alternative, shown when it does not load.
            PropId::AccessibilityLabel if node.node_type == NodeType::Image => "alt",
            PropId::AccessibilityLabel => "aria-label",
            PropId::AccessibilityRole => "role",
            PropId::AccessibilityHint => "aria-description",
            PropId::AccessibilityHeadingLevel => "aria-level",
            PropId::Placeholder => "placeholder",
            PropId::Value => "value",
            PropId::Href => "data-href",
            PropId::Disabled => "disabled",
            PropId::Lang => "lang",
            PropId::ImageSource => "src",
            PropId::SemanticTag => continue,
            PropId::ToggleValue => "checked",
            other => {
                // Every other prop rides as `data-<name>` so nothing is lost.
                out.insert(format!("data-{}", other.name().to_lowercase()), text);
                continue;
            }
        };
        out.insert(name.to_string(), text);
    }
    if node.node_type.scrolls_by_default() {
        out.insert("data-scroll".into(), "true".into());
    }
    if node.node_type == NodeType::Toggle {
        out.insert("type".into(), "checkbox".into());
    }
    out
}
