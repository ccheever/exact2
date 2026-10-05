//! The columnar node arena.
//!
//! Every node is a slot; every attribute is a column indexed by slot. Topology
//! is index-based (`parents`, `children`), identity is generation-checked
//! ([`NodeKey`]), and destroyed slots go on a free list. Nothing here hashes on
//! the read path except the wire-id lookup a producer's op needs once.
//!
//! The arena is the authored truth. Frames and Taffy handles are derived
//! columns: rehydration is columns-plus-rebuild, never serialized engine state.

use crate::id::IdMap;
use crate::shared_style::SharedStyles;
use crate::sorted::SlotSet;
use std::rc::Rc;

use taffy::NodeId;

use crate::error::ApplyError;
use crate::generated::{
    FieldSizing, InheritedStyle, NodeType, PropId, StyleId, StyleMask, StyleProps,
};
use crate::id::{Frame, NodeFlags, NodeKey, ViewId};
use crate::props::{PropList, PropValue};
use crate::style::Env;
use crate::text::{ParagraphStamp, TextDomain, TextRevisions, TextRun, TextStyle};

/// Columnar node storage. Cloning forks authored state into a fresh paragraph
/// namespace: either arena can subsequently receive independent transactions.
#[derive(Debug, Default)]
pub struct NodeArena {
    generations: Vec<u32>,
    live: Vec<bool>,
    node_types: Vec<NodeType>,
    local_ids: Vec<ViewId>,
    parents: Vec<Option<u32>>,
    children: Vec<Vec<u32>>,
    child_indices: Vec<usize>,
    // Sources whose pass flags need consuming. Geometry flags expire by pass,
    // so a small edit after a large layout never clears N prior changed nodes.
    pub(crate) layout_dirty: SlotSet,
    // Equal styles are one allocation (`shared_style`).
    styles: Vec<Rc<StyleProps>>,
    shared: SharedStyles,
    props: Vec<PropList>,
    /// Live nodes per prop id and per node type: a host's scan for a few
    /// props or types skips the walk when no node has them.
    prop_counts: Vec<u32>,
    type_counts: Vec<u32>,
    flags: Vec<NodeFlags>,
    layout_passes: Vec<u64>,
    geometry_passes: Vec<(u32, u64)>,
    frames: Vec<Frame>,
    // @ref LLP 1043.000 §3 D4 — no per-node vector or allocation.
    pub(crate) flow: IdMap<u32, crate::flow::FlowState>,
    pub(crate) exclusion_slots: SlotSet,
    /// Slots whose style writes a row in `rem`/`em` (LLP 1069.000 D3).
    pub(crate) relative_slots: SlotSet,
    /// Slots that are `position: sticky` (LLP 1083 D3).
    pub(crate) sticky_slots: SlotSet,
    /// The timeline-bearing nodes and what each follower's name resolves
    /// to (LLP 1057.003 D4), kept by the linked lookup after each commit.
    pub(crate) timelines: crate::timeline::Registry,
    /// A root font size the host set since the last commit, which that
    /// commit applies (LLP 1069.000 D3).
    pub(crate) root_font_size_next: Option<f32>,
    /// Scrollable overflow from the last layout: the content's extent in the
    /// node's own space (width, height), Taffy's `content_size`.
    contents: Vec<(f32, f32)>,
    /// Host intrinsic size: a replaced element's natural size or a projected
    /// tablist's native control size; `None` before measurement or after removal.
    intrinsic: Vec<Option<(f32, f32)>>,
    /// What a host container covers of a box (LLP 1075.003 §3.5): a few
    /// route nodes at most, so a list by slot rather than a column (and not a
    /// map, whose code every web core would carry).
    covers: Vec<(u32, crate::kernel::HostCover)>,
    taffy: Vec<Option<NodeId>>,
    is_root: Vec<bool>,
    free: Vec<u32>,
    roots: Vec<u32>,
    by_local: IdMap<ViewId, u32>,
    live_count: usize,
    /// The page's environment (LLP 1001 §2): what `env()` lengths resolve
    /// to. The host's, not the tree's — a reset keeps it.
    env: Env,
    pub(crate) document_language: String,
    pub(crate) document_style: StyleProps,
    // Current metadata only: O(arena slot high-water), never revision history.
    text_revisions: Vec<TextRevisions>,
    text_domain: TextDomain,
    text_serial: u64,
}

impl Clone for NodeArena {
    fn clone(&self) -> Self {
        Self {
            generations: self.generations.clone(),
            live: self.live.clone(),
            node_types: self.node_types.clone(),
            local_ids: self.local_ids.clone(),
            parents: self.parents.clone(),
            children: self.children.clone(),
            child_indices: self.child_indices.clone(),
            layout_dirty: self.layout_dirty.clone(),
            styles: self.styles.clone(),
            shared: self.shared.clone(),
            props: self.props.clone(),
            prop_counts: self.prop_counts.clone(),
            type_counts: self.type_counts.clone(),
            flags: self.flags.clone(),
            layout_passes: self.layout_passes.clone(),
            geometry_passes: self.geometry_passes.clone(),
            frames: self.frames.clone(),
            flow: self.flow.clone(),
            exclusion_slots: self.exclusion_slots.clone(),
            relative_slots: self.relative_slots.clone(),
            sticky_slots: self.sticky_slots.clone(),
            timelines: self.timelines.clone(),
            root_font_size_next: self.root_font_size_next,
            contents: self.contents.clone(),
            intrinsic: self.intrinsic.clone(),
            covers: self.covers.clone(),
            taffy: self.taffy.clone(),
            is_root: self.is_root.clone(),
            free: self.free.clone(),
            roots: self.roots.clone(),
            by_local: self.by_local.clone(),
            live_count: self.live_count,
            env: self.env.clone(),
            document_language: self.document_language.clone(),
            document_style: self.document_style.clone(),
            text_revisions: vec![TextRevisions::default(); self.text_revisions.len()],
            text_domain: TextDomain::default(),
            text_serial: 0,
        }
    }
}

impl NodeArena {
    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Live nodes.
    pub fn live_count(&self) -> usize {
        self.live_count
    }

