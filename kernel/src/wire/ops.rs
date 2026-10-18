//! The decoded operation vocabulary and its payload codec.
//!
//! An [`Op`] is what both ingress forms produce: EXWF byte frames decode into
//! ops, and in-process Rust producers construct ops directly. Both enter the
//! same validate-then-apply engine; there is no second write path.

use crate::error::DecodeError;
use crate::generated::{NodeType, OpCode, PropId, PropKind, StyleMask, StyleProps};
use crate::id::ViewId;
use crate::props::PropValue;
use crate::wire::codec::{Reader, Writer};

/// Bound on one `SetChildren` list.
pub const MAX_CHILDREN: u32 = 1 << 20;

/// One tree mutation.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// Allocate `id` as a node of `node_type`. Idempotent on a live node of the
    /// same type; a live node of another type is a rejection.
    CreateView { id: ViewId, node_type: NodeType },
    /// Destroy `id` and its whole subtree.
    DestroyView { id: ViewId },
    /// Set one typed prop.
    SetProp {
        id: ViewId,
        prop: PropId,
        value: PropValue,
    },
    /// Remove one prop (a no-op when absent).
    ClearProp { id: ViewId, prop: PropId },
    /// Apply a masked style patch.
    SetStyle { id: ViewId, patch: Box<StyleProps> },
    /// Clear own values; `written` retains an explicit authored CSS clear.
    ClearStyle {
        id: ViewId,
        mask: StyleMask,
        written: bool,
    },
    /// Replace the ordered child list.
    SetChildren { id: ViewId, children: Vec<ViewId> },
    /// Register `id` as a layout root. Roots have no parent.
    AttachRoot { id: ViewId },
    /// Replace `id`'s host-known state looks (LLP 1121 D8): each a state
    /// ([`LOOK_ACTIVE`], [`LOOK_HOVER`], [`LOOK_FOCUS_VISIBLE`]; ascending,
    /// each once) and a patch of [`look_rows`] only, resolved as a static
    /// row is. Empty clears them.
    SetLooks {
        id: ViewId,
        looks: Vec<(u8, StyleProps)>,
    },
}

/// A look's state: `:active`, the host's press.
pub const LOOK_ACTIVE: u8 = 0;
/// A look's state: `:hover`.
pub const LOOK_HOVER: u8 = 1;
/// A look's state: `:focus-visible`.
pub const LOOK_FOCUS_VISIBLE: u8 = 2;

/// The rows a state look may set (LLP 1121 D8, stage 8 A's lowering).
pub fn look_rows() -> StyleMask {
    let mut mask = StyleMask::EMPTY;
    for id in [
        crate::StyleId::BackgroundColor,
        crate::StyleId::BorderColorTop,
        crate::StyleId::BorderColorRight,
        crate::StyleId::BorderColorBottom,
        crate::StyleId::BorderColorLeft,
        crate::StyleId::Opacity,
        crate::StyleId::BoxShadow,
        crate::StyleId::PressScale,
    ] {
        mask.set(id);
    }
    mask
}

impl Op {
    /// The opcode.
    pub fn opcode(&self) -> OpCode {
        match self {
            Op::CreateView { .. } => OpCode::CreateView,
            Op::DestroyView { .. } => OpCode::DestroyView,
            Op::SetProp { .. } => OpCode::SetProp,
            Op::ClearProp { .. } => OpCode::ClearProp,
            Op::SetStyle { .. } => OpCode::SetStyle,
            Op::ClearStyle { .. } => OpCode::ClearStyle,
            Op::SetChildren { .. } => OpCode::SetChildren,
            Op::AttachRoot { .. } => OpCode::AttachRoot,
            Op::SetLooks { .. } => OpCode::SetLooks,
        }
    }

    /// The node the op targets.
    pub fn target(&self) -> ViewId {
        match self {
            Op::CreateView { id, .. }
            | Op::DestroyView { id }
            | Op::SetProp { id, .. }
            | Op::ClearProp { id, .. }
            | Op::SetStyle { id, .. }
            | Op::ClearStyle { id, .. }
            | Op::SetChildren { id, .. }
            | Op::AttachRoot { id }
            | Op::SetLooks { id, .. } => *id,
        }
    }

