//! EXNODE — the columnar node export: one crossing per sync.
//!
//! A host reads the whole tree as one sectioned envelope instead of probing
//! fields one FFI call at a time. In-process Rust hosts use the typed rows
//! ([`rows`]); FFI and transport consumers use the binary envelope
//! ([`encode`] / [`decode`]) — one schema, two projections.
//!
//! ```text
//! header (40 bytes)
//!   0  magic        "EXNO"
//!   4  version      u16 = 1
//!   6  header_len   u16 = 40
//!   8  total_len    u32
//!  12  root_id      u32   wire id of the exported root, or 0 for every root
//!  16  epoch        u64
//!  24  node_count   u32
//!  28  section_count u32
//!  32  reserved     u64
//! section directory: section_count × 16 bytes
//!   kind u32 · offset u32 · len u32 · fnv1a32 u32
//! sections (each 8-aligned)
//!   NODES  (1): node_count × 32-byte rows
//!   STYLES (2): per node, the masked patch encoding of its set rows
//!   PROPS  (3): per node, u16 count, then (u16 id, u8 kind, value)
//! node row (32 bytes)
//!   0  id          u32
//!   4  parent      u32   u32::MAX for none
//!   8  generation  u32
//!  12  node_type   u8
//!  13  flags       u8    bit 0: root, bit 1: inline run, bit 2: geometry changed
//!  14  depth       u16
//!  16  x, y, w, h  f32 × 4
//! ```
//! The decoder validates every length and checksum before adopting anything,
//! and never allocates from a count it has not first bounded by the bytes
//! actually present.

use crate::arena::NodeArena;
use crate::error::DecodeError;
use crate::generated::{NodeType, PropId, PropKind, StyleProps};
use crate::id::{Frame, NodeFlags, NodeKey, ViewId};
use crate::props::{PropList, PropValue};
use crate::wire::codec::{Reader, Writer};

/// Magic bytes.
pub const MAGIC: [u8; 4] = *b"EXNO";
/// Envelope version.
pub const VERSION: u16 = 1;
/// Header size.
pub const HEADER_LEN: usize = 40;
/// Directory entry size.
pub const DIR_ENTRY_LEN: usize = 16;
/// Node row size.
pub const ROW_LEN: usize = 32;
/// Section kinds.
pub const SECTION_NODES: u32 = 1;
/// Section kinds.
pub const SECTION_STYLES: u32 = 2;
/// Section kinds.
pub const SECTION_PROPS: u32 = 3;

/// Row flag: the node is a root.
pub const ROW_ROOT: u8 = 1;
/// Row flag: the node is an inline text run (no geometry of its own).
pub const ROW_INLINE_RUN: u8 = 2;
/// Row flag: the frame changed in the last layout.
pub const ROW_GEOMETRY_CHANGED: u8 = 4;

/// One node as the typed in-process projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRow {
    /// Wire id.
    pub id: ViewId,
    /// Parent wire id.
    pub parent: Option<ViewId>,
    /// Generation-checked key.
    pub key: NodeKey,
    /// Node type.
    pub node_type: NodeType,
    /// Row flags.
    pub flags: u8,
    /// Depth below the root.
    pub depth: u16,
    /// Absolute frame.
    pub frame: Frame,
}