    /// The environment `env()` lengths resolve against.
    pub fn env(&self) -> &Env {
        &self.env
    }

    /// The root element's font size, what `rem` resolves against and what
    /// a node no ancestor gives a `font-size` inherits (CSS's `medium`;
    /// LLP 1069.000 D3). The host's, not the tree's — a reset keeps it.
    pub fn root_font_size(&self) -> f32 {
        self.root_font_size_next
            .unwrap_or(self.document_style.font_size)
    }

    /// Set the environment. The engine styles that read it are the
    /// kernel's to re-derive (`Kernel::set_env`).
    pub fn set_env(&mut self, env: Env) {
        self.env = env;
    }

    /// Forget every allocation while retaining each slot's generation.
    ///
    /// Resetting the kernel must not make a pre-reset [`NodeKey`] valid for a
    /// later allocation at the same slot. Keeping the generation column means
    /// [`alloc`](Self::alloc) advances identity exactly as ordinary reuse does.
    pub(crate) fn reset(&mut self) {
        self.renew_text_namespace();
        self.flow.clear();
        self.exclusion_slots.clear();
        self.relative_slots.clear();
        self.sticky_slots.clear();
        self.timelines = Default::default();
        for slot in 0..self.live.len() {
            self.live[slot] = false;
            self.parents[slot] = None;
            self.children[slot].clear();
            self.styles[slot] = self.shared.default_style();
            self.props[slot].clear();
            self.flags[slot] = NodeFlags::default();
            self.prop_counts.clear();
            self.type_counts.clear();
            self.frames[slot] = Frame::default();
            self.contents[slot] = (0.0, 0.0);
            self.intrinsic[slot] = None;
            self.taffy[slot] = None;
            self.is_root[slot] = false;
        }
        self.covers.clear();
        self.free = (0..self.live.len() as u32).rev().collect();
        self.roots.clear();
        self.by_local.clear();
        self.live_count = 0;
    }

    /// Slots the columns hold (live plus free).
    pub fn slot_count(&self) -> usize {
        self.live.len()
    }

    /// Slots ever allocated: identities never repeat below this.
    pub(crate) fn slot_space(&self) -> usize {
        self.generations.len()
    }

    /// Return what the columns hold beyond the live nodes, once they hold
    /// more than twice as many slots: trailing free slots go (their
    /// generations stay, so a stale key never names a later node), and each
    /// column gives back its spare capacity. A list's columns grow to the
    /// most rows a fling mounts; this is how they come back once it settles.
    /// Below that, a trim changes nothing, so a steady tree never churns.
    pub(crate) fn trim(&mut self) {
        self.shared.trim();
        if self.live.capacity() <= 2 * self.live_count.max(64) {
            return;
        }
        let keep = self.live.iter().rposition(|l| *l).map_or(0, |s| s + 1);
        self.free.retain(|s| (*s as usize) < keep);
        self.free.shrink_to_fit();
        macro_rules! fit {
            ($($column:ident),*) => {$(
                self.$column.truncate(keep);
                self.$column.shrink_to_fit();
            )*};
        }
        fit!(
            live,
            node_types,
            local_ids,
            parents,
            children,
            child_indices,
            styles,
            props,
            flags,
            layout_passes,
            geometry_passes,
            frames,
            contents,
            intrinsic,
            taffy,
            is_root,
            text_revisions
        );
        self.flow.shrink_to_fit();
        self.by_local.shrink_to_fit();
    }

    /// Whether `slot` holds a live node.
    pub fn is_live(&self, slot: u32) -> bool {
        (slot as usize) < self.live.len() && self.live[slot as usize]
    }

    /// The generation-checked key for a live slot.
    pub fn key(&self, slot: u32) -> NodeKey {
        NodeKey {
            index: slot,
            generation: self.generations[slot as usize],
        }
    }

    /// The slot a key names, if that allocation is still live.
    pub fn resolve(&self, key: NodeKey) -> Option<u32> {
        if self.is_live(key.index) && self.generations[key.index as usize] == key.generation {
            Some(key.index)
        } else {
            None
        }
    }

    /// The slot holding wire id `id`.
    pub fn slot_of(&self, id: ViewId) -> Option<u32> {
        self.by_local.get(&id).copied()
    }

    /// The key for wire id `id`.
    pub fn key_of(&self, id: ViewId) -> Option<NodeKey> {
        self.slot_of(id).map(|s| self.key(s))
    }

    /// Wire id of a slot.
    pub fn local_id(&self, slot: u32) -> ViewId {
        self.local_ids[slot as usize]
    }

    /// Node type of a slot.
    pub fn node_type(&self, slot: u32) -> NodeType {
        self.node_types[slot as usize]
    }

    /// Whether a slot is a `<button>` element on the web: a Pressable,
    /// whatever its ARIA role, unless an `href` makes it an `<a>`.
    pub(crate) fn is_button(&self, slot: u32) -> bool {
        self.node_type(slot) == NodeType::Pressable
            && self.props(slot).str(crate::PropId::Href).is_none()
    }

    /// Parent slot.
    pub fn parent(&self, slot: u32) -> Option<u32> {
        self.parents[slot as usize]
    }

    /// Ordered child slots.
    pub fn children(&self, slot: u32) -> &[u32] {
        &self.children[slot as usize]
    }

    /// Style rows.
    pub fn style(&self, slot: u32) -> &StyleProps {
        &self.styles[slot as usize]
    }

    /// Props.
    pub fn props(&self, slot: u32) -> &PropList {
        &self.props[slot as usize]
    }

    /// Dirty flags.
    pub fn flags(&self, slot: u32) -> NodeFlags {
        let mut flags = self.flags[slot as usize];
        let (root, pass) = self.geometry_passes[slot as usize];
        if self.layout_passes[root as usize] != pass {
            flags.remove(NodeFlags::GEOMETRY_CHANGED);
        }
        flags
    }

    /// Resolved exclusions in this leaf's border-box coordinates.
    pub fn flow_shapes(&self, slot: u32) -> &[exact_textflow::FlowShape] {
        self.flow.get(&slot).map_or(&[], |s| s.shapes.as_slice())
    }

