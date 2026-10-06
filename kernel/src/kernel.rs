//! The kernel facade: one object, one write path, columnar reads.
//!
//! A [`Kernel`] owns a tree and its derived layout. Producers mutate it through
//! [`Kernel::apply_frame`] (EXWF bytes) or [`Kernel::apply`] (in-process ops);
//! both share the validate-then-apply engine. Hosts read it through receipts,
//! the typed rows, or the EXNODE envelope. It is single-owner and adds no
//! threads.

use std::collections::VecDeque;
use std::sync::LazyLock;

use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::export::{self, NodeRow};
use crate::generated::{BoxSizing, Display, NodeType, PropId, StyleId, StyleMask, StyleProps};
use crate::id::{Frame, NodeFlags, NodeKey, Offer, ViewId};
use crate::layout::{self, LayoutMirror, LayoutReceipt, LayoutTree, Unmirrored};
use crate::props::PropList;
use crate::selector::SelectorIndex;
use crate::style::{uses_env, ColorValue, Dimension, Env, Rect, RowValue};
use crate::text::{MonospaceMeasurer, TextMeasurer, TextRun, TextStyle};

/// The initial value of every row: what a computed read returns when neither
/// the node nor an ancestor sets an inherited row.
static INITIAL: LazyLock<StyleProps> = LazyLock::new(StyleProps::default);
use crate::txn::{self, CommitReceipt, Target};
use crate::wire::{self, Op};
mod cover;
mod document;
mod geometry;
mod intrinsic;
mod sticky;
mod trim;

pub use cover::HostCover;
pub(crate) use cover::{children_changed as cover_children_changed, header_inset};
pub use sticky::StickyConstraint;

/// How many receipts the kernel retains for late readers.
pub const RECEIPT_RING: usize = 64;

/// One sampled CSS height for a live numeric-height box or border-box auto height.
/// This replaces only derived layout height, respecting current box sizing,
/// min/max constraints and aspect ratio. It never authors a style or a commit.
/// Runtime/engine identity remains the caller's responsibility, as with NodeKey.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentedHeight {
    /// The live generational allocation whose height is presented.
    pub node: NodeKey,
    /// The current authored kernel epoch; refresh after unrelated commits too.
    pub epoch: u64,
    /// Nonnegative finite CSS height in logical pixels (not border-box height).
    pub px: f32,
}

/// One node, borrowed.
#[derive(Debug, Clone, Copy)]
pub struct NodeRef<'a> {
    /// Wire id.
    pub id: ViewId,
    /// Generation-checked key.
    pub key: NodeKey,
    /// Type.
    pub node_type: NodeType,
    /// Parent wire id.
    pub parent: Option<ViewId>,
    /// Style rows.
    pub style: &'a StyleProps,
    /// Props.
    pub props: &'a PropList,
    /// Absolute frame from the last layout.
    pub frame: Frame,
    /// Scrollable overflow from the last layout: the content's extent in the
    /// node's own space (width, height) — what a scroll container's document
    /// is sized to. Includes padding and every descendant's overflow, as CSS's
    /// `scrollWidth`/`scrollHeight` do.
    pub content: (f32, f32),
    /// Whether the node is a root.
    pub is_root: bool,
    pub(crate) arena: &'a NodeArena,
    pub(crate) slot: u32,
}

impl<'a> NodeRef<'a> {
    /// Resolved exclusions in leaf border-box coordinates, in document order.
    /// @ref LLP 1043.000 §3 D4 — derived geometry, never paragraph inputs.
    pub fn flow_shapes(&self) -> &'a [exact_textflow::FlowShape] {
        self.arena.flow_shapes(self.slot)
    }

    /// Why this auto-height leaf keeps ordinary layout beside an exclusion.
    pub fn flow_refusal(&self) -> Option<crate::FlowRefusal> {
        self.arena.flow_refusal(self.slot)
    }

    /// Current input identity for an independent Text/TextInput paragraph.
    /// Inline children return None; use the owner's runs and stamp together.
    /// This is not a layout-offer, catalog, attachment or publication proof.
    pub fn paragraph_stamp(&self) -> Option<crate::text::ParagraphStamp> {
        self.arena.paragraph_stamp(self.slot)
    }

    /// The node whose own row supplies `id` here: this node when it sets the
    /// row; for a row the schema marks inherited, the nearest logical ancestor
    /// that does; `None` when the initial value applies (LLP 1035.000 D1).
    /// Authored presence stays in `style.mask`; this is where a computed
    /// value came from.
    pub fn source_of(&self, id: StyleId) -> Option<ViewId> {
        self.arena
            .inherited_source(self.slot, id)
            .map(|s| self.arena.local_id(s))
    }

    /// CSS's computed value of a row: the own row; else, for an inherited
    /// row, the nearest logical ancestor's; else the initial value.
    pub fn computed(&self, id: StyleId) -> RowValue<'a> {
        match self.arena.inherited_source(self.slot, id) {
            Some(s) => self.arena.style(s).get(id),
            None if self.arena.document_style.mask.has(id) => self.arena.document_style.get(id),
            None => INITIAL.get(id),
        }
    }

    /// The node's rows with the inherited rows in `rows` resolved through the
    /// logical ancestors (`NodeArena::computed_style`): what a host paints
    /// and measures with.
    pub fn computed_style(&self, rows: StyleMask) -> StyleProps {
        self.arena.computed_style(self.slot, rows)
    }

    /// One row of [`NodeRef::computed_style`], read where it is set
    /// (`NodeArena::computed_source`), without copying a style.
    pub fn computed_row<T>(&self, id: StyleId, read: impl FnOnce(&StyleProps) -> T) -> T {
        read(self.arena.computed_source(self.slot, id))
    }

    /// The run style this node's text measures and paints with: its own text
    /// rows, else its paragraph's, else the initial values.
    pub fn text_style(&self) -> TextStyle {
        self.arena.text_style(self.slot)
    }

    /// The nearest explicit HTML spelling-check hint in the logical tree.
    /// Empty means true; missing/invalid values inherit. None leaves the
    /// editor's platform/user default in charge, without changing authored props.
    pub fn spellcheck(&self) -> Option<bool> {
        let mut slot = Some(self.slot);
        while let Some(current) = slot {
            if let Some(value) = self.arena.props(current).str(PropId::Spellcheck) {
                if value.is_empty() || value.eq_ignore_ascii_case("true") {
                    return Some(true);
                }
                if value.eq_ignore_ascii_case("false") {
                    return Some(false);
                }
            }
            slot = self.arena.parent(current);
        }
        None
    }

    /// CSS `color`, computed: the nearest declared value through the logical
    /// tree, a light/dark pair kept intact for the painting host to resolve.
    /// One instance of [`NodeRef::computed`].
    pub fn text_color(&self) -> ColorValue {
        match self.computed(StyleId::TextColor) {
            RowValue::ColorValue(c) => c,
            _ => unreachable!("text_color is a colour row"),
        }
    }

    /// The colour scheme this node's subtree asks for (LLP 1034 §8): its
    /// computed `color-scheme`, `None` for `normal`, the surrounding one.
    pub fn color_scheme_dark(&self) -> Option<bool> {
        match self.computed_row(StyleId::ColorScheme, |s| s.color_scheme) {
            crate::ColorScheme::Normal => None,
            crate::ColorScheme::Light => Some(false),
            crate::ColorScheme::Dark => Some(true),
        }
    }

    /// Whether this text node is an inline run owned by a Text parent.
    pub fn is_inline_run(&self) -> bool {
        self.arena.is_inline_run(self.slot)
    }

    /// The canonical ordered runs used to measure this paragraph. Inline
    /// descendants have no independent boxes; painting uses the owner's
    /// content width and these same inherited metric styles.
    pub fn text_runs(&self) -> Vec<TextRun<'a>> {
        let mut runs = Vec::new();
        self.arena.text_runs(self.slot, &mut runs);
        runs
    }

    /// Child wire ids, in order.
    pub fn children(&self) -> Vec<ViewId> {
        self.arena
            .children(self.slot)
            .iter()
            .map(|c| self.arena.local_id(*c))
            .collect()
    }
}

