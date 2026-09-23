//! Literal-Chrome layout cases from the 2026-09-22 kernel review.
//!
//! Chrome 153, 2026-09-23: each case is plain HTML/CSS (not Exact lowering),
//! measured with `getBoundingClientRect` relative to the case's outer box,
//! whose descendants are `box-sizing: content-box` and inherit
//! `font: 16px/18px`. The outer box is the kernel root, id 1.
use crate::support::reader::{kernel, number as n, text as t};
use exact_kernel::StyleId::*;
use exact_kernel::{
    GridTrack, GridTracks, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue,
};
use std::collections::BTreeMap;

type Rows = Vec<(StyleId, StyleValue)>;

fn props(rows: &Rows) -> StyleProps {
    let mut p = StyleProps::default();
    for (row, value) in rows {
        p.set_dynamic(*row, value).unwrap();
    }
    p
}

/// Lays out one case: the outer box and its descendants, each
/// `(id, parent, rows)`; a node given text is a text node.
fn lay_out(root: StyleProps, nodes: Vec<(u32, u32, Rows)>, texts: &[(u32, &str)]) -> Kernel {
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root),
        },
    ];
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (id, parent, rows) in &nodes {
        let text = texts.iter().find(|(t, _)| t == id);
        ops.push(Op::CreateView {
            id: *id,
            node_type: if text.is_some() {
                NodeType::Text
            } else {
                NodeType::View
            },
        });
        let mut rows = rows.clone();
        rows.extend([(FontSize, n(16.0)), (LineHeight, t("18px"))]);
        ops.push(Op::SetStyle {
            id: *id,
            patch: Box::new(props(&rows)),
        });
        if let Some((_, text)) = text {
            ops.push(Op::SetProp {
                id: *id,
                prop: PropId::Text,
                value: (*text).into(),
            });
        }
        children.entry(*parent).or_default().push(*id);
    }
    for (id, list) in children {
        ops.push(Op::SetChildren { id, children: list });
    }
    ops.push(Op::AttachRoot { id: 1 });
    let mut k = kernel();
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(800.0, 600.0)).unwrap();
    k
}

/// Every mismatch of `[x, y, width, height]`, named by case and node.
fn mismatches(case: &str, k: &Kernel, expected: &[(u32, [f32; 4])]) -> Vec<String> {
    let mut out = Vec::new();
    for (id, want) in expected {
        let f = k.node(*id).unwrap().frame;
        let got = [f.x, f.y, f.width, f.height];
        if got.iter().zip(want).any(|(g, w)| (g - w).abs() > 0.01) {
            out.push(format!("{case} #{id}: kernel {got:?}, Chrome {want:?}"));
        }
    }
    out
}

fn empty(parent: u32, ids: &[u32]) -> Vec<(u32, u32, Rows)> {
    ids.iter().map(|id| (*id, parent, vec![])).collect()
}

/// A 400px-wide `display: grid` with these rows and track lists.
fn grid(rows: Rows, columns: &[GridTrack], track_rows: &[GridTrack]) -> StyleProps {
    let mut p = props(&[vec![(Display, t("grid")), (Width, n(400.0))], rows].concat());
    if !columns.is_empty() {
        p.grid_template_columns = GridTracks(columns.to_vec());
        p.mask.set(GridTemplateColumns);
    }
    if !track_rows.is_empty() {
        p.grid_template_rows = GridTracks(track_rows.to_vec());
        p.mask.set(GridTemplateRows);
    }
    p
}

/// A 400px-wide `display: flex` with these rows.
fn flex(rows: Rows) -> StyleProps {
    props(&[vec![(Display, t("flex")), (Width, n(400.0))], rows].concat())
}