    /// Why an intersecting auto-height leaf keeps ordinary layout, if it does.
    pub fn flow_refusal(&self, slot: u32) -> Option<crate::FlowRefusal> {
        self.flow.get(&slot).and_then(|s| s.refusal)
    }

    /// Whether auto-height flow's admission rule refuses this text leaf, by
    /// tree and style alone — what a host that lays out without the kernel's
    /// engine (the web) applies. `flow_refusal` is the laid-out answer.
    pub fn auto_flow_refusal(&self, slot: u32) -> Option<crate::FlowRefusal> {
        crate::flow::structural_refusal(self, slot)
    }

    /// Number of entries in the sparse derived flow column (including skips).
    pub fn flow_entry_count(&self) -> usize {
        self.flow.len()
    }

    /// Absolute frame from the last layout.
    pub fn frame(&self, slot: u32) -> Frame {
        self.frames[slot as usize]
    }

    /// Layout-engine handle.
    pub fn taffy(&self, slot: u32) -> Option<NodeId> {
        self.taffy[slot as usize]
    }

    /// Whether the slot is a registered root.
    pub fn is_root(&self, slot: u32) -> bool {
        self.is_root[slot as usize]
    }

    /// Root slots in attach order.
    pub fn roots(&self) -> &[u32] {
        &self.roots
    }

