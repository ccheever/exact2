//! The columnar node arena.
//!
//! Every node is a slot; every attribute is a column indexed by slot. Topology
//! is index-based (`parents`, `children`), identity is generation-checked
//! ([`NodeKey`]), and destroyed slots go on a free list. Nothing here hashes on
//! the read path except the wire-id lookup a producer's op needs once.
//!
//! Rehydration rebuilds frames and Taffy handles from authored columns.

use crate::id::{BoxInset, IdMap};
use crate::shared_style::SharedStyles;
use crate::sorted::SlotSet;
use std::rc::Rc;
pub(crate) mod alert;
pub(crate) mod button;
pub(crate) mod control_text;
pub(crate) mod defaults;
mod inline_image;
mod owners;
pub mod text_style;

use taffy::NodeId;

use crate::error::ApplyError;
use crate::generated::{
    FieldSizing, InheritedStyle, NodeType, PropId, StyleId, StyleMask, StyleProps,
};
use crate::id::{Frame, NodeFlags, NodeKey, ViewId};
use crate::props::{PropList, PropValue};
use crate::style::Env;
use crate::text::{ParagraphStamp, TextDomain, TextRevisions, TextRun};

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
    /// Each root's latest publication, numbered across all roots from
    /// `last_pass` (0: none). A freed root's publications are over.
    layout_passes: Vec<u64>,
    last_pass: u64,
    /// The root and publication that last changed each node's geometry. Its
    /// `GEOMETRY_CHANGED` holds while that is still the root's latest; the
    /// root may since have been freed, reused or trimmed away, which ends it.
    geometry_passes: Vec<(u32, u64)>,
    frames: Vec<Frame>,
    // @ref LLP 1043.000 §3 D4 — no per-node vector or allocation.
    pub(crate) flow: IdMap<u32, crate::flow::FlowState>,
    pub(crate) exclusion_slots: SlotSet,
    /// @ref LLP 1093 D7 — multi-column containers and their last cuts.
    pub(crate) frag: crate::fragment::FragState,
    /// Slots whose style writes a row in `rem`/`em` (LLP 1069.000 D3).
    pub(crate) relative_slots: SlotSet,
    /// System symbols whose role or font changed: their natural size is
    /// measured again before the next layout (`Kernel::measure_symbols`).
    pub(crate) symbols_owed: SlotSet,
    /// Symbols whose natural size the kernel measured itself, for their
    /// current role and font, under the measurer's revision the kernel last
    /// measured at (`Kernel::measures_symbol`).
    pub(crate) measured_symbols: SlotSet,
    /// Slots that are `position: sticky` (LLP 1083 D3).
    pub(crate) sticky_slots: SlotSet,
    /// Slots in HTML's top layer: popovers, unless their author made them
    /// sticky (`top_layer::lifted`). Each is laid out in the viewport, a
    /// block the kernel implies (`Kernel::imply_blocks`).
    pub(crate) top_layer_slots: SlotSet,
    /// The timeline-bearing nodes and what each follower's name resolves
    /// to (LLP 1057.003 D4), kept by the linked lookup after each commit.
    pub(crate) timelines: crate::timeline::Registry,
    /// A root font size the host set since the last commit, which that
    /// commit applies (LLP 1069.000 D3).
    pub(crate) root_font_size_next: Option<f32>,
    /// The display's scale, device pixels a px, which a hairline resolves
    /// against (LLP 1115.000 D10.3): 1 until the host says (`None`), and
    /// its next commit applies what it says.
    pub(crate) display_scale: Option<f32>,
    pub(crate) display_scale_next: Option<f32>,
    /// Bold Text is on (LLP 1115.000 D12.3): the person's setting, which the
    /// host says and its next commit applies.
    pub(crate) bold_text: bool,
    pub(crate) bold_text_next: Option<bool>,
    /// Which font stacks are the system's, by id, as the plan says.
    pub(crate) system_stacks: Vec<bool>,
    /// Scrollable overflow from the last layout: the content's extent in the
    /// node's own space (width, height), Taffy's `content_size`.
    contents: Vec<(f32, f32)>,
    /// Padding and border published by the document or content-region
    /// layout: the slots it published for, and of those the few whose inset
    /// is not all zero (most boxes have neither padding nor border).
    insets: SlotSet,
    nonzero_insets: IdMap<u32, BoxInset>,
    /// Host intrinsic size: a replaced element's natural size or a projected
    /// tablist's or radio group's native control size; absent before
    /// measurement or after removal. A few nodes have one.
    intrinsic: IdMap<u32, (f32, f32)>,
    /// What a host container covers of a box (LLP 1075.003 §3.5): a few
    /// route nodes at most, so a list by slot rather than a column (and not a
    /// map, whose code every web core would carry).
    covers: Vec<(u32, crate::kernel::HostCover)>,
    /// The content owners a host published values for (LLP 1123 D9), by their root's slot:
    /// each with the environment its subtree's rows resolve against (the document's, carrying
    /// its values) and the one a top-layer box in it does, which carries only its container
    /// (D12). A few owners at most, as the covers.
    owners: Vec<(u32, Env, Env)>,
    /// The margins the platform's own list gives a grouped list's cards and
    /// cells, by the list's slot: a few lists at most, as the covers.
    grouped_margins: Vec<(u32, crate::grouped::GroupedMargins)>,
    /// The margin the platform's cell gives a cells list's rows at each end,
    /// by the list's slot (LLP 1138): a few lists at most.
    list_margins: Vec<(u32, [f32; 2])>,
    /// Alert roots whose host supplies the CSS `::before` title item. The
    /// engine derives the item from the root's live label and inherited text.
    pub(crate) alert_prefixes: SlotSet,
    /// The containing blocks hosts gave subtrees (LLP 1115.002 D8.2).
    pub(crate) blocks: crate::kernel::Blocks,
    taffy: Vec<Option<NodeId>>,
    is_root: Vec<bool>,
    free: Vec<u32>,
    roots: Vec<u32>,
    by_local: IdMap<ViewId, u32>,
    live_count: usize,
    /// The page's environment (LLP 1001 §2): what `env()` lengths resolve
    /// to. The host's, not the tree's — a reset keeps it.
    env: Env,
    control_styles: Option<Box<[StyleProps; 10]>>,
    pub(crate) field_content: IdMap<u32, Frame>,
    pub(crate) button_bases: IdMap<u32, Option<f32>>,
    /// Each node's host-known state looks (LLP 1121 D8, `Op::SetLooks`):
    /// a state and its resolved patch, ascending; a node with none has no
    /// entry.
    looks: IdMap<u32, Rc<[(u8, StyleProps)]>>,
    /// The scale each fitted paragraph's last layout chose, below 1 only
    /// (`-exact-fit: shrink`, LLP 1116.001 D1.4; `layout::fit`).
    pub(crate) fit_scales: IdMap<u32, f32>,
    pub(crate) document_language: String,
    pub(crate) document_style: StyleProps,
    /// A clone's wide colours, kept while it lives apart from a kernel:
    /// let go once a kernel holding it publishes its own mark.
    pub(crate) kept: crate::style::wide::Keep,
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
            last_pass: self.last_pass,
            geometry_passes: self.geometry_passes.clone(),
            frames: self.frames.clone(),
            flow: self.flow.clone(),
            exclusion_slots: self.exclusion_slots.clone(),
            frag: self.frag.clone(),
            relative_slots: self.relative_slots.clone(),
            symbols_owed: self.symbols_owed.clone(),
            measured_symbols: self.measured_symbols.clone(),
            sticky_slots: self.sticky_slots.clone(),
            top_layer_slots: self.top_layer_slots.clone(),
            timelines: self.timelines.clone(),
            root_font_size_next: self.root_font_size_next,
            display_scale: self.display_scale,
            display_scale_next: self.display_scale_next,
            bold_text: self.bold_text,
            bold_text_next: self.bold_text_next,
            system_stacks: self.system_stacks.clone(),
            contents: self.contents.clone(),
            insets: self.insets.clone(),
            nonzero_insets: self.nonzero_insets.clone(),
            intrinsic: self.intrinsic.clone(),
            covers: self.covers.clone(),
            owners: self.owners.clone(),
            grouped_margins: self.grouped_margins.clone(),
            list_margins: self.list_margins.clone(),
            alert_prefixes: self.alert_prefixes.clone(),
            blocks: self.blocks.clone(),
            taffy: self.taffy.clone(),
            is_root: self.is_root.clone(),
            free: self.free.clone(),
            roots: self.roots.clone(),
            by_local: self.by_local.clone(),
            live_count: self.live_count,
            env: self.env.clone(),
            control_styles: self.control_styles.clone(),
            field_content: self.field_content.clone(),
            button_bases: self.button_bases.clone(),
            looks: self.looks.clone(),
            fit_scales: self.fit_scales.clone(),
            document_language: self.document_language.clone(),
            document_style: self.document_style.clone(),
            text_revisions: vec![TextRevisions::default(); self.text_revisions.len()],
            text_domain: TextDomain::default(),
            text_serial: 0,
            kept: crate::style::wide::Keep::of(|m| self.wide_colors_shared(m)),
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
        self.rebase_owners(&env);
        self.env = env;
        self.refresh_control_styles();
    }

    /// Forget every allocation while retaining each slot's generation.
    ///
    /// Resetting the kernel must not make a pre-reset [`NodeKey`] valid for a
    /// later allocation at the same slot. Keeping the generation column means
    /// [`alloc`](Self::alloc) advances identity exactly as ordinary reuse does.
    pub(crate) fn reset(&mut self) {
        self.renew_text_namespace();
        self.field_content.clear();
        self.button_bases.clear();
        self.looks.clear();
        self.fit_scales.clear();
        self.flow.clear();
        self.exclusion_slots.clear();
        self.frag = Default::default();
        self.relative_slots.clear();
        self.symbols_owed.clear();
        self.measured_symbols.clear();
        self.sticky_slots.clear();
        self.top_layer_slots.clear();
        self.timelines = Default::default();
        for slot in 0..self.live.len() {
            self.live[slot] = false;
            self.parents[slot] = None;
            self.children[slot].clear();
            self.styles[slot] = self.shared.default_style();
            self.props[slot].clear();
            self.flags[slot] = NodeFlags::default();
            self.layout_passes[slot] = 0;
            self.prop_counts.clear();
            self.type_counts.clear();
            self.frames[slot] = Frame::default();
            self.contents[slot] = (0.0, 0.0);
            self.taffy[slot] = None;
            self.is_root[slot] = false;
        }
        self.insets.clear();
        self.nonzero_insets.clear();
        self.intrinsic.clear();
        self.covers.clear();
        self.owners.clear();
        self.grouped_margins.clear();
        self.list_margins.clear();
        self.alert_prefixes.clear();
        self.blocks = Default::default();
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

    /// The node's host-known state looks (LLP 1121 D8), ascending by
    /// state; empty for a node with none.
    pub fn looks(&self, slot: u32) -> &[(u8, StyleProps)] {
        self.looks.get(&slot).map_or(&[], |l| l)
    }

    /// Replace the node's looks; whether they changed.
    pub(crate) fn set_looks(&mut self, slot: u32, looks: &[(u8, StyleProps)]) -> bool {
        if self.looks(slot) == looks {
            return false;
        }
        if looks.is_empty() {
            self.looks.remove(&slot);
        } else {
            self.looks.insert(slot, looks.into());
        }
        true
    }

    /// Props.
    pub fn props(&self, slot: u32) -> &PropList {
        &self.props[slot as usize]
    }

    /// Dirty flags.
    pub fn flags(&self, slot: u32) -> NodeFlags {
        let mut flags = self.flags[slot as usize];
        let (root, pass) = self.geometry_passes[slot as usize];
        // A root trimmed away is past the column's end.
        if self.layout_passes.get(root as usize) != Some(&pass) {
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

    /// A copy for a trial layout that is dropped after it
    /// (`Kernel::measure_block`): the same text, so each paragraph keeps its
    /// stamp, and a measurer that holds paragraphs by stamp answers the trial
    /// from what it holds instead of taking the text for new (a plain clone
    /// is another kernel's, in a namespace of its own).
    pub(crate) fn trial_copy(&self) -> Self {
        let mut copy = self.clone();
        copy.text_revisions.clone_from(&self.text_revisions);
        copy.text_domain = self.text_domain.clone();
        copy.text_serial = self.text_serial;
        copy
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

    /// A Markdown leaf's atom kinds and faces (LLP 1045.000 D2); empty for
    /// any other node.
    pub fn markup_atoms(&self, slot: u32) -> crate::text::MarkupAtoms<'_> {
        let props = &self.props[slot as usize];
        if self.markup(slot) != crate::text::Markup::Markdown {
            return crate::text::MarkupAtoms::default();
        }
        crate::text::MarkupAtoms {
            kinds: props.str(PropId::Atoms).unwrap_or(""),
            faces: props.str(PropId::AtomFaces).unwrap_or(""),
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
        self.paragraph_at(slot, self.fit_scale(slot))
    }

    /// The scale a fitted paragraph is drawn at, as its last layout chose
    /// it (LLP 1116.001 D1.4): 1 for any other node. An inline run's is its
    /// paragraph's.
    pub fn fit_scale(&self, slot: u32) -> f32 {
        if self.fit_scales.is_empty() {
            return 1.0;
        }
        let owner = self.measure_owner(slot);
        self.fit_scales.get(&owner).copied().unwrap_or(1.0)
    }

    /// [`Self::paragraph`] at `scale` of its size, its strut included.
    pub(crate) fn paragraph_at(&self, slot: u32, scale: f32) -> crate::text::Paragraph {
        let mut paragraph =
            crate::text::Paragraph::from_rows(&self.styles[slot as usize], |rows, take| {
                self.copy_inherited(slot, rows, take)
            });
        paragraph.markup = self.markup(slot);
        // Bold Text, as its runs are (LLP 1115.000 D12.3).
        self.embolden(slot, &mut paragraph.strut);
        if scale != 1.0 {
            paragraph.strut = paragraph.strut.scaled(scale);
        }
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
    /// A fitted paragraph's runs are at the scale its layout chose.
    pub fn text_runs<'a>(&'a self, slot: u32, out: &mut Vec<TextRun<'a>>) {
        self.text_runs_at(slot, self.fit_scale(slot), out);
    }

    pub(crate) fn reservation_text<'a>(
        &self,
        slot: u32,
        text: &'a str,
    ) -> std::borrow::Cow<'a, str> {
        let rows = crate::text::RunRows::from_rows(self.style(slot), |rows, take| {
            self.copy_inherited(slot, rows, take)
        });
        rows.hyphens.shown(crate::text::case::shown(
            rows.transform,
            text,
            Default::default(),
        ))
    }

    /// [`Self::text_runs`] at `scale` of their size (a fit's measurement).
    pub(crate) fn text_runs_at<'a>(&'a self, slot: u32, scale: f32, out: &mut Vec<TextRun<'a>>) {
        let start = out.len();
        let rows = crate::text::RunRows::from_rows(&self.styles[slot as usize], |rows, take| {
            self.copy_inherited(slot, rows, take)
        });
        self.text_runs_after(
            slot,
            rows,
            out,
            &mut crate::text::case::WordBoundary::default(),
        );
        if scale != 1.0 {
            for run in &mut out[start..] {
                run.style = run.style.scaled(scale);
            }
        }
    }

    fn text_runs_after<'a>(
        &'a self,
        slot: u32,
        rows: crate::text::RunRows,
        out: &mut Vec<TextRun<'a>>,
        boundary: &mut crate::text::case::WordBoundary,
    ) {
        let s = slot as usize;
        match self.node_types[s] {
            NodeType::TextInput => {
                let mut style = rows.style();
                self.embolden(slot, &mut style);
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
                    inline: None,
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
                        inline: None,
                    });
                }
            }
            _ => {
                if let Some(text) = self.props[s].str(PropId::Text) {
                    let shown = if self.markup(slot) == crate::text::Markup::Markdown {
                        text.into()
                    } else {
                        rows.hyphens.shown(crate::text::case::shown(
                            rows.transform,
                            text,
                            *boundary,
                        ))
                    };
                    boundary.push(text);
                    let mut style = rows.style();
                    // Bold Text (LLP 1115.000 D12.3), as `text_style` reads it.
                    self.embolden(slot, &mut style);
                    out.push(TextRun {
                        text: shown,
                        style,
                        inline: None,
                    });
                } else {
                    for child in &self.children[s] {
                        match self.node_types[*child as usize] {
                            NodeType::Text => {
                                let rows = self.run_style_rows(*child, rows);
                                self.text_runs_after(*child, rows, out, boundary);
                            }
                            // Its alignment reads its parent's font (CSS 2
                            // §10.8.1), so its run is in the paragraph's style.
                            NodeType::Image if self.is_run_image(*child) => {
                                out.push(TextRun {
                                    text: crate::text::inline_box::INLINE_BOX_TEXT.into(),
                                    style: rows.style(),
                                    inline: Some(
                                        crate::text::inline_box::inline_image(self, *child)
                                            .reserved,
                                    ),
                                });
                            }
                            _ => {}
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
        let transform = self
            .computed_source(slot, StyleId::TextTransform)
            .text_transform;
        let hyphens = self.computed_source(slot, StyleId::Hyphens).hyphens;
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

    /// The leaves [`Self::text_runs`] makes a run of, in its order: its
    /// texts, and its images (LLP 1011.002).
    pub fn text_leaves(&self, slot: u32, out: &mut Vec<u32>) {
        if self.props[slot as usize].str(PropId::Text).is_some() {
            return out.push(slot);
        }
        for child in &self.children[slot as usize] {
            match self.node_types[*child as usize] {
                NodeType::Text => self.text_leaves(*child, out),
                NodeType::Image if self.is_run_image(*child) => out.push(*child),
                _ => {}
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
        self.set_inset(slot, None);
        self.intrinsic.remove(&slot);
        self.taffy[s] = None;
        self.is_root[s] = false;
        self.by_local.insert(id, slot);
        self.live_count += 1;
        self.revise_text(slot, true);
        Ok(slot)
    }

    pub(crate) fn free_slot(&mut self, slot: u32) {
        self.field_content.remove(&slot);
        self.exclusion_slots.remove(slot);
        self.frag.forget(slot);
        self.relative_slots.remove(slot);
        self.symbols_owed.remove(slot);
        self.measured_symbols.remove(slot);
        self.button_bases.remove(&slot);
        self.looks.remove(&slot);
        self.fit_scales.remove(&slot);
        self.sticky_slots.remove(slot);
        self.top_layer_slots.remove(slot);
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
        self.layout_passes[s] = 0;
        self.frames[s] = Frame::default();
        self.contents[s] = (0.0, 0.0);
        self.set_inset(slot, None);
        self.intrinsic.remove(&slot);
        self.covers.retain(|(s, _)| *s != slot);
        self.owners.retain(|(s, _, _)| *s != slot);
        self.grouped_margins.retain(|(s, _)| *s != slot);
        self.list_margins.retain(|(s, _)| *s != slot);
        self.alert_prefixes.remove(slot);
        self.blocks.remove(slot);
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

    /// The latest publication's number across all roots.
    pub(crate) fn last_pass(&self) -> u64 {
        self.last_pass
    }

    pub(crate) fn begin_layout_publication(&mut self, root: u32) {
        self.last_pass += 1;
        self.layout_passes[root as usize] = self.last_pass;
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
        let style = self.shared.intern(style);
        self.set_shared_style(slot, style);
    }

    /// Patch a slot without copying a style another node already holds.
    pub(crate) fn patch_style(&mut self, slot: u32, patch: &StyleProps) {
        let style = self.shared.patch(&self.styles[slot as usize], patch);
        self.set_shared_style(slot, style);
    }

    /// Clear rows without copying a style another node already holds.
    pub(crate) fn clear_style(&mut self, slot: u32, mask: crate::StyleMask, written: bool) {
        let style = self
            .shared
            .clear(&self.styles[slot as usize], mask, written);
        self.set_shared_style(slot, style);
    }

    fn set_shared_style(&mut self, slot: u32, style: Rc<StyleProps>) {
        if style.relative.is_empty() {
            self.relative_slots.remove(slot);
        } else {
            self.relative_slots.insert(slot);
        }
        if crate::fragment::is_multicol(&style) {
            self.frag.containers.insert(slot);
        } else {
            self.frag.containers.remove(slot);
        }
        self.styles[slot as usize] = style;
        self.refresh_sticky(slot);
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

    /// Paint and prop export flags do not invalidate layout publication.
    /// Their callers mark any changed geometry or measure owner separately.
    pub(crate) fn mark_export_dirty(&mut self, slot: u32, flag: NodeFlags) {
        debug_assert!(flag == NodeFlags::PAINT_DIRTY || flag == NodeFlags::PROPS_DIRTY);
        self.flags[slot as usize] = self.flags(slot);
        self.flags[slot as usize].insert(flag);
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
        self.intrinsic.get(&slot).copied()
    }

    pub(crate) fn set_intrinsic(&mut self, slot: u32, size: Option<(f32, f32)>) {
        match size {
            Some(size) => self.intrinsic.insert(slot, size),
            None => self.intrinsic.remove(&slot),
        };
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

    /// The margins the platform's list gives the grouped list `slot`.
    pub(crate) fn grouped_margins(&self, slot: u32) -> Option<crate::grouped::GroupedMargins> {
        self.grouped_margins
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, m)| *m)
    }

    pub(crate) fn has_grouped_margins(&self) -> bool {
        !self.grouped_margins.is_empty()
    }

    /// The margin the platform's cell gives the cells list `slot`'s rows.
    pub(crate) fn list_margins(&self, slot: u32) -> Option<[f32; 2]> {
        self.list_margins
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, m)| *m)
    }

    pub(crate) fn set_list_margins(&mut self, slot: u32, margins: Option<[f32; 2]>) {
        self.list_margins.retain(|(s, _)| *s != slot);
        self.list_margins.extend(margins.map(|m| (slot, m)));
    }

    pub(crate) fn set_grouped_margins(
        &mut self,
        slot: u32,
        margins: Option<crate::grouped::GroupedMargins>,
    ) {
        self.grouped_margins.retain(|(s, _)| *s != slot);
        self.grouped_margins.extend(margins.map(|m| (slot, m)));
    }

    pub(crate) fn set_content(&mut self, slot: u32, content: (f32, f32)) {
        self.contents[slot as usize] = content;
    }

    pub(crate) fn set_inset(&mut self, slot: u32, inset: Option<BoxInset>) {
        match inset {
            None => {
                self.insets.remove(slot);
                self.nonzero_insets.remove(&slot);
            }
            Some(inset) => {
                self.insets.insert(slot);
                // Bitwise, so a -0 comes back as it went in.
                let zero = |v: &[f32; 4]| v.iter().all(|x| x.to_bits() == 0);
                if zero(&inset.padding) && zero(&inset.border) && zero(&inset.cover) {
                    self.nonzero_insets.remove(&slot);
                } else {
                    self.nonzero_insets.insert(slot, inset);
                }
            }
        }
    }

    pub(crate) fn inset(&self, slot: u32) -> Option<BoxInset> {
        let published = self.insets.contains(slot);
        published.then(|| (self.nonzero_insets.get(&slot).copied()).unwrap_or_default())
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
        fn text(s: &str) -> StyleValue<'_> {
            StyleValue::Text(s.into())
        }
        let samples = [
            (StyleId::Direction, text("rtl")),
            (StyleId::CaretColor, text("light-dark(#ffffff, #112233)")),
            (StyleId::AccentColor, text("light-dark(#ff9500, #0a84ff)")),
            (StyleId::FontSize, number(24.0)),
            (StyleId::FontWeight, number(700.0)),
            (StyleId::FontStyle, text("italic")),
            (StyleId::FontStretch, text("75%")),
            (StyleId::FontVariationSettings, text(r#""GRAD" 150"#)),
            (StyleId::FontOpticalSizing, text("none")),
            (StyleId::FontFamily, number(3.0)),
            (StyleId::Cursor, text("grab")),
            (StyleId::TextAlign, text("right")),
            (StyleId::LineHeight, number(1.5)),
            (StyleId::LetterSpacing, number(2.0)),
            (StyleId::FontVariantNumeric, text("tabular-nums")),
            (StyleId::TextColor, text("light-dark(#112233, #ffffff)")),
            (StyleId::WhiteSpace, text("pre-wrap")),
            (StyleId::DynamicRangeLimit, text("constrained")),
            (StyleId::ColorScheme, text("dark")),
            // LLP 1121 D5: the app tuple, plus one.
            (StyleId::Mode, number(2.0)),
            (StyleId::OverflowWrap, text("anywhere")),
            (StyleId::InterpolateSize, text("allow-keywords")),
            (StyleId::TextTransform, text("uppercase")),
            (StyleId::TextIndent, number(24.0)),
            (StyleId::Hyphens, text("auto")),
            // LLP 1093 D4.
            (StyleId::Widows, number(4.0)),
            (StyleId::Orphans, number(3.0)),
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
            (StyleId::TextBoxEdge, text("cap-alphabetic")),
            (StyleId::TextWrapStyle, text("balance")),
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
mod paragraph_stamp_tests;

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

impl NodeArena {
    /// Mark the colours its styles hold: the shared table's, which every
    /// node's style is one of, and the control and document styles.
    pub(crate) fn wide_colors(&mut self, m: &mut crate::style::wide::WideMark) {
        use crate::style::wide::Colors;
        self.shared.wide_colors(m);
        if let Some(control) = &self.control_styles {
            control.colors(m);
        }
        self.document_style.colors(m);
    }

    /// The same, visiting every style (for a clone, once).
    fn wide_colors_shared(&self, m: &mut crate::style::wide::WideMark) {
        use crate::style::wide::Colors;
        self.shared.visit(m);
        if let Some(control) = &self.control_styles {
            control.colors(m);
        }
        self.document_style.colors(m);
    }
}