    /// Encode the payload (everything after the op header).
    pub fn encode_payload(&self, w: &mut Writer) {
        match self {
            Op::CreateView { node_type, .. } => w.u8(*node_type as u8),
            Op::DestroyView { .. } | Op::AttachRoot { .. } => {}
            Op::SetProp { prop, value, .. } => {
                w.u16(*prop as u16);
                w.u8(value.kind() as u8);
                match value {
                    PropValue::Str(s) => w.string(s),
                    PropValue::Bool(b) => w.u8(*b as u8),
                    PropValue::Int(i) => w.i64(*i),
                    PropValue::Float(f) => w.f64(*f),
                }
            }
            Op::ClearProp { prop, .. } => w.u16(*prop as u16),
            Op::SetStyle { patch, .. } => patch.encode_patch(w),
            Op::ClearStyle { mask, written, .. } => {
                w.style_mask(*mask);
                w.u8(u8::from(*written));
            }
            Op::SetChildren { children, .. } => {
                w.u32(children.len() as u32);
                for child in children {
                    w.u32(*child);
                }
            }
            Op::SetLooks { looks, .. } => {
                w.u8(looks.len() as u8);
                for (state, patch) in looks {
                    w.u8(*state);
                    patch.encode_patch(w);
                }
            }
        }
    }