/// The kernel.
pub struct Kernel {
    arena: NodeArena,
    /// The layout engine, when it mirrors the arena (LLP 1047 D4). Boxed only
    /// on the layout path, so a kernel that never lays out links no engine.
    layout: Option<Box<dyn LayoutMirror>>,
    measurer: Box<dyn TextMeasurer>,
    pub(crate) selectors: SelectorIndex,
    epoch: u64,
    incarnation: u64,
    receipts: VecDeque<CommitReceipt>,
    region: Option<crate::region::RegionState>,
    region_leases: crate::region::RegionLeases,
    /// A kernel with layout on demand builds the engine only when a layout
    /// is first asked for, and again only then after a reset.
    on_demand: bool,
    /// Keeps nothing ([`Kernel::detached`]).
    detached: bool,
}

/// The engine tree, which a layout path has made sure of with `mirror`.
fn engine(layout: &mut Option<Box<dyn LayoutMirror>>) -> &mut LayoutTree {
    layout
        .as_deref_mut()
        .and_then(LayoutMirror::tree)
        .expect("the layout path mirrors the arena")
}

impl Kernel {
    /// Nodes the layout engine holds: the live nodes, when every removal
    /// reached it.
    #[cfg(test)]
    pub(crate) fn engine_nodes(&self) -> usize {
        self.layout
            .as_deref()
            .and_then(LayoutMirror::tree_ref)
            .map_or(0, LayoutTree::node_count)
    }

    /// The engine tree (tests read its counters and presentation).
    #[cfg(test)]
    pub(crate) fn tree(&self) -> &LayoutTree {
        self.layout
            .as_deref()
            .and_then(LayoutMirror::tree_ref)
            .expect("a mirrored kernel")
    }

    /// The engine tree, mutably.
    #[cfg(test)]
    pub(crate) fn tree_mut(&mut self) -> &mut LayoutTree {
        engine(&mut self.layout)
    }

    /// A kernel with the given host text measurer.
    pub fn new(measurer: Box<dyn TextMeasurer>) -> Self {
        let mut kernel = Self::on_demand(measurer);
        kernel.layout = Some(Box::new(LayoutTree::new()));
        kernel.on_demand = false;
        kernel
    }

    /// A kernel keeping no layout engine tree until a layout is first asked
    /// for, when it builds one from the arena. For a host whose platform lays
    /// out (the browser): commits then create no engine nodes and derive no
    /// engine styles, and a kernel that never lays out links no engine code.
    /// Layout, when asked for, is the same.
    pub fn on_demand(measurer: Box<dyn TextMeasurer>) -> Self {
        Kernel {
            arena: NodeArena::new(),
            layout: None,
            measurer,
            selectors: SelectorIndex::new(),
            epoch: 0,
            incarnation: 1,
            receipts: VecDeque::new(),
            region: None,
            region_leases: Default::default(),
            on_demand: true,
            detached: false,
        }
    }

    /// A kernel with the deterministic reference measurer.
    pub fn with_monospace() -> Self {
        Self::new(Box::new(MonospaceMeasurer::default()))
    }

    /// [`Kernel::on_demand`] with the deterministic reference measurer.
    pub fn with_monospace_on_demand() -> Self {
        Self::on_demand(Box::new(MonospaceMeasurer::default()))
    }

    /// Build the engine's tree when it doesn't yet mirror the arena.
    fn mirror(&mut self) {
        if self.layout.is_none() {
            self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
        }
    }

    /// The published epoch: bumps on every commit that changed something.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The kernel incarnation: bumps on every [`Kernel::reset`]. Keys from an
    /// earlier incarnation never resolve.
    pub fn incarnation(&self) -> u64 {
        self.incarnation
    }

    /// Live node count.
    pub fn live_count(&self) -> usize {
        self.arena.live_count()
    }

    /// The arena, for readers that want the columns directly.
    pub fn arena(&self) -> &NodeArena {
        &self.arena
    }

    /// The arena, the engine tree and the measurer, for the multicol probe.
    #[cfg(test)]
    pub(crate) fn parts(&mut self) -> (&NodeArena, &LayoutTree, &mut dyn TextMeasurer) {
        let tree = self.layout.as_deref().and_then(LayoutMirror::tree_ref);
        (&self.arena, tree.expect("laid out"), self.measurer.as_mut())
    }

    /// Root wire ids in attach order.
    pub fn roots(&self) -> Vec<ViewId> {
        self.arena
            .roots()
            .iter()
            .map(|r| self.arena.local_id(*r))
            .collect()
    }

