//! EXWF — the one wire format, frame revision 1.
//!
//! A frame is a fixed 40-byte header followed by ops. Every op is a 16-byte
//! header plus a payload padded to 8 bytes, so op headers are always 8-aligned.
//! The frame carries its own checked length and the schema digest of the
//! producer's generated tables; there is no trusted op count — a consumer
//! iterates by validated lengths until the frame ends.
//!
//! ```text
//! frame header (40 bytes)
//!   0  magic       "EXWF"
//!   4  revision    u16 = 1
//!   6  header_len  u16 = 40
//!   8  frame_len   u32   total bytes including this header; multiple of 8
//!  12  root_id     u32   root scope (0 = whole kernel)
//!  16  batch       u64   producer batch number (monotonic per producer)
//!  24  schema      u64   SCHEMA_DIGEST of the producer's tables
//!  32  flags       u32   reserved, must be 0
//!  36  reserved    u32   must be 0
//! op header (16 bytes, 8-aligned)
//!   0  opcode      u16
//!   2  flags       u16   reserved, must be 0
//!   4  view_id     u32
//!   8  payload_len u32   unpadded payload length
//!  12  reserved    u32   must be 0
//! payload (payload_len bytes, then zero padding to 8)
//! ```
//!
//! Decoding is total: every malformation names a [`DecodeError`], and a frame
//! that fails anywhere applies nothing.

use crate::error::DecodeError;
use crate::generated::{OpCode, SCHEMA_DIGEST};
use crate::id::ViewId;
use crate::wire::codec::{Reader, Writer};
use crate::wire::ops::Op;

/// The four magic bytes.
pub const MAGIC: [u8; 4] = *b"EXWF";
/// The frame revision this kernel reads and writes.
pub const REVISION: u16 = 1;
/// Fixed header size.
pub const HEADER_LEN: usize = 40;
/// Op header size.
pub const OP_HEADER_LEN: usize = 16;

/// A decoded frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Root scope (0 = whole kernel).
    pub root_id: u32,
    /// Producer batch number.
    pub batch: u64,
    /// The ops, in order.
    pub ops: Vec<Op>,
}

/// Decode a complete frame. `bytes` must be exactly one frame.
pub fn decode(bytes: &[u8]) -> Result<Frame, DecodeError> {
    let mut r = Reader::new(bytes);
    let magic = r.bytes(4)?;
    if magic != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let revision = r.u16()?;
    if revision != REVISION {
        return Err(DecodeError::UnsupportedRevision(revision));
    }
    let header_len = r.u16()?;
    if (header_len as usize) < HEADER_LEN || header_len % 8 != 0 {
        return Err(DecodeError::BadHeaderLength(header_len));
    }
    let frame_len = r.u32()?;
    if frame_len as usize != bytes.len() {
        return Err(DecodeError::FrameLengthMismatch {
            declared: frame_len,
            actual: bytes.len(),
        });
    }
    if frame_len % 8 != 0 {
        return Err(DecodeError::FrameNotAligned(frame_len));
    }
    let root_id = r.u32()?;
    let batch = r.u64()?;
    let digest = r.u64()?;
    if digest != SCHEMA_DIGEST {
        return Err(DecodeError::SchemaDigestMismatch {
            expected: SCHEMA_DIGEST,
            actual: digest,
        });
    }
    let flags = r.u32()?;
    let reserved = r.u32()?;
    if flags != 0 || reserved != 0 {
        return Err(DecodeError::ReservedFlags);
    }
    // A future revision may grow the header; revision 1 readers skip what they
    // do not know only because the revision check above already passed.
    r.bytes(header_len as usize - HEADER_LEN)?;

    let mut ops = Vec::new();
    while !r.is_empty() {
        let raw_opcode = r.u16()?;
        let opcode = OpCode::from_wire(raw_opcode).ok_or(DecodeError::UnknownOpcode(raw_opcode))?;
        let op_flags = r.u16()?;
        let view_id: ViewId = r.u32()?;
        let payload_len = r.u32()?;
        let op_reserved = r.u32()?;
        if op_flags != 0 || op_reserved != 0 {
            return Err(DecodeError::ReservedFlags);
        }
        // u64 so a 32-bit target cannot overflow on a hostile length.
        let padded = (payload_len as u64 + 7) & !7;
        if padded > r.remaining() as u64 {
            return Err(DecodeError::PayloadOverrun {
                opcode,
                declared: payload_len,
            });
        }
        let payload = r.bytes(payload_len as usize)?;
        let mut pr = Reader::new(payload);
        ops.push(Op::decode_payload(opcode, view_id, &mut pr)?);
        r.bytes((padded - payload_len as u64) as usize)?;
    }
    Ok(Frame {
        root_id,
        batch,
        ops,
    })
}

/// Builds one frame.
#[derive(Debug, Clone)]
pub struct FrameBuilder {
    root_id: u32,
    batch: u64,
    body: Writer,
}

