// Default64 one-paragraph baseline produced the exact f32 accumulation RED.
// Candidate controls below are source-only until the next validation release.
use super::*;

fn fractional_projection_fixture() -> Kernel {
    let mut k = fixture();
    let mut root = StyleProps::default();
    root.mask = mask(&[StyleId::PaddingTop, StyleId::Height]);
    root.padding_top = Dimension::Points(213.6);
    root.height = Dimension::Points(600.);
    let mut fraction = StyleProps::default();
    fraction.mask = mask(&[StyleId::PaddingTop]);
    fraction.padding_top = Dimension::Points(0.1);
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 7,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(root),
            },
            Op::SetStyle {
                id: 3,
                patch: Box::new(fraction.clone()),
            },
            Op::SetStyle {
                id: 7,
                patch: Box::new(fraction),
            },
            Op::SetChildren {
                id: 7,
                children: vec![4],
            },
            Op::SetChildren {
                id: 3,
                children: vec![7],
            },
        ],
    )
    .unwrap();
    k
}

#[test]
fn fractional_origin_small_region_matches_ordinary_every_descendant() {
    assert_eq!(exact_kernel::region::REGION_OFFERS, 64);
    let mut k = fractional_projection_fixture();
    let mut ordinary = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    ordinary
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    register(&mut k);
    let receipt = ready(&mut k, 400., 1);
    assert!(receipt.current);
    assert_eq!(receipt.origin.y.to_bits(), 213.6_f32.to_bits());
    let RegionSelection::Accepted(p) = &receipt.selection else {
        panic!("complete")
    };
    assert!(p.artifacts().len() <= 64);
    assert_eq!(p.frames().len(), 3); // content3, nested7, paragraph4: nothing omitted
    let mut mismatches = 0;
    for id in [3, 7, 4] {
        let expected = ordinary.node(id).unwrap();
        let actual = receipt.current_frame(key(&k, id)).unwrap();
        let local = p.frames().iter().find(|f| f.node == key(&k, id)).unwrap();
        eprintln!(
            "fractional id={id} origin={:?} local={:?} projected={actual:?} ordinary={:?}",
            receipt.origin, local.frame, expected.frame
        );
        assert_eq!(local.content, expected.content);
        assert!(
            k.node(id).unwrap().frame.bits_eq(actual),
            "arena/publication agree"
        );
        assert!(
            p.frame(key(&k, id), receipt.origin)
                .unwrap()
                .bits_eq(actual),
            "reader agrees"
        );
        mismatches += usize::from(!actual.bits_eq(expected.frame));
    }
    let origin = receipt.origin.y;
    // This models only the authored 0.1 +0.1 offset chain, not text metrics.
    // Print exact actual bits; do not use epsilon or pixel rounding.
    let root_first = (origin + 0.1_f32) + 0.1_f32;
    let local_then_origin = origin + (0.1_f32 + 0.1_f32);
    eprintln!(
        "accumulation root_first={root_first:?}/{:x} local_then_origin={local_then_origin:?}/{:x}",
        root_first.to_bits(),
        local_then_origin.to_bits()
    );
    assert_eq!(
        ordinary.node(4).unwrap().frame.y.to_bits(),
        root_first.to_bits()
    );
    assert_eq!(
        mismatches, 0,
        "every descendant must match ordinary parent-first accumulation"
    );
}

fn style(k: &mut Kernel, id: ViewId, patch: StyleProps) {
    k.apply(
        0,
        0,
        &[Op::SetStyle {
            id,
            patch: Box::new(patch),
        }],
    )
    .unwrap();
}

fn projected_matches_ordinary(k: &Kernel, ordinary: &Kernel, r: &RegionLayoutReceipt) {
    let RegionSelection::Accepted(p) = &r.selection else {
        panic!("accepted")
    };
    let all = p.projected_frames(r.origin).unwrap();
    assert_eq!(all.len(), p.frames().len());
    let zero = p.projected_frames(Frame::default()).unwrap();
    for (local, zero) in p.frames().iter().zip(&zero) {
        assert_eq!(local.node, zero.node);
        assert!(
            local.frame.bits_eq(zero.frame),
            "origin-zero accessor identity"
        );
        assert_eq!(local.content, zero.content);
    }
    for (local, projected) in p.frames().iter().zip(&all) {
        let expected = ordinary.node_by_key(projected.node).unwrap();
        assert_eq!(local.node, projected.node, "paint order unchanged");
        assert!(
            projected.frame.bits_eq(expected.frame),
            "{:?}: {:?} != {:?}",
            projected.node,
            projected.frame,
            expected.frame
        );
        assert_eq!(projected.content, expected.content);
        assert!(p
            .frame(projected.node, r.origin)
            .unwrap()
            .bits_eq(projected.frame));
        if r.current {
            assert!(r
                .current_frame(projected.node)
                .unwrap()
                .bits_eq(projected.frame));
            assert!(k
                .node_by_key(projected.node)
                .unwrap()
                .frame
                .bits_eq(projected.frame));
        }
    }
}