/// The preorder rows for one root (or every root), with their slots.
pub fn rows(arena: &NodeArena, root: Option<u32>) -> Vec<(u32, NodeRow)> {
    let roots: Vec<u32> = match root {
        Some(r) => vec![r],
        None => arena.roots().to_vec(),
    };
    let mut out = Vec::new();
    for r in roots {
        let mut stack: Vec<(u32, u16)> = vec![(r, 0)];
        while let Some((slot, depth)) = stack.pop() {
            let mut flags = 0u8;
            if arena.is_root(slot) {
                flags |= ROW_ROOT;
            }
            if arena.is_inline_run(slot) {
                flags |= ROW_INLINE_RUN;
            }
            if arena.flags(slot).has(NodeFlags::GEOMETRY_CHANGED) {
                flags |= ROW_GEOMETRY_CHANGED;
            }
            out.push((
                slot,
                NodeRow {
                    id: arena.local_id(slot),
                    parent: arena.parent(slot).map(|p| arena.local_id(p)),
                    key: arena.key(slot),
                    node_type: arena.node_type(slot),
                    flags,
                    depth,
                    frame: arena.frame(slot),
                },
            ));
            for child in arena.children(slot).iter().rev() {
                stack.push((*child, depth.saturating_add(1)));
            }
        }
    }
    out
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in bytes {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn write_props(w: &mut Writer, props: &PropList) {
    w.u16(props.len() as u16);
    for (id, value) in props.iter() {
        w.u16(id as u16);
        w.u8(value.kind() as u8);
        match value {
            PropValue::Str(s) => w.string(s),
            PropValue::Bool(b) => w.u8(*b as u8),
            PropValue::Int(i) => w.i64(*i),
            PropValue::Float(f) => w.f64(*f),
        }
    }
}

/// Encode the envelope for `root` (a slot) or every root.
pub fn encode(arena: &NodeArena, root: Option<u32>, epoch: u64) -> Vec<u8> {
    let rows = rows(arena, root);
    let root_id = root.map(|r| arena.local_id(r)).unwrap_or(0);

    let mut nodes = Writer::new();
    let mut styles = Writer::new();
    let mut props = Writer::new();
    for (slot, row) in &rows {
        nodes.u32(row.id);
        nodes.u32(row.parent.unwrap_or(u32::MAX));
        nodes.u32(row.key.generation);
        nodes.u8(row.node_type as u8);
        nodes.u8(row.flags);
        nodes.u16(row.depth);
        nodes.f32(row.frame.x);
        nodes.f32(row.frame.y);
        nodes.f32(row.frame.width);
        nodes.f32(row.frame.height);
        let style = arena.style(*slot);
        style.encode_masked(style.mask, &mut styles);
        write_props(&mut props, arena.props(*slot));
    }
    nodes.pad8();
    styles.pad8();
    props.pad8();

    let sections = [
        (SECTION_NODES, nodes),
        (SECTION_STYLES, styles),
        (SECTION_PROPS, props),
    ];
    let dir_len = sections.len() * DIR_ENTRY_LEN;
    let mut offset = HEADER_LEN + dir_len;
    let mut dir = Writer::new();
    for (kind, section) in &sections {
        dir.u32(*kind);
        dir.u32(offset as u32);
        dir.u32(section.len() as u32);
        dir.u32(fnv1a32(section.as_slice()));
        offset += section.len();
    }

    let mut w = Writer::new();
    w.bytes(&MAGIC);
    w.u16(VERSION);
    w.u16(HEADER_LEN as u16);
    w.u32(offset as u32);
    w.u32(root_id);
    w.u64(epoch);
    w.u32(rows.len() as u32);
    w.u32(sections.len() as u32);
    w.u64(0);
    debug_assert_eq!(w.len(), HEADER_LEN);
    w.bytes(dir.as_slice());
    for (_, section) in &sections {
        w.bytes(section.as_slice());
    }
    w.into_vec()
}

/// A decoded envelope.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Wire id of the exported root, or 0 for every root.
    pub root_id: u32,
    /// Kernel epoch at export.
    pub epoch: u64,
    /// Node rows, preorder.
    pub rows: Vec<NodeRow>,
    /// Per-row style rows (only set rows are meaningful; see each `mask`).
    pub styles: Vec<StyleProps>,
    /// Per-row props.
    pub props: Vec<PropList>,
}

/// The directory entry for `kind`, its bytes bounds-checked and its checksum verified.
fn section<'a>(
    bytes: &'a [u8],
    dir: &[(u32, u32, u32, u32)],
    kind: u32,
) -> Result<&'a [u8], DecodeError> {
    let (_, offset, len, checksum) = *dir
        .iter()
        .find(|(k, ..)| *k == kind)
        .ok_or(DecodeError::MissingSection { section: kind })?;
    let start = offset as u64;
    let end = start + len as u64;
    if end > bytes.len() as u64 {
        return Err(DecodeError::SectionOverrun { declared: len });
    }
    let payload = &bytes[start as usize..end as usize];
    if fnv1a32(payload) != checksum {
        return Err(DecodeError::ChecksumMismatch { section: kind });
    }
    Ok(payload)
}