    /// Register one explicitly sized content region. No schema/authoring change.
    /// Ordinary layout is refused while registered so hosts cannot bypass the
    /// selected publication and accidentally measure/paint pending live source.
    pub fn set_content_region(
        &mut self,
        binding: Option<crate::ContentRegion>,
    ) -> Result<bool, KernelError> {
        self.set_content_region_profile(binding, crate::region::RegionProfile::PinnedOffers)
    }

    /// Explicit kernel count policy. SplitFacts callers must understand request
    /// purpose and separately admit native bytes/external retained owners.
    /// Existing registration keeps PinnedOffers64 and all its artifact semantics.
    pub fn set_content_region_profile(
        &mut self,
        binding: Option<crate::ContentRegion>,
        profile: crate::region::RegionProfile,
    ) -> Result<bool, KernelError> {
        self.mirror();
        if binding.is_some() && engine(&mut self.layout).has_presented_height() {
            return Err(LayoutError::ContentRegion(
                "clear the presented height before region registration",
            )
            .into());
        }
        if self.region.as_ref().map(|r| (r.binding, r.profile)) == binding.map(|b| (b, profile)) {
            return Ok(false);
        }
        if binding.is_some()
            && profile == crate::region::RegionProfile::PinnedOffers
            && self.region_leases.has_live()
        {
            return Err(
                LayoutError::ContentRegion("split leases prevent profile downgrade").into(),
            );
        }
        let replacing = self.region.is_some();
        let next = binding
            .map(|b| {
                crate::region::RegionState::new(&self.arena, b, profile, self.region_leases.clone())
            })
            .transpose()?;
        self.region = next;
        // The previous region cut its owner's ordinary child edge. Restore
        // current authored topology when replacing or removing it, only AFTER
        // the new binding passed preflight. Invalid replacement leaves it intact.
        if replacing || binding.is_none() {
            self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
        }
        Ok(true)
    }

    /// Publish the shell and attempt one UI-owned content layout. A miss returns
    /// a usable shell plus an explicit selected branch, never fake final metrics.
    pub fn compute_region_layout(
        &mut self,
        root: ViewId,
        offer: Offer,
        inputs: crate::RegionInputs,
    ) -> Result<crate::RegionLayoutReceipt, KernelError> {
        self.mirror();
        if !offer.is_finite() {
            return Err(LayoutError::InvalidOffer.into());
        }
        let slot = self
            .arena
            .slot_of(root)
            .ok_or(LayoutError::UnknownView(root))?;
        if !self.arena.is_root(slot) {
            return Err(LayoutError::NotARoot(root).into());
        }
        self.replace_env(self.arena.env().with_viewport(offer))?;
        let region = self
            .region
            .as_mut()
            .ok_or(LayoutError::ContentRegion("no registered region"))?;
        let result = region.compute(
            &mut self.arena,
            engine(&mut self.layout),
            self.measurer.as_mut(),
            (slot, offer),
            inputs,
            self.epoch,
        );
        if result.is_err() {
            // A numeric callback error can have cached its containment zero in
            // the shell too. No failed derived cache is reused on recovery.
            self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
        }
        Ok(result?)
    }

    /// Bounded kernel-owned source/offer retention, separately from native heap.
    pub fn region_retention(&self) -> crate::region::RegionRetention {
        self.region
            .as_ref()
            .map_or_else(Default::default, |r| r.retention())
    }

    /// At most one immutable first-missing request. Copying shares its snapshot.
    pub fn region_text_request(&self) -> Option<&crate::RegionTextRequest> {
        self.region.as_ref()?.pending.as_ref()
    }

    /// Deliver an exact answer. Default/final-paint requests retain their
    /// source/shape owner; explicit split measurements release the payload.
    /// Final-paint metrics must exactly match the prior scalar fact.
    /// Stale/duplicate delivery returns false before metric validation. The host
    /// must budget opaque allocations; this API bounds their number, not heap.
    pub fn resolve_region_text(
        &mut self,
        request: &crate::RegionTextRequest,
        metrics: crate::TextMetrics,
        artifact: std::rc::Rc<dyn std::any::Any>,
    ) -> Result<bool, KernelError> {
        match &mut self.region {
            Some(r) => Ok(r.resolve(request, metrics, artifact)?),
            None => Ok(false),
        }
    }

    /// Lay out one root under an offer and publish frames. The receipt names
    /// every node whose frame changed.
    pub fn compute_layout(
        &mut self,
        root: ViewId,
        offer: Offer,
    ) -> Result<LayoutReceipt, KernelError> {
        self.compute_layout_presented(root, offer, &[])
    }

    /// Lay out with cached derived heights; an empty slice clears all owners.
    /// Preflight is atomic with respect to the previous projection/publication.
    /// Equal samples reuse layout caches; central authored style writes refresh
    /// every other field while retaining the sample. Clearing/switching restores
    /// current authored lowering, never a saved target. Publication stays per
    /// root: clearing a projection in another root dirties that root for its next
    /// layout but does not publish it in this receipt.
    ///
    /// Matches ordinary authored lowering: a root keeps its authored box sizing.
    ///
    /// Auto height requires border-box sizing; percent, env, negative, hidden
    /// and inline heights are unsupported. Sampling auto is explicit host intent,
    /// not automatic transition adoption or a change to authored CSS defaults.
    /// If authoring changes eligibility, the adapter must retire its height
    /// ownership and omit that sample; unsupported samples are refused.
    pub fn compute_layout_presented(
        &mut self,
        root: ViewId,
        offer: Offer,
        presented: &[PresentedHeight],
    ) -> Result<LayoutReceipt, KernelError> {
        self.mirror();
        let slot = self.layout_root_slot(root, offer)?;
        for (i, sample) in presented.iter().enumerate() {
            self.validate_presented_height(slot, *sample)?;
            if presented[..i].iter().any(|p| p.node == sample.node) {
                return Err(LayoutError::DuplicatePresentedHeight(sample.node).into());
            }
        }
        self.replace_env(self.arena.env().with_viewport(offer))?;
        engine(&mut self.layout).present_heights(&self.arena, presented);
        let result = match layout::compute(
            &mut self.arena,
            engine(&mut self.layout),
            self.measurer.as_mut(),
            slot,
            offer,
        ) {
            ok @ Ok(_) => ok,
            Err(LayoutError::Engine(_)) => {
                // The engine tree is derived state: rebuild it from the columns and retry once.
                self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
                engine(&mut self.layout).present_heights(&self.arena, presented);
                layout::compute(
                    &mut self.arena,
                    engine(&mut self.layout),
                    self.measurer.as_mut(),
                    slot,
                    offer,
                )
            }
            Err(e) => Err(e),
        };
        let mut receipt = match result {
            Ok(receipt) => receipt,
            Err(e) => {
                // Taffy may have cached the safe zero used to contain the bad
                // callback result. Rebuild derived state so the next valid
                // measurement retries instead of publishing that cache.
                self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
                return Err(e.into());
            }
        };
        receipt.epoch = self.epoch;
        Ok(receipt)
    }

