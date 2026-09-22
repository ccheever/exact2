//! Generation-safe, ancestor-only authored sheet handles; no selector scan.

use exact_kernel::{
    wire, BoxSizing, Dimension, Display, Kernel, NodeKey, NodeType, Op, PropId, PropValue, StyleId,
    StyleProps,
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

fn apply(k: &mut Kernel, ops: &[Op]) {
    k.apply(0, k.epoch() + 1, ops).unwrap();
}

fn fixture() -> (Kernel, NodeKey, NodeKey) {
    let mut k = Kernel::with_monospace();
    let mut ops: Vec<_> = (1..=5)
        .map(|id| Op::CreateView {
            id,
            node_type: NodeType::View,
        })
        .collect();
    ops.extend([
        prop(2, PropId::Id, "sheet"),
        prop(4, PropId::HeightDragFor, "sheet"),
        style(2, |s| {
            s.height = Dimension::Points(360.0);
            s.box_sizing = BoxSizing::BorderBox;
            s.max_height = Dimension::Percent(1.0);
            for id in [StyleId::Height, StyleId::BoxSizing, StyleId::MaxHeight] {
                s.mask.set(id);
            }
        }),
        Op::SetChildren {
            id: 1,
            children: vec![2, 5],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3],
        },
        Op::SetChildren {
            id: 3,
            children: vec![4],
        },
        Op::AttachRoot { id: 1 },
    ]);
    // The new prop must traverse the same typed EXWF codec as all other props.
    let mut wire = wire::FrameBuilder::new(0, 1);
    for op in &ops {
        wire.op(op);
    }
    k.apply_frame(&wire.build()).unwrap();
    let (handle, target) = (k.node(4).unwrap().key, k.node(2).unwrap().key);
    (k, handle, target)
}

#[test]
fn only_the_unique_authored_ancestor_id_resolves() {
    let (mut k, handle, target) = fixture();
    assert_eq!(PropId::HeightDragFor as u16, 75);
    assert_eq!(k.height_drag_target(handle), Some(target));
    // An unrelated sibling ID does not make the ancestor-scoped binding ambiguous.
    apply(&mut k, &[prop(5, PropId::Id, "sheet")]);
    assert_eq!(k.height_drag_target(handle), Some(target));
    // A duplicate ancestor refuses even when that duplicate is not height-eligible.
    apply(&mut k, &[prop(1, PropId::Id, "sheet")]);
    assert_eq!(k.height_drag_target(handle), None);
    apply(
        &mut k,
        &[
            prop(1, PropId::Id, "root"),
            prop(2, PropId::Id, "other"),
            prop(2, PropId::TestId, "sheet"),
            prop(2, PropId::NativeId, "sheet"),
        ],
    );
    assert_eq!(k.height_drag_target(handle), None);
    apply(&mut k, &[prop(4, PropId::Id, "sheet")]);
    assert_eq!(
        k.height_drag_target(handle),
        None,
        "self is not an ancestor"
    );
    apply(
        &mut k,
        &[
            prop(2, PropId::Id, "sheet"),
            prop(4, PropId::HeightDragFor, ""),
        ],
    );
    assert_eq!(k.height_drag_target(handle), None);
}

#[test]
fn every_node_on_the_handle_to_root_path_can_revoke_binding() {
    for id in 1..=4 {
        for property in [PropId::Inert, PropId::Disabled] {
            let (mut k, handle, target) = fixture();
            apply(&mut k, &[prop(id, property, true)]);
            assert_eq!(k.height_drag_target(handle), None, "{id}/{property:?}");
            apply(&mut k, &[prop(id, property, false)]);
            assert_eq!(k.height_drag_target(handle), Some(target));
        }
        let (mut k, handle, target) = fixture();
        apply(
            &mut k,
            &[style(id, |s| {
                s.display = Display::None;
                s.mask.set(StyleId::Display);
            })],
        );
        assert_eq!(k.height_drag_target(handle), None);
        apply(
            &mut k,
            &[style(id, |s| {
                s.display = Display::Block;
                s.mask.set(StyleId::Display);
            })],
        );
        assert_eq!(k.height_drag_target(handle), Some(target));
    }
}

