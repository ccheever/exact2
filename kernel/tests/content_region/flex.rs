use super::*;

fn flex_style(id: ViewId, patch: StyleProps) -> Op {
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}

fn flex_region_fixture() -> Kernel {
    let mut k = fixture();
    let mut root = StyleProps::default();
    root.mask = mask(&[
        StyleId::Display,
        StyleId::FlexDirection,
        StyleId::FlexWrap,
        StyleId::Width,
    ]);
    root.display = Display::Flex;
    root.flex_direction = FlexDirection::Column;
    root.flex_wrap = FlexWrap::Nowrap;
    root.width = Dimension::Percent(100.);
    let mut owner = StyleProps::default();
    owner.mask = mask(&[
        StyleId::Display,
        StyleId::FlexDirection,
        StyleId::Height,
        StyleId::FlexGrow,
        StyleId::FlexShrink,
        StyleId::FlexBasis,
        StyleId::MinHeight,
    ]);
    owner.display = Display::Flex;
    owner.flex_direction = FlexDirection::Column;
    owner.height = Dimension::Auto;
    owner.flex_grow = 1.;
    owner.flex_shrink = 1.;
    owner.flex_basis = Dimension::Percent(0.);
    owner.min_height = Dimension::Points(80.);
    k.apply(0, 0, &[flex_style(1, root), flex_style(2, owner)])
        .unwrap();
    k
}

#[test]
fn flex_region_complete_a_survives_pending_then_latest_b_matches_ordinary_geometry() {
    let mut k = flex_region_fixture();
    let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    oracle
        .compute_layout(1, Offer::definite(400., 300.))
        .unwrap();
    register(&mut k);
    let RegionSelection::Accepted(a) = ready(&mut k, 400., 1).selection else {
        panic!()
    };
    for id in [1, 2, 3, 4, 6] {
        assert!(
            k.node(id)
                .unwrap()
                .frame
                .bits_eq(oracle.node(id).unwrap().frame),
            "A id {id}"
        );
    }
    let old_frames: Vec<_> = a
        .frames()
        .iter()
        .map(|f| (f.node, f.frame, f.content))
        .collect();
    let pending = pass(&mut k, 300., 1);
    assert!(!pending.current);
    assert!(pending.current_frame(key(&k, 4)).is_none());
    let RegionSelection::Accepted(retained) = pending.selection else {
        panic!()
    };
    assert!(Rc::ptr_eq(&a, &retained));
    let stale = k.region_text_request().unwrap().clone();
    k.apply(
        0,
        0,
        &[
            Op::SetProp {
                id: 6,
                prop: PropId::Text,
                value: "typed composer\nsecond line\nlatest line".into(),
            },
            Op::SetProp {
                id: 4,
                prop: PropId::Text,
                value: "latest B body wrapping across a narrower viewport".into(),
            },
        ],
    )
    .unwrap();
    let latest = pass(&mut k, 280., 1);
    assert!(!latest.current);
    let RegionSelection::Accepted(retained) = latest.selection else {
        panic!()
    };
    assert!(Rc::ptr_eq(&a, &retained));
    assert_eq!(
        old_frames,
        a.frames()
            .iter()
            .map(|f| (f.node, f.frame, f.content))
            .collect::<Vec<_>>()
    );
    let request = k.region_text_request().unwrap().clone();
    assert_ne!(request.ticket(), stale.ticket());
    assert!(!k
        .resolve_region_text(
            &stale,
            TextMetrics {
                width: f32::NAN,
                height: -1.,
                first_baseline: None
            },
            Rc::new(())
        )
        .unwrap());
    let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
    oracle
        .compute_layout(1, Offer::definite(280., 300.))
        .unwrap();
    assert!(
        k.node(6)
            .unwrap()
            .frame
            .bits_eq(oracle.node(6).unwrap().frame),
        "shell typing before B ready"
    );
    let complete = ready(&mut k, 280., 1);
    assert!(complete.current);
    let RegionSelection::Accepted(b) = complete.selection else {
        panic!()
    };
    assert!(!Rc::ptr_eq(&a, &b));
    for id in [1, 2, 3, 4, 6] {
        assert!(
            k.node(id)
                .unwrap()
                .frame
                .bits_eq(oracle.node(id).unwrap().frame),
            "B id {id}"
        );
        if id == 2 {
            // The established region contract cuts this clip owner's shell
            // edge and excludes it from candidate publication. Its content
            // extent is intentionally empty; descendant extents remain exact.
            assert_eq!(k.node(id).unwrap().content, (0., 0.));
            assert!(b.frames().iter().all(|f| f.node != key(&k, id)));
        } else {
            assert_eq!(
                k.node(id).unwrap().content,
                oracle.node(id).unwrap().content,
                "B extent {id}"
            );
        }
    }
    let painted = b.paint_artifact(key(&k, 4)).unwrap();
    painted.request().with_request(|q| {
        assert_eq!(
            q.runs[0].text,
            "latest B body wrapping across a narrower viewport"
        )
    });
    let old_owner = key(&k, 2);
    k.reset();
    assert!(k.node_by_key(old_owner).is_none());
    assert!(!k
        .resolve_region_text(&request, TextMetrics::default(), Rc::new(()))
        .unwrap());
}

