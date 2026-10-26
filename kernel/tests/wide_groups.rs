//! A style's rare and cold rows are shared between held styles
//! (`shared_style::Groups`); a group no held style holds any more keeps no
//! wide colour's id alive, so it is never shared again after the table
//! reclaims that id. Its own process: the table is the process's.

use exact_kernel::style::wide;
use exact_kernel::{ColorValue, Kernel, NodeType, Op, RowValue, StyleId, StyleProps, StyleValue};

fn patch(id: u32, rows: &[(StyleId, &str)]) -> Op {
    let mut s = StyleProps::default();
    for (row, css) in rows {
        s.set_dynamic(*row, &StyleValue::Text((*css).into()))
            .unwrap_or_else(|e| panic!("{css}: {e:?}"));
    }
    Op::SetStyle {
        id,
        patch: Box::new(s),
    }
}

#[test]
fn a_group_no_style_holds_keeps_no_wide_colour() {
    let glow = "0px 0px 8px color(display-p3 0.1 0.2 0.3)";
    let mut k = Kernel::with_monospace();
    let mut batch = 1;
    k.apply(
        0,
        batch,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::View,
            },
            Op::SetChildren {
                id: 1,
                children: vec![2],
            },
            Op::AttachRoot { id: 1 },
            patch(1, &[(StyleId::Width, "10px"), (StyleId::BoxShadow, glow)]),
        ],
    )
    .unwrap();
    // The shadow goes; then more wide colours than the table holds pass
    // through another box, with no trim, so the glow's slot is reclaimed.
    batch += 1;
    k.apply(0, batch, &[patch(1, &[(StyleId::BoxShadow, "none")])])
        .unwrap();
    for i in 0..6_000 {
        let css = format!(
            "color(display-p3 {} {} 0.5)",
            (i % 100) as f64 / 100.0,
            (i / 100) as f64 / 100.0
        );
        batch += 1;
        k.apply(0, batch, &[patch(2, &[(StyleId::BackgroundColor, &css)])])
            .unwrap();
    }
    // The shadow comes back: an equal group, under its old hash.
    batch += 1;
    k.apply(0, batch, &[patch(1, &[(StyleId::BoxShadow, glow)])])
        .unwrap();
    let RowValue::BoxShadow(shadows) = k.node(1).unwrap().computed(StyleId::BoxShadow) else {
        panic!("no shadow")
    };
    let colour = shadows.0[0].color;
    let ColorValue::Wide(id) = colour else {
        panic!("{colour:?}")
    };
    assert_eq!(
        wide::wide(id).unwrap().text.as_ref(),
        "color(display-p3 0.1 0.2 0.3)"
    );
}