/// CSS Box Alignment §5.1: the initial `normal` behaves as `stretch` in a
/// grid (auto tracks share the free space, items fill their areas) and as
/// `flex-start`/`stretch` in a flex container. The schema could not say
/// `normal`, so its `flex-start` default left grid auto tracks at content
/// size: `auto auto` columns were 0 wide and a bare grid's implicit column
/// was as wide as its text.
#[test]
fn normal_alignment_matches_chrome_in_grid_and_flex() {
    use GridTrack::{Auto, Fr, Points};
    let mut failures = Vec::new();
    let mut case = |name: &str, root, nodes, texts: &[(u32, &str)], want: &[_]| {
        failures.extend(mismatches(name, &lay_out(root, nodes, texts), want));
    };
    let halves = [(2, [0.0, 0.0, 200.0, 0.0]), (3, [200.0, 0.0, 200.0, 0.0])];
    case(
        "auto auto",
        grid(vec![], &[Auto, Auto], &[]),
        empty(1, &[2, 3]),
        &[],
        &halves,
    );
    case(
        "auto auto, justify-content: normal",
        grid(vec![(JustifyContent, t("normal"))], &[Auto, Auto], &[]),
        empty(1, &[2, 3]),
        &[],
        &halves,
    );
    case(
        "bare grid",
        grid(vec![], &[], &[]),
        empty(1, &[2]),
        &[(2, "Hello")],
        &[(2, [0.0, 0.0, 400.0, 18.0])],
    );
    case(
        "auto auto, justify-content: start",
        grid(vec![(JustifyContent, t("flex-start"))], &[Auto, Auto], &[]),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0; 4]), (3, [0.0; 4])],
    );
    case(
        "auto rows",
        grid(vec![(Height, n(300.0))], &[], &[Auto, Auto]),
        empty(1, &[2, 3]),
        &[],
        &[
            (2, [0.0, 0.0, 400.0, 150.0]),
            (3, [0.0, 150.0, 400.0, 150.0]),
        ],
    );
    case(
        "auto rows, align-content: start",
        grid(
            vec![(Height, n(200.0)), (AlignContent, t("flex-start"))],
            &[],
            &[Auto, Auto],
        ),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0, 0.0, 400.0, 0.0]), (3, [0.0, 0.0, 400.0, 0.0])],
    );
    case(
        "fixed tracks do not stretch",
        grid(vec![], &[Points(100.0), Points(100.0)], &[]),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0, 0.0, 100.0, 0.0]), (3, [100.0, 0.0, 100.0, 0.0])],
    );
    case(
        "a sized item starts in its stretched column",
        grid(vec![], &[], &[]),
        vec![(2, 1, vec![(Width, n(100.0)), (Height, n(10.0))])],
        &[],
        &[(2, [0.0, 0.0, 100.0, 10.0])],
    );
    case(
        "fr takes the free space before auto stretches",
        grid(vec![], &[Auto, Fr(1.0)], &[]),
        empty(1, &[2, 3]),
        &[],
        &[(2, [0.0; 4]), (3, [0.0, 0.0, 400.0, 0.0])],
    );
    case(
        "grid items: normal",
        grid(
            vec![
                (Height, n(200.0)),
                (JustifyItems, t("normal")),
                (AlignItems, t("normal")),
            ],
            &[],
            &[],
        ),
        empty(1, &[2]),
        &[],
        &[(2, [0.0, 0.0, 400.0, 200.0])],
    );
    case(
        "flex: justify-content normal",
        flex(vec![(JustifyContent, t("normal"))]),
        vec![(2, 1, vec![(Width, n(100.0)), (Height, n(10.0))])],
        &[],
        &[(2, [0.0, 0.0, 100.0, 10.0])],
    );
    case(
        "flex: align-content normal",
        flex(vec![
            (Height, n(100.0)),
            (FlexWrap, t("wrap")),
            (AlignContent, t("normal")),
        ]),
        vec![
            (2, 1, vec![(Width, n(300.0)), (Height, n(10.0))]),
            (3, 1, vec![(Width, n(300.0)), (Height, n(10.0))]),
        ],
        &[],
        &[(2, [0.0, 0.0, 300.0, 10.0]), (3, [0.0, 50.0, 300.0, 10.0])],
    );
    case(
        "flex: align-items normal",
        flex(vec![(Height, n(100.0)), (AlignItems, t("normal"))]),
        vec![(2, 1, vec![(Width, n(100.0))])],
        &[],
        &[(2, [0.0, 0.0, 100.0, 100.0])],
    );
    // In a `display: flow-root` case box: `normal` lets the child's top
    // margin collapse through its block parent; any other value makes the
    // parent an independent formatting context that keeps it.
    let nested = |align: &str| {
        vec![
            (2, 1, vec![(Width, n(400.0)), (AlignContent, t(align))]),
            (3, 2, vec![(MarginTop, n(20.0)), (Height, n(10.0))]),
        ]
    };
    case(
        "block: align-content normal",
        props(&vec![]),
        nested("normal"),
        &[],
        &[(2, [0.0, 20.0, 400.0, 10.0]), (3, [0.0, 20.0, 400.0, 10.0])],
    );
    case(
        "block: align-content start",
        props(&vec![]),
        nested("flex-start"),
        &[],
        &[(2, [0.0, 0.0, 400.0, 30.0]), (3, [0.0, 20.0, 400.0, 10.0])],
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