impl FrameBuilder {
    /// Start a frame for `root_id` (0 = whole kernel) with producer batch `batch`.
    pub fn new(root_id: u32, batch: u64) -> Self {
        FrameBuilder {
            root_id,
            batch,
            body: Writer::new(),
        }
    }

    /// Append one op.
    pub fn op(&mut self, op: &Op) -> &mut Self {
        let mut payload = Writer::new();
        op.encode_payload(&mut payload);
        self.body.u16(op.opcode() as u16);
        self.body.u16(0);
        self.body.u32(op.target());
        self.body.u32(payload.len() as u32);
        self.body.u32(0);
        self.body.bytes(payload.as_slice());
        self.body.pad8();
        self
    }

    /// Append every op.
    pub fn ops<'a>(&mut self, ops: impl IntoIterator<Item = &'a Op>) -> &mut Self {
        for op in ops {
            self.op(op);
        }
        self
    }

    /// Finish the frame.
    pub fn build(self) -> Vec<u8> {
        let mut w = Writer::new();
        w.bytes(&MAGIC);
        w.u16(REVISION);
        w.u16(HEADER_LEN as u16);
        w.u32((HEADER_LEN + self.body.len()) as u32);
        w.u32(self.root_id);
        w.u64(self.batch);
        w.u64(SCHEMA_DIGEST);
        w.u32(0);
        w.u32(0);
        debug_assert_eq!(w.len(), HEADER_LEN);
        w.bytes(self.body.as_slice());
        w.into_vec()
    }
}

/// Encode `ops` as one frame.
pub fn encode(root_id: u32, batch: u64, ops: &[Op]) -> Vec<u8> {
    let mut b = FrameBuilder::new(root_id, batch);
    b.ops(ops);
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{NodeType, PropId};

    fn sample_ops() -> Vec<Op> {
        vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "hello".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
        ]
    }

    #[test]
    fn frame_round_trips_and_is_aligned() {
        let ops = sample_ops();
        let bytes = encode(7, 42, &ops);
        assert_eq!(bytes.len() % 8, 0);
        let frame = decode(&bytes).unwrap();
        assert_eq!(frame.root_id, 7);
        assert_eq!(frame.batch, 42);
        assert_eq!(frame.ops, ops);
    }

    #[test]
    fn empty_frame_decodes_to_no_ops() {
        let bytes = encode(0, 1, &[]);
        assert_eq!(bytes.len(), HEADER_LEN);
        assert!(decode(&bytes).unwrap().ops.is_empty());
    }

    #[test]
    fn bad_magic_is_rejected() {
        let mut bytes = encode(0, 1, &sample_ops());
        bytes[0] = b'X';
        assert_eq!(decode(&bytes), Err(DecodeError::BadMagic));
    }

    #[test]
    fn wrong_digest_is_rejected() {
        let mut bytes = encode(0, 1, &sample_ops());
        bytes[24] ^= 0xff;
        assert!(matches!(
            decode(&bytes),
            Err(DecodeError::SchemaDigestMismatch { .. })
        ));
    }

    #[test]
    fn length_mismatch_is_rejected() {
        let bytes = encode(0, 1, &sample_ops());
        let truncated = &bytes[..bytes.len() - 8];
        assert!(matches!(
            decode(truncated),
            Err(DecodeError::FrameLengthMismatch { .. })
        ));
    }

    #[test]
    fn payload_overrun_is_rejected() {
        let mut bytes = encode(
            0,
            1,
            &[Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            }],
        );
        // Claim a payload longer than the frame.
        let pos = HEADER_LEN + 8;
        bytes[pos..pos + 4].copy_from_slice(&1000u32.to_le_bytes());
        assert!(matches!(
            decode(&bytes),
            Err(DecodeError::PayloadOverrun { .. })
        ));
    }

    #[test]
    fn reserved_flags_are_rejected() {
        let mut bytes = encode(0, 1, &sample_ops());
        bytes[32] = 1;
        assert_eq!(decode(&bytes), Err(DecodeError::ReservedFlags));
        let mut bytes = encode(0, 1, &sample_ops());
        bytes[HEADER_LEN + 2] = 1;
        assert_eq!(decode(&bytes), Err(DecodeError::ReservedFlags));
    }

    #[test]
    fn unknown_opcode_is_rejected() {
        let mut bytes = encode(0, 1, &sample_ops());
        bytes[HEADER_LEN] = 0xee;
        assert_eq!(decode(&bytes), Err(DecodeError::UnknownOpcode(0xee)));
    }

    #[test]
    fn unsupported_revision_is_rejected() {
        let mut bytes = encode(0, 1, &sample_ops());
        bytes[4] = 2;
        assert_eq!(decode(&bytes), Err(DecodeError::UnsupportedRevision(2)));
    }
}