    /// Measure current authored border-box heights without publishing layout.
    /// Active presentations are temporarily removed for this pass and restored
    /// before returning, including after failure. Frames, overflow, flags,
    /// authored styles, receipts and epoch remain unchanged. Results are used
    /// border-box sizes after constraints, not unconstrained CSS intrinsic sizes.
    ///
    /// Call when a target's content or offer changes, not on every motion tick.
    /// This does not register owners or define an automatic `height: auto`
    /// transition policy. The caller still chooses which targets to animate.
    pub fn measure_height_targets(
        &mut self,
        root: ViewId,
        offer: Offer,
        owners: &[NodeKey],
    ) -> Result<Vec<PresentedHeight>, KernelError> {
        self.mirror();
        let slot = self.layout_root_slot(root, offer)?;
        for (i, owner) in owners.iter().enumerate() {
            self.validate_presented_height(
                slot,
                PresentedHeight {
                    node: *owner,
                    epoch: self.epoch,
                    px: 0.0,
                },
            )?;
            if self.arena.style(owner.index).box_sizing != BoxSizing::BorderBox {
                return Err(LayoutError::UnsupportedHeightMeasurement(*owner).into());
            }
            if owners[..i].contains(owner) {
                return Err(LayoutError::DuplicatePresentedHeight(*owner).into());
            }
        }
        if owners.is_empty() {
            return Ok(Vec::new());
        }
        let previous = engine(&mut self.layout).height_samples(self.epoch);
        engine(&mut self.layout).present_heights(&self.arena, &[]);
        let measure = |kernel: &mut Self| {
            let root = kernel
                .arena
                .taffy(slot)
                .ok_or_else(|| LayoutError::Engine("root has no engine node".into()))?;
            engine(&mut kernel.layout).compute(
                root,
                offer,
                &kernel.arena,
                kernel.measurer.as_mut(),
            )?;
            owners
                .iter()
                .map(|node| {
                    let engine_node = kernel.arena.taffy(node.index).ok_or_else(|| {
                        LayoutError::Engine("height target has no engine node".into())
                    })?;
                    let px = engine(&mut kernel.layout).layout(engine_node).size.height;
                    if !px.is_finite() || px < 0.0 {
                        return Err(LayoutError::InvalidPresentedHeight);
                    }
                    Ok(PresentedHeight {
                        node: *node,
                        epoch: kernel.epoch,
                        px,
                    })
                })
                .collect()
        };
        let result = match measure(self) {
            Err(LayoutError::Engine(_)) => {
                self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
                measure(self)
            }
            result => result,
        };
        if result.is_err() {
            // A contained invalid metric may have populated Taffy's cache with
            // zero. Rebuild so the next measurement actually invokes the host.
            self.layout = Some(Box::new(LayoutTree::rebuild(&mut self.arena)));
        }
        engine(&mut self.layout).present_heights(&self.arena, &previous);
        result.map_err(KernelError::from)
    }

    fn layout_root_slot(&self, root: ViewId, offer: Offer) -> Result<u32, KernelError> {
        if self.region.is_some() {
            return Err(
                LayoutError::ContentRegion("use compute_region_layout while registered").into(),
            );
        }
        if !offer.is_finite() {
            return Err(LayoutError::InvalidOffer.into());
        }
        let slot = self
            .arena
            .slot_of(root)
            .ok_or(LayoutError::UnknownView(root))?;
        if !self.arena.is_root(slot) {
            return Err(LayoutError::NotARoot(root).into());
        }
        Ok(slot)
    }

    fn validate_presented_height(&self, root: u32, p: PresentedHeight) -> Result<(), LayoutError> {
        if !p.px.is_finite() || p.px < 0.0 {
            return Err(LayoutError::InvalidPresentedHeight);
        }
        if p.epoch != self.epoch {
            return Err(LayoutError::StalePresentedHeight {
                expected: self.epoch,
                actual: p.epoch,
            });
        }
        let slot = self
            .arena
            .resolve(p.node)
            .ok_or(LayoutError::UnknownPresentedNode(p.node))?;
        if slot != root && !self.arena.is_ancestor(root, slot) {
            return Err(LayoutError::PresentedHeightOutsideRoot(p.node));
        }
        let style = self.arena.style(slot);
        let supported = match style.height {
            Dimension::Points(px) => px.is_finite() && px >= 0.0,
            Dimension::Auto => style.box_sizing == BoxSizing::BorderBox,
            _ => false,
        };
        if self.arena.is_inline_run(slot) || !supported {
            return Err(LayoutError::UnsupportedPresentedHeight(p.node));
        }
        let mut ancestor = Some(slot);
        while let Some(s) = ancestor {
            if self.arena.style(s).display == Display::None {
                return Err(LayoutError::UnsupportedPresentedHeight(p.node));
            }
            ancestor = self.arena.parent(s);
        }
        Ok(())
    }

    /// A native worker replaced pending paragraph metrics. Stale allocation or
    /// source completions cannot invalidate a successor paragraph.
    pub fn invalidate_text_metrics(&mut self, key: NodeKey, revision: u64) -> bool {
        let Some(node) = self.node_by_key(key) else {
            return false;
        };
        if !node
            .paragraph_stamp()
            .is_some_and(|s| s.metric_revision() == revision)
        {
            return false;
        }
        if let (Some(node), Some(layout)) =
            (self.arena.taffy(key.index), self.layout.as_deref_mut())
        {
            layout.mark_dirty(node);
        }
        true
    }

