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
use std::{collections::BTreeMap, rc::Rc};

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

    fn publish(
        &mut self,
        receipts: &[CommitReceipt],
        layout: bool,
        error: Option<String>,
    ) -> Vec<u8> {
        let mut p = Publication::default();
        let inherited = self.spellcheck_changes(receipts);
        let receipts = receipts.iter().chain(std::iter::once(&inherited));
        let env = self.runner.kernel().env();
        let environment_changed = env != self.env;
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
        let roots = self.runner.roots();
        if roots != self.roots {
            p.cold.roots(&roots);
            self.roots = roots;
        }
        let mut error = error;
        if layout || receipts.clone().any(|r| r.layout_invalidated) {
            for root in self.roots.clone() {
                let viewport = self.runner.viewport();
                match self.runner.kernel_mut().compute_layout(
                    root,
                    Offer::definite(viewport.width as f32, viewport.height as f32),
                ) {
                    Ok(r) => {
                        for key in r.updated {
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
                            if style::effective_overflow(&node)
                                != (Overflow::Visible, Overflow::Visible)
                            {
                                let size = content_size(node, self.runner.kernel());
                                if m.content != Some(size) {
                                    p.floats(CONTENT, node.id, &[size.0, size.1]);
                                    m.content = Some(size);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error.get_or_insert_with(|| format!("layout: {e:?}"));
                    }
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
        Dimension::Auto | Dimension::Env(..) | Dimension::Segment(..) | Dimension::Viewport(..) => {
            0.
        }
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
mod tests {
    use super::*;
    use android_core_data::Core as Data;
    use exact_kernel::MonospaceMeasurer;

    fn baked() -> Vec<u8> {
        let plan =
            contract::compile_path(std::path::Path::new("../../apps/android-core/app.contract"))
                .unwrap();
        contract::bake(plan, Data).unwrap().encode()
    }

    fn assert_kernel_parity(core: &Core<Data>, reference: &exact_apple::Host<Data>) {
        let actual = core.runner.kernel();
        let expected = reference.runner().kernel();
        let rows = actual.rows(None).unwrap();
        let baseline = expected.rows(None).unwrap();
        assert_eq!(rows.len(), baseline.len());
        for (a, b) in rows.iter().zip(&baseline) {
            assert_eq!((a.id, a.parent, a.node_type), (b.id, b.parent, b.node_type));
            assert!(
                a.frame.bits_eq(b.frame),
                "{}: {:?} != {:?}",
                a.id,
                a.frame,
                b.frame
            );
            let a = actual.node(a.id).unwrap();
            let b = expected.node(b.id).unwrap();
            assert_eq!(a.props, b.props);
            assert_eq!(a.style, b.style);
            assert_eq!(a.content, b.content);
        }
    }

    #[test]
    fn receipt_adapter_keeps_shared_native_layout_state_and_css_geometry() {
        let bytes = baked();
        let (mut core, _) = Core::boot(
            Plan::decode(&bytes).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let (mut reference, _) = exact_apple::Host::boot(
            &bytes,
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
        )
        .unwrap();
        assert_kernel_parity(&core, &reference);
        for (index, target) in [
            "increment",
            "increment",
            "toggle-batch",
            "rows-1",
            "rows-1000",
            "toggle-move",
            "toggle-batch",
        ]
        .iter()
        .enumerate()
        {
            let key = core.runner.kernel().find_first_by_test_id(target).unwrap();
            let id = core.runner.kernel().node_by_key(key).unwrap().id;
            core.dispatch(id, Event::Press, index as f64 + 1.);
            reference.dispatch_at(id, Event::Press, index as f64 + 1.);
            assert_kernel_parity(&core, &reference);
        }
        let id = core
            .runner
            .kernel()
            .node_by_key(core.runner.kernel().find_first_by_test_id("draft").unwrap())
            .unwrap()
            .id;
        core.dispatch(id, Event::Input("Zażółć 🦀".into()), 10.);
        reference.dispatch_at(id, Event::Input("Zażółć 🦀".into()), 10.);
        assert_kernel_parity(&core, &reference);
        core.resize(420., 860.);
        reference.resize(420., 860.);
        assert_kernel_parity(&core, &reference);
        core.insets(Env::new(24., 0., 16., 0.));
        reference.set_insets(24., 0., 16., 0.);
        assert_kernel_parity(&core, &reference);
    }

    #[test]
    fn unsupported_plan_chooses_general_owner_before_core_initialization() {
        let mut plan = Plan::decode(&baked()).unwrap();
        assert!(eligible(&plan, &Data));
        plan.nodes[0].node_type = NodeType::Canvas as u8;
        assert!(!eligible(&plan, &Data));
        plan.nodes[0].node_type = NodeType::View as u8;
        plan.nodes[1].parent = Some(exact_plan::NodesId(2));
        plan.nodes[2].node_type = NodeType::Text as u8;
        assert!(!eligible(&plan, &Data));
    }

    #[test]
    fn all_compositor_records_keep_the_existing_two_coordinate_wire_contract() {
        let bytes = baked();
        let (core, _) = Core::boot(
            Plan::decode(&bytes).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let key = core
            .runner
            .kernel()
            .find_first_by_test_id("rows-container")
            .unwrap();
        let node = core.runner.kernel().node_by_key(key).unwrap();
        let mut value = resolved(node);
        value.transform = [[12., 3.], [0.5, 0.], [45., 0.], [0.2, 0.]];
        let mut p = Publication::default();
        p.compositor(node.id, None, &value);
        assert_eq!(p.records, 4);
        for record in p.hot.chunks_exact(27) {
            assert_eq!(record[0], PRESENT);
            assert_eq!(u32::from_le_bytes(record[1..5].try_into().unwrap()), 22);
            assert_eq!(record[10], 2);
        }
    }

    #[test]
    fn padded_block_scroll_keeps_css_end_padding_in_the_published_content_extent() {
        let path = std::path::Path::new("../../apps/android-core/app.contract");
        let source = std::fs::read_to_string(path).unwrap().replace(
            "scroll testId=\"core-scroll\"",
            "scroll testId=\"core-scroll\" display=\"block\" padding-right=20 padding-bottom=30",
        );
        let plan =
            contract::bake(contract::compile_path_source(path, &source).unwrap(), Data).unwrap();
        let bytes = plan.encode();
        let (core, _) = Core::boot(
            plan,
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let (reference, publication) = exact_apple::Host::boot(
            &bytes,
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
        )
        .unwrap();
        assert_kernel_parity(&core, &reference);
        let key = core
            .runner
            .kernel()
            .find_first_by_test_id("core-scroll")
            .unwrap();
        let size = core.mirror[&key].content.unwrap();
        let id = core.mirror[&key].id;
        let marker = format!(
            "\"op\":\"content\",\"id\":{id},\"w\":{},\"h\":{}",
            style::num(size.0),
            style::num(size.1)
        );
        assert!(publication.contains(&marker), "{marker}");
        let node = core.runner.kernel().node_by_key(key).unwrap();
        let child = core.runner.kernel().node(node.children()[0]).unwrap();
        assert!(size.1 >= child.frame.y - node.frame.y + child.frame.height + 30.);
    }

    #[test]
    fn initial_language_and_direction_follow_the_shared_runner() {
        let mut plan = Plan::decode(&baked()).unwrap();
        let name = exact_plan::StrId(plan.strings.len() as u32);
        plan.strings.push("ar".into());
        plan.locales.push(exact_plan::LocalesRow {
            name,
            rtl: true,
            texts: exact_plan::TextsRange::default(),
        });
        let (_, publication) = Core::boot(
            plan,
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        assert!(String::from_utf8_lossy(&publication)
            .contains("\"op\":\"language\",\"lang\":\"ar\",\"dir\":\"rtl\""));
    }

    #[test]
    fn display_frame_tasks_do_not_also_schedule_virtual_timer_frames() {
        let plan = Plan::decode(&baked()).unwrap();
        let index = plan
            .actions
            .iter()
            .position(|a| plan.str(a.name) == "increment")
            .unwrap();
        let mut builder = exact_plan::builder::PlanBuilder::from_plan(plan);
        builder.frame_timer(exact_plan::ActionsId(index as u32));
        let plan = builder.finish().unwrap();
        let bytes = plan.encode();
        let (mut reference, _) = exact_apple::Host::boot(
            &bytes,
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
        )
        .unwrap();
        let (mut core, _) = Core::boot(
            plan,
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        core.advance(16., true);
        reference.frame(16.);
        assert_eq!(
            core.runner.timer_due_ms(),
            reference.runner().timer_due_ms()
        );
        assert_eq!(core.runner.timer_due_ms(), None);
        core.advance(100., false);
        reference.advance(100.);
        assert_kernel_parity(&core, &reference);
        assert_eq!(
            core.agent(r#"{"op":"state"}"#),
            reference.agent(r#"{"op":"state"}"#)
        );
    }

    fn records(wire: &[u8]) -> Vec<(u8, &[u8])> {
        let count = u32::from_le_bytes(wire[8..12].try_into().unwrap());
        let mut at = HEADER_BYTES;
        (0..count)
            .map(|_| {
                let opcode = wire[at];
                let length = u32::from_le_bytes(wire[at + 1..at + 5].try_into().unwrap()) as usize;
                at += 5;
                let payload = &wire[at..at + length];
                at += length;
                (opcode, payload)
            })
            .collect()
    }

    #[test]
    fn new_leaves_need_no_child_list_but_existing_parents_can_be_cleared() {
        let (mut core, initial) = Core::boot(
            Plan::decode(&baked()).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let initial_lists: Vec<_> = records(&initial)
            .into_iter()
            .filter(|(opcode, payload)| {
                *opcode == crate::wire::JSON && payload.starts_with(br#"{"op":"children""#)
            })
            .collect();
        assert!(!initial_lists.is_empty());
        for (_, list) in initial_lists {
            assert!(!std::str::from_utf8(list).unwrap().contains(r#""ids":[]"#));
        }
        let row = core.runner.kernel().find_first_by_test_id("row-0").unwrap();
        let parent = core
            .runner
            .kernel()
            .node_by_key(row)
            .unwrap()
            .parent
            .unwrap();
        let root = core.runner.roots()[0];
        let receipt = core
            .runner
            .kernel_mut()
            .apply(
                root,
                1,
                &[exact_kernel::Op::SetChildren {
                    id: parent,
                    children: vec![],
                }],
            )
            .unwrap();
        let wire = core.publish(&[receipt], false, None);
        let expected = format!(r#"{{"op":"children","id":{parent},"ids":[]}}"#);
        assert!(records(&wire).iter().any(|(opcode, payload)| {
            *opcode == crate::wire::JSON && *payload == expected.as_bytes()
        }));
    }

    #[test]
    fn bulk_paint_keeps_typed_row_props_and_publishes_only_color_pairs() {
        let (mut core, _) = Core::boot(
            Plan::decode(&baked()).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let rows: Vec<_> = (0..100)
            .map(|index| {
                let key = core
                    .runner
                    .kernel()
                    .find_first_by_test_id(&format!("row-{index}"))
                    .unwrap();
                let props = &core.mirror[&key].props;
                (key, props.str(PropId::Text).unwrap().as_ptr())
            })
            .collect();
        let key = core
            .runner
            .kernel()
            .find_first_by_test_id("toggle-batch")
            .unwrap();
        let id = core.runner.kernel().node_by_key(key).unwrap().id;
        let wire = core.dispatch(id, Event::Press, 1.);
        let operations = records(&wire);
        assert_eq!(operations.len(), rows.len());
        for (opcode, payload) in operations {
            assert_eq!(opcode, PAINT);
            assert_eq!(payload.len(), 16);
            assert_eq!(u32::from_le_bytes(payload[4..8].try_into().unwrap()), 1);
            for pair in payload[8..].chunks_exact(4) {
                assert_eq!(u32::from_le_bytes(pair.try_into().unwrap()), 0xffd1e4ff);
            }
        }
        for (key, pointer) in &rows {
            assert_eq!(
                core.mirror[key].props.str(PropId::Text).unwrap().as_ptr(),
                *pointer
            );
        }
        let first = core.mirror[&rows[0].0].style.as_ref().unwrap();
        assert!(rows
            .iter()
            .all(|(key, _)| Rc::ptr_eq(first, core.mirror[key].style.as_ref().unwrap())));
    }

    #[test]
    fn typed_props_keep_scalar_values_clears_and_dynamic_spellcheck_inheritance() {
        use exact_kernel::wire::Op;

        let (mut core, _) = Core::boot(
            Plan::decode(&baked()).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let key = core.runner.kernel().find_first_by_test_id("draft").unwrap();
        let id = core.runner.kernel().node_by_key(key).unwrap().id;
        let root = core.roots[0];
        let values = [
            (PropId::Placeholder, PropValue::Str("東京 🌍".into())),
            (PropId::Disabled, PropValue::Bool(true)),
            (PropId::TabIndex, PropValue::Int(-3)),
            (PropId::HitSlop, PropValue::Float(3.75)),
        ];
        let ops: Vec<_> = values
            .iter()
            .map(|(prop, value)| Op::SetProp {
                id,
                prop: *prop,
                value: value.clone(),
            })
            .collect();
        let receipt = core.runner.kernel_mut().apply(root, 1, &ops).unwrap();
        let wire = core.publish(&[receipt], false, None);
        let operations = records(&wire);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].0, crate::wire::JSON);
        let text = std::str::from_utf8(operations[0].1).unwrap();
        for pair in [
            r#""placeholder":"東京 🌍""#,
            r#""disabled":"true""#,
            r#""tabIndex":"-3""#,
            r#""hitSlop":"3.75""#,
        ] {
            assert!(text.contains(pair), "{text}");
        }
        let ops: Vec<_> = values
            .iter()
            .map(|(prop, _)| Op::ClearProp { id, prop: *prop })
            .collect();
        let receipt = core.runner.kernel_mut().apply(root, 2, &ops).unwrap();
        let wire = core.publish(&[receipt], false, None);
        let text = std::str::from_utf8(records(&wire)[0].1).unwrap();
        assert!(
            text.contains(r#""clear":["disabled","hitSlop","placeholder","tabIndex"]"#),
            "{text}"
        );

        for (batch, target, value, expected) in [
            (3, root, Some("false"), Some(false)),
            (4, id, Some(""), Some(true)),
            (5, root, None, Some(true)),
            (6, id, None, None),
        ] {
            let op = match value {
                Some(value) => Op::SetProp {
                    id: target,
                    prop: PropId::Spellcheck,
                    value: PropValue::Str(value.into()),
                },
                None => Op::ClearProp {
                    id: target,
                    prop: PropId::Spellcheck,
                },
            };
            let receipt = core.runner.kernel_mut().apply(root, batch, &[op]).unwrap();
            let wire = core.publish(&[receipt], false, None);
            assert_eq!(core.mirror[&key].spellcheck, expected);
            let input_marker = format!(r#""id":{id},"#);
            let input = records(&wire).into_iter().find_map(|(opcode, payload)| {
                if opcode != crate::wire::JSON {
                    return None;
                }
                let text = std::str::from_utf8(payload).unwrap();
                text.contains(&input_marker).then_some(text)
            });
            if batch == 5 {
                assert!(
                    input.is_none(),
                    "own spelling hint overrides the removed ancestor hint"
                );
            } else {
                let text = input.unwrap();
                let pair = match expected {
                    Some(value) => format!(r#""spellcheck":"{value}""#),
                    None => r#""clear":["spellcheck"]"#.to_owned(),
                };
                assert!(text.contains(&pair), "{text}");
            }
        }
    }

    #[test]
    fn stack_paint_payload_keeps_every_light_dark_color_pair_and_noop_silence() {
        let (core, _) = Core::boot(
            Plan::decode(&baked()).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let node = core.runner.kernel().node(core.roots[0]).unwrap();
        let before = resolved(node);
        let mut after = before.clone();
        after.colors = std::array::from_fn(|index| [index as u32 + 1, index as u32 + 101]);
        let mut publication = Publication::default();
        publication.paint(node.id, &before, &before);
        assert!(publication.hot.is_empty());
        publication.paint(node.id, &before, &after);
        assert_eq!(publication.records, 1);
        assert_eq!(publication.hot[0], PAINT);
        assert_eq!(
            u32::from_le_bytes(publication.hot[1..5].try_into().unwrap()),
            56
        );
        assert_eq!(
            u32::from_le_bytes(publication.hot[9..13].try_into().unwrap()),
            63
        );
        for (bytes, color) in publication.hot[13..]
            .chunks_exact(4)
            .zip(after.colors.into_iter().flatten())
        {
            assert_eq!(u32::from_le_bytes(bytes.try_into().unwrap()), color);
        }
    }

    #[test]
    fn new_shared_style_leaf_keeps_full_colors_after_an_existing_hot_touch() {
        use crate::wire::{STYLE_DEFINE, STYLE_REFERENCE};
        use exact_kernel::{
            style::{Color, ColorValue},
            wire::Op,
        };
        let (mut core, _) = Core::boot(
            Plan::decode(&baked()).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let row = core.runner.kernel().find_first_by_test_id("row-0").unwrap();
        let node = core.runner.kernel().node_by_key(row).unwrap();
        let (id, parent, mut full) = (node.id, node.parent.unwrap(), node.style.clone());
        let color = ColorValue::LightDark(Color(0x991122ff), Color(0x223344ff));
        full.background_color = Some(color);
        full.mask.set(StyleId::BackgroundColor);
        let mut hot = StyleProps {
            background_color: Some(color),
            ..StyleProps::default()
        };
        hot.mask.set(StyleId::BackgroundColor);
        let added = 10001;
        let mut children = core.runner.kernel().node(parent).unwrap().children();
        children.push(added);
        let receipt = core
            .runner
            .kernel_mut()
            .apply(
                core.roots[0],
                1000,
                &[
                    Op::SetStyle {
                        id,
                        patch: Box::new(hot),
                    },
                    Op::CreateView {
                        id: added,
                        node_type: NodeType::Text,
                    },
                    Op::SetStyle {
                        id: added,
                        patch: Box::new(full),
                    },
                    Op::SetProp {
                        id: added,
                        prop: PropId::Text,
                        value: "Added".into(),
                    },
                    Op::SetChildren {
                        id: parent,
                        children,
                    },
                ],
            )
            .unwrap();
        assert!(
            Rc::ptr_eq(
                &core.runner.kernel().node(id).unwrap().shared_style(),
                &core.runner.kernel().node(added).unwrap().shared_style(),
            ),
            "fixture must exercise the same authored-style memo key"
        );
        let first = CommitReceipt {
            touched: vec![row],
            ..CommitReceipt::default()
        };
        let wire = core.publish(&[first, receipt], false, None);
        let operations = records(&wire);
        let paint = operations
            .iter()
            .find_map(|(opcode, payload)| {
                (*opcode == PAINT && u32::from_le_bytes(payload[..4].try_into().unwrap()) == id)
                    .then_some(*payload)
            })
            .unwrap();
        assert_eq!(u32::from_le_bytes(paint[4..8].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(paint[8..12].try_into().unwrap()),
            0xff991122
        );
        assert_eq!(
            u32::from_le_bytes(paint[12..16].try_into().unwrap()),
            0xff223344
        );
        let create = format!(r#"{{"op":"create","id":{added},"#);
        let reference = operations
            .iter()
            .find_map(|(opcode, payload)| {
                (*opcode == STYLE_REFERENCE && payload[4..].starts_with(create.as_bytes()))
                    .then_some(*payload)
            })
            .expect("new leaf needs a complete style definition");
        let json = operations
            .iter()
            .find_map(|(opcode, payload)| {
                (*opcode == STYLE_DEFINE && payload[..4] == reference[..4])
                    .then(|| std::str::from_utf8(&payload[4..]).unwrap())
            })
            .unwrap();
        assert!(
            json.contains(r#""background_color":[[153,17,34,255],[34,51,68,255]]"#),
            "{json}"
        );
        assert!(json.contains(r#""text_color":[26,28,30,255]"#), "{json}");
        assert!(json.contains(r#""font_size":16"#), "{json}");
    }
    #[test]
    fn explicit_color_mask_survives_equal_inherited_publication_then_clear() {
        use exact_kernel::{
            style::{Color, ColorValue},
            wire::Op,
        };
        let plan = contract::compile(
            r##"component A
  view
    column testId="parent" color="#112233" font-size=14
      text "Inherited" testId="inherited"
      text "Own" testId="own" color="#112233"
"##,
        )
        .unwrap();
        let (mut core, _) = Core::boot(
            plan,
            (),
            Box::new(MonospaceMeasurer::default()),
            320.,
            640.,
            None,
        )
        .unwrap();
        let id = |core: &Core<()>, name: &str| {
            let key = core.runner.kernel().find_first_by_test_id(name).unwrap();
            core.runner.kernel().node_by_key(key).unwrap().id
        };
        let (parent, inherited, own) = (
            id(&core, "parent"),
            id(&core, "inherited"),
            id(&core, "own"),
        );
        let mut patch = StyleProps {
            text_color: ColorValue::Fixed(Color(0x445566ff)),
            ..StyleProps::default()
        };
        patch.mask.set(StyleId::TextColor);
        for (batch, op, changed, unchanged) in [
            (
                1,
                Op::SetStyle {
                    id: parent,
                    patch: Box::new(patch),
                },
                inherited,
                own,
            ),
            (
                2,
                Op::ClearStyle {
                    id: own,
                    mask: StyleMask::of(StyleId::TextColor),
                },
                own,
                inherited,
            ),
        ] {
            let receipt = core
                .runner
                .kernel_mut()
                .apply(core.roots[0], batch, &[op])
                .unwrap();
            let wire = core.publish(&[receipt], false, None);
            let paints: Vec<_> = records(&wire)
                .into_iter()
                .filter_map(|(opcode, payload)| (opcode == PAINT).then_some(payload))
                .collect();
            assert!(paints
                .iter()
                .all(|payload| u32::from_le_bytes(payload[..4].try_into().unwrap()) != unchanged));
            let paint = paints
                .into_iter()
                .find(|payload| u32::from_le_bytes(payload[..4].try_into().unwrap()) == changed)
                .expect("only the leaf affected by inheritance/clear must repaint");
            assert_ne!(u32::from_le_bytes(paint[4..8].try_into().unwrap()) & 2, 0);
            assert_eq!(
                u32::from_le_bytes(paint[8..12].try_into().unwrap()),
                0xff445566
            );
            assert_eq!(
                u32::from_le_bytes(paint[12..16].try_into().unwrap()),
                0xff445566
            );
        }
    }
    #[test]
    fn cold_layout_change_keeps_color_delta_after_a_direct_paint_turn() {
        use exact_kernel::{
            style::{Color, ColorValue},
            wire::Op,
        };

        let (mut core, _) = Core::boot(
            Plan::decode(&baked()).unwrap(),
            Data,
            Box::new(MonospaceMeasurer::default()),
            390.,
            844.,
            None,
        )
        .unwrap();
        let action = core
            .runner
            .kernel()
            .find_first_by_test_id("toggle-batch")
            .unwrap();
        let action = core.runner.kernel().node_by_key(action).unwrap().id;
        core.dispatch(action, Event::Press, 1.);
        let row = core.runner.kernel().find_first_by_test_id("row-0").unwrap();
        let id = core.runner.kernel().node_by_key(row).unwrap().id;
        let mut patch = StyleProps {
            width: Dimension::Points(200.),
            background_color: Some(ColorValue::Fixed(Color::WHITE)),
            ..StyleProps::default()
        };
        patch.mask.set(StyleId::Width);
        patch.mask.set(StyleId::BackgroundColor);
        let root = core.roots[0];
        let receipt = core
            .runner
            .kernel_mut()
            .apply(
                root,
                1000,
                &[Op::SetStyle {
                    id,
                    patch: Box::new(patch),
                }],
            )
            .unwrap();
        let wire = core.publish(&[receipt], false, None);
        let paint = records(&wire)
            .into_iter()
            .find_map(|(opcode, payload)| {
                (opcode == PAINT && u32::from_le_bytes(payload[..4].try_into().unwrap()) == id)
                    .then_some(payload)
            })
            .expect("layout-only cold JSON must not suppress the changed background");
        assert_eq!(u32::from_le_bytes(paint[4..8].try_into().unwrap()), 1);
        assert_eq!(paint[8..], [255; 8]);
        assert_eq!(
            core.mirror[&row].style.as_ref().unwrap().colors[0],
            [u32::MAX; 2]
        );
    }
}
