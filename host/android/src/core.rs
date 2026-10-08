//! Receipt-driven Android core presentation, with the shared runner and kernel.
//!
//! Cold dictionaries use the existing native vocabulary. Paint, placement and
//! compositor deltas write EXA1 directly; no full native mirror walk runs for a
//! counter or a background change. A plan is selected before either owner boots.

#[path = "resolved_pool.rs"]
mod resolved_pool;
use resolved_pool::ResolvedPool;

use crate::wire::{Encoder, CONTENT, FRAME, HEADER_BYTES, PAINT, PRESENT};
use exact_apple::{batch::Batch, style};
use exact_kernel::id::IdMap;
use exact_kernel::{
    CommitReceipt, Dimension, Env, Frame, Kernel, NodeKey, NodeRef, NodeType, Offer, Overflow,
    PropId, PropList, PropValue, StyleId, StyleMask, StyleProps, TextMeasurer, ViewId,
};
use exact_plan::{BindingKind, Opcode, Plan};
use exact_runner::{DataSource, Event, Preferences, Runner, RunnerError, Viewport};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

const COLORS: [StyleId; 6] = [
    StyleId::BackgroundColor,
    StyleId::TextColor,
    StyleId::BorderColorTop,
    StyleId::BorderColorRight,
    StyleId::BorderColorBottom,
    StyleId::BorderColorLeft,
];
const COMPOSITOR: [StyleId; 4] = [
    StyleId::Translate,
    StyleId::Scale,
    StyleId::Rotate,
    StyleId::Opacity,
];

/// Only a complete, statically proven core plan chooses this adapter. Anything
/// outside it keeps the existing general native owner, before any tree exists.
pub fn eligible<D: DataSource>(plan: &Plan, data: &D) -> bool {
    if !data.grants().is_empty()
        || !data.ready()
        || data.revision().is_some()
        || data.native().is_some()
        || data.placement() != exact_runner::Placement::Main
        || !exact_runner::uses(plan).is_empty()
        || !plan.mutations.is_empty()
        || !plan.routes.is_empty()
    {
        return false;
    }
    if plan.nodes.iter().any(|n| {
        !matches!(
            NodeType::from_wire(n.node_type),
            Some(
                NodeType::View
                    | NodeType::Text
                    | NodeType::TextInput
                    | NodeType::Pressable
                    | NodeType::ScrollView
            )
        ) || n
            .parent
            .is_some_and(|p| NodeType::from_wire(plan.node(p).node_type) == Some(NodeType::Text))
            || n.surface.is_some()
    }) {
        return false;
    }
    // Runtime requests, commands and separately provided reserved facts require
    // the general executor. Even unreachable bytecode is checked here.
    if plan.resources.iter().any(|r| {
        let source = plan.str(r.source);
        (source.starts_with("exact") && source != "exactViewport")
            || (source != "exactViewport" && (r.reader || r.initial.len == 0 || !r.args.is_empty()))
    }) {
        return false;
    }
    if plan.bindings.iter().any(|b| {
        (b.kind == BindingKind::Prop && matches!(PropId::from_wire(b.id), Some(PropId::Markup)))
            || (b.kind == BindingKind::Style
                && matches!(
                    StyleId::from_bit(u32::from(b.id)),
                    Some(
                        StyleId::Transition
                            | StyleId::LayoutTransition
                            | StyleId::ShapeOutside
                            | StyleId::Animation
                            | StyleId::ExitAnimation
                            | StyleId::DragTimeline
                            | StyleId::AnimationTimeline
                            | StyleId::AnimationRange
                            | StyleId::TimelineScope
                    )
                ))
    }) {
        return false;
    }
    let mut effect = false;
    plan.each_code(&mut |code| {
        effect |= exact_runner::vm::instructions(plan.code(code)).any(|i| {
            i.map_or(true, |i| {
                matches!(
                    i.op,
                    Opcode::Send | Opcode::Command | Opcode::Refresh | Opcode::NativeProps
                )
            })
        });
    });
    !effect
}

#[derive(Clone)]
struct Resolved {
    // Keep the immutable authored allocation alive while its address is a key.
    _authored: Rc<StyleProps>,
    other: StyleProps,
    colors: [[u32; 2]; 6],
    transform: [[f64; 2]; 4],
}

