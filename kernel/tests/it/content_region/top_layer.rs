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