    /// Every live slot, ascending.
    pub fn iter_live(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.live.len() as u32).filter(move |s| self.live[*s as usize])
    }

    /// Whether `ancestor` is a proper ancestor of `node`.
    pub fn is_ancestor(&self, ancestor: u32, node: u32) -> bool {
        let mut cur = self.parents[node as usize];
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.parents[p as usize];
        }
        false
    }

    /// Depth below the root (a root is 0).
    pub fn depth(&self, slot: u32) -> u32 {
        let mut depth = 0;
        let mut cur = self.parents[slot as usize];
        while let Some(p) = cur {
            depth += 1;
            cur = self.parents[p as usize];
        }
        depth
    }

    /// The subtree rooted at `slot`, preorder, `slot` first. A listed child
    /// that no longer names its lister as parent — destroyed earlier in a
    /// batch whose detaching is still pending — is not part of it.
    pub fn subtree(&self, slot: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack = vec![slot];
        while let Some(s) = stack.pop() {
            out.push(s);
            stack.extend(
                self.children[s as usize]
                    .iter()
                    .rev()
                    .filter(|c| self.parents[**c as usize] == Some(s)),
            );
        }
        out
    }

    /// The nearest ancestor-or-self that is measured as one text leaf: a `Text`
    /// under a `Text` is an inline run of its parent, so measurement is owned
    /// by the topmost consecutive `Text` ancestor.
    pub fn measure_owner(&self, slot: u32) -> u32 {
        let mut owner = slot;
        while let Some(p) = self.parents[owner as usize] {
            if self.node_types[p as usize] == NodeType::Text {
                owner = p;
            } else {
                break;
            }
        }
        owner
    }

    /// The canonical independent paragraph's current input proof. Inline nodes
    /// return None: their own text_runs payload is not their owner's payload.
    pub fn paragraph_stamp(&self, slot: u32) -> Option<ParagraphStamp> {
        if !self.is_live(slot)
            || !matches!(self.node_type(slot), NodeType::Text | NodeType::TextInput)
            || self.measure_owner(slot) != slot
        {
            return None;
        }
        let r = self.text_revisions[slot as usize];
        Some(ParagraphStamp {
            domain: self.text_domain.clone(),
            owner: self.key(slot),
            view: self.local_id(slot),
            metrics: r.metrics,
            paint_source: r.paint_source,
        })
    }

    pub(crate) fn renew_text_namespace(&mut self) {
        self.text_domain = TextDomain::default();
        self.text_serial = 0;
        self.text_revisions.fill(TextRevisions::default());
    }

    /// Called only while applying validated mutations. Rollover cannot fail
    /// halfway through a commit: rotating the namespace invalidates every old
    /// proof atomically with that successful mutation. It retains no history.
    pub(crate) fn revise_text(&mut self, slot: u32, metrics: bool) {
        let owner = self.measure_owner(slot);
        if !matches!(self.node_type(owner), NodeType::Text | NodeType::TextInput) {
            return;
        }
        if self.text_serial == u64::MAX {
            self.renew_text_namespace();
        }
        self.text_serial += 1;
        let revision = &mut self.text_revisions[owner as usize];
        if metrics {
            revision.metrics = self.text_serial;
        }
        revision.paint_source = self.text_serial;
    }

    /// Whether this slot is an inline run (a `Text` whose parent is a `Text`).
    pub fn is_inline_run(&self, slot: u32) -> bool {
        self.node_types[slot as usize] == NodeType::Text
            && self.parents[slot as usize]
                .is_some_and(|p| self.node_types[p as usize] == NodeType::Text)
    }

    /// The slot whose own row supplies `id` for `slot`: the slot itself when
    /// it sets the row; for a row the schema marks inherited, the nearest
    /// logical ancestor that does; `None` when the initial value applies
    /// (CSS inheritance, LLP 1035.000 D1).
    pub fn inherited_source(&self, slot: u32, id: StyleId) -> Option<u32> {
        let mut cur = Some(slot);
        while let Some(s) = cur {
            if self.styles[s as usize].mask.has(id) {
                return Some(s);
            }
            if !id.inherited() {
                return None;
            }
            cur = self.parents[s as usize];
        }
        None
    }

    /// The node's rows with the inherited rows in `rows` resolved through the
    /// logical ancestors: an own row stays; a missing inherited row takes the
    /// nearest ancestor's and is marked set; a row no ancestor sets keeps the
    /// initial value, unmarked. Rows outside `rows`, and rows the schema does
    /// not mark inherited, are the node's own. One walk, however many rows.
    pub fn computed_style(&self, slot: u32, rows: StyleMask) -> StyleProps {
        let mut out = StyleProps::clone(&self.styles[slot as usize]);
        self.copy_inherited(slot, rows, |from, mask| out.copy_rows(from, mask));
        out
    }

    /// The style that supplies row `id` of [`NodeArena::computed_style`]:
    /// one row read without copying a whole style.
    pub fn computed_source(&self, slot: u32, id: StyleId) -> &StyleProps {
        let own = &self.styles[slot as usize];
        if own.mask.has(id) || !id.inherited() {
            return own;
        }
        let mut cur = self.parents[slot as usize];
        while let Some(p) = cur {
            if self.styles[p as usize].mask.has(id) {
                return &self.styles[p as usize];
            }
            cur = self.parents[p as usize];
        }
        if self.document_style.mask.has(id) {
            return &self.document_style;
        }
        own
    }

    /// Values used only to compare inherited rows across a topology change.
    /// No unrelated style payloads are copied into these transient snapshots.
    pub(crate) fn computed_inherited(&self, slot: u32) -> InheritedStyle {
        let mut out = InheritedStyle::new(&self.styles[slot as usize]);
        self.copy_inherited(slot, StyleMask::INHERITED, |from, mask| {
            out.copy_rows(from, mask)
        });
        out
    }

    fn copy_inherited(
        &self,
        slot: u32,
        rows: StyleMask,
        mut copy: impl FnMut(&StyleProps, StyleMask),
    ) {
        let mut pending = rows
            .intersect(StyleMask::INHERITED)
            .minus(self.styles[slot as usize].mask);
        let mut cur = self.parents[slot as usize];
        while !pending.is_empty() {
            let Some(p) = cur else { break };
            let ancestor: &StyleProps = &self.styles[p as usize];
            let found = pending.intersect(ancestor.mask);
            if !found.is_empty() {
                copy(ancestor, found);
                pending = pending.minus(found);
            }
            cur = self.parents[p as usize];
        }
        let found = pending.intersect(self.document_style.mask);
        if !found.is_empty() {
            copy(&self.document_style, found);
        }
    }

    /// What the leaf's string is (LLP 1045 D3): its `markup` prop.
    pub fn markup(&self, slot: u32) -> crate::text::Markup {
        crate::text::Markup::from_prop(self.props[slot as usize].str(PropId::Markup))
    }

    /// The paragraph style of the leaf at `slot`. A text input's value is never
    /// collapsed: the web's `<input>` and `<textarea>` preserve what was typed
    /// (the UA sheet's `pre` and `pre-wrap`), so a collapsing row preserves here.
    /// @ref LLP 1053 §0 G5
    pub fn paragraph(&self, slot: u32) -> crate::text::Paragraph {
        let mut paragraph =
            crate::text::Paragraph::from_style(&self.computed_style(slot, StyleMask::INHERITED));
        paragraph.markup = self.markup(slot);
        if self.node_types[slot as usize] == NodeType::TextInput
            && !paragraph.white_space.model().preserves()
        {
            paragraph.white_space = crate::WhiteSpace::PreWrap;
        }
        paragraph
    }

    /// Append the text runs of the leaf rooted at `slot`, in order. A run
    /// measures with its computed style: the rows it sets, else its
    /// paragraph's, else the initial values — as a `<span>` inside a `<div>`.
    /// Its text is `text-transform`ed here, once, for every measurer and
    /// painter (LLP 1064 D5) — except a field's, which shows what was typed
    /// (the web's form controls reset the row), and Markdown source, which a
    /// host expands itself.
    pub fn text_runs<'a>(&'a self, slot: u32, out: &mut Vec<TextRun<'a>>) {
        self.text_runs_after(slot, out, &mut crate::text::case::WordBoundary::default());
    }

    fn text_runs_after<'a>(
        &'a self,
        slot: u32,
        out: &mut Vec<TextRun<'a>>,
        boundary: &mut crate::text::case::WordBoundary,
    ) {
        let s = slot as usize;
        let computed = self.computed_style(slot, StyleMask::INHERITED);
        let style = TextStyle::from_style(&computed);
        match self.node_types[s] {
            NodeType::TextInput => {
                // An input has a line box even when empty (the web's
                // `<input>`): its value, else its placeholder, else one space.
                let props = &self.props[s];
                // Fixed controls have a preferred size independent of their
                // current value. HTML's default character width is 20; a
                // textarea's default is two rows. Explicit CSS dimensions
                // still win in layout. Content sizing measures the live value.
                let text = if self.styles[s].field_sizing == FieldSizing::Fixed {
                    if props.str(PropId::SemanticTag) == Some("textarea") {
                        "00000000000000000000\n00000000000000000000"
                    } else {
                        "00000000000000000000"
                    }
                } else {
                    props
                        .str(PropId::Value)
                        .filter(|v| !v.is_empty())
                        .or_else(|| props.str(PropId::Placeholder).filter(|p| !p.is_empty()))
                        .unwrap_or(" ")
                };
                out.push(TextRun {
                    text: text.into(),
                    style,
                });
                // A textarea's final Return creates a caret line. Paragraph
                // shapers can omit a terminal break, so retain that line with
                // zero-width measurement content, never in the field's value.
                if self.styles[s].field_sizing == FieldSizing::Content
                    && props.str(PropId::SemanticTag) == Some("textarea")
                    && text.ends_with('\n')
                {
                    out.push(TextRun {
                        text: "\u{200b}".into(),
                        style,
                    });
                }
            }
            _ => {
                if let Some(text) = self.props[s].str(PropId::Text) {
                    let shown = if self.markup(slot) == crate::text::Markup::Markdown {
                        text.into()
                    } else {
                        computed.hyphens.shown(crate::text::case::shown(
                            computed.text_transform,
                            text,
                            *boundary,
                        ))
                    };
                    boundary.push(text);
                    out.push(TextRun { text: shown, style });
                } else {
                    for child in &self.children[s] {
                        if self.node_types[*child as usize] == NodeType::Text {
                            self.text_runs_after(*child, out, boundary);
                        }
                    }
                }
            }
        }
    }

    /// The string the `Text` leaf at `slot` shows: its `text` prop as its
    /// paragraph's runs carry it ([`Self::text_runs`]), for a host that
    /// paints from props rather than from runs. `capitalize` reads the
    /// runs before this one, so it asks the whole paragraph.
    pub fn shown_text(&self, slot: u32) -> Option<std::borrow::Cow<'_, str>> {
        let text = self.props[slot as usize].str(PropId::Text)?;
        let computed = self.computed_style(
            slot,
            StyleMask::of(StyleId::TextTransform).union(StyleMask::of(StyleId::Hyphens)),
        );
        let (transform, hyphens) = (computed.text_transform, computed.hyphens);
        if self.node_types[slot as usize] != NodeType::Text
            || self.markup(slot) == crate::text::Markup::Markdown
        {
            return Some(text.into());
        }
        if transform != crate::TextTransform::Capitalize {
            return Some(hyphens.shown(transform.apply(text, "")));
        }
        let mut owner = slot;
        while self.is_inline_run(owner) {
            owner = self.parents[owner as usize]?;
        }
        let (mut runs, mut leaves) = (Vec::new(), Vec::new());
        self.text_runs(owner, &mut runs);
        self.text_leaves(owner, &mut leaves);
        let at = leaves.iter().position(|leaf| *leaf == slot)?;
        runs.into_iter().nth(at).map(|run| run.text)
    }

    /// The leaves [`Self::text_runs`] makes a run of, in its order.
    fn text_leaves(&self, slot: u32, out: &mut Vec<u32>) {
        if self.props[slot as usize].str(PropId::Text).is_some() {
            return out.push(slot);
        }
        for child in &self.children[slot as usize] {
            if self.node_types[*child as usize] == NodeType::Text {
                self.text_leaves(*child, out);
            }
        }
    }

    // ---- mutation (crate-private: only the transaction engine writes) ------

    pub(crate) fn alloc(&mut self, id: ViewId, node_type: NodeType) -> Result<u32, ApplyError> {
        debug_assert!(!self.by_local.contains_key(&id), "alloc of a live wire id");
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None => {
                // A trimmed slot's generation outlives its columns.
                let slot = self.live.len();
                if slot == self.generations.len() {
                    if slot >= u32::MAX as usize {
                        return Err(ApplyError::SlotSpaceExhausted);
                    }
                    self.generations.push(0);
                }
                self.live.push(false);
                self.node_types.push(node_type);
                self.local_ids.push(0);
                self.parents.push(None);
                self.children.push(Vec::new());
                self.child_indices.push(0);
                self.styles.push(self.shared.default_style());
                self.props.push(PropList::new());
                self.flags.push(NodeFlags::default());
                self.layout_passes.push(0);
                self.geometry_passes.push((0, 0));
                self.frames.push(Frame::default());
                self.contents.push((0.0, 0.0));
                self.intrinsic.push(None);
                self.taffy.push(None);
                self.is_root.push(false);
                self.text_revisions.push(TextRevisions::default());
                slot as u32
            }
        };
        let s = slot as usize;
        let next = self.generations[s].wrapping_add(1);
        self.generations[s] = if next == 0 { 1 } else { next };
        self.live[s] = true;
        self.node_types[s] = node_type;
        bump(&mut self.type_counts, node_type as usize, true);
        self.local_ids[s] = id;
        self.parents[s] = None;
        self.children[s].clear();
        self.styles[s] = self.shared.default_style();
        self.uncount_props(slot);
        self.props[s].clear();
        self.flags[s] = NodeFlags::CREATED;
        self.layout_dirty.insert(slot);
        self.frames[s] = Frame::default();
        self.contents[s] = (0.0, 0.0);
        self.intrinsic[s] = None;
        self.taffy[s] = None;
        self.is_root[s] = false;
        self.by_local.insert(id, slot);
        self.live_count += 1;
        self.revise_text(slot, true);
        Ok(slot)
    }

    pub(crate) fn free_slot(&mut self, slot: u32) {
        self.exclusion_slots.remove(slot);
        self.relative_slots.remove(slot);
        self.sticky_slots.remove(slot);
        self.layout_dirty.remove(slot);
        self.flow.remove(&slot);
        let s = slot as usize;
        debug_assert!(self.live[s], "free of a dead slot");
        self.by_local.remove(&self.local_ids[s]);
        self.live[s] = false;
        bump(&mut self.type_counts, self.node_types[s] as usize, false);
        self.uncount_props(slot);
        self.text_revisions[s] = TextRevisions::default();
        self.parents[s] = None;
        self.children[s] = Vec::new();
        self.styles[s] = self.shared.default_style();
        self.props[s] = PropList::new();
        self.flags[s] = NodeFlags::default();
        self.frames[s] = Frame::default();
        self.contents[s] = (0.0, 0.0);
        self.intrinsic[s] = None;
        self.covers.retain(|(s, _)| *s != slot);
        self.taffy[s] = None;
        if self.is_root[s] {
            self.roots.retain(|r| *r != slot);
            self.is_root[s] = false;
        }
        self.free.push(slot);
        self.live_count -= 1;
    }

    pub(crate) fn set_parent(&mut self, slot: u32, parent: Option<u32>) {
        if parent.is_some() && self.parents[slot as usize] != parent {
            self.flags[slot as usize].insert(NodeFlags::ATTACHED);
        }
        self.parents[slot as usize] = parent;
    }

    pub(crate) fn set_children(&mut self, slot: u32, children: Vec<u32>) {
        for (index, &child) in children.iter().enumerate() {
            self.child_indices[child as usize] = index;
        }
        self.children[slot as usize] = children;
    }

    /// Drop every listed child of `parent` that no longer names it as its
    /// parent, in one pass: the transaction detaches moved and destroyed
    /// children lazily and prunes each old parent once per op or run.
    pub(crate) fn prune_children(&mut self, parent: u32) {
        let parents = &self.parents;
        let list = &mut self.children[parent as usize];
        list.retain(|&c| parents[c as usize] == Some(parent));
        for (index, &child) in list.iter().enumerate() {
            self.child_indices[child as usize] = index;
        }
    }

    pub(crate) fn child_index(&self, slot: u32) -> usize {
        self.child_indices[slot as usize]
    }

    pub(crate) fn begin_layout_publication(&mut self, root: u32) {
        self.layout_passes[root as usize] += 1;
    }

    pub(crate) fn mark_geometry_changed(&mut self, slot: u32, root: u32) {
        self.flags[slot as usize].insert(NodeFlags::GEOMETRY_CHANGED);
        self.geometry_passes[slot as usize] = (root, self.layout_passes[root as usize]);
    }

    pub(crate) fn consume_layout_flags(&mut self, slot: u32) {
        self.layout_dirty.remove(slot);
        for clear in [
            NodeFlags::CREATED,
            NodeFlags::STYLE_DIRTY,
            NodeFlags::TEXT_DIRTY,
            NodeFlags::CHILDREN_DIRTY,
            NodeFlags::GEOMETRY_CHANGED,
            NodeFlags::ATTACHED,
        ] {
            self.flags[slot as usize].remove(clear);
        }
    }

    pub(crate) fn update_exclusion_count(&mut self, slot: u32, _was: bool) {
        if crate::flow::is_exclusion(self, slot) {
            self.exclusion_slots.insert(slot);
        } else {
            self.exclusion_slots.remove(slot);
        }
    }

    /// A test's write in place, copied first when shared. Production writes
    /// go through [`set_style`](Self::set_style), which keeps an unset row
    /// at its initial value, as sharing assumes.
    #[cfg(test)]
    pub(crate) fn style_mut(&mut self, slot: u32) -> &mut StyleProps {
        Rc::make_mut(&mut self.styles[slot as usize])
    }

    /// Replace the slot's style, sharing an equal one's allocation.
    pub(crate) fn set_style(&mut self, slot: u32, style: StyleProps) {
        if style.relative.is_empty() {
            self.relative_slots.remove(slot);
        } else {
            self.relative_slots.insert(slot);
        }
        if style.position_type == crate::PositionType::Sticky {
            self.sticky_slots.insert(slot);
        } else {
            self.sticky_slots.remove(slot);
        }
        self.styles[slot as usize] = self.shared.intern(style);
    }

    /// Distinct styles the arena holds for its nodes, beyond the initial one.
    pub fn shared_style_count(&self) -> usize {
        self.shared.len()
    }

    /// Set a prop; the previous value.
    pub(crate) fn set_prop(
        &mut self,
        slot: u32,
        id: PropId,
        value: PropValue,
    ) -> Option<PropValue> {
        let old = self.props[slot as usize].set(id, value);
        if old.is_none() {
            bump(&mut self.prop_counts, id as usize, true);
        }
        old
    }

    /// Clear a prop; the value it had.
    pub(crate) fn remove_prop(&mut self, slot: u32, id: PropId) -> Option<PropValue> {
        let old = self.props[slot as usize].remove(id);
        if old.is_some() {
            bump(&mut self.prop_counts, id as usize, false);
        }
        old
    }

    fn uncount_props(&mut self, slot: u32) {
        for (id, _) in self.props[slot as usize].iter() {
            bump(&mut self.prop_counts, id as usize, false);
        }
    }

    /// Whether any live node (attached or not) has prop `id`.
    pub fn has_prop(&self, id: PropId) -> bool {
        self.prop_counts.get(id as usize).is_some_and(|n| *n > 0)
    }

    /// Whether any live node (attached or not) is a `node_type`.
    pub fn has_type(&self, node_type: NodeType) -> bool {
        self.type_counts
            .get(node_type as usize)
            .is_some_and(|n| *n > 0)
    }

    pub(crate) fn flags_mut(&mut self, slot: u32) -> &mut NodeFlags {
        self.layout_dirty.insert(slot);
        self.flags[slot as usize] = self.flags(slot);
        &mut self.flags[slot as usize]
    }

    /// The content's extent from the last layout (width, height).
    pub fn content(&self, slot: u32) -> (f32, f32) {
        self.contents[slot as usize]
    }

    /// An intrinsic size, when the host has reported it.
    pub fn intrinsic(&self, slot: u32) -> Option<(f32, f32)> {
        self.intrinsic[slot as usize]
    }

    pub(crate) fn set_intrinsic(&mut self, slot: u32, size: Option<(f32, f32)>) {
        self.intrinsic[slot as usize] = size;
    }

    /// What a host container covers of a box, when it has said.
    pub fn cover(&self, slot: u32) -> Option<crate::kernel::HostCover> {
        self.covers
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, c)| *c)
    }

    pub(crate) fn set_cover(&mut self, slot: u32, cover: Option<crate::kernel::HostCover>) {
        self.covers.retain(|(s, _)| *s != slot);
        if let Some(c) = cover {
            self.covers.push((slot, c));
        }
    }

    pub(crate) fn set_content(&mut self, slot: u32, content: (f32, f32)) {
        self.contents[slot as usize] = content;
    }

    pub(crate) fn set_frame(&mut self, slot: u32, frame: Frame) {
        self.frames[slot as usize] = frame;
    }

    pub(crate) fn set_taffy(&mut self, slot: u32, node: Option<NodeId>) {
        self.taffy[slot as usize] = node;
    }

    pub(crate) fn set_root(&mut self, slot: u32, root: bool) {
        let s = slot as usize;
        if self.is_root[s] == root {
            return;
        }
        self.is_root[s] = root;
        if root {
            self.roots.push(slot);
        } else {
            self.roots.retain(|r| *r != slot);
        }
    }

    /// Drop every derived layout handle (before a rebuild).
    pub(crate) fn clear_taffy(&mut self) {
        for t in self.taffy.iter_mut() {
            *t = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_fail_closed_after_reuse() {
        let mut arena = NodeArena::new();
        let a = arena.alloc(1, NodeType::View).unwrap();
        let key_a = arena.key(a);
        assert_eq!(arena.resolve(key_a), Some(a));
        arena.free_slot(a);
        assert_eq!(arena.resolve(key_a), None);
        let b = arena.alloc(1, NodeType::Text).unwrap();
        assert_eq!(b, a, "the freed slot is reused");
        assert_ne!(arena.key(b), key_a, "with a new generation");
        assert_eq!(
            arena.resolve(key_a),
            None,
            "the stale key never aliases the new node"
        );
        assert_eq!(arena.slot_of(1), Some(b));
        assert_eq!(arena.node_type(b), NodeType::Text);
        assert_eq!(arena.live_count(), 1);
    }

    #[test]
    fn generation_zero_is_never_minted() {
        let mut arena = NodeArena::new();
        let a = arena.alloc(1, NodeType::View).unwrap();
        assert_eq!(arena.key(a).generation, 1);
    }

    #[test]
    fn subtree_and_ancestry() {
        let mut arena = NodeArena::new();
        let root = arena.alloc(1, NodeType::View).unwrap();
        let a = arena.alloc(2, NodeType::View).unwrap();
        let b = arena.alloc(3, NodeType::View).unwrap();
        let c = arena.alloc(4, NodeType::Text).unwrap();
        arena.set_children(root, vec![a, b]);
        arena.set_parent(a, Some(root));
        arena.set_parent(b, Some(root));
        arena.set_children(a, vec![c]);
        arena.set_parent(c, Some(a));
        assert_eq!(arena.subtree(root), vec![root, a, c, b]);
        assert!(arena.is_ancestor(root, c));
        assert!(!arena.is_ancestor(b, c));
        assert_eq!(arena.depth(c), 2);
        assert_eq!(arena.measure_owner(c), c);
    }

    #[test]
    fn compact_inheritance_matches_full_styles_through_overrides_and_detach() {
        use crate::style::StyleValue;
        let number = StyleValue::Number;
        let text = |s: &str| StyleValue::Text(s.into());
        let samples = [
            (StyleId::Direction, text("rtl")),
            (StyleId::CaretColor, text("light-dark(#ffffff, #112233)")),
            (StyleId::AccentColor, text("light-dark(#ff9500, #0a84ff)")),
            (StyleId::FontSize, number(24.0)),
            (StyleId::FontWeight, number(700.0)),
            (StyleId::FontStyle, text("italic")),
            (StyleId::FontFamily, number(3.0)),
            (StyleId::Cursor, text("grab")),
            (StyleId::TextAlign, text("right")),
            (StyleId::LineHeight, number(1.5)),
            (StyleId::LetterSpacing, number(2.0)),
            (StyleId::FontVariantNumeric, text("tabular-nums")),
            (StyleId::TextColor, text("light-dark(#112233, #ffffff)")),
            (StyleId::WhiteSpace, text("pre-wrap")),
            (StyleId::OverflowWrap, text("anywhere")),
            (StyleId::InterpolateSize, text("allow-keywords")),
            (StyleId::TextTransform, text("uppercase")),
            (StyleId::TextIndent, number(24.0)),
            (StyleId::Hyphens, text("auto")),
            (StyleId::TextShadow, text("1px 2px 3px #000")),
            (StyleId::TextStrokeWidth, number(2.0)),
            (StyleId::TextStrokeColor, text("#ff0000")),
            (StyleId::SymbolRendering, text("palette")),
            (StyleId::SymbolPalette, text("#ff0000 #0000ff")),
            // SVG 2 presentation properties (LLP 1055 D2).
            (StyleId::Fill, text("#16a34a")),
            (StyleId::Stroke, text("currentcolor")),
            (StyleId::StrokeWidth, number(1.5)),
            (StyleId::StrokeLinecap, text("round")),
            (StyleId::StrokeLinejoin, text("round")),
            (StyleId::StrokeMiterlimit, number(2.0)),
            (StyleId::StrokeDasharray, text("1 2")),
            (StyleId::StrokeDashoffset, number(1.0)),
            (StyleId::FillOpacity, number(0.5)),
            (StyleId::StrokeOpacity, number(0.5)),
            (StyleId::FillRule, text("evenodd")),
            (StyleId::Visibility, text("hidden")),
            (StyleId::PaintOrder, text("stroke")),
            (StyleId::ClipRule, text("evenodd")),
            (StyleId::TextAnchor, text("middle")),
            (StyleId::DominantBaseline, text("central")),
            (StyleId::PointerEvents, text("stroke")),
            (StyleId::Cursor, text("crosshair")),
            (StyleId::MarkerStart, text("url(#a)")),
            (StyleId::MarkerMid, text("url(#a)")),
            (StyleId::MarkerEnd, text("url(#a)")),
            (StyleId::ColorInterpolationFilters, text("sRGB")),
        ];
        let mut covered = StyleMask::EMPTY;
        for (id, value) in samples {
            covered.set(id);
            let mut arena = NodeArena::new();
            let root = arena.alloc(1, NodeType::View).unwrap();
            let parent = arena.alloc(2, NodeType::View).unwrap();
            let leaf = arena.alloc(3, NodeType::Text).unwrap();
            arena.set_parent(parent, Some(root));
            arena.set_parent(leaf, Some(parent));
            let mut full = arena.computed_style(leaf, StyleMask::INHERITED);
            let mut compact = arena.computed_inherited(leaf);
            for step in 0..5 {
                match step {
                    0 => arena.style_mut(root).set_dynamic(id, &value).unwrap(),
                    // Explicit initial values must override the ancestor.
                    1 => arena.style_mut(parent).mask.set(id),
                    2 => arena.style_mut(parent).clear(StyleMask::of(id)),
                    3 => arena.set_parent(leaf, None),
                    _ => arena
                        .style_mut(leaf)
                        .set_dynamic(StyleId::Width, &number(75.0))
                        .unwrap(),
                }
                let next_full = arena.computed_style(leaf, StyleMask::INHERITED);
                let next_compact = arena.computed_inherited(leaf);
                let mut full_changed = StyleMask::EMPTY;
                for row in StyleMask::INHERITED.iter() {
                    if full.get(row) != next_full.get(row) {
                        full_changed.set(row);
                    }
                }
                let expected = if step == 4 {
                    StyleMask::EMPTY
                } else {
                    StyleMask::of(id)
                };
                assert_eq!(full_changed, expected, "{id:?}, step {step}");
                assert_eq!(compact.changed_mask(&next_compact), full_changed);
                full = next_full;
                compact = next_compact;
            }
            assert_eq!(full.width, crate::style::Dimension::Points(75.0));
        }
        assert_eq!(covered, StyleMask::INHERITED);
        println!(
            "inheritance snapshot: {} bytes; full style: {} bytes",
            std::mem::size_of::<InheritedStyle>(),
            std::mem::size_of::<StyleProps>()
        );
    }

    #[test]
    fn text_runs_flatten_inline_children() {
        let mut arena = NodeArena::new();
        let p = arena.alloc(1, NodeType::Text).unwrap();
        let r1 = arena.alloc(2, NodeType::Text).unwrap();
        let r2 = arena.alloc(3, NodeType::Text).unwrap();
        arena.set_children(p, vec![r1, r2]);
        arena.set_parent(r1, Some(p));
        arena.set_parent(r2, Some(p));
        arena.set_prop(r1, PropId::Text, "Hello ".into());
        arena.set_prop(r2, PropId::Text, "world".into());
        arena.style_mut(r2).font_size = 20.0;
        let mut runs = Vec::new();
        arena.text_runs(p, &mut runs);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].text, "Hello ");
        assert_eq!(runs[1].text, "world");
        assert_eq!(runs[1].style.font_size, 20.0);
        assert!(arena.is_inline_run(r1));
        assert_eq!(arena.measure_owner(r2), p);
        // An own `text` prop suppresses the children.
        arena.set_prop(p, PropId::Text, "override".into());
        let mut runs = Vec::new();
        arena.text_runs(p, &mut runs);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "override");
    }
}