    /// The page's environment: what `env(safe-area-inset-*)` and
    /// `env(viewport-segment-*)` lengths resolve to (LLP 1001 §2; LLP 1078 D3).
    pub fn env(&self) -> Env {
        self.arena.env().clone()
    }

    /// Set the insets of the environment — the safe-area insets the host
    /// reports with the viewport (a rotation changes them); the segment grid
    /// `env` carries is ignored, [`Kernel::set_segments`] being its twin.
    /// Every node whose style holds an `env()` length gets its engine style
    /// re-derived and is marked dirty; returns whether any did (a layout is
    /// owed then). A non-finite inset is refused. A `reset` keeps the
    /// environment: it is the host's.
    pub fn set_env(&mut self, env: Env) -> Result<bool, KernelError> {
        if !env.is_finite() {
            return Err(LayoutError::InvalidEnv.into());
        }
        let next = self
            .arena
            .env()
            .with_insets(env.top, env.right, env.bottom, env.left);
        self.replace_env(next)
    }

    /// Set the viewport segments (LLP 1078 D3): `cols × rows` rects,
    /// row-major, in the layout viewport's points — none for one segment,
    /// where CSS defines no segment variable. Refuses a zero count, a count
    /// that is not `cols × rows` (any rect on a 1 × 1 grid), and a
    /// non-finite rect. Re-derives and dirties exactly the nodes whose style
    /// reads the environment, as `set_env` does, and says whether any did.
    pub fn set_segments(
        &mut self,
        cols: u8,
        rows: u8,
        segments: Vec<Rect>,
    ) -> Result<bool, KernelError> {
        let next = self.arena.env().with_segments(cols, rows, segments);
        if !next.segments_consistent() || !next.is_finite() {
            return Err(LayoutError::InvalidSegments.into());
        }
        self.replace_env(next)
    }

    fn replace_env(&mut self, env: Env) -> Result<bool, KernelError> {
        if *self.arena.env() == env {
            return Ok(false);
        }
        self.arena.set_env(env);
        if let Some(r) = &mut self.region {
            r.invalidate();
        }
        let users: Vec<u32> = self
            .arena
            .iter_live()
            // A covered box too: its top cover can hold an inset (cover.rs).
            .filter(|s| uses_env(self.arena.style(*s)) || self.arena.cover(*s).is_some())
            .collect();
        for slot in &users {
            if let (Some(node), Some(layout)) =
                (self.arena.taffy(*slot), self.layout.as_deref_mut())
            {
                layout.restyle(&self.arena, *slot, node);
                layout.mark_dirty(node);
            }
            self.arena.flags_mut(*slot).insert(NodeFlags::STYLE_DIRTY);
        }
        Ok(!users.is_empty())
    }

    /// Set the root font size (CSS's `medium`, 16 by default): what `rem`
    /// resolves against and what text no ancestor sizes inherits. The host's
    /// fact — Dynamic Type on iOS, the browser's root size on the web — which
    /// the next commit applies, re-resolving every `rem`/`em` row and
    /// invalidating what inherits it, so an empty batch is enough. `px`
    /// never scales. `false` when it is the size already set.
    /// @ref LLP 1069.000 D3
    pub fn set_root_font_size(&mut self, px: f32) -> Result<bool, KernelError> {
        if !px.is_finite() || px <= 0.0 {
            return Err(LayoutError::InvalidRootFontSize.into());
        }
        if self.arena.root_font_size() == px {
            return Ok(false);
        }
        self.arena.root_font_size_next = Some(px);
        Ok(true)
    }

    /// The root font size, as last set (LLP 1069.000 D3).
    pub fn root_font_size(&self) -> f32 {
        self.arena.root_font_size()
    }

    /// The EXNODE envelope for `root`, or for every root when `None`.
    pub fn export(&self, root: Option<ViewId>) -> Result<Vec<u8>, KernelError> {
        let slot = match root {
            Some(id) => Some(self.arena.slot_of(id).ok_or(LayoutError::UnknownView(id))?),
            None => None,
        };
        Ok(export::encode(&self.arena, slot, self.epoch)?)
    }

    /// The typed preorder rows for `root`, or for every root when `None`.
    pub fn rows(&self, root: Option<ViewId>) -> Result<Vec<NodeRow>, KernelError> {
        let slot = match root {
            Some(id) => Some(self.arena.slot_of(id).ok_or(LayoutError::UnknownView(id))?),
            None => None,
        };
        Ok(export::rows(&self.arena, slot)
            .into_iter()
            .map(|(_, row)| row)
            .collect())
    }

    /// One typed export row, with depth zero as for an exported subtree root.
    /// Does not visit the node's descendants; detached live nodes are included.
    pub fn row(&self, id: ViewId) -> Option<NodeRow> {
        Some(export::row(&self.arena, self.arena.slot_of(id)?, 0))
    }

