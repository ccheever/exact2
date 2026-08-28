//! Identity, flags, and geometry primitives.
//!
//! A producer names nodes by a wire-local [`ViewId`]. Inside the kernel every
//! node is a slot in a columnar arena addressed by a generation-checked
//! [`NodeKey`]: a key minted for one allocation fails closed after that node is
//! destroyed and its slot reused, so a stale reference can never alias a newer
//! node. Receipts and agent refs carry `NodeKey`s, never bare `u32`s.

/// The producer's wire-local node id. Unique among live nodes in one kernel.
pub type ViewId = u32;

/// Generation-checked handle to one node allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeKey {
    /// Slot index in the arena columns.
    pub index: u32,
    /// Allocation generation of that slot. Bumped every time the slot is reused.
    pub generation: u32,
}

/// Per-node dirty flags, cleared by the pass that consumes them.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct NodeFlags(pub u32);

impl NodeFlags {
    /// A layout-affecting style row changed since the last layout.
    pub const STYLE_DIRTY: NodeFlags = NodeFlags(1 << 0);
    /// A measure-affecting prop or text row changed since the last layout.
    pub const TEXT_DIRTY: NodeFlags = NodeFlags(1 << 1);
    /// The child list changed since the last layout.
    pub const CHILDREN_DIRTY: NodeFlags = NodeFlags(1 << 2);
    /// A non-layout prop changed since the last export.
    pub const PROPS_DIRTY: NodeFlags = NodeFlags(1 << 3);
    /// A paint-only style row changed since the last export.
    pub const PAINT_DIRTY: NodeFlags = NodeFlags(1 << 4);
    /// The absolute frame changed in the last layout pass.
    pub const GEOMETRY_CHANGED: NodeFlags = NodeFlags(1 << 5);
    /// The node was created in the batch that produced the current epoch.
    pub const CREATED: NodeFlags = NodeFlags(1 << 6);

    /// Whether every bit of `flag` is set.
    pub const fn has(self, flag: NodeFlags) -> bool {
        self.0 & flag.0 == flag.0
    }

    /// Set the bits of `flag`.
    pub fn insert(&mut self, flag: NodeFlags) {
        self.0 |= flag.0;
    }

    /// Clear the bits of `flag`.
    pub fn remove(&mut self, flag: NodeFlags) {
        self.0 &= !flag.0;
    }

    /// Whether any layout-relevant dirty bit is set.
    pub const fn layout_dirty(self) -> bool {
        self.0 & (Self::STYLE_DIRTY.0 | Self::TEXT_DIRTY.0 | Self::CHILDREN_DIRTY.0) != 0
    }
}

/// Absolute layout frame, in layout points, relative to the root's origin.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Frame {
    /// Horizontal origin.
    pub x: f32,
    /// Vertical origin.
    pub y: f32,
    /// Resolved width.
    pub width: f32,
    /// Resolved height.
    pub height: f32,
}

impl Frame {
    /// Bitwise equality — the comparison the result-equality gate uses, so a
    /// one-ULP drift between incremental and full relayout is a failure rather
    /// than a rounding opinion.
    pub fn bits_eq(self, other: Frame) -> bool {
        self.x.to_bits() == other.x.to_bits()
            && self.y.to_bits() == other.y.to_bits()
            && self.width.to_bits() == other.width.to_bits()
            && self.height.to_bits() == other.height.to_bits()
    }
}

/// Typed available-space offer for one layout axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisOffer {
    /// A definite number of layout points.
    Definite(f32),
    /// An intrinsic max-content constraint.
    MaxContent,
    /// An intrinsic min-content constraint.
    MinContent,
}

/// The available space offered to a root.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offer {
    /// Horizontal offer.
    pub width: AxisOffer,
    /// Vertical offer.
    pub height: AxisOffer,
}

impl Offer {
    /// A definite viewport.
    pub const fn definite(width: f32, height: f32) -> Offer {
        Offer {
            width: AxisOffer::Definite(width),
            height: AxisOffer::Definite(height),
        }
    }

    /// Both axes max-content: the root sizes to its content.
    pub const MAX_CONTENT: Offer = Offer {
        width: AxisOffer::MaxContent,
        height: AxisOffer::MaxContent,
    };
}
