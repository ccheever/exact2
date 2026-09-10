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
use crate::generated::{NodeType, PropId, StyleId, StyleMask, StyleProps};
use crate::id::{Frame, NodeFlags, NodeKey, Offer, ViewId};
use crate::layout::{self, LayoutReceipt, LayoutTree};
use crate::props::PropList;
use crate::selector::SelectorIndex;
use crate::style::{taffy_style, uses_env, ColorValue, Env, RowValue};
use crate::text::{MonospaceMeasurer, TextMeasurer, TextStyle};

/// The initial value of every row: what a computed read returns when neither
/// the node nor an ancestor sets an inherited row.
static INITIAL: LazyLock<StyleProps> = LazyLock::new(StyleProps::default);
use crate::txn::{self, CommitReceipt, Target};
use crate::wire::{self, Op};

/// How many receipts the kernel retains for late readers.
pub const RECEIPT_RING: usize = 64;

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
        self.receipts.push_back(receipt.clone());
        Ok(receipt)
    }

    /// Lay out one root under an offer and publish frames. The receipt names
    /// every node whose frame changed.
    pub fn compute_layout(
        &mut self,
        root: ViewId,
        offer: Offer,
    ) -> Result<LayoutReceipt, KernelError> {
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
        let changed = match result {
            Ok(changed) => changed,
            Err(e @ LayoutError::InvalidTextMetrics(_)) => {
                // Taffy may have cached the safe zero used to contain the bad
                // callback result. Rebuild derived state so the next valid
                // measurement retries instead of publishing that cache.
                self.layout = LayoutTree::rebuild(&mut self.arena);
                return Err(e.into());
            }
            Err(e) => return Err(e.into()),
        };
        Ok(LayoutReceipt {
            epoch: self.epoch,
            root: self.arena.key(slot),
            changed: changed.iter().map(|s| self.arena.key(*s)).collect(),
        })
    }

    /// A replaced element's intrinsic size — the bitmap's pixel counts,
    /// taken one-for-one as layout units — reported by the host once the
    /// image has loaded (`None` to forget it): the node is measured from it
    /// and keeps its ratio unless a row sets one. Refused for a node that is
    /// not an `Image` and for a size that is not finite and positive on both
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
        if self.arena.node_type(slot) != NodeType::Image {
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
        if let Some(node) = self.arena.taffy(slot) {
            self.layout.set_style(node, taffy_style(&self.arena, slot));
            self.layout.mark_dirty(node);
        }
        Ok(())
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

    /// Retained commit receipts, oldest first.
    pub fn receipts(&self) -> impl Iterator<Item = &CommitReceipt> {
        self.receipts.iter()
    }

    /// Destroy every node and bump the incarnation. Keys minted before never resolve again.
    pub fn reset(&mut self) {
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
