//! A popover in a content region (stage 14 B, theme2 09:54): it stays in
//! the region's trial, laid out in-region as on main (the native region
//! captures it: QUEUE); out of the region it is in the viewport again.
use super::support::*;
use exact_kernel::*;

/// The fixture with a popover (7, `width: 50%`, 30 points high) in the
/// content branch, and the owner static when `static_owner`.
fn with_popover(static_owner: bool) -> Kernel {
    let mut k = fixture();
    let mut popover = StyleProps::default();
    popover.mask = mask(&[StyleId::Width, StyleId::Height]);
    popover.width = Dimension::Percent(50.);
    popover.height = Dimension::Points(30.);
    let mut ops = vec![
        Op::CreateView {
            id: 7,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 7,
            patch: Box::new(popover),
        },
        Op::SetProp {
            id: 7,
            prop: PropId::Popover,
            value: "auto".into(),
        },
        Op::SetChildren {
            id: 3,
            children: vec![4, 7],
        },
    ];
    if static_owner {
        // The placeholder static too: nothing else absolute is under it.
        for id in [2, 5] {
            let mut s = StyleProps::default();
            s.mask = mask(&[StyleId::PositionType]);
            s.position_type = PositionType::Static;
            ops.push(Op::SetStyle {
                id,
                patch: Box::new(s),
            });
        }
    }
    k.apply(0, 0, &ops).unwrap();
    k
}

#[test]
fn a_popover_under_a_region_stays_in_its_trial() {
    let mut k = with_popover(false);
    register(&mut k);
    let r = ready(&mut k, 400., 1);
    let popover = key(&k, 7);
    assert_eq!(k.host_block(7), None, "no viewport block under a region");
    // In its owner, the trial's containing block: 50% of the owner's 400.
    let f = r.current_frame(popover).expect("one of the trial's frames");
    assert_eq!((f.width, f.height), (200., 30.));
    // Out of the region, the same popover is in the viewport again.
    k.set_content_region(None).unwrap();
    k.compute_layout(1, Offer::definite(800., 300.)).unwrap();
    assert!(k.host_block(7).is_some());
    assert_eq!(k.node(7).unwrap().frame.width, 400.);
}

/// A static owner over a popover is refused, as over any absolutely
/// positioned box (LLP 1074 T1): under a region the popover is in the trial.
#[test]
fn a_static_owner_over_a_popover_is_refused() {
    let k = with_popover(true);
    let b = binding(&k);
    let mut k = k;
    assert!(k.set_content_region(Some(b)).is_err());
}

/// A region whose owner is in the top layer is refused: the owner is laid
/// out in its own block, after the shell, where no trial can place it.
#[test]
fn a_region_owner_in_a_popover_is_refused() {
    let mut k = fixture();
    k.apply(
        0,
        0,
        &[Op::SetProp {
            id: 2,
            prop: PropId::Popover,
            value: "manual".into(),
        }],
    )
    .unwrap();
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_err());
}

/// A popover in the region's shell is laid out in its block after the
/// region; a bad measurement there is recovered as the region's own is: the
/// next pass measures again, and nothing cached keeps the bad zero.
#[test]
fn a_shell_popovers_bad_measurement_is_recovered() {
    use std::cell::Cell;
    use std::rc::Rc;
    struct Toggle(Rc<Cell<bool>>);
    impl TextMeasurer for Toggle {
        fn measure(&mut self, r: &TextMeasureRequest<'_>) -> TextMetrics {
            if self.0.get() && r.runs.iter().any(|r| r.text.starts_with("Popped")) {
                TextMetrics {
                    width: f32::NAN,
                    ..Default::default()
                }
            } else {
                MonospaceMeasurer::default().measure(r)
            }
        }
    }
    let bad = Rc::new(Cell::new(false));
    let mut k = fixture_with(Box::new(Toggle(bad.clone())));
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
                node_type: NodeType::Text,
            },
            Op::SetProp {
                id: 8,
                prop: PropId::Popover,
                value: "manual".into(),
            },
            Op::SetProp {
                id: 9,
                prop: PropId::Text,
                value: "Popped".into(),
            },
            Op::SetChildren {
                id: 8,
                children: vec![9],
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 6, 8],
            },
        ],
    )
    .unwrap();
    register(&mut k);
    ready(&mut k, 400., 1);
    let good = k.node(9).unwrap().frame.width;
    assert!(good > 0.);
    assert!(
        k.host_block(8).is_some(),
        "the shell's popover is in its block"
    );
    // The text changes, so the next pass measures it, badly.
    bad.set(true);
    k.apply(
        1,
        1,
        &[Op::SetProp {
            id: 9,
            prop: PropId::Text,
            value: "Popped again".into(),
        }],
    )
    .unwrap();
    let r = k.compute_region_layout(
        1,
        Offer::definite(300., 300.),
        RegionInputs {
            catalog: 1,
            consumer_revision: 1,
        },
    );
    assert!(r.is_err(), "the bad measurement is the pass's error");
    bad.set(false);
    pass(&mut k, 300., 1);
    assert!(k.node(9).unwrap().frame.width > good, "measured again");
}

/// An owner that was a popover and is no longer registers before the next
/// layout drops its old block: it is judged as it is now.
#[test]
fn a_former_popover_owner_registers_before_the_next_layout() {
    let mut k = fixture();
    let popover = Op::SetProp {
        id: 2,
        prop: PropId::Popover,
        value: "manual".into(),
    };
    k.apply(0, 0, &[popover]).unwrap();
    k.compute_layout(1, Offer::definite(400., 300.)).unwrap();
    assert!(k.host_block(2).is_some());
    let cleared = Op::ClearProp {
        id: 2,
        prop: PropId::Popover,
    };
    k.apply(1, 1, &[cleared]).unwrap();
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_ok());
    assert!(ready(&mut k, 400., 1).current);
}