#[test]
fn flex_region_intrinsic_outer_axes_refuse_before_shell_or_request_mutation() {
    let calls = Rc::new(std::cell::Cell::new(0));
    let mut k = flex_region_fixture().rehydrate(Box::new(CatalogMetrics {
        height: Rc::new(std::cell::Cell::new(20.)),
        calls: calls.clone(),
    }));
    // Use point sizes as well: the old percentage-only guard cannot protect
    // auto-height when both declared root/cross-axis dimensions are points.
    let mut root = StyleProps::default();
    root.mask = mask(&[StyleId::Width]);
    root.width = Dimension::Points(400.);
    let mut owner = StyleProps::default();
    owner.mask = mask(&[StyleId::Width]);
    owner.width = Dimension::Points(400.);
    k.apply(0, 0, &[flex_style(1, root), flex_style(2, owner)])
        .unwrap();
    register(&mut k);
    pass(&mut k, 400., 1);
    let request = k.region_text_request().unwrap().clone();
    let before = frames(&k);
    let retention = k.region_retention();
    let count = calls.get();
    assert!(count > 0);
    for axis in [
        AxisOffer::MinContent,
        AxisOffer::MaxContent,
        AxisOffer::Definite(-1.),
    ] {
        for offer in [
            Offer {
                width: axis,
                height: AxisOffer::Definite(300.),
            },
            Offer {
                width: AxisOffer::Definite(400.),
                height: axis,
            },
        ] {
            let result = k.compute_region_layout(
                1,
                offer,
                RegionInputs {
                    catalog: 2,
                    consumer_revision: 2,
                },
            );
            assert!(matches!(
                result,
                Err(KernelError::Layout(LayoutError::ContentRegion(
                    "flex region requires definite outer axes"
                )))
            ));
            assert_eq!(frames(&k), before);
            assert_eq!(
                calls.get(),
                count,
                "no shell text callback on preflight refusal"
            );
            assert_eq!(k.region_text_request().unwrap().ticket(), request.ticket());
            assert_eq!(k.region_text_request().unwrap().catalog(), 1);
            assert_eq!(k.region_retention(), retention);
        }
    }
    let metrics = request.with_request(|q| MonospaceMeasurer::default().measure(q));
    assert!(k
        .resolve_region_text(&request, metrics, Rc::new(()))
        .unwrap());
    assert!(ready(&mut k, 400., 1).current);
}