    /// Decode a payload for `opcode` targeting `id`. The reader must be scoped
    /// to exactly the payload; leftover bytes are a rejection.
    pub fn decode_payload(
        opcode: OpCode,
        id: ViewId,
        r: &mut Reader<'_>,
    ) -> Result<Op, DecodeError> {
        let op = match opcode {
            OpCode::CreateView => {
                let raw = r.u8()?;
                let node_type =
                    NodeType::from_wire(raw).ok_or(DecodeError::UnknownNodeType(raw))?;
                Op::CreateView { id, node_type }
            }
            OpCode::DestroyView => Op::DestroyView { id },
            OpCode::SetProp => {
                let raw = r.u16()?;
                let prop = PropId::from_wire(raw).ok_or(DecodeError::UnknownProp(raw))?;
                let raw_kind = r.u8()?;
                let kind =
                    PropKind::from_wire(raw_kind).ok_or(DecodeError::UnknownPropKind(raw_kind))?;
                if kind != prop.kind() {
                    return Err(DecodeError::PropKindMismatch {
                        prop,
                        expected: prop.kind(),
                        actual: kind,
                    });
                }
                let value = match kind {
                    PropKind::Str => PropValue::Str(r.string()?.into()),
                    PropKind::Bool => PropValue::Bool(r.u8()? != 0),
                    PropKind::Int => PropValue::Int(r.i64()?),
                    PropKind::Float => PropValue::Float(r.f64()?),
                };
                Op::SetProp { id, prop, value }
            }
            OpCode::ClearProp => {
                let raw = r.u16()?;
                let prop = PropId::from_wire(raw).ok_or(DecodeError::UnknownProp(raw))?;
                Op::ClearProp { id, prop }
            }
            OpCode::SetStyle => Op::SetStyle {
                id,
                patch: Box::new(StyleProps::decode_patch(r)?),
            },
            OpCode::ClearStyle => Op::ClearStyle {
                id,
                mask: r.style_mask()?,
                written: match r.u8()? {
                    0 => false,
                    1 => true,
                    value => return Err(DecodeError::NonCanonicalBool(value)),
                },
            },
            OpCode::SetChildren => {
                let count = r.u32()?;
                if count > MAX_CHILDREN {
                    return Err(DecodeError::TooManyChildren(count));
                }
                // Bounded pre-allocation: a 24-byte op must not buy megabytes up front.
                let mut children = Vec::with_capacity((count as usize).min(1024));
                for _ in 0..count {
                    children.push(r.u32()?);
                }
                Op::SetChildren { id, children }
            }
            OpCode::AttachRoot => Op::AttachRoot { id },
            OpCode::SetLooks => {
                let count = r.u8()?;
                if count > 3 {
                    return Err(DecodeError::TooManyLooks(count));
                }
                let mut looks = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let state = r.u8()?;
                    // Ascending, each once: one encoding of one set.
                    if state > LOOK_FOCUS_VISIBLE || looks.last().is_some_and(|(s, _)| *s >= state)
                    {
                        return Err(DecodeError::LookState(state));
                    }
                    looks.push((state, StyleProps::decode_patch(r)?));
                }
                Op::SetLooks { id, looks }
            }
        };
        if !r.is_empty() {
            return Err(DecodeError::TrailingPayload {
                opcode,
                remaining: r.remaining(),
            });
        }
        Ok(op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{FlexDirection, StyleId};
    use crate::style::Dimension;

    fn round_trip(op: Op) {
        let mut w = Writer::new();
        op.encode_payload(&mut w);
        let bytes = w.into_vec();
        let decoded =
            Op::decode_payload(op.opcode(), op.target(), &mut Reader::new(&bytes)).unwrap();
        assert_eq!(decoded, op);
    }

    #[test]
    fn set_looks_round_trips_and_refuses_a_state_twice_or_out_of_order() {
        let mut active = StyleProps::default();
        active.opacity = 0.5;
        active.mask.set(StyleId::Opacity);
        let hover = active.clone();
        round_trip(Op::SetLooks {
            id: 7,
            looks: vec![(LOOK_ACTIVE, active.clone()), (LOOK_HOVER, hover)],
        });
        round_trip(Op::SetLooks {
            id: 7,
            looks: vec![],
        });
        for looks in [
            vec![(LOOK_HOVER, active.clone()), (LOOK_ACTIVE, active.clone())],
            vec![(LOOK_ACTIVE, active.clone()), (LOOK_ACTIVE, active.clone())],
            vec![(3, active.clone())],
        ] {
            let mut w = Writer::new();
            Op::SetLooks { id: 7, looks }.encode_payload(&mut w);
            let bytes = w.into_vec();
            assert!(Op::decode_payload(OpCode::SetLooks, 7, &mut Reader::new(&bytes)).is_err());
        }
    }

    #[test]
    fn every_op_round_trips() {
        let mut patch = StyleProps::default();
        patch.width = Dimension::Percent(50.0);
        patch.mask.set(StyleId::Width);
        // One row its author wrote, one the compiler implied (LLP 1115.000 D2).
        patch.written.set(StyleId::Width);
        patch.flex_direction = FlexDirection::Row;
        patch.mask.set(StyleId::FlexDirection);
        let mut mask = StyleMask::EMPTY;
        mask.set(StyleId::Opacity);
        for op in [
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::DestroyView { id: 2 },
            Op::SetProp {
                id: 3,
                prop: PropId::Text,
                value: "hi".into(),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::Disabled,
                value: true.into(),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::TabIndex,
                value: (-2i64).into(),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::HitSlop,
                value: 4.5f64.into(),
            },
            Op::ClearProp {
                id: 3,
                prop: PropId::Text,
            },
            Op::SetStyle {
                id: 4,
                patch: Box::new(patch),
            },
            Op::ClearStyle {
                id: 4,
                mask,
                written: false,
            },
            Op::ClearStyle {
                id: 4,
                mask,
                written: true,
            },
            Op::SetChildren {
                id: 5,
                children: vec![1, 2, 3],
            },
            Op::AttachRoot { id: 5 },
        ] {
            round_trip(op);
        }
    }

    #[test]
    fn a_clear_with_a_noncanonical_author_bit_is_rejected() {
        let mut w = Writer::new();
        w.style_mask(StyleMask::of(StyleId::TextColor));
        w.u8(2);
        assert_eq!(
            Op::decode_payload(OpCode::ClearStyle, 1, &mut Reader::new(w.as_slice())),
            Err(DecodeError::NonCanonicalBool(2))
        );
    }

    #[test]
    fn explicit_clear_intent_round_trips_with_and_without_values() {
        for value in [false, true] {
            let mut patch = StyleProps::default();
            patch.written.set(StyleId::TextColor);
            if value {
                patch.width = Dimension::Points(24.0);
                patch.mask.set(StyleId::Width);
                patch.written.set(StyleId::Width);
            }
            round_trip(Op::SetStyle {
                id: 1,
                patch: Box::new(patch),
            });
        }
    }

    #[test]
    fn wrong_prop_kind_is_a_decode_rejection() {
        let mut w = Writer::new();
        w.u16(PropId::Disabled as u16);
        w.u8(PropKind::Str as u8);
        w.string("true");
        let bytes = w.into_vec();
        assert_eq!(
            Op::decode_payload(OpCode::SetProp, 1, &mut Reader::new(&bytes)),
            Err(DecodeError::PropKindMismatch {
                prop: PropId::Disabled,
                expected: PropKind::Bool,
                actual: PropKind::Str
            })
        );
    }

    #[test]
    fn trailing_payload_bytes_are_rejected() {
        let mut w = Writer::new();
        w.u8(NodeType::View as u8);
        w.u8(0);
        let bytes = w.into_vec();
        assert_eq!(
            Op::decode_payload(OpCode::CreateView, 1, &mut Reader::new(&bytes)),
            Err(DecodeError::TrailingPayload {
                opcode: OpCode::CreateView,
                remaining: 1
            })
        );
    }

    #[test]
    fn unknown_node_type_is_rejected() {
        let bytes = [200u8];
        assert_eq!(
            Op::decode_payload(OpCode::CreateView, 1, &mut Reader::new(&bytes)),
            Err(DecodeError::UnknownNodeType(200))
        );
    }
}
