//! Source eligibility for the one centered-fill photo trial; geometry is host-owned.
use exact_kernel::{
    wire, BoxSizing, Dimension, Display, Kernel, NodeKey, NodeType, Op, Overflow, PropId,
    PropValue, StyleId, StyleProps, TransformDragBinding, Vec2,
};

fn prop(id: u32, prop: PropId, value: impl Into<PropValue>) -> Op {
    Op::SetProp {
        id,
        prop,
        value: value.into(),
    }
}
fn style(id: u32, f: impl FnOnce(&mut StyleProps)) -> Op {
    let mut patch = StyleProps::default();
    f(&mut patch);
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn children(id: u32, children: &[u32]) -> Op {
    Op::SetChildren {
        id,
        children: children.to_vec(),
    }
}
fn apply(k: &mut Kernel, ops: &[Op]) {
    k.apply(0, k.epoch() + 1, ops).unwrap();
}
fn clip_style(id: u32) -> Op {
    style(id, |s| {
        s.overflow_x = Overflow::Hidden;
        s.overflow_y = Overflow::Hidden;
        s.mask.set(StyleId::OverflowX);
        s.mask.set(StyleId::OverflowY);
    })
}
fn fixture() -> (Kernel, TransformDragBinding) {
    let mut k = Kernel::with_monospace();
    let mut ops: Vec<_> = (1..=6)
        .map(|id| Op::CreateView {
            id,
            node_type: NodeType::View,
        })
        .collect();
    ops.extend([
        clip_style(2),
        prop(3, PropId::Id, "photo"),
        prop(5, PropId::TransformDragFor, "photo"),
        style(3, |s| {
            s.width = Dimension::Percent(100.0);
            s.height = Dimension::Percent(100.0);
            s.box_sizing = BoxSizing::BorderBox;
            for id in [StyleId::Width, StyleId::Height, StyleId::BoxSizing] {
                s.mask.set(id);
            }
        }),
        children(1, &[2, 6]),
        children(2, &[3]),
        children(3, &[4]),
        children(4, &[5]),
        Op::AttachRoot { id: 1 },
    ]);
    k.apply_frame(&wire::encode(0, 1, &ops)).unwrap();
    let b = TransformDragBinding {
        handle: k.node(5).unwrap().key,
        target: k.node(3).unwrap().key,
        clip: k.node(2).unwrap().key,
    };
    (k, b)
}

#[test]
fn coherent_binding_uses_unique_strict_ancestor_id_and_immediate_clip() {
    let (mut k, b) = fixture();
    assert_eq!(PropId::TransformDragFor as u16, 76);
    assert_eq!(k.transform_drag_binding(b.handle), Some(b));
    apply(&mut k, &[prop(6, PropId::Id, "photo")]);
    assert_eq!(k.transform_drag_binding(b.handle), Some(b));
    // Root is not an eligible target, but duplicate ID still makes resolution ambiguous.
    apply(&mut k, &[prop(1, PropId::Id, "photo")]);
    assert_eq!(k.transform_drag_binding(b.handle), None);
    apply(
        &mut k,
        &[
            prop(1, PropId::Id, "root"),
            prop(3, PropId::Id, "other"),
            prop(3, PropId::TestId, "photo"),
            prop(3, PropId::NativeId, "photo"),
            prop(5, PropId::Id, "photo"),
        ],
    );
    assert_eq!(k.transform_drag_binding(b.handle), None);
}

#[test]
fn every_handle_to_root_node_can_revoke_source_eligibility() {
    for id in 1..=5 {
        for p in [PropId::Disabled, PropId::Inert] {
            let (mut k, b) = fixture();
            apply(&mut k, &[prop(id, p, true)]);
            assert_eq!(k.transform_drag_binding(b.handle), None, "{id}/{p:?}");
            apply(&mut k, &[prop(id, p, false)]);
            assert_eq!(k.transform_drag_binding(b.handle), Some(b));
        }
        let (mut k, b) = fixture();
        apply(
            &mut k,
            &[style(id, |s| {
                s.display = Display::None;
                s.mask.set(StyleId::Display);
            })],
        );
        assert_eq!(k.transform_drag_binding(b.handle), None);
    }
}

#[test]
fn centered_fill_sources_refuse_offsets_insets_nonclip_and_nonpositive_scale() {
    let cases = [
        style(3, |s| {
            s.width = Dimension::Auto;
            s.mask.set(StyleId::Width);
        }),
        style(3, |s| {
            s.height = Dimension::Percent(90.0);
            s.mask.set(StyleId::Height);
        }),
        style(3, |s| {
            s.box_sizing = BoxSizing::ContentBox;
            s.mask.set(StyleId::BoxSizing);
        }),
        style(3, |s| {
            s.margin_left = Dimension::Points(1.0);
            s.mask.set(StyleId::MarginLeft);
        }),
        style(3, |s| {
            s.left = Dimension::Points(1.0);
            s.mask.set(StyleId::Left);
        }),
        style(3, |s| {
            s.padding_left = Dimension::Percent(1.0);
            s.mask.set(StyleId::PaddingLeft);
        }),
        style(2, |s| {
            s.padding_top = Dimension::Points(1.0);
            s.mask.set(StyleId::PaddingTop);
        }),
        style(2, |s| {
            s.overflow_x = Overflow::Scroll;
            s.mask.set(StyleId::OverflowX);
        }),
        style(2, |s| {
            s.overflow_y = Overflow::Visible;
            s.mask.set(StyleId::OverflowY);
        }),
        style(3, |s| {
            s.scale = 0.0;
            s.mask.set(StyleId::Scale);
        }),
        style(3, |s| {
            s.scale = -1.0;
            s.mask.set(StyleId::Scale);
        }),
    ];
    for op in cases {
        let (mut k, b) = fixture();
        apply(&mut k, &[op]);
        assert_eq!(k.transform_drag_binding(b.handle), None);
    }
    let (mut k, b) = fixture();
    apply(
        &mut k,
        &[style(3, |s| {
            s.translate = Vec2 { x: -50.0, y: 25.0 };
            s.scale = f32::from_bits(1);
            s.mask.set(StyleId::Translate);
            s.mask.set(StyleId::Scale);
        })],
    );
    assert_eq!(
        k.transform_drag_binding(b.handle),
        Some(b),
        "positive representable subnormals are not invented zero"
    );
}

#[test]
fn rotation_everywhere_and_transform_outside_target_are_unsupported() {
    for id in 1..=5 {
        let (mut k, b) = fixture();
        apply(
            &mut k,
            &[style(id, |s| {
                s.rotate = 1.0;
                s.mask.set(StyleId::Rotate);
            })],
        );
        assert_eq!(k.transform_drag_binding(b.handle), None);
        if id != 3 {
            for scale in [false, true] {
                let (mut k, b) = fixture();
                apply(
                    &mut k,
                    &[style(id, |s| {
                        if scale {
                            s.scale = 2.0;
                            s.mask.set(StyleId::Scale);
                        } else {
                            s.translate = Vec2 { x: 1.0, y: 0.0 };
                            s.mask.set(StyleId::Translate);
                        }
                    })],
                );
                assert_eq!(k.transform_drag_binding(b.handle), None);
            }
        }
    }
}

#[test]
fn reparent_detach_and_clip_generation_changes_cannot_keep_old_binding() {
    let (mut k, b) = fixture();
    apply(&mut k, &[children(2, &[])]);
    assert_eq!(k.transform_drag_binding(b.handle), None);
    apply(
        &mut k,
        &[
            Op::DestroyView { id: 2 },
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            clip_style(2),
            children(1, &[2, 6]),
            children(2, &[3]),
        ],
    );
    let new = k.transform_drag_binding(b.handle).unwrap();
    assert_eq!(new.target, b.target);
    assert_eq!(new.handle, b.handle);
    assert_ne!(new.clip, b.clip);
    apply(
        &mut k,
        &[
            Op::DestroyView { id: 5 },
            Op::CreateView {
                id: 5,
                node_type: NodeType::View,
            },
            prop(5, PropId::TransformDragFor, "photo"),
            children(4, &[5]),
        ],
    );
    assert_eq!(k.transform_drag_binding(b.handle), None);
    assert!(k.transform_drag_binding(k.node(5).unwrap().key).is_some());
    assert_eq!(
        k.transform_drag_binding(NodeKey {
            index: u32::MAX,
            generation: 1
        }),
        None
    );
    k.reset();
    assert_eq!(k.transform_drag_binding(new.handle), None);
}

#[test]
fn invalid_wire_prop_leaves_existing_binding_intact() {
    let (mut k, b) = fixture();
    let epoch = k.epoch();
    let bytes = wire::encode(0, epoch + 1, &[prop(5, PropId::TransformDragFor, true)]);
    assert!(k.apply_frame(&bytes).is_err());
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.transform_drag_binding(b.handle), Some(b));
}
