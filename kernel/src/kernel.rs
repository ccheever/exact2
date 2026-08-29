//! The kernel facade: one object, one write path, columnar reads.
//!
//! A [`Kernel`] owns a tree and its derived layout. Producers mutate it through
//! [`Kernel::apply_frame`] (EXWF bytes) or [`Kernel::apply`] (in-process ops);
//! both share the validate-then-apply engine. Hosts read it through receipts,
//! the typed rows, or the EXNODE envelope. It is single-owner and adds no
//! threads.

use std::collections::VecDeque;

use crate::arena::NodeArena;
use crate::error::{KernelError, LayoutError};
use crate::export::{self, NodeRow};
use crate::generated::{NodeType, StyleProps};
use crate::id::{Frame, NodeKey, Offer, ViewId};
use crate::layout::{self, LayoutReceipt, LayoutTree};
use crate::props::PropList;
use crate::selector::SelectorIndex;
use crate::text::{MonospaceMeasurer, TextMeasurer};
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

impl NodeRef<'_> {
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
        let slot = self
            .arena
            .slot_of(root)
            .ok_or(LayoutError::UnknownView(root))?;
        if !self.arena.is_root(slot) {
            return Err(LayoutError::NotARoot(root).into());
        }
        let changed = match layout::compute(
            &mut self.arena,
            &mut self.layout,
            self.measurer.as_mut(),
            slot,
            offer,
        ) {
            Ok(changed) => changed,
            Err(LayoutError::Engine(_)) => {
                // The engine tree is derived state: rebuild it from the columns and retry once.
                self.layout = LayoutTree::rebuild(&mut self.arena);
                layout::compute(
                    &mut self.arena,
                    &mut self.layout,
                    self.measurer.as_mut(),
                    slot,
                    offer,
                )?
            }
            Err(e) => return Err(e.into()),
        };
        Ok(LayoutReceipt {
            epoch: self.epoch,
            root: self.arena.key(slot),
            changed: changed.iter().map(|s| self.arena.key(*s)).collect(),
        })
    }

    /// The EXNODE envelope for `root`, or for every root when `None`.
    pub fn export(&self, root: Option<ViewId>) -> Result<Vec<u8>, KernelError> {
        let slot = match root {
            Some(id) => Some(self.arena.slot_of(id).ok_or(LayoutError::UnknownView(id))?),
            None => None,
        };
        Ok(export::encode(&self.arena, slot, self.epoch))
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
        self.arena = NodeArena::new();
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