fn resolved(node: NodeRef<'_>) -> Resolved {
    let mut other = node.computed_style(StyleMask::INHERITED);
    let text = node.text_color();
    let borders = other.border_colors(text);
    let colors = [
        other.background_color.unwrap_or(text),
        text,
        borders[0],
        borders[1],
        borders[2],
        borders[3],
    ]
    .map(|c| [false, true].map(|dark| c.resolve(dark).0.rotate_right(8)));
    let transform = [
        [f64::from(other.translate.x), f64::from(other.translate.y)],
        [f64::from(other.scale), 0.],
        [f64::from(other.rotate), 0.],
        [f64::from(other.opacity), 0.],
    ];
    for row in COLORS.into_iter().chain(COMPOSITOR) {
        other.clear(StyleMask::of(row));
    }
    Resolved {
        _authored: node.shared_style(),
        other,
        colors,
        transform,
    }
}

// Resolved snapshots retain only typed values needed by later comparisons.
// A cold dictionary is generated from the current live node only when it crosses.
// This memo lives for one publication: a hot-touched node cannot lend stale JSON
// to a newly-created sibling that shares its authored style and logical parent.
struct StyleMemo {
    // A canonical snapshot may keep another node's authored allocation.
    // Keep this key's allocation independently alive throughout publication.
    _authored: Rc<StyleProps>,
    value: Rc<Resolved>,
    json: Option<String>,
}

impl StyleMemo {
    fn new(node: NodeRef<'_>, canonical: &mut ResolvedPool) -> Self {
        let current = resolved(node);
        let authored = current._authored.clone();
        Self {
            _authored: authored,
            value: canonical.intern(current),
            json: None,
        }
    }

    fn json(&mut self, node: NodeRef<'_>, env: &Env) -> &str {
        self.json
            .get_or_insert_with(|| style::style_json_for(&node, env).0)
            .as_str()
    }
}

#[derive(Default)]
struct Mirror {
    id: ViewId,
    props: PropList,
    spellcheck: Option<bool>,
    style: Option<Rc<Resolved>>,
    children: Vec<ViewId>,
    frame: Option<Frame>,
    content: Option<(f32, f32)>,
}

#[derive(Default)]
struct Publication {
    cold: Batch,
    hot: Vec<u8>,
    records: u32,
}

impl Publication {
    fn record(&mut self, opcode: u8, payload: &[u8]) {
        self.hot.push(opcode);
        self.hot
            .extend_from_slice(&(payload.len() as u32).to_le_bytes());
        self.hot.extend_from_slice(payload);
        self.records += 1;
    }
    fn floats(&mut self, opcode: u8, id: u32, values: &[f32]) {
        let mut bytes = [0u8; 20];
        bytes[..4].copy_from_slice(&id.to_le_bytes());
        for (slot, value) in bytes[4..].chunks_exact_mut(4).zip(values) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        self.record(opcode, &bytes[..4 + values.len() * 4]);
    }
    fn compositor(&mut self, id: u32, before: Option<&Resolved>, after: &Resolved) {
        for (index, values) in after.transform.iter().enumerate() {
            if before.is_some_and(|old| old.transform[index] == *values) {
                continue;
            }
            let identity = [[0., 0.], [1., 0.], [0., 0.], [1., 0.]];
            if before.is_none() && *values == identity[index] {
                continue;
            }
            let arity = 2;
            let mut bytes = [0u8; 22];
            bytes[..4].copy_from_slice(&id.to_le_bytes());
            bytes[4] = index as u8 + 1;
            bytes[5] = arity;
            for (slot, value) in bytes[6..].chunks_exact_mut(8).zip(values) {
                slot.copy_from_slice(&value.to_le_bytes());
            }
            self.record(PRESENT, &bytes[..6 + usize::from(arity) * 8]);
        }
    }
    fn paint(&mut self, id: u32, before: &Resolved, after: &Resolved) {
        let mut bytes = [0u8; 56];
        bytes[..4].copy_from_slice(&id.to_le_bytes());
        let mut length = 8;
        let mut mask = 0u32;
        for (index, colors) in after.colors.iter().enumerate() {
            if before.colors[index] != *colors {
                mask |= 1 << index;
                for color in colors {
                    bytes[length..length + 4].copy_from_slice(&color.to_le_bytes());
                    length += 4;
                }
            }
        }
        if mask != 0 {
            bytes[4..8].copy_from_slice(&mask.to_le_bytes());
            self.record(PAINT, &bytes[..length]);
        }
    }
    fn finish(
        mut self,
        runner: &impl Facts,
        error: Option<&str>,
        encoder: &mut Encoder,
    ) -> Vec<u8> {
        self.cold.frames = runner.frames();
        let json = self.cold.finish(runner.due(), false, runner.clock(), error);
        let mut out = Vec::with_capacity(json.len() + self.hot.len());
        encoder
            .encode(json.as_bytes(), &mut out)
            .expect("native Batch is valid");
        if self.records != 0 {
            let old_count = u32::from_le_bytes(out[8..12].try_into().unwrap());
            let metadata = u32::from_le_bytes(out[28..32].try_into().unwrap()) as usize;
            let at = out.len() - metadata;
            out.splice(at..at, self.hot);
            out[8..12].copy_from_slice(&(old_count + self.records).to_le_bytes());
        }
        debug_assert!(out.len() >= HEADER_BYTES);
        out
    }
}

