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
use crate::layout::{self, LayoutReceipt, LayoutTree};
use crate::props::PropList;
use crate::selector::SelectorIndex;
use crate::style::{taffy_style, uses_env, ColorValue, Dimension, Env, RowValue};
use crate::text::{MonospaceMeasurer, TextMeasurer, TextRun, TextStyle};

/// The initial value of every row: what a computed read returns when neither
/// the node nor an ancestor sets an inherited row.
static INITIAL: LazyLock<StyleProps> = LazyLock::new(StyleProps::default);
use crate::txn::{self, CommitReceipt, Target};
use crate::wire::{self, Op};

/// How many receipts the kernel retains for late readers.
pub const RECEIPT_RING: usize = 64;

/// One sampled CSS height for a live numeric-height box or border-box auto height.
/// This replaces only derived layout height, respecting current box sizing,
/// min/max constraints and aspect ratio. It never authors a style or a commit.
/// Existing root lowering is retained: an auto-width, nonabsolute root uses
/// derived border-box sizing even when its authored box sizing is content-box.
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
    arena: &'a NodeArena,
    slot: u32,
}

impl<'a> NodeRef<'a> {
    /// Resolved exclusions in leaf border-box coordinates, in document order.
    /// @ref LLP 1043.000 §3 D4 — derived geometry, never paragraph inputs.
    pub fn flow_shapes(&self) -> &'a [exact_textflow::FlowShape] {
        self.arena.flow_shapes(self.slot)
    }

    /// Intersecting exclusions were skipped because Taffy measured this height.
    pub fn flow_skipped(&self) -> bool {
        self.arena.flow_skipped(self.slot)
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
            None => INITIAL.get(id),
        }
    }

    /// The node's rows with the inherited rows in `rows` resolved through the
    /// logical ancestors (`NodeArena::computed_style`): what a host paints
    /// and measures with.
    pub fn computed_style(&self, rows: StyleMask) -> StyleProps {
        self.arena.computed_style(self.slot, rows)
    }

    /// The run style this node's text measures and paints with: its own text
    /// rows, else its paragraph's, else the initial values.
    pub fn text_style(&self) -> TextStyle {
        TextStyle::from_style(&self.computed_style(StyleMask::INHERITED))
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
    layout: LayoutTree,
    measurer: Box<dyn TextMeasurer>,
    selectors: SelectorIndex,
    epoch: u64,
    incarnation: u64,
    receipts: VecDeque<CommitReceipt>,
    region: Option<crate::region::RegionState>,
    region_leases: crate::region::RegionLeases,
}

impl Kernel {
    /// A kernel with the given host text measurer.
    pub fn new(measurer: Box<dyn TextMeasurer>) -> Self {
        Kernel {
            arena: NodeArena::new(),
            layout: LayoutTree::new(),
            measurer,
            selectors: SelectorIndex::new(),
            epoch: 0,
            incarnation: 1,
            receipts: VecDeque::new(),
            region: None,
            region_leases: Default::default(),
        }
    }