    /// One node by wire id.
    pub fn node(&self, id: ViewId) -> Option<NodeRef<'_>> {
        let slot = self.arena.slot_of(id)?;
        Some(self.node_at(slot))
    }

    /// The nodes under `roots`, in preorder, whose type and props `keep`
    /// accepts: a
    /// walk over the arena's own child lists, allocating nothing per node
    /// (a host's per-commit scan for the few nodes it cares about).
    pub fn preorder_where(
        &self,
        roots: &[ViewId],
        mut keep: impl FnMut(NodeType, &PropList) -> bool,
    ) -> Vec<ViewId> {
        let mut out = Vec::new();
        let mut stack: Vec<u32> = roots
            .iter()
            .rev()
            .filter_map(|id| self.arena.slot_of(*id))
            .collect();
        while let Some(slot) = stack.pop() {
            if keep(self.arena.node_type(slot), self.arena.props(slot)) {
                out.push(self.arena.local_id(slot));
            }
            stack.extend(self.arena.children(slot).iter().rev());
        }
        out
    }

    /// Whether any live node has prop `id` (a [`Kernel::preorder_where`]
    /// for it can be skipped when none does).
    pub fn has_prop(&self, id: PropId) -> bool {
        self.arena.has_prop(id)
    }

    /// Whether any live node is a `node_type`.
    pub fn has_type(&self, node_type: NodeType) -> bool {
        self.arena.has_type(node_type)
    }

    /// One node by key; `None` once that allocation is gone.
    pub fn node_by_key(&self, key: NodeKey) -> Option<NodeRef<'_>> {
        let slot = self.arena.resolve(key)?;
        Some(self.node_at(slot))
    }

    fn node_at(&self, slot: u32) -> NodeRef<'_> {
        NodeRef {
            id: self.arena.local_id(slot),
            key: self.arena.key(slot),
            node_type: self.arena.node_type(slot),
            parent: self.arena.parent(slot).map(|p| self.arena.local_id(p)),
            style: self.arena.style(slot),
            props: self.arena.props(slot),
            frame: self.arena.frame(slot),
            content: self.arena.content(slot),
            is_root: self.arena.is_root(slot),
            arena: &self.arena,
            slot,
        }
    }

    /// Every node whose `id` prop is `id`, oldest first: what a command
    /// naming an element (`showPicker("attach")`) resolves.
    pub fn find_by_id(&self, id: &str) -> Vec<NodeKey> {
        self.selectors
            .lookup_id(id)
            .iter()
            .map(|slot| self.arena.key(*slot))
            .collect()
    }

    /// Every node carrying `test_id`, in structural tree order.
    pub fn find_by_test_id(&self, test_id: &str) -> Vec<NodeKey> {
        let mut hits = crate::sorted::SlotSet::default();
        for &slot in self.selectors.lookup(test_id) {
            hits.insert(slot);
        }
        if hits.is_empty() {
            return Vec::new();
        }
        let order: Vec<u32> = export::rows(&self.arena, None)
            .into_iter()
            .map(|(slot, _)| slot)
            .collect();
        let mut out: Vec<NodeKey> = order
            .iter()
            .filter(|s| hits.contains(**s))
            .map(|s| self.arena.key(*s))
            .collect();
        // Detached nodes (no root above them) come last, by slot.
        let mut ordered = crate::sorted::SlotSet::default();
        for &slot in &order {
            ordered.insert(slot);
        }
        out.extend(
            hits.iter()
                .filter(|s| !ordered.contains(*s))
                .map(|s| self.arena.key(s)),
        );
        out
    }

    /// First match in the same order as `find_by_test_id`, without collecting all matches.
    pub fn find_first_by_test_id(&self, test_id: &str) -> Option<NodeKey> {
        let indexed = self.selectors.lookup(test_id);
        if indexed.len() <= 1 {
            return indexed.first().map(|slot| self.arena.key(*slot));
        }
        let mut stack: Vec<u32> = self.arena.roots().iter().rev().copied().collect();
        while let Some(slot) = stack.pop() {
            if self.arena.props(slot).str(crate::generated::PropId::TestId) == Some(test_id) {
                return Some(self.arena.key(slot));
            }
            stack.extend(self.arena.children(slot).iter().rev().copied());
        }
        // No attached match: the existing all-match query orders detached slots last.
        indexed.iter().min().map(|slot| self.arena.key(*slot))
    }

    /// Retained commit receipts, oldest first.
    pub fn receipts(&self) -> impl Iterator<Item = &CommitReceipt> {
        self.receipts.iter()
    }

    /// Destroy every node and bump the incarnation. Keys minted before never resolve again.
    pub fn reset(&mut self) {
        self.region = None;
        self.arena.reset();
        self.layout =
            (!self.on_demand).then(|| Box::new(LayoutTree::new()) as Box<dyn LayoutMirror>);
        self.selectors.clear();
        self.receipts.clear();
        self.incarnation += 1;
        self.epoch += 1;
    }

    /// A second kernel built from this one's columns alone — the layout engine
    /// and indexes are reconstructed, never copied. Used by the result-equality
    /// gate: a rehydrated kernel must lay out bit-identically to the original.
    pub fn rehydrate(&self, measurer: Box<dyn TextMeasurer>) -> Kernel {
        // Public arena cloning itself creates a fresh paragraph namespace;
        // rehydration is not the only way callers can fork authored state.
        let mut arena = self.arena.clone();
        let layout: Option<Box<dyn LayoutMirror>> = Some(Box::new(LayoutTree::rebuild(&mut arena)));
        let mut selectors = SelectorIndex::new();
        for slot in arena.iter_live() {
            selectors.index(slot, arena.props(slot));
        }
        Kernel {
            arena,
            layout,
            measurer,
            selectors,
            epoch: self.epoch,
            incarnation: self.incarnation,
            receipts: VecDeque::new(),
            region: None,
            region_leases: Default::default(),
            on_demand: self.on_demand,
            detached: self.detached,
        }
    }
}

impl std::fmt::Debug for Kernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kernel")
            .field("live", &self.arena.live_count())
            .field("roots", &self.roots())
            .field("epoch", &self.epoch)
            .field("incarnation", &self.incarnation)
            .finish()
    }
}

#[cfg(test)]
mod presented_height_tests;

#[cfg(test)]
mod paragraph_domain_tests {
    use super::*;
    #[test]
    fn derived_layout_fault_rebuild_keeps_authored_paragraph_stamp() {
        let mut k = Kernel::with_monospace();
        k.apply(
            0,
            0,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::Text,
                },
                Op::SetProp {
                    id: 1,
                    prop: crate::PropId::Text,
                    value: "preserve identity".into(),
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
        let before = k.node(1).unwrap().paragraph_stamp().unwrap();
        k.arena.set_taffy(before.owner().index, None);
        k.compute_layout(1, Offer::definite(300.0, 200.0)).unwrap();
        assert_eq!(before, k.node(1).unwrap().paragraph_stamp().unwrap());
    }
}

#[cfg(test)]
mod locality_tests {
    use super::*;
    use crate::{AxisOffer, NodeType, PropId, StyleId, StyleValue};
    use std::{cell::Cell, rc::Rc};