#[test]
fn projection_both_axes_and_negative_origins_are_parent_first() {
    for negative in [false, true] {
        let mut k = fractional_projection_fixture();
        let mut shift = StyleProps::default();
        shift.mask = mask(&[StyleId::PositionType, StyleId::Left, StyleId::Top]);
        shift.position_type = PositionType::Relative;
        shift.left = Dimension::Points(if negative { -213.6 } else { 213.6 });
        shift.top = Dimension::Points(if negative { -427.2 } else { 0. });
        style(&mut k, 2, shift);
        for id in [3, 7] {
            let mut inset = StyleProps::default();
            inset.mask = mask(&[StyleId::PaddingLeft]);
            inset.padding_left = Dimension::Points(0.1);
            style(&mut k, id, inset);
        }
        if negative {
            let mut offset = StyleProps::default();
            offset.mask = mask(&[StyleId::PositionType, StyleId::Left, StyleId::Top]);
            offset.position_type = PositionType::Relative;
            offset.left = Dimension::Points(-0.2);
            offset.top = Dimension::Points(-0.2);
            style(&mut k, 7, offset);
        }
        let mut ordinary = k.rehydrate(Box::new(MonospaceMeasurer::default()));
        ordinary
            .compute_layout(1, Offer::definite(400., 300.))
            .unwrap();
        register(&mut k);
        let r = ready(&mut k, 400., 1);
        assert_eq!(r.origin.x.is_sign_negative(), negative);
        assert_eq!(r.origin.y.is_sign_negative(), negative);
        projected_matches_ordinary(&k, &ordinary, &r);
    }
}

#[test]
fn retained_projection_uses_new_origin_and_original_width_without_mutating_a() {
    let mut k = fractional_projection_fixture();
    register(&mut k);
    let a = ready(&mut k, 400., 1);
    let RegionSelection::Accepted(old) = &a.selection else {
        panic!()
    };
    let saved = old.projected_frames(a.origin).unwrap();
    let mut shift = StyleProps::default();
    shift.mask = mask(&[StyleId::PaddingTop]);
    shift.padding_top = Dimension::Points(217.6);
    style(&mut k, 1, shift);
    let mut ordinary_old_width = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    ordinary_old_width
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    let pending = pass(&mut k, 300., 1);
    assert!(!pending.current);
    let RegionSelection::Accepted(selected) = &pending.selection else {
        panic!()
    };
    assert!(Rc::ptr_eq(old, selected));
    assert_eq!(pending.origin.y.to_bits(), 217.6_f32.to_bits());
    assert!(pending.current_frame(key(&k, 4)).is_none());
    projected_matches_ordinary(&k, &ordinary_old_width, &pending);
    for (was, now) in saved.iter().zip(old.projected_frames(a.origin).unwrap()) {
        assert_eq!(was.node, now.node);
        assert!(was.frame.bits_eq(now.frame));
        assert_eq!(was.content, now.content);
    }
    let mut ordinary_new = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    ordinary_new
        .compute_layout(1, Offer::definite(300., 300.))
        .unwrap();
    let b = ready(&mut k, 300., 1);
    projected_matches_ordinary(&k, &ordinary_new, &b);
}

