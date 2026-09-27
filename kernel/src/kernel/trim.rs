//! Giving back what a settled tree no longer needs.

use super::Kernel;
use crate::layout::LayoutMirror;

impl Kernel {
    /// Return the storage the live nodes no longer need: columns sized to
    /// the most nodes ever live, styles no node holds, and tables' spare
    /// room. A host calls it when its content settles (a scroll comes to
    /// rest); nothing a reader sees changes.
    pub fn trim(&mut self) {
        self.arena.trim();
        if let Some(tree) = self.layout.as_deref_mut().and_then(LayoutMirror::tree) {
            tree.trim();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Dimension, Kernel, NodeType, Offer, Op, StyleId, StyleProps};

    fn styled(width: f32) -> Box<StyleProps> {
        let mut s = StyleProps::default();
        s.width = Dimension::Points(width);
        s.mask.set(StyleId::Width);
        Box::new(s)
    }

    fn create(ops: &mut Vec<Op>, ids: std::ops::RangeInclusive<u32>, width: f32) {
        for id in ids {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::View,
            });
            ops.push(Op::SetStyle {
                id,
                patch: styled(width),
            });
        }
    }

    #[test]
    fn a_trim_gives_back_trailing_slots_and_keeps_identity_and_layout() {
        let mut k = Kernel::with_monospace();
        let mut ops = vec![Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        }];
        create(&mut ops, 2..=500, 10.0);
        ops.push(Op::SetChildren {
            id: 1,
            children: (2..=500).collect(),
        });
        ops.push(Op::AttachRoot { id: 1 });
        k.apply(0, 1, &ops).unwrap();
        let before = k.node(50).unwrap().key;
        assert_eq!(
            k.arena().shared_style_count(),
            1,
            "499 equal styles, one allocation"
        );
        let mut ops = vec![Op::SetChildren {
            id: 1,
            children: (2..=10).collect(),
        }];
        ops.extend((11..=500).map(|id| Op::DestroyView { id }));
        k.apply(0, 2, &ops).unwrap();
        k.trim();
        assert_eq!(k.arena().slot_count(), 10);
        // A node in a trimmed slot is a new identity; layout is unchanged.
        let mut ops = Vec::new();
        create(&mut ops, 501..=600, 20.0);
        ops.push(Op::SetChildren {
            id: 1,
            children: (2..=10).chain(501..=600).collect(),
        });
        k.apply(0, 3, &ops).unwrap();
        assert!(k.arena().resolve(before).is_none());
        assert_eq!(k.arena().shared_style_count(), 2);
        k.compute_layout(1, Offer::definite(400.0, 400.0)).unwrap();
        assert_eq!(k.node(550).unwrap().frame.width, 20.0);
        assert_eq!(k.node(5).unwrap().frame.width, 10.0);
    }
}