    struct Count(Rc<Cell<usize>>);
    impl TextMeasurer for Count {
        fn measure(&mut self, r: &crate::TextMeasureRequest<'_>) -> crate::TextMetrics {
            self.0.set(self.0.get() + 1);
            MonospaceMeasurer::default().measure(r)
        }
    }
    fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
        let mut patch = StyleProps::default();
        for (row, value) in rows {
            patch.set_dynamic(*row, value).unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    }
    fn text(id: u32, value: &str) -> Op {
        Op::SetProp {
            id,
            prop: PropId::Text,
            value: value.into(),
        }
    }
    fn fixture(n: u32, extra: Vec<Op>) -> (Kernel, Rc<Cell<usize>>) {
        let calls = Rc::new(Cell::new(0));
        let mut k = Kernel::new(Box::new(Count(calls.clone())));
        let mut ops = vec![];
        for id in 1..=n + 3 {
            ops.push(Op::CreateView {
                id,
                node_type: if id <= 2 {
                    NodeType::View
                } else {
                    NodeType::Text
                },
            });
            if id >= 3 {
                ops.push(text(id, "short text"));
            }
        }
        ops.push(style(
            2,
            &[
                (StyleId::Width, StyleValue::Number(200.0)),
                (StyleId::Height, StyleValue::Number(80.0)),
                (StyleId::OverflowX, StyleValue::Text("hidden".into())),
                (StyleId::OverflowY, StyleValue::Text("scroll".into())),
            ],
        ));
        ops.extend(extra);
        ops.extend([
            Op::SetChildren {
                id: 2,
                children: vec![3],
            },
            Op::SetChildren {
                id: 1,
                children: std::iter::once(2).chain(4..=n + 3).collect(),
            },
            Op::AttachRoot { id: 1 },
        ]);
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, Offer::definite(900.0, 700.0)).unwrap();
        calls.set(0);
        (k, calls)
    }
    fn equal_fresh(k: &Kernel, offer: Offer) {
        let mut fresh = k.rehydrate(Box::new(MonospaceMeasurer::default()));
        fresh.compute_layout(1, offer).unwrap();
        for slot in k.arena.iter_live() {
            assert!(
                k.arena.frame(slot).bits_eq(fresh.arena.frame(slot)),
                "frame {}",
                k.arena.local_id(slot)
            );
            let bits = |(w, h): (f32, f32)| (w.to_bits(), h.to_bits());
            assert_eq!(
                bits(k.arena.content(slot)),
                bits(fresh.arena.content(slot)),
                "overflow {}",
                k.arena.local_id(slot)
            );
        }
    }
    #[test]
    fn native_metric_completion_remeasures_only_current_paragraph_revision() {
        let (mut k, calls) = fixture(100, vec![]);
        let stamp = k.node(3).unwrap().paragraph_stamp().unwrap();
        let offer = Offer::definite(900.0, 700.0);
        assert!(k.invalidate_text_metrics(stamp.owner(), stamp.metric_revision()));
        k.compute_layout(1, offer).unwrap();
        assert!(calls.get() > 0);
        equal_fresh(&k, offer);
        k.apply(0, 2, &[text(3, "a different source")]).unwrap();
        k.compute_layout(1, offer).unwrap();
        calls.set(0);
        assert!(!k.invalidate_text_metrics(stamp.owner(), stamp.metric_revision()));
        assert!(!k.invalidate_text_metrics(
            NodeKey {
                index: stamp.owner().index,
                generation: stamp.owner().generation + 1
            },
            stamp.metric_revision()
        ));
        k.compute_layout(1, offer).unwrap();
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn contained_text_edits_visit_only_the_dirty_path_and_publish_internal_overflow() {
        for n in [100, 2000] {
            let (mut k, calls) = fixture(n, vec![]);
            let offer = Offer::definite(900.0, 700.0);
            for (i, words) in [
                "other text".to_string(),
                "long text ".repeat(200),
                "tiny".into(),
            ]
            .iter()
            .enumerate()
            {
                calls.set(0);
                k.apply(0, i as u64 + 2, &[text(3, words)]).unwrap();
                let r = k.compute_layout(1, offer).unwrap();
                assert_eq!(k.tree().boundary_replays, 1);
                assert_eq!(k.tree().publication_visits, 3, "unrelated nodes: {n}");
                assert_eq!(calls.get(), 1);
                assert!(r
                    .changed
                    .iter()
                    .all(|key| k.node_by_key(*key).unwrap().id == 3));
                assert!(r
                    .updated
                    .iter()
                    .all(|key| [2, 3].contains(&k.node_by_key(*key).unwrap().id)));
                if i == 1 {
                    assert!(r.updated.contains(&k.node(2).unwrap().key));
                }
                equal_fresh(&k, offer);
                assert!(!k
                    .arena
                    .flags(k.node(4).unwrap().key.index)
                    .has(crate::NodeFlags::GEOMETRY_CHANGED));
            }
            assert!(k.compute_layout(1, offer).unwrap().updated.is_empty());
            assert_eq!(k.tree().publication_visits, 0);
        }
    }
    // Whether an edit inside the 200x80 clipping box stays there depends on
    // what its ancestors asked of it, not on their display: a fixed-size box
    // stays local under flex, grid, auto and percentage widths; one whose
    // size its content decides, or that does not clip, or is out of flow,
    // or is beside an exclusion, lays out from the root. Both equal fresh.
    #[test]
    fn coupled_styles_and_changed_viewports_refuse_local_replay() {
        use StyleId::*;
        let cases = [
            (
                true,
                vec![style(
                    1,
                    &[
                        (Display, StyleValue::Text("flex".into())),
                        (AlignItems, StyleValue::Text("baseline".into())),
                    ],
                )],
            ),
            (
                true,
                vec![style(1, &[(Display, StyleValue::Text("grid".into()))])],
            ),
            (
                false,
                vec![style(
                    2,
                    &[
                        (OverflowX, StyleValue::Text("visible".into())),
                        (OverflowY, StyleValue::Text("visible".into())),
                    ],
                )],
            ),
            (true, vec![style(2, &[(Width, StyleValue::Auto)])]),
            (true, vec![style(2, &[(Width, StyleValue::Percent(50.0))])]),
            (false, vec![style(2, &[(Height, StyleValue::Auto)])]),
            // The root's height is its content's: a percentage is auto.
            (
                false,
                vec![style(2, &[(Height, StyleValue::Percent(50.0))])],
            ),
            (
                false,
                vec![style(
                    2,
                    &[(PositionType, StyleValue::Text("absolute".into()))],
                )],
            ),
            (
                false,
                vec![style(
                    4,
                    &[
                        (PositionType, StyleValue::Text("absolute".into())),
                        (WrapFlow, StyleValue::Text("both".into())),
                        (Width, StyleValue::Number(40.0)),
                        (Height, StyleValue::Number(40.0)),
                    ],
                )],
            ),
        ];
        for (i, (contained, extra)) in cases.into_iter().enumerate() {
            let (mut k, _) = fixture(8, extra);
            k.apply(0, 2, &[text(3, &"more words ".repeat(80))])
                .unwrap();
            let offer = Offer::definite(900.0, 700.0);
            k.compute_layout(1, offer).unwrap();
            assert_eq!(k.tree().boundary_replays, contained as usize, "case {i}");
            equal_fresh(&k, offer);
        }
        let (mut k, _) = fixture(8, vec![]);
        for (i, offer) in [
            Offer::definite(800.0, 600.0),
            Offer {
                width: AxisOffer::MinContent,
                height: AxisOffer::MaxContent,
            },
        ]
        .into_iter()
        .enumerate()
        {
            k.apply(0, i as u64 + 2, &[text(3, &"wider ".repeat(20 + i))])
                .unwrap();
            k.compute_layout(1, offer).unwrap();
            assert_eq!(k.tree().boundary_replays, 0);
            equal_fresh(&k, offer);
        }
    }
    #[test]
    fn contained_and_coupled_edits_in_one_batch_propagate_together() {
        let (mut k, _) = fixture(20, vec![]);
        let offer = Offer::definite(900.0, 700.0);
        k.apply(
            0,
            2,
            &[
                text(3, &"inside ".repeat(80)),
                text(4, &"outside ".repeat(200)),
            ],
        )
        .unwrap();
        let r = k.compute_layout(1, offer).unwrap();
        assert_eq!(k.tree().boundary_replays, 0);
        assert!(r.changed.contains(&k.node(5).unwrap().key));
        equal_fresh(&k, offer);
        // Style and topology edits after deferred text invalidation must flush it.
        k.apply(
            0,
            3,
            &[
                text(3, "small"),
                style(2, &[(StyleId::Width, StyleValue::Number(250.0))]),
            ],
        )
        .unwrap();
        k.compute_layout(1, offer).unwrap();
        equal_fresh(&k, offer);
        k.apply(
            0,
            4,
            &[
                text(3, "moved"),
                Op::SetChildren {
                    id: 2,
                    children: vec![4, 3],
                },
            ],
        )
        .unwrap();
        k.compute_layout(1, offer).unwrap();
        equal_fresh(&k, offer);
    }
}

#[cfg(test)]
mod layout_on_demand_tests {
    use super::*;
    use crate::{NodeType, PropId, StyleId, StyleValue};

    fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
        let mut patch = StyleProps::default();
        for (row, value) in rows {
            patch.set_dynamic(*row, value).unwrap();
        }
        Op::SetStyle {
            id,
            patch: Box::new(patch),
        }
    }
    fn create(id: u32, node_type: NodeType) -> Op {
        Op::CreateView { id, node_type }
    }
    fn text(id: u32, value: &str) -> Op {
        Op::SetProp {
            id,
            prop: PropId::Text,
            value: value.into(),
        }
    }
    fn frames(k: &Kernel) -> Vec<(ViewId, Frame, (f32, f32))> {
        let mut out: Vec<_> = (1..=9)
            .filter_map(|id| k.node(id).map(|n| (id, n.frame, n.content)))
            .collect();
        out.sort_by_key(|(id, ..)| *id);
        out
    }

    /// The same commits, before and after a first layout, lay out the same
    /// in a kernel that builds its engine tree on demand and in one that
    /// mirrors every commit, and the on-demand one holds no engine node until
    /// a layout is asked for.
    #[test]
    fn a_kernel_with_layout_on_demand_lays_out_as_a_mirrored_one() {
        let first = [
            create(1, NodeType::View),
            create(2, NodeType::View),
            create(3, NodeType::Text),
            create(4, NodeType::Image),
            create(5, NodeType::Text),
            create(6, NodeType::View),
            style(
                1,
                &[(StyleId::FlexDirection, StyleValue::Text("column".into()))],
            ),
            style(
                2,
                &[
                    (StyleId::Width, StyleValue::Number(200.0)),
                    (StyleId::Height, StyleValue::Number(80.0)),
                ],
            ),
            text(3, "a paragraph of text that wraps"),
            text(5, "short"),
            Op::SetChildren {
                id: 2,
                children: vec![3, 6],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 4, 5],
            },
            Op::AttachRoot { id: 1 },
        ];
        let second = [
            style(2, &[(StyleId::Width, StyleValue::Number(120.0))]),
            text(5, "a longer line of text now"),
            Op::DestroyView { id: 6 },
        ];
        let third = [
            create(7, NodeType::View),
            style(7, &[(StyleId::Height, StyleValue::Number(30.0))]),
            Op::SetChildren {
                id: 1,
                children: vec![2, 7, 4, 5],
            },
        ];
        let offer = Offer::definite(300.0, 600.0);
        let mut mirrored = Kernel::with_monospace();
        let mut on_demand = Kernel::with_monospace_on_demand();
        for k in [&mut mirrored, &mut on_demand] {
            k.apply(0, 1, &first).unwrap();
            k.apply(0, 2, &second).unwrap();
            k.set_intrinsic_size(4, Some((40.0, 20.0))).unwrap();
            k.set_env(Env::new(10.0, 0.0, 5.0, 0.0)).unwrap();
        }
        assert_eq!(on_demand.engine_nodes(), 0);
        assert_eq!(mirrored.engine_nodes(), mirrored.live_count());
        for k in [&mut mirrored, &mut on_demand] {
            k.compute_layout(1, offer).unwrap();
        }
        assert_eq!(on_demand.engine_nodes(), on_demand.live_count());
        assert_eq!(frames(&on_demand), frames(&mirrored));
        let box2 = on_demand.node(2).unwrap().frame;
        assert_eq!((box2.width, box2.height), (120.0, 80.0));
        assert!(on_demand.node(5).unwrap().frame.y >= 80.0);
        // Mirrored from its first layout on, it keeps up with later commits.
        for k in [&mut mirrored, &mut on_demand] {
            k.apply(0, 3, &third).unwrap();
            k.compute_layout(1, offer).unwrap();
        }
        assert_eq!(on_demand.engine_nodes(), on_demand.live_count());
        assert_eq!(frames(&on_demand), frames(&mirrored));
        assert_eq!(on_demand.node(7).unwrap().frame.height, 30.0);
        // A reset returns it to building on demand.
        on_demand.reset();
        on_demand.apply(0, 4, &first).unwrap();
        assert_eq!(on_demand.engine_nodes(), 0);
    }
}