#[cfg(test)]
mod paragraph_stamp_tests {
    use super::*;
    use crate::{layout::LayoutTree, selector::SelectorIndex, txn, wire::Op};
    use std::sync::Arc;

    #[test]
    fn rollover_is_atomic_and_invalid_or_noop_batches_do_not_rotate() {
        let mut arena = NodeArena::new();
        let mut layout = LayoutTree::new();
        let mut selectors = SelectorIndex::new();
        let commit = |a: &mut NodeArena, l: &mut LayoutTree, s: &mut SelectorIndex, ops: &[Op]| {
            txn::apply(
                txn::Target {
                    arena: a,
                    layout: l,
                    selectors: s,
                },
                ops,
                0,
                0,
                0,
            )
        };
        commit(
            &mut arena,
            &mut layout,
            &mut selectors,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::Text,
                },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::Text,
                },
            ],
        )
        .unwrap();
        let a = arena.slot_of(1).unwrap();
        let b = arena.slot_of(2).unwrap();
        let old = [arena.paragraph_stamp(a), arena.paragraph_stamp(b)];
        arena.text_serial = u64::MAX - 1;
        let set = |id| Op::SetProp {
            id,
            prop: PropId::Text,
            value: "new".into(),
        };
        assert!(commit(&mut arena, &mut layout, &mut selectors, &[set(1), set(99)]).is_err());
        assert_eq!(old, [arena.paragraph_stamp(a), arena.paragraph_stamp(b)]);
        assert_eq!(arena.text_serial, u64::MAX - 1);
        commit(
            &mut arena,
            &mut layout,
            &mut selectors,
            &[Op::ClearProp {
                id: 1,
                prop: PropId::Text,
            }],
        )
        .unwrap();
        assert_eq!(old, [arena.paragraph_stamp(a), arena.paragraph_stamp(b)]);
        commit(&mut arena, &mut layout, &mut selectors, &[set(1), set(2)]).unwrap();
        for (slot, previous) in [a, b].into_iter().zip(old) {
            assert!(!previous
                .unwrap()
                .same_metrics(&arena.paragraph_stamp(slot).unwrap()));
            assert_eq!(arena.props(slot).str(PropId::Text), Some("new"));
        }
        assert_eq!(arena.text_serial, 1);
    }

    #[test]
    fn current_slot_storage_and_namespace_lifetimes_do_not_retain_history() {
        let mut arena = NodeArena::new();
        let slot = arena.alloc(1, NodeType::Text).unwrap();
        let old = arena.paragraph_stamp(slot).unwrap();
        let weak = Arc::downgrade(&arena.text_domain.0);
        let fork = arena.clone();
        assert!(!old.same_metrics(&fork.paragraph_stamp(slot).unwrap()));
        arena.reset();
        assert!(
            weak.upgrade().is_some(),
            "the caller still holds a payload-free stamp"
        );
        drop(old);
        assert!(
            weak.upgrade().is_none(),
            "neither reset nor a live public fork retains the retired namespace"
        );
        drop(fork);
        for _ in 0..500 {
            let slot = arena.alloc(1, NodeType::Text).unwrap();
            arena.free_slot(slot);
            assert_eq!(arena.text_revisions[slot as usize], Default::default());
        }
        assert_eq!(arena.text_revisions.len(), arena.slot_count());
        assert_eq!(
            arena.slot_count(),
            1,
            "high-water storage, not allocation history"
        );
        let slot = arena.alloc(1, NodeType::Text).unwrap();
        let held = arena.paragraph_stamp(slot).unwrap();
        let weak = Arc::downgrade(&arena.text_domain.0);
        drop(arena);
        assert!(weak.upgrade().is_some());
        drop(held);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn node_generation_wrap_does_not_reuse_a_paragraph_proof() {
        let mut arena = NodeArena::new();
        let slot = arena.alloc(1, NodeType::Text).unwrap();
        let old = arena.paragraph_stamp(slot).unwrap();
        arena.free_slot(slot);
        arena.generations[slot as usize] = u32::MAX;
        let reused = arena.alloc(1, NodeType::Text).unwrap();
        let new = arena.paragraph_stamp(reused).unwrap();
        assert_eq!(
            old.owner(),
            new.owner(),
            "exercise existing u32 generation wrap"
        );
        assert!(
            !old.same_metrics(&new),
            "allocation serial must still differ"
        );
    }
}

/// One more (`up`) or one fewer node in a per-id count.
fn bump(counts: &mut Vec<u32>, index: usize, up: bool) {
    if counts.len() <= index {
        counts.resize(index + 1, 0);
    }
    if up {
        counts[index] += 1;
    } else {
        counts[index] = counts[index].saturating_sub(1);
    }
}