#[test]
fn numeric_border_box_is_required_without_resolving_auto_or_percent() {
    for height in [Dimension::Auto, Dimension::Percent(0.5)] {
        let (mut k, handle, _) = fixture();
        apply(
            &mut k,
            &[style(2, |s| {
                s.height = height;
                s.mask.set(StyleId::Height);
            })],
        );
        assert_eq!(k.height_drag_target(handle), None);
    }
    let (mut k, handle, _) = fixture();
    apply(
        &mut k,
        &[style(2, |s| {
            s.box_sizing = BoxSizing::ContentBox;
            s.mask.set(StyleId::BoxSizing);
        })],
    );
    assert_eq!(k.height_drag_target(handle), None);
}

#[test]
fn detach_reparent_and_generation_reuse_refuse_old_bindings() {
    let (mut k, handle, target) = fixture();
    apply(
        &mut k,
        &[Op::SetChildren {
            id: 1,
            children: vec![5],
        }],
    );
    assert_eq!(k.height_drag_target(handle), None);
    apply(
        &mut k,
        &[Op::SetChildren {
            id: 1,
            children: vec![2, 5],
        }],
    );
    assert_eq!(k.height_drag_target(handle), Some(target));
    apply(
        &mut k,
        &[
            Op::SetChildren {
                id: 3,
                children: vec![],
            },
            Op::SetChildren {
                id: 5,
                children: vec![4],
            },
        ],
    );
    assert_eq!(k.height_drag_target(handle), None);
    apply(&mut k, &[Op::DestroyView { id: 4 }]);
    apply(
        &mut k,
        &[
            Op::CreateView {
                id: 4,
                node_type: NodeType::View,
            },
            prop(4, PropId::HeightDragFor, "sheet"),
            Op::SetChildren {
                id: 3,
                children: vec![4],
            },
        ],
    );
    let replacement = k.node(4).unwrap().key;
    assert_eq!(replacement.index, handle.index);
    assert_ne!(replacement.generation, handle.generation);
    assert_eq!(k.height_drag_target(handle), None);
    assert_eq!(k.height_drag_target(replacement), Some(target));
    apply(&mut k, &[Op::DestroyView { id: 2 }]);
    assert_eq!(k.height_drag_target(replacement), None);
}

#[test]
fn recreated_target_with_same_wire_and_authored_id_has_a_new_binding_key() {
    let (mut k, handle, old_target) = fixture();
    // Keep the handle alive while replacing just its former target allocation.
    apply(
        &mut k,
        &[
            Op::SetChildren {
                id: 3,
                children: vec![],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 4, 5],
            },
            Op::DestroyView { id: 2 },
        ],
    );
    assert_eq!(k.height_drag_target(handle), None);
    apply(
        &mut k,
        &[
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            prop(2, PropId::Id, "sheet"),
            style(2, |s| {
                s.height = Dimension::Points(180.0);
                s.box_sizing = BoxSizing::BorderBox;
                s.mask.set(StyleId::Height);
                s.mask.set(StyleId::BoxSizing);
            }),
            Op::SetChildren {
                id: 1,
                children: vec![2, 5],
            },
            Op::SetChildren {
                id: 2,
                children: vec![4],
            },
        ],
    );
    let replacement = k.node(2).unwrap().key;
    assert_ne!(replacement, old_target);
    assert!(k.node_by_key(old_target).is_none());
    assert_eq!(k.height_drag_target(handle), Some(replacement));
    assert_ne!(k.height_drag_target(handle), Some(old_target));
}

#[test]
fn malformed_wire_prop_is_atomic_and_inline_target_does_not_qualify() {
    let (mut k, handle, target) = fixture();
    let epoch = k.epoch();
    let wire = wire::encode(0, epoch + 1, &[prop(4, PropId::HeightDragFor, true)]);
    assert!(k.apply_frame(&wire).is_err());
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.height_drag_target(handle), Some(target));

    let mut k = Kernel::with_monospace();
    apply(
        &mut k,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 3,
                node_type: NodeType::Text,
            },
            prop(2, PropId::Id, "inline"),
            prop(3, PropId::HeightDragFor, "inline"),
            style(2, |s| {
                s.height = Dimension::Points(180.0);
                s.box_sizing = BoxSizing::BorderBox;
                s.mask.set(StyleId::Height);
                s.mask.set(StyleId::BoxSizing);
            }),
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::SetChildren {
                id: 2,
                children: vec![3],
            },
            Op::AttachRoot { id: 1 },
        ],
    );
    let handle = k.node(3).unwrap().key;
    assert!(k.node(2).unwrap().is_inline_run());
    assert_eq!(k.height_drag_target(handle), None);
}
