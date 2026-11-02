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
        // What it held may have gone with the styles it let go.
        self.wide_changed();
    }
}

#[cfg(test)]
mod tests {
    use crate::{Dimension, Kernel, NodeFlags, NodeType, Offer, Op, PropId, StyleId, StyleProps};

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

    /// A node's changed geometry names the pass of the root that published
    /// it. That root can be freed and its slot trimmed while the node lives
    /// on (it moved to another root), or the node's own slot can be freed
    /// and reused: neither may read the gone root's passes, and the change
    /// expires with its root.
    #[test]
    fn a_geometry_change_published_by_a_trimmed_root_expires_with_it() {
        let mut k = Kernel::with_monospace();
        // Slots in creation order: the page root 1 at 0, then 2 (freed and
        // reused), 3 (kept), 4 (moved), filler, and a second root 200 last.
        let mut ops = vec![Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        }];
        create(&mut ops, 2..=199, 10.0);
        ops.push(Op::CreateView {
            id: 200,
            node_type: NodeType::View,
        });
        ops.push(Op::SetChildren {
            id: 1,
            children: (3..=3).chain(5..=199).collect(),
        });
        ops.push(Op::SetChildren {
            id: 200,
            children: vec![2, 4],
        });
        ops.push(Op::AttachRoot { id: 1 });
        ops.push(Op::AttachRoot { id: 200 });
        k.apply(0, 1, &ops).unwrap();
        let offer = Offer::definite(400.0, 400.0);
        k.compute_layout(1, offer).unwrap();
        k.compute_layout(200, offer).unwrap();
        let moved = k.arena().slot_of(4).unwrap();
        assert!(k.arena().flags(moved).has(NodeFlags::GEOMETRY_CHANGED));

        // 4 moves to the page; its root and everything above slot 3 go.
        let mut ops = vec![Op::SetChildren {
            id: 1,
            children: vec![3, 4],
        }];
        ops.push(Op::DestroyView { id: 200 });
        ops.extend((5..=199).map(|id| Op::DestroyView { id }));
        k.apply(0, 2, &ops).unwrap();
        k.trim();
        assert_eq!(k.arena().slot_count(), 4);
        assert!(!k.arena().flags(moved).has(NodeFlags::GEOMETRY_CHANGED));

        // A new node takes 2's freed slot, and a prop write marks it and 4.
        let ops = [
            Op::CreateView {
                id: 300,
                node_type: NodeType::View,
            },
            Op::SetProp {
                id: 300,
                prop: PropId::TestId,
                value: "fresh".into(),
            },
            Op::SetProp {
                id: 4,
                prop: PropId::TestId,
                value: "moved".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![3, 4, 300],
            },
        ];
        k.apply(0, 3, &ops).unwrap();
        assert_eq!(k.arena().slot_of(300), Some(1));
        k.compute_layout(1, offer).unwrap();
        assert_eq!(k.node(300).unwrap().frame.width, 400.0);
    }
}
