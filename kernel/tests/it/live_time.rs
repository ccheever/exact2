//! Width reservation is measured at the element's inherited font, not
//! at a guessed digit width. @ref LLP 1126 D6
use exact_kernel::*;

#[test]
fn live_time_keeps_its_frame_and_following_sibling_when_its_text_ticks() {
    for font_size in [15., 28.] {
        let mut k = Kernel::with_monospace();
        let mut row = StyleProps::default();
        row.set_dynamic(StyleId::Display, &StyleValue::Text("flex".into()))
            .unwrap();
        row.set_dynamic(StyleId::FontSize, &StyleValue::Number(font_size))
            .unwrap();
        let mut ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(row),
            },
            Op::AttachRoot { id: 1 },
        ];
        for id in [2, 3] {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::Text,
            });
        }
        ops.extend([
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "in 5 min.".into(),
            },
            Op::SetProp {
                id: 2,
                prop: PropId::TextReservation,
                value: "59 min, 59 sec ago\nin 59 min, 59 sec".into(),
            },
            Op::SetProp {
                id: 3,
                prop: PropId::Text,
                value: "Following".into(),
            },
            Op::SetChildren {
                id: 1,
                children: vec![2, 3],
            },
        ]);
        k.apply(0, 1, &ops).unwrap();
        k.compute_layout(1, Offer::definite(1000., 100.)).unwrap();
        let before = (k.node(2).unwrap().frame, k.node(3).unwrap().frame);
        assert!(before.0.width >= 18. * font_size as f32 * 0.6);
        k.apply(
            0,
            2,
            &[Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "59 min, 59 sec ago".into(),
            }],
        )
        .unwrap();
        k.compute_layout(1, Offer::definite(1000., 100.)).unwrap();
        assert_eq!(before, (k.node(2).unwrap().frame, k.node(3).unwrap().frame));
    }
}

#[test]
fn inline_time_uses_natural_runs_and_can_grow() {
    let mut k = Kernel::with_monospace();
    let mut row = StyleProps::default();
    row.set_dynamic(StyleId::Display, &StyleValue::Text("flex".into()))
        .unwrap();
    let mut ops = vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::View,
        },
        Op::AttachRoot { id: 1 },
        Op::SetStyle {
            id: 1,
            patch: Box::new(row),
        },
    ];
    for id in 2..=5 {
        ops.push(Op::CreateView {
            id,
            node_type: NodeType::Text,
        });
    }
    ops.extend([
        Op::SetChildren {
            id: 1,
            children: vec![2],
        },
        Op::SetChildren {
            id: 2,
            children: vec![3, 4, 5],
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Text,
            value: "Updated ".into(),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "9 min. ago".into(),
        },
        Op::SetProp {
            id: 4,
            prop: PropId::TextReservation,
            value: "59 min, 59 sec ago".into(),
        },
        Op::SetProp {
            id: 5,
            prop: PropId::Text,
            value: " here".into(),
        },
    ]);
    k.apply(0, 1, &ops).unwrap();
    k.compute_layout(1, Offer::definite(1000., 200.)).unwrap();
    let before = k.node(2).unwrap().frame.width;
    let runs = k.node(2).unwrap().text_runs();
    assert_eq!(
        runs.iter().map(|r| &*r.text).collect::<Vec<_>>(),
        ["Updated ", "9 min. ago", " here"]
    );
    k.apply(
        0,
        2,
        &[Op::SetProp {
            id: 4,
            prop: PropId::Text,
            value: "10 min. ago".into(),
        }],
    )
    .unwrap();
    k.compute_layout(1, Offer::definite(1000., 200.)).unwrap();
    assert!(k.node(2).unwrap().frame.width > before);
}