trait Facts {
    fn frames(&self) -> bool;
    fn due(&self) -> Option<f64>;
    fn clock(&self) -> f64;
}
impl<D: DataSource> Facts for Runner<D> {
    fn frames(&self) -> bool {
        self.wants_frames()
    }
    fn due(&self) -> Option<f64> {
        self.timer_due_ms()
    }
    fn clock(&self) -> f64 {
        self.now_ms()
    }
}

/// Exactly one runner/kernel owner and the memo of published Android values.
pub(super) struct Core<D: DataSource> {
    runner: Runner<D>,
    mirror: IdMap<NodeKey, Mirror>,
    roots: Vec<ViewId>,
    encoder: Encoder,
    env: Env,
    language: Option<(String, &'static str)>,
}

impl<D: DataSource> Core<D> {
    pub(super) fn boot(
        plan: Plan,
        data: D,
        measurer: Box<dyn TextMeasurer>,
        w: f32,
        h: f32,
        initial_press: Option<&str>,
    ) -> Result<(Self, Vec<u8>), String> {
        let mut runner = Runner::boot(
            plan,
            data,
            Kernel::new(measurer),
            Viewport::sized(f64::from(w), f64::from(h)),
            "https://exact.invalid/",
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut receipts = vec![runner
            .kernel()
            .receipts()
            .last()
            .cloned()
            .unwrap_or_default()];
        if let Some(target) = initial_press {
            let keys = runner.kernel().find_by_test_id(target);
            let node = keys
                .first()
                .filter(|_| keys.len() == 1)
                .and_then(|key| runner.kernel().node_by_key(*key))
                .ok_or_else(|| {
                    "initial press target must identify exactly one live node".to_owned()
                })?;
            let id = node.id;
            if !runner
                .handlers_of(id)
                .contains(&exact_plan::EventKind::Press)
            {
                return Err("initial press target has no press handler".into());
            }
            receipts.push(
                runner
                    .dispatch(id, Event::Press)
                    .map_err(|e| format!("initial press: {e:?}"))?,
            );
        }
        let mut core = Self {
            runner,
            mirror: IdMap::default(),
            roots: Vec::new(),
            encoder: Encoder::default(),
            env: Env::default(),
            language: None,
        };
        let out = core.publish(&receipts, true, None);
        let metadata = u32::from_le_bytes(out[28..32].try_into().unwrap()) as usize;
        if metadata != 0 {
            return Err(format!(
                "initial layout refused: {}",
                String::from_utf8_lossy(&out[out.len() - metadata..])
            ));
        }
        Ok((core, out))
    }

    pub(crate) fn scrolled(&mut self, view: ViewId, left: f64, top: f64) {
        self.runner.scrolled(Some(view), left, top);
    }

    fn publish(
        &mut self,
        receipts: &[CommitReceipt],
        layout: bool,
        error: Option<String>,
    ) -> Vec<u8> {
        let mut p = Publication::default();
        let roots = self.runner.roots();
        let mut error = error;
        let mut placed = Vec::new();
        if layout || receipts.iter().any(|r| r.layout_invalidated) {
            let viewport = self.runner.viewport();
            for root in &roots {
                match self.runner.kernel_mut().compute_layout(
                    *root,
                    Offer::definite(viewport.width as f32, viewport.height as f32),
                ) {
                    Ok(receipt) => placed.extend(receipt.updated),
                    Err(e) => {
                        error.get_or_insert_with(|| format!("layout: {e:?}"));
                    }
                }
            }
        }
        // Layout updates the kernel's viewport environment. Resolve the first
        // native dictionary against that final environment, then remember it.
        // A resize can change env() or vw/vh without a Contract commit; refresh
        // kept dictionaries on that rare turn, never on ordinary paint updates.
        let env = self.runner.kernel().env();
        let environment_changed = env != self.env;
        let inherited = self.spellcheck_changes(receipts);
        let environment = CommitReceipt {
            touched: if environment_changed {
                let touched: BTreeSet<_> = receipts
                    .iter()
                    .chain(std::iter::once(&inherited))
                    .flat_map(|r| r.created.iter().chain(&r.touched))
                    .copied()
                    .collect();
                self.mirror
                    .keys()
                    .filter(|key| !touched.contains(*key))
                    .copied()
                    .collect()
            } else {
                Vec::new()
            },
            ..CommitReceipt::default()
        };
        let receipts = receipts.iter().chain([&inherited, &environment]);
        let language = self.runner.resolved_locale();
        let direction = self.runner.direction();
        if self
            .language
            .as_ref()
            .is_none_or(|(previous, dir)| previous != language || *dir != direction)
        {
            p.cold.language(language, direction);
            self.language = Some((language.to_owned(), direction));
        }
        let mut styles: BTreeMap<(usize, Option<u32>, u8), StyleMemo> = BTreeMap::new();
        let mut canonical = ResolvedPool::default();
        for receipt in receipts.clone() {
            for key in &receipt.destroyed {
                if let Some(m) = self.mirror.remove(key) {
                    p.cold.destroy(m.id);
                }
            }
        }
        // All final surviving nodes exist before a parent's final child list.
        for receipt in receipts.clone() {
            for key in receipt.created.iter().chain(&receipt.touched) {
                let Some(node) = self.runner.kernel().node_by_key(*key) else {
                    continue;
                };
                let id = node.id;
                let spellcheck = (node.node_type == NodeType::TextInput)
                    .then(|| node.spellcheck())
                    .flatten();
                let memo = (
                    node.style as *const StyleProps as usize,
                    node.parent,
                    node.node_type as u8,
                );
                let prepared = styles
                    .entry(memo)
                    .or_insert_with(|| StyleMemo::new(node, &mut canonical));
                let next = prepared.value.clone();
                let m = self.mirror.entry(*key).or_default();
                if let Some(old) = &m.style {
                    if m.props != *node.props || m.spellcheck != spellcheck {
                        let (set, clear) = changed_props(node, &m.props, m.spellcheck, spellcheck);
                        if !set.is_empty() || !clear.is_empty() {
                            p.cold.props(id, &set, &clear);
                        }
                    }
                    if environment_changed || old.other != next.other {
                        p.cold.style(id, prepared.json(node, &env));
                    }
                    // The cold encoder has not seen preceding direct paint
                    // deltas. Keep the authoritative color delta even when a
                    // layout-only change serializes to a stale cold dictionary.
                    p.paint(id, old, &next);
                    p.compositor(id, Some(old), &next);
                } else {
                    let props = props_for(node);
                    let pairs: Vec<_> =
                        props.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                    let handlers = self.runner.handlers_of(id);
                    let names: Vec<_> = handlers.iter().map(|e| e.name()).collect();
                    p.cold
                        .create(id, kind(node), &pairs, prepared.json(node, &env), &names);
                    if node.node_type == NodeType::Text {
                        p.cold.paragraph(id, "[]");
                    }
                    p.compositor(id, None, &next);
                    m.id = id;
                }
                // Compare the typed snapshot before serializing. A style-only
                // turn does not clone each row's text and test id or rebuild a
                // property dictionary. Inherited spellcheck remains separate.
                if m.style.is_none() || m.props != *node.props {
                    m.props = node.props.clone();
                }
                m.spellcheck = spellcheck;
                m.style = Some(next);
            }
        }
        for receipt in receipts.clone() {
            for key in receipt.created.iter().chain(&receipt.touched) {
                if let Some(node) = self.runner.kernel().node_by_key(*key) {
                    let children = node.children();
                    if let Some(m) = self.mirror.get_mut(key) {
                        // A newly created native node already has no authored
                        // children. Publish only a changed list, including an
                        // existing parent's transition back to an empty list.
                        if m.children != children {
                            p.cold.children(node.id, &children);
                            m.children = children;
                        }
                    }
                }
            }
        }
        if roots != self.roots {
            p.cold.roots(&roots);
            self.roots = roots;
        }
        // CREATE and child operations precede the retained placement records.
        // Computing layout early does not expose a FRAME for a missing view.
        for key in placed {
            let Some(node) = self.runner.kernel().node_by_key(key) else {
                continue;
            };
            let parent = node.parent.and_then(|id| self.runner.kernel().node(id));
            let frame = Frame {
                x: node.frame.x - parent.map_or(0., |n| n.frame.x),
                y: node.frame.y - parent.map_or(0., |n| n.frame.y),
                ..node.frame
            };
            let Some(m) = self.mirror.get_mut(&key) else {
                continue;
            };
            if m.frame.is_none_or(|old| !old.bits_eq(frame)) {
                p.floats(
                    FRAME,
                    node.id,
                    &[frame.x, frame.y, frame.width, frame.height],
                );
                m.frame = Some(frame);
            }
            if style::effective_overflow(&node) != (Overflow::Visible, Overflow::Visible) {
                let size = content_size(node, self.runner.kernel());
                if m.content != Some(size) {
                    p.floats(CONTENT, node.id, &[size.0, size.1]);
                    m.content = Some(size);
                }
            }
        }
        self.env = env;
        p.finish(&self.runner, error.as_deref(), &mut self.encoder)
    }

    fn spellcheck_changes(&self, receipts: &[CommitReceipt]) -> CommitReceipt {
        let mut inherited = CommitReceipt::default();
        for key in receipts.iter().flat_map(|r| &r.touched) {
            let Some(node) = self.runner.kernel().node_by_key(*key) else {
                continue;
            };
            if self.mirror.get(key).is_none_or(|old| {
                old.props.get(PropId::Spellcheck) == node.props.get(PropId::Spellcheck)
            }) {
                continue;
            }
            // This hint inherits through logical ancestors but is a prop,
            // so the kernel receipt does not carry descendant style touches.
            let mut children = node.children();
            while let Some(id) = children.pop() {
                let Some(child) = self.runner.kernel().node(id) else {
                    continue;
                };
                if child.node_type == NodeType::TextInput
                    && self
                        .mirror
                        .get(&child.key)
                        .is_some_and(|old| old.spellcheck != child.spellcheck())
                {
                    inherited.touched.push(child.key);
                }
                children.extend(child.children());
            }
        }
        inherited.touched.sort_unstable();
        inherited.touched.dedup();
        inherited
    }

    pub(super) fn dispatch(&mut self, view: u32, event: Event, now: f64) -> Vec<u8> {
        if !now.is_finite() || now < 0. {
            return self.publish(&[], false, Some("invalid event clock".into()));
        }
        let result = self.runner.dispatch(view, event);
        self.result(result.map(Some), false)
    }
    fn result(
        &mut self,
        result: Result<Option<CommitReceipt>, RunnerError>,
        layout: bool,
    ) -> Vec<u8> {
        match result {
            Ok(receipt) => self.publish(&receipt.into_iter().collect::<Vec<_>>(), layout, None),
            Err(e) => self.publish(&[], false, Some(format!("{e:?}"))),
        }
    }
    pub(super) fn resize(&mut self, w: f32, h: f32) -> Vec<u8> {
        let result = self.runner.set_viewport(f64::from(w), f64::from(h));
        self.result(result, true)
    }
    pub(super) fn preferences(&mut self, bits: u32) -> Vec<u8> {
        let result = self.runner.set_preferences(Preferences::from_bits(bits));
        self.result(result, false)
    }
    pub(super) fn insets(&mut self, env: Env) -> Vec<u8> {
        match self.runner.kernel_mut().set_env(env) {
            Ok(true) => {
                let receipt = CommitReceipt {
                    touched: self.mirror.keys().copied().collect(),
                    layout_invalidated: true,
                    ..CommitReceipt::default()
                };
                self.publish(&[receipt], true, None)
            }
            Ok(false) => self.publish(&[], false, None),
            Err(e) => self.publish(&[], false, Some(format!("insets: {e:?}"))),
        }
    }
    pub(super) fn intrinsic(&mut self, values: &[(u32, Option<(f32, f32)>)]) -> Vec<u8> {
        // Validate every id/size before mutating the derived natural sizes.
        if values.iter().any(|(id, size)| {
            self.runner.kernel().node(*id).is_none()
                || size.is_some_and(|(w, h)| !w.is_finite() || !h.is_finite() || w < 0. || h < 0.)
        }) {
            return self.publish(&[], false, Some("invalid intrinsic size".into()));
        }
        for (id, size) in values {
            if let Err(e) = self.runner.kernel_mut().set_intrinsic_size(*id, *size) {
                return self.publish(&[], false, Some(format!("intrinsic: {e:?}")));
            }
        }
        self.publish(&[], true, None)
    }
    pub(super) fn advance(&mut self, now: f64, frame: bool) -> Vec<u8> {
        let result = if frame {
            self.runner.present_frames(true);
            self.runner.frame(now)
        } else {
            self.runner.advance_timed(now)
        };
        let error = result.error.map(|e| format!("{e:?}"));
        let receipts: Vec<_> = result.receipts.into_iter().map(|t| t.receipt).collect();
        self.publish(&receipts, false, error)
    }
    pub(super) fn painted(&mut self) -> Vec<u8> {
        let result = self.runner.data_ready();
        self.result(result, false)
    }
    pub(super) fn quiet(&mut self) -> Vec<u8> {
        self.publish(&[], false, None)
    }
    pub(super) fn agent(&self, request: &str) -> String {
        exact_runner::agent::handle(&self.runner, request)
    }
}

fn props_for(node: NodeRef<'_>) -> BTreeMap<String, String> {
    let mut out: BTreeMap<_, _> = node
        .props
        .iter()
        .map(|(id, value)| (id.name().to_owned(), prop_string(value)))
        .collect();
    if node.node_type == NodeType::TextInput {
        out.remove("spellcheck");
        if let Some(value) = node.spellcheck() {
            out.insert("spellcheck".into(), value.to_string());
        }
    }
    out
}

fn prop_string(value: &PropValue) -> String {
    match value {
        PropValue::Str(s) => s.clone(),
        PropValue::Bool(b) => b.to_string(),
        PropValue::Int(i) => i.to_string(),
        PropValue::Float(f) => style::num(*f as f32),
    }
}

type PropChanges = (Vec<(&'static str, String)>, Vec<&'static str>);

fn changed_props(
    node: NodeRef<'_>,
    before: &PropList,
    old_spellcheck: Option<bool>,
    spellcheck: Option<bool>,
) -> PropChanges {
    let input = node.node_type == NodeType::TextInput;
    let mut set: Vec<_> = node
        .props
        .iter()
        .filter(|(id, value)| {
            !(input && *id == PropId::Spellcheck) && before.get(*id) != Some(*value)
        })
        .map(|(id, value)| (id.name(), prop_string(value)))
        .collect();
    let mut clear: Vec<_> = before
        .iter()
        .filter(|(id, _)| !(input && *id == PropId::Spellcheck) && !node.props.contains(*id))
        .map(|(id, _)| id.name())
        .collect();
    if input && old_spellcheck != spellcheck {
        if let Some(value) = spellcheck {
            set.push(("spellcheck", value.to_string()));
        } else {
            clear.push("spellcheck");
        }
    }
    set.sort_unstable_by_key(|(name, _)| *name);
    clear.sort_unstable();
    (set, clear)
}
fn kind(node: NodeRef<'_>) -> &'static str {
    match node.node_type {
        NodeType::Text => "text",
        NodeType::Pressable => "button",
        NodeType::ScrollView => "scroll",
        NodeType::TextInput if node.props.str(PropId::SemanticTag) == Some("textarea") => {
            "textarea"
        }
        NodeType::TextInput => "input",
        _ => "view",
    }
}

// The shared native host's CSS end-padding correction: Taffy's block
// content_size may omit the trailing padding while its flex result includes it.
fn content_size(node: NodeRef<'_>, kernel: &Kernel) -> (f32, f32) {
    let pad = |d: Dimension| match d.resolve(&kernel.env()) {
        Dimension::Points(v) => v,
        Dimension::Percent(v) => node.frame.width * v / 100.,
        Dimension::Calc(p, v) => node.frame.width * p / 100. + v,
        Dimension::Auto => 0.,
        Dimension::Env(..)
        | Dimension::Segment(..)
        | Dimension::Viewport(..)
        | Dimension::Compare(..) => unreachable!("resolved above"),
    };
    let (right, bottom) = (
        pad(node.style.padding_right),
        pad(node.style.padding_bottom),
    );
    let (mut w, mut h) = node.content;
    for id in node.children() {
        if let Some(child) = kernel.node(id) {
            w = w.max(child.frame.x - node.frame.x + child.frame.width + right);
            h = h.max(child.frame.y - node.frame.y + child.frame.height + bottom);
        }
    }
    (w, h)
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
