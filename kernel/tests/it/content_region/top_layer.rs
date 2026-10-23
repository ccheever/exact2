//! A popover in a content region (stage 14 B): it leaves the region's trial
//! as it leaves its parent's layout, and is laid out in the viewport.
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
fn a_popover_leaves_the_regions_trial_for_the_viewport() {
    let mut k = with_popover(false);
    register(&mut k);
    let popover = key(&k, 7);
    let first = pass(&mut k, 400., 1);
    assert!(
        first.shell.updated.contains(&popover),
        "published after the region"
    );
    let r = ready(&mut k, 400., 1);
    assert_eq!(
        r.current_frame(popover),
        None,
        "not one of the trial's frames"
    );
    let f = k.node(7).unwrap().frame;
    assert_eq!((f.x, f.y, f.width, f.height), (0., 0., 200., 30.));
    assert_eq!(k.host_block_size(7), Some((200., 30.)));
    // The text beside it lays out as without it.
    let text = k.node(4).unwrap().frame;
    let mut plain = fixture();
    register(&mut plain);
    ready(&mut plain, 400., 1);
    assert_eq!(text, plain.node(4).unwrap().frame);
}

/// A static owner over a popover registers, as over nothing absolute: the
/// popover is not in the trial (LLP 1074 T1's refusal is for boxes that are).
#[test]
fn a_static_owner_over_a_popover_registers() {
    let mut k = with_popover(true);
    let b = binding(&k);
    assert!(k.set_content_region(Some(b)).is_ok());
    ready(&mut k, 400., 1);
    assert_eq!(k.node(7).unwrap().frame.width, 200.);
}
