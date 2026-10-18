//! Derived node motion, resolved independently against each model level.
use crate::{Data, DataError, Reader, Transform, Writer};

/// A model node name. Strings determine identity; the hash accelerates lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeName {
    name: &'static str,
    hash: u64,
}
impl NodeName {
    /// Build a reusable node name at compile time.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            hash: Self::hash_str(name),
        }
    }
    /// The authored glTF node name.
    pub const fn as_str(self) -> &'static str {
        self.name
    }
    /// FNV-1a lookup hint. Always compare the string after a hash match.
    pub const fn hash_str(name: &str) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let bytes = name.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            h = (h ^ bytes[i] as u64).wrapping_mul(0x100_0000_01b3);
            i += 1;
        }
        h
    }
    /// Cached lookup hint.
    pub const fn hash(self) -> u64 {
        self.hash
    }
}
impl Default for NodeName {
    fn default() -> Self {
        Self::new("")
    }
}
impl Data for NodeName {
    fn write(&self, w: &mut dyn Writer) {
        w.string(self.name);
    }
    fn read(&mut self, _: &mut dyn Reader) -> Result<(), DataError> {
        Err(DataError::new("presentation node names are write-only"))
    }
}
/// One node-frame offset applied after clips, layers and IK.
#[derive(Default, Clone, Copy, Debug, Data)]
pub struct NodeOffset {
    /// Name resolved only against marked nodes.
    pub node: NodeName,
    /// Local TRS delta; scale must be positive and values finite.
    pub transform: Transform,
}
/// Up to sixteen drawn node offsets. Rebuilt by every `Game::present`.
/// Invisible to simulation, including IK, picking and animation sockets.
#[derive(Default, Clone, Copy, Debug)]
pub struct NodeOffsets {
    rows: [NodeOffset; 16],
    len: u8,
    overflow: bool,
}
impl NodeOffsets {
    /// Add or replace a named row. Excess distinct names are dropped, never panic.
    pub fn with(mut self, node: NodeName, transform: Transform) -> Self {
        if let Some(row) = self.rows[..self.len as usize]
            .iter_mut()
            .find(|r| r.node.as_str() == node.as_str())
        {
            row.transform = transform;
        } else if (self.len as usize) < self.rows.len() {
            self.rows[self.len as usize] = NodeOffset { node, transform };
            self.len += 1;
        } else {
            self.overflow = true;
        }
        self
    }
    /// Written rows, in insertion order.
    pub fn rows(&self) -> &[NodeOffset] {
        &self.rows[..self.len as usize]
    }
    /// Whether this present exceeded the fixed row capacity.
    pub fn overflowed(&self) -> bool {
        self.overflow
    }
}

impl Data for NodeOffsets {
    fn write(&self, w: &mut dyn Writer) {
        w.begin_struct();
        w.field("rows");
        w.begin_seq(self.rows().len());
        for row in self.rows() {
            w.item();
            row.write(w);
        }
        w.end_seq();
        w.field("overflow");
        self.overflow.write(w);
        w.end_struct();
    }
    fn read(&mut self, _: &mut dyn Reader) -> Result<(), DataError> {
        Err(DataError::new("presentation node offsets are write-only"))
    }
}
impl crate::Component for NodeOffsets {
    const NAME: &'static str = "NodeOffsets";
    const PRESENTATION: bool = true;
}
impl crate::PresentationComponent for NodeOffsets {}