    /// A kernel with the deterministic reference measurer.
    pub fn with_monospace() -> Self {
        Self::new(Box::new(MonospaceMeasurer::default()))
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

    /// Root wire ids in attach order.
    pub fn roots(&self) -> Vec<ViewId> {
        self.arena
            .roots()
            .iter()
            .map(|r| self.arena.local_id(*r))
            .collect()
    }

    /// Decode one EXWF frame and apply it.
    pub fn apply_frame(&mut self, bytes: &[u8]) -> Result<CommitReceipt, KernelError> {
        let frame = wire::decode(bytes)?;
        self.apply(frame.root_id, frame.batch, &frame.ops)
    }

    /// Apply ops in-process. Validation covers the whole batch before any write;
    /// a rejection leaves everything untouched.
    pub fn apply(
        &mut self,
        root_id: u32,
        batch: u64,
        ops: &[Op],
    ) -> Result<CommitReceipt, KernelError> {
        let next_epoch = self.epoch + 1;
        let target = Target {
            arena: &mut self.arena,
            layout: &mut self.layout,
            selectors: &mut self.selectors,
        };
        let receipt = txn::apply(target, ops, batch, root_id, next_epoch)?;
        let changed = !receipt.created.is_empty()
            || !receipt.destroyed.is_empty()
            || !receipt.touched.is_empty();
        if changed {
            self.epoch = next_epoch;
        }
        let mut receipt = receipt;
        receipt.epoch = self.epoch;
        if self.receipts.len() == RECEIPT_RING {
            self.receipts.pop_front();
        }
        if self
            .region
            .as_mut()
            .is_some_and(|r| !r.observe(&self.arena, &receipt))
        {
            self.region = None;
        }
        self.receipts.push_back(receipt.clone());
        Ok(receipt)
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
        if binding.is_some() && self.layout.has_presented_height() {
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
            self.layout = LayoutTree::rebuild(&mut self.arena);
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
        let region = self
            .region
            .as_mut()
            .ok_or(LayoutError::ContentRegion("no registered region"))?;
        let result = region.compute(
            &mut self.arena,
            &mut self.layout,
            self.measurer.as_mut(),
            (slot, offer),
            inputs,
            self.epoch,
        );
        if result.is_err() {
            // A numeric callback error can have cached its containment zero in
            // the shell too. No failed derived cache is reused on recovery.
            self.layout = LayoutTree::rebuild(&mut self.arena);
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
    /// Matches ordinary authored lowering, including its auto-width,
    /// nonabsolute root exception: derived box sizing there is border-box even
    /// when authored as content-box. No root sizing repair is made by projection.
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
        let slot = self.layout_root_slot(root, offer)?;
        for (i, sample) in presented.iter().enumerate() {
            self.validate_presented_height(slot, *sample)?;
            if presented[..i].iter().any(|p| p.node == sample.node) {
                return Err(LayoutError::DuplicatePresentedHeight(sample.node).into());
            }
        }
        self.layout.present_heights(&self.arena, presented);
        let result = match layout::compute(
            &mut self.arena,
            &mut self.layout,
            self.measurer.as_mut(),
            slot,
            offer,
        ) {
            ok @ Ok(_) => ok,
            Err(LayoutError::Engine(_)) => {
                // The engine tree is derived state: rebuild it from the columns and retry once.
                self.layout = LayoutTree::rebuild(&mut self.arena);
                self.layout.present_heights(&self.arena, presented);
                layout::compute(
                    &mut self.arena,
                    &mut self.layout,
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
                self.layout = LayoutTree::rebuild(&mut self.arena);
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
        let previous = self.layout.height_samples(self.epoch);
        self.layout.present_heights(&self.arena, &[]);
        let measure = |kernel: &mut Self| {
            let root = kernel
                .arena
                .taffy(slot)
                .ok_or_else(|| LayoutError::Engine("root has no engine node".into()))?;
            kernel
                .layout
                .compute(root, offer, &kernel.arena, kernel.measurer.as_mut())?;
            owners
                .iter()
                .map(|node| {
                    let engine_node = kernel.arena.taffy(node.index).ok_or_else(|| {
                        LayoutError::Engine("height target has no engine node".into())
                    })?;
                    let px = kernel.layout.layout(engine_node).size.height;
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
                self.layout = LayoutTree::rebuild(&mut self.arena);
                measure(self)
            }
            result => result,
        };
        if result.is_err() {
            // A contained invalid metric may have populated Taffy's cache with
            // zero. Rebuild so the next measurement actually invokes the host.
            self.layout = LayoutTree::rebuild(&mut self.arena);
        }
        self.layout.present_heights(&self.arena, &previous);
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

    /// A replaced element's intrinsic size — the bitmap's pixel counts,
    /// taken one-for-one as layout units — reported by the host once the
    /// image or video metadata has loaded (`None` to forget it): the node is measured from it
    /// and keeps its ratio unless a row sets one. Refused for a node that is
    /// not an `Image` or `Video` and for a size that is not finite and positive on both
    /// axes. Marks layout dirty.
    pub fn set_intrinsic_size(
        &mut self,
        view: ViewId,
        size: Option<(f32, f32)>,
    ) -> Result<(), KernelError> {
        let slot = self
            .arena
            .slot_of(view)
            .ok_or(LayoutError::UnknownView(view))?;
        if !self.arena.node_type(slot).is_replaced() {
            return Err(LayoutError::NotAnImage(view).into());
        }
        if let Some((w, h)) = size {
            if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
                return Err(LayoutError::InvalidIntrinsicSize(view).into());
            }
        }
        if self.arena.intrinsic(slot) == size {
            return Ok(());
        }
        self.arena.set_intrinsic(slot, size);
        if let Some(r) = &mut self.region {
            r.intrinsic(slot);
        }
        if let Some(node) = self.arena.taffy(slot) {
            self.layout.set_style(node, taffy_style(&self.arena, slot));
            self.layout.mark_dirty(node);
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
        if let Some(node) = self.arena.taffy(key.index) {
            self.layout.mark_dirty(node);
        }
        true
    }

    /// The page's environment: what `env(safe-area-inset-*)` lengths
    /// resolve to (LLP 1001 §2).
    pub fn env(&self) -> Env {
        *self.arena.env()
    }

    /// Set the environment — the safe-area insets the host reports with
    /// the viewport (a rotation changes them). Every node whose style holds
    /// an `env()` length gets its engine style re-derived and is marked
    /// dirty; returns whether any did (a layout is owed then). A non-finite
    /// inset is refused. A `reset` keeps the environment: it is the host's.
    pub fn set_env(&mut self, env: Env) -> Result<bool, KernelError> {
        if !env.is_finite() {
            return Err(LayoutError::InvalidEnv.into());
        }
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
            .filter(|s| uses_env(self.arena.style(*s)))
            .collect();
        for slot in &users {
            if let Some(node) = self.arena.taffy(*slot) {
                self.layout.set_style(node, taffy_style(&self.arena, *slot));
                self.layout.mark_dirty(node);
            }
            self.arena.flags_mut(*slot).insert(NodeFlags::STYLE_DIRTY);
        }
        Ok(!users.is_empty())
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

    /// Every node carrying `test_id`, in structural tree order.
    pub fn find_by_test_id(&self, test_id: &str) -> Vec<NodeKey> {
        let hits: std::collections::HashSet<u32> =
            self.selectors.lookup(test_id).iter().copied().collect();
        if hits.is_empty() {
            return Vec::new();
        }
        let order: Vec<u32> = export::rows(&self.arena, None)
            .into_iter()
            .map(|(slot, _)| slot)
            .collect();
        let mut out: Vec<NodeKey> = order
            .iter()
            .filter(|s| hits.contains(*s))
            .map(|s| self.arena.key(*s))
            .collect();
        // Detached nodes (no root above them) come last, by slot.
        let ordered: std::collections::HashSet<u32> = order.iter().copied().collect();
        let mut detached: Vec<u32> = hits
            .iter()
            .copied()
            .filter(|s| !ordered.contains(s))
            .collect();
        detached.sort_unstable();
        out.extend(detached.into_iter().map(|s| self.arena.key(s)));
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
        self.layout = LayoutTree::new();
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
        let layout = LayoutTree::rebuild(&mut arena);
        let mut selectors = SelectorIndex::new();
        for slot in arena.iter_live() {
            if let Some(test_id) = arena.props(slot).str(crate::generated::PropId::TestId) {
                selectors.update(slot, None, Some(test_id));
            }
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
mod presented_height_tests {
    use super::*;
    use crate::{Dimension, PresentedHeight};

    #[test]
    fn engine_fault_rebuild_reapplies_projection_and_equal_sample_stays_clean() {
        let mut k = Kernel::with_monospace();
        let mut style = StyleProps::default();
        style.height = Dimension::Points(180.0);
        style.mask.set(StyleId::Height);
        style.box_sizing = BoxSizing::BorderBox;
        style.mask.set(StyleId::BoxSizing);
        k.apply(
            0,
            1,
            &[
                Op::CreateView {
                    id: 1,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 1,
                    patch: Box::new(style.clone()),
                },
                Op::CreateView {
                    id: 2,
                    node_type: NodeType::View,
                },
                Op::SetStyle {
                    id: 2,
                    patch: Box::new(style),
                },
                Op::SetChildren {
                    id: 1,
                    children: vec![2],
                },
                Op::AttachRoot { id: 1 },
            ],
        )
        .unwrap();
        let p = PresentedHeight {
            node: k.node(1).unwrap().key,
            epoch: k.epoch(),
            px: 320.0,
        };
        let samples = [
            p,
            PresentedHeight {
                node: k.node(2).unwrap().key,
                px: 90.0,
                ..p
            },
        ];
        let offer = Offer::definite(400.0, 600.0);
        k.compute_layout_presented(1, offer, &samples).unwrap();
        // Missing derived state exercises the real Engine error/rebuild path,
        // without changing the authored columns or a production test hook.
        k.arena.set_taffy(p.node.index, None);
        k.compute_layout_presented(1, offer, &samples).unwrap();
        assert!(!k.layout.faulted());
        assert_eq!(k.node(1).unwrap().frame.height, 320.0);
        assert_eq!(k.node(2).unwrap().frame.height, 90.0);
        let node = k.arena.taffy(p.node.index).unwrap();
        assert!(!k.layout.is_dirty(node));
        k.layout.present_heights(&k.arena, &samples);
        assert!(
            !k.layout.is_dirty(node),
            "equal projection must not call Taffy set_style"
        );
        // Target measurement recovers the derived tree without publishing the
        // authored endpoint, and restores every active sample after rebuilding.
        let before = k.export(None).unwrap();
        k.arena.set_taffy(p.node.index, None);
        let owners = samples.map(|p| p.node);
        let targets = k.measure_height_targets(1, offer, &owners).unwrap();
        assert_eq!(
            targets.iter().map(|p| p.px).collect::<Vec<_>>(),
            vec![180.0, 180.0]
        );
        assert_eq!(k.export(None).unwrap(), before);
        assert_eq!(k.layout.height_samples(k.epoch()), samples);
        k.arena.set_taffy(samples[1].node.index, None);
        assert_eq!(
            k.measure_height_targets(1, offer, &owners).unwrap(),
            targets
        );
        assert_eq!(k.export(None).unwrap(), before);
        assert_eq!(k.layout.height_samples(k.epoch()), samples);
        assert!(k
            .compute_layout_presented(1, offer, &samples)
            .unwrap()
            .changed
            .is_empty());
        let node = k.arena.taffy(p.node.index).unwrap();
        let mut style = k.arena.style(p.node.index).clone();
        style.height = Dimension::Points(400.0);
        k.apply(
            0,
            2,
            &[Op::SetStyle {
                id: 1,
                patch: Box::new(style),
            }],
        )
        .unwrap();
        assert!(
            !k.layout.is_dirty(node),
            "central authored write preserves identical derived height"
        );
        k.compute_layout(1, offer).unwrap();
        assert_eq!(k.node(1).unwrap().frame.height, 400.0);
        assert_eq!(k.node(2).unwrap().frame.height, 180.0);
    }
}

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
                assert_eq!(k.layout.boundary_replays, 1);
                assert_eq!(k.layout.publication_visits, 3, "unrelated nodes: {n}");
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
            assert_eq!(k.layout.publication_visits, 0);
        }
    }
    #[test]
    fn coupled_styles_and_changed_viewports_refuse_local_replay() {
        use StyleId::*;
        let cases = [
            vec![style(
                1,
                &[
                    (Display, StyleValue::Text("flex".into())),
                    (AlignItems, StyleValue::Text("baseline".into())),
                ],
            )],
            vec![style(1, &[(Display, StyleValue::Text("grid".into()))])],
            vec![style(
                2,
                &[
                    (OverflowX, StyleValue::Text("visible".into())),
                    (OverflowY, StyleValue::Text("visible".into())),
                ],
            )],
            vec![style(2, &[(Width, StyleValue::Auto)])],
            vec![style(2, &[(Width, StyleValue::Percent(50.0))])],
            vec![style(2, &[(Height, StyleValue::Auto)])],
            vec![style(2, &[(Height, StyleValue::Percent(50.0))])],
            vec![style(
                2,
                &[(PositionType, StyleValue::Text("absolute".into()))],
            )],
            vec![style(
                4,
                &[
                    (PositionType, StyleValue::Text("absolute".into())),
                    (WrapFlow, StyleValue::Text("both".into())),
                    (Width, StyleValue::Number(40.0)),
                    (Height, StyleValue::Number(40.0)),
                ],
            )],
        ];
        for extra in cases {
            let (mut k, _) = fixture(8, extra);
            k.apply(0, 2, &[text(3, &"more words ".repeat(80))])
                .unwrap();
            let offer = Offer::definite(900.0, 700.0);
            k.compute_layout(1, offer).unwrap();
            assert_eq!(k.layout.boundary_replays, 0);
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
            assert_eq!(k.layout.boundary_replays, 0);
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
        assert_eq!(k.layout.boundary_replays, 0);
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