#[test]
fn flex_region_certificate_refuses_content_dependent_or_different_parent_modes() {
    use StyleValue::{Auto, Number, Percent, Text};
    for (id, row, value) in [
        (2, StyleId::FlexBasis, Auto),
        (2, StyleId::FlexBasis, Percent(10.)),
        (2, StyleId::MinHeight, Auto),
        (
            2,
            StyleId::MinHeight,
            Text("env(safe-area-inset-top)".into()),
        ),
        (
            2,
            StyleId::MaxHeight,
            Text("env(safe-area-inset-top)".into()),
        ),
        (2, StyleId::AspectRatio, Number(2.)),
        (2, StyleId::Width, Auto),
        (2, StyleId::OverflowY, Text("visible".into())),
        (2, StyleId::PaddingTop, Number(1.)),
        (2, StyleId::MarginTop, Auto),
        (2, StyleId::Top, Number(1.)),
        (2, StyleId::PositionType, Text("absolute".into())),
        (2, StyleId::FlexGrow, Number(0.)),
        (2, StyleId::AlignSelf, Text("baseline".into())),
        (1, StyleId::AlignItems, Text("baseline".into())),
        (1, StyleId::FlexWrap, Text("wrap".into())),
        (1, StyleId::FlexDirection, Text("row".into())),
        (1, StyleId::FlexDirection, Text("column-reverse".into())),
        (1, StyleId::Display, Text("block".into())),
        (1, StyleId::Height, Auto),
        (1, StyleId::Width, Auto),
    ] {
        let mut k = flex_region_fixture();
        let mut patch = StyleProps::default();
        patch.set_dynamic(row, &value).unwrap();
        k.apply(0, 0, &[flex_style(id, patch)]).unwrap();
        let before = frames(&k);
        let b = binding(&k);
        assert!(
            k.set_content_region(Some(b)).is_err(),
            "id {id} {row:?} {value:?}"
        );
        assert_eq!(frames(&k), before, "id {id} {row:?} {value:?}");
        assert!(k.region_text_request().is_none());
    }
}

#[test]
fn flex_region_refuses_nested_parent_even_when_its_sizes_are_definite() {
    let mut k = flex_region_fixture();
    let mut wrapper = k.node(1).unwrap().style.clone();
    wrapper.mask = mask(&[
        StyleId::Width,
        StyleId::Height,
        StyleId::Display,
        StyleId::FlexDirection,
    ]);
    k.apply(
        0,
        0,
        &[
            Op::CreateView {
                id: 8,
                node_type: NodeType::View,
            },
            flex_style(8, wrapper),
            Op::SetChildren {
                id: 1,
                children: vec![6],
            },
            Op::SetChildren {
                id: 8,
                children: vec![2],
            },
            Op::SetChildren {
                id: 1,
                children: vec![8, 6],
            },
        ],
    )
    .unwrap();
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_err());
}

#[test]
fn flex_region_explicit_min_max_and_zero_basis_forms_match_ordinary_layout() {
    for basis in [Dimension::Points(0.), Dimension::Percent(0.)] {
        for (min, max) in [
            (Dimension::Points(0.), Dimension::Auto),
            (Dimension::Points(180.), Dimension::Points(120.)),
            (Dimension::Percent(30.), Dimension::Percent(60.)),
        ] {
            let mut k = flex_region_fixture();
            let mut patch = StyleProps::default();
            patch.mask = mask(&[StyleId::FlexBasis, StyleId::MinHeight, StyleId::MaxHeight]);
            patch.flex_basis = basis;
            patch.min_height = min;
            patch.max_height = max;
            k.apply(0, 0, &[flex_style(2, patch)]).unwrap();
            let mut oracle = k.rehydrate(Box::new(MonospaceMeasurer::default()));
            oracle
                .compute_layout(1, Offer::definite(400., 300.))
                .unwrap();
            register(&mut k);
            assert!(ready(&mut k, 400., 1).current);
            for id in [1, 2, 3, 4, 6] {
                assert!(
                    k.node(id)
                        .unwrap()
                        .frame
                        .bits_eq(oracle.node(id).unwrap().frame),
                    "id {id} basis {basis:?} min {min:?} max {max:?}"
                );
            }
            if min == Dimension::Points(180.) {
                assert_eq!(
                    k.node(2).unwrap().frame.height,
                    180.,
                    "minimum beats maximum"
                );
            }
        }
    }
}

#[test]
fn flex_region_live_certificate_loss_preserves_publication_until_explicit_clear() {
    let mut k = flex_region_fixture();
    register(&mut k);
    ready(&mut k, 400., 1);
    let before = frames(&k);
    let mut patch = StyleProps::default();
    patch.mask = mask(&[StyleId::FlexBasis]);
    patch.flex_basis = Dimension::Auto;
    k.apply(0, 0, &[flex_style(2, patch)]).unwrap();
    assert!(k
        .compute_region_layout(
            1,
            Offer::definite(400., 300.),
            RegionInputs {
                catalog: 1,
                consumer_revision: 2
            }
        )
        .is_err());
    assert_eq!(frames(&k), before);
    assert!(k.region_text_request().is_none());
    assert!(k.compute_layout(1, Offer::definite(400., 300.)).is_err());
    k.set_content_region(None).unwrap();
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
}