#[test]
fn pending_placeholder_nested_offsets_use_the_same_parent_first_projection() {
    let mut k = fractional_projection_fixture();
    let mut pending = StyleProps::default();
    pending.mask = mask(&[
        StyleId::PositionType,
        StyleId::Top,
        StyleId::Left,
        StyleId::PaddingTop,
        StyleId::PaddingLeft,
    ]);
    pending.position_type = PositionType::Absolute;
    pending.top = Dimension::Points(0.);
    pending.left = Dimension::Points(0.);
    pending.padding_top = Dimension::Points(0.1);
    pending.padding_left = Dimension::Points(0.1);
    let mut nested = StyleProps::default();
    nested.mask = mask(&[StyleId::PaddingTop, StyleId::PaddingLeft]);
    nested.padding_top = Dimension::Points(0.1);
    nested.padding_left = Dimension::Points(0.1);
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 8,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 9,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 10,
                node_type: NodeType::Text,
            },
            Op::SetStyle {
                id: 8,
                patch: Box::new(pending),
            },
            Op::SetStyle {
                id: 9,
                patch: Box::new(nested),
            },
            Op::SetProp {
                id: 10,
                prop: PropId::Text,
                value: "actual pending text".into(),
            },
            Op::SetChildren {
                id: 9,
                children: vec![10],
            },
            Op::SetChildren {
                id: 8,
                children: vec![9],
            },
            Op::SetChildren {
                id: 2,
                children: vec![3, 8],
            },
        ],
    )
    .unwrap();
    let mut ordinary = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    ordinary
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    let b = ContentRegion {
        owner: key(&k, 2),
        content: key(&k, 3),
        pending: key(&k, 8),
    };
    k.set_content_region(Some(b)).unwrap();
    let p = pass(&mut k, 400., 1);
    assert!(!p.current);
    assert!(matches!(p.selection, RegionSelection::Pending(_)));
    for id in [8, 9, 10] {
        assert!(
            k.node(id)
                .unwrap()
                .frame
                .bits_eq(ordinary.node(id).unwrap().frame),
            "pending {id}"
        );
        assert_eq!(
            k.node(id).unwrap().content,
            ordinary.node(id).unwrap().content
        );
    }
}

#[test]
fn projected_overflow_refuses_before_any_publication_and_recovers() {
    let mut k = fractional_projection_fixture();
    let mut far = StyleProps::default();
    far.mask = mask(&[StyleId::PositionType, StyleId::Left]);
    far.position_type = PositionType::Relative;
    far.left = Dimension::Points(f32::MAX / 2.);
    style(&mut k, 3, far);
    register(&mut k);
    let accepted = ready(&mut k, 400., 1);
    let RegionSelection::Accepted(a) = &accepted.selection else {
        panic!()
    };
    let before = frames(&k);
    for x in [f32::MAX, f32::INFINITY, f32::NAN] {
        let origin = Frame {
            x,
            ..accepted.origin
        };
        assert!(a.projected_frames(origin).is_err());
        assert!(a.frame(key(&k, 4), origin).is_none());
        assert_eq!(before, frames(&k), "pure accessor never publishes");
    }
    let mut shift = StyleProps::default();
    shift.mask = mask(&[StyleId::PositionType, StyleId::Left]);
    shift.position_type = PositionType::Relative;
    shift.left = Dimension::Points(f32::MAX);
    style(&mut k, 2, shift.clone());
    let before = frames(&k);
    let failure = k.compute_region_layout(
        1,
        Offer::definite(400., 300.),
        RegionInputs {
            catalog: 1,
            consumer_revision: 1,
        },
    );
    assert!(matches!(
        failure,
        Err(KernelError::Layout(LayoutError::ContentRegion(
            "projection overflow"
        )))
    ));
    assert_eq!(
        before,
        frames(&k),
        "no shell or content publication on projection failure"
    );
    assert_eq!(k.region_retention().accepted_offers, a.artifacts().len());
    shift.left = Dimension::Points(0.);
    style(&mut k, 2, shift);
    assert!(ready(&mut k, 400., 1).current);
    assert!(
        a.projected_frames(accepted.origin).is_ok(),
        "old immutable A survives"
    );
}

#[test]
fn inline_zero_frames_and_rehydrated_complete_geometry_remain_exact() {
    let mut k = fractional_projection_fixture();
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 8,
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 8,
                prop: PropId::Text,
                value: " inline".into(),
            },
            Op::SetChildren {
                id: 4,
                children: vec![8],
            },
        ],
    )
    .unwrap();
    let mut ordinary = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    ordinary
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    register(&mut k);
    let accepted = ready(&mut k, 400., 1);
    projected_matches_ordinary(&k, &ordinary, &accepted);
    let RegionSelection::Accepted(a) = accepted.selection else {
        panic!()
    };
    assert_eq!(a.frame(key(&k, 8), accepted.origin), Some(Frame::default()));
    assert!(a
        .frame(
            NodeKey {
                index: u32::MAX,
                generation: 0
            },
            accepted.origin
        )
        .is_none());
    let mut rebuilt = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    register(&mut rebuilt);
    let again = ready(&mut rebuilt, 400., 1);
    projected_matches_ordinary(&rebuilt, &ordinary, &again);
}
