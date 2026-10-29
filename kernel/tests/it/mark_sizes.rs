//! A host's reports of a mark's size (a checkbox's, a radio's, a switch's, by
//! kind and font size) and the size the kind's marks share
//! (`control::shared_size`): what a withdrawn report takes back, and what
//! holds it (9960b11949 and its post-hoc review).

use crate::browser_cases::{css_rows as rows, props};
use exact_kernel::{Kernel, NodeType, Offer, Op, PropId, PropValue};

/// A host that withdraws the size it reported for a mark sizes that kind no
/// more (Linux's Material look leaves a checkbox to the kernel): the size the
/// kind's marks shared goes with it, back to the look's mark, while a report
/// the kind's marks don't share is withdrawn alone.
#[test]
fn a_withdrawn_report_takes_its_kinds_shared_size_with_it() {
    let checkbox = |id| {
        [
            Op::CreateView {
                id,
                node_type: NodeType::Control,
            },
            Op::SetProp {
                id,
                prop: PropId::Type,
                value: PropValue::Str("checkbox".into()),
            },
        ]
    };
    let size = |kernel: &Kernel, id| {
        let f = kernel.node(id).unwrap().frame;
        (f.width, f.height)
    };
    let offer = Offer::definite(400.0, 600.0);
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&rows(
                "display:flex;align-items:flex-start;width:400px",
            ))),
        },
    ];
    ops.extend(checkbox(2));
    ops.extend(checkbox(3));
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2, 3],
        },
        Op::AttachRoot { id: 1 },
    ]);
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    kernel.set_intrinsic_size(2, Some((38.0, 22.0))).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        (size(&kernel, 2), size(&kernel, 3)),
        ((38.0, 22.0), (38.0, 22.0))
    );
    kernel.set_intrinsic_size(2, None).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        (size(&kernel, 2), size(&kernel, 3)),
        ((13.0, 13.0), (13.0, 13.0)),
        "the shared size withdrawn with the report that set it"
    );
    // A report of the kind's size, then another node's own of a size the kind
    // no longer shares, withdrawn: the kind's stays.
    kernel.set_intrinsic_size(2, Some((22.0, 22.0))).unwrap();
    kernel.set_intrinsic_size(3, Some((30.0, 30.0))).unwrap();
    kernel.set_intrinsic_size(2, Some((24.0, 24.0))).unwrap();
    kernel.set_intrinsic_size(3, None).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        size(&kernel, 3),
        (24.0, 24.0),
        "the kind's size stands for it"
    );
}

/// A withdrawn report takes back its kind's shared size only when no other
/// mark of that kind still holds it as its own (post-hoc review of
/// 9960b11949, Astra and Grok): with two equal reports, withdrawing one
/// leaves the size the other still reports, for the marks with none; and a
/// switch that became a checkbox, withdrawing a size the checkboxes' happens
/// to equal, leaves the checkboxes' alone while one of them reports it.
#[test]
fn a_withdrawn_report_leaves_a_shared_size_another_mark_still_reports() {
    let control = |id, kind: &str| {
        [
            Op::CreateView {
                id,
                node_type: NodeType::Control,
            },
            Op::SetProp {
                id,
                prop: PropId::Type,
                value: PropValue::Str(kind.into()),
            },
        ]
    };
    let size = |kernel: &Kernel, id| {
        let f = kernel.node(id).unwrap().frame;
        (f.width, f.height)
    };
    let offer = Offer::definite(400.0, 600.0);
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&rows(
                "display:flex;align-items:flex-start;width:400px",
            ))),
        },
    ];
    for id in [2, 3, 4] {
        ops.extend(control(id, "checkbox"));
    }
    ops.extend(control(5, "checkbox"));
    ops.push(Op::SetProp {
        id: 5,
        prop: PropId::AccessibilityRole,
        value: PropValue::Str("switch".into()),
    });
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2, 3, 4, 5],
        },
        Op::AttachRoot { id: 1 },
    ]);
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    kernel.set_intrinsic_size(2, Some((22.0, 22.0))).unwrap();
    kernel.set_intrinsic_size(3, Some((22.0, 22.0))).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(size(&kernel, 4), (22.0, 22.0));
    kernel.set_intrinsic_size(3, None).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        (size(&kernel, 3), size(&kernel, 4)),
        ((22.0, 22.0), (22.0, 22.0)),
        "mark 2 still reports the kind's size"
    );
    // The switch reported the checkboxes' size, then became a checkbox and withdrew it.
    kernel.set_intrinsic_size(5, Some((22.0, 22.0))).unwrap();
    kernel
        .apply(
            1,
            2,
            &[Op::ClearProp {
                id: 5,
                prop: PropId::AccessibilityRole,
            }],
        )
        .unwrap();
    kernel.set_intrinsic_size(5, None).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(size(&kernel, 4), (22.0, 22.0), "mark 2 still reports it");
}

/// A mark holds its kind's shared size by the kind it reported as, not the
/// kind it is now (round 1, Astra): a radio's report of the checkboxes' size
/// keeps no checkbox size once it becomes a checkbox, so the checkbox that
/// reported it, withdrawing, takes the size back.
#[test]
fn a_mark_holds_a_shared_size_by_the_kind_it_reported_as() {
    let control = |id, kind: &str| {
        [
            Op::CreateView {
                id,
                node_type: NodeType::Control,
            },
            Op::SetProp {
                id,
                prop: PropId::Type,
                value: PropValue::Str(kind.into()),
            },
        ]
    };
    let size = |kernel: &Kernel, id| {
        let f = kernel.node(id).unwrap().frame;
        (f.width, f.height)
    };
    let offer = Offer::definite(400.0, 600.0);
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(props(&rows(
                "display:flex;align-items:flex-start;width:400px",
            ))),
        },
    ];
    ops.extend(control(2, "checkbox"));
    ops.extend(control(3, "radio"));
    ops.extend(control(4, "checkbox"));
    ops.extend(control(5, "radio"));
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2, 3, 4, 5],
        },
        Op::AttachRoot { id: 1 },
    ]);
    let mut kernel = Kernel::with_monospace();
    kernel.apply(0, 1, &ops).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    kernel.set_intrinsic_size(2, Some((22.0, 22.0))).unwrap();
    kernel.set_intrinsic_size(3, Some((22.0, 22.0))).unwrap();
    kernel
        .apply(
            1,
            2,
            &[Op::SetProp {
                id: 3,
                prop: PropId::Type,
                value: PropValue::Str("checkbox".into()),
            }],
        )
        .unwrap();
    kernel.set_intrinsic_size(2, None).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        size(&kernel, 4),
        (13.0, 13.0),
        "the radio's report holds no checkbox size"
    );
    // The same size reported again as a checkbox (round 2, Astra): now it holds
    // the checkboxes' size, and a checkbox withdrawing leaves it.
    kernel.set_intrinsic_size(2, Some((22.0, 22.0))).unwrap();
    kernel.set_intrinsic_size(3, Some((22.0, 22.0))).unwrap();
    kernel.set_intrinsic_size(2, None).unwrap();
    kernel.compute_layout(1, offer).unwrap();
    assert_eq!(
        size(&kernel, 4),
        (22.0, 22.0),
        "its report again, as a checkbox, holds it"
    );
    assert_eq!(
        size(&kernel, 5),
        (13.0, 13.0),
        "and is no radio's report any more (round 2's re-check, Grok)"
    );
}