/// Decode an envelope, validating every length and checksum first.
pub fn decode(bytes: &[u8]) -> Result<Snapshot, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.bytes(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = r.u16()?;
    if version != VERSION {
        return Err(DecodeError::UnsupportedRevision(version));
    }
    let header_len = r.u16()?;
    if (header_len as usize) < HEADER_LEN {
        return Err(DecodeError::BadHeaderLength(header_len));
    }
    let total_len = r.u32()?;
    if total_len as usize != bytes.len() {
        return Err(DecodeError::FrameLengthMismatch {
            declared: total_len,
            actual: bytes.len(),
        });
    }
    let root_id = r.u32()?;
    let epoch = r.u64()?;
    let node_count = r.u32()?;
    let section_count = r.u32()?;
    let _reserved = r.u64()?;
    r.bytes(header_len as usize - HEADER_LEN)?;

    // Bound every count by the bytes present before allocating from it. All
    // arithmetic is in u64 so a 32-bit target cannot overflow on the way.
    if section_count as u64 * DIR_ENTRY_LEN as u64 > r.remaining() as u64 {
        return Err(DecodeError::SectionOverrun {
            declared: section_count,
        });
    }
    let mut dir = Vec::with_capacity(section_count as usize);
    for _ in 0..section_count {
        dir.push((r.u32()?, r.u32()?, r.u32()?, r.u32()?));
    }

    let nodes = section(bytes, &dir, SECTION_NODES)?;
    if node_count as u64 * ROW_LEN as u64 > nodes.len() as u64 {
        return Err(DecodeError::SectionOverrun {
            declared: node_count,
        });
    }
    let node_count = node_count as usize;
    let mut rows = Vec::with_capacity(node_count);
    let mut nr = Reader::new(nodes);
    for _ in 0..node_count {
        let id = nr.u32()?;
        let parent = nr.u32()?;
        let generation = nr.u32()?;
        let raw_type = nr.u8()?;
        let node_type =
            NodeType::from_wire(raw_type).ok_or(DecodeError::UnknownNodeType(raw_type))?;
        let flags = nr.u8()?;
        let depth = nr.u16()?;
        let frame = Frame {
            x: nr.f32()?,
            y: nr.f32()?,
            width: nr.f32()?,
            height: nr.f32()?,
        };
        rows.push(NodeRow {
            id,
            parent: (parent != u32::MAX).then_some(parent),
            key: NodeKey {
                index: u32::MAX,
                generation,
            },
            node_type,
            flags,
            depth,
            frame,
        });
    }

    let mut sr = Reader::new(section(bytes, &dir, SECTION_STYLES)?);
    let mut styles = Vec::new();
    for _ in 0..node_count {
        styles.push(StyleProps::decode_patch(&mut sr)?);
    }

    let mut pr = Reader::new(section(bytes, &dir, SECTION_PROPS)?);
    let mut props = Vec::new();
    for _ in 0..node_count {
        let count = pr.u16()?;
        let mut list = PropList::new();
        for _ in 0..count {
            let raw = pr.u16()?;
            let id = PropId::from_wire(raw).ok_or(DecodeError::UnknownProp(raw))?;
            let raw_kind = pr.u8()?;
            let kind =
                PropKind::from_wire(raw_kind).ok_or(DecodeError::UnknownPropKind(raw_kind))?;
            if kind != id.kind() {
                return Err(DecodeError::PropKindMismatch {
                    prop: id,
                    expected: id.kind(),
                    actual: kind,
                });
            }
            let value = match kind {
                PropKind::Str => PropValue::Str(pr.string()?.to_string()),
                PropKind::Bool => PropValue::Bool(pr.u8()? != 0),
                PropKind::Int => PropValue::Int(pr.i64()?),
                PropKind::Float => PropValue::Float(pr.f64()?),
            };
            list.set(id, value);
        }
        props.push(list);
    }

    Ok(Snapshot {
        root_id,
        epoch,
        rows,
        styles,
        props,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a32_matches_reference_vectors() {
        assert_eq!(fnv1a32(b""), 0x811c_9dc5);
        assert_eq!(fnv1a32(b"a"), 0xe40c_292c);
        assert_eq!(fnv1a32(b"foobar"), 0xbf9c_f968);
    }

    #[test]
    fn a_huge_section_count_is_refused_before_any_allocation() {
        let mut w = Writer::new();
        w.bytes(&MAGIC);
        w.u16(VERSION);
        w.u16(HEADER_LEN as u16);
        w.u32(HEADER_LEN as u32);
        w.u32(0);
        w.u64(0);
        w.u32(0);
        w.u32(u32::MAX);
        w.u64(0);
        let bytes = w.into_vec();
        assert_eq!(
            decode(&bytes),
            Err(DecodeError::SectionOverrun { declared: u32::MAX })
        );
    }
}
