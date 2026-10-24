use exact_kernel::{Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps, StyleValue};
fn create(id: u32, node_type: NodeType) -> Op {
    Op::CreateView { id, node_type }
}
fn prop(id: u32, text: &str) -> Op {
    Op::SetProp {
        id,
        prop: PropId::Text,
        value: text.into(),
    }
}
fn style(id: u32, rows: &[(StyleId, StyleValue)]) -> Op {
    let mut patch = StyleProps::default();
    for (id, v) in rows {
        patch.set_dynamic(*id, v).unwrap();
    }
    Op::SetStyle {
        id,
        patch: Box::new(patch),
    }
}
fn n(v: f64) -> StyleValue<'static> {
    StyleValue::Number(v)
}
fn t(v: &str) -> StyleValue<'_> {
    StyleValue::Text(v.into())
}
fn fixture() -> Kernel {
    use StyleId::*;
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            create(1, NodeType::Text),
            create(2, NodeType::Text),
            create(3, NodeType::Image),
            create(4, NodeType::Text),
            create(5, NodeType::Text),
            style(
                1,
                &[
                    (Width, n(180.0)),
                    (FontSize, n(10.0)),
                    (LineHeight, t("12px")),
                ],
            ),
            prop(2, "Hello "),
            style(3, &[(Width, n(20.0)), (Height, n(20.0))]),
            prop(4, " @Alice "),
            style(
                4,
                &[
                    (Display, t("inline-block")),
                    (PaddingLeft, n(4.0)),
                    (PaddingRight, n(4.0)),
                    (BackgroundColor, t("#ddeeff")),
                    (BorderRadiusTopLeft, n(8.0)),
                ],
            ),
            prop(5, " after the mention"),
            Op::SetChildren {
                id: 1,
                children: vec![2, 3, 4, 5],
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    k
}
#[test]
fn images_and_atomic_mentions_share_the_paragraph_baseline_and_wrap() {
    use StyleId::*;
    let mut k = fixture();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    let image = k.node(3).unwrap().frame;
    let pill = k.node(4).unwrap().frame;
    assert_eq!(image.width, 20.0);
    assert_eq!(image.height, 20.0);
    assert_eq!(pill.width, 44.0);
    assert!(pill.x >= image.x + image.width);
    assert!((image.y + image.height - (pill.y + 9.6)).abs() < 0.01);
    let before = k.node(1).unwrap().frame.height;
    k.apply(0, 2, &[style(1, &[(Width, n(65.0))])]).unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    let narrow = k.node(4).unwrap().frame;
    assert_eq!(
        narrow.width, pill.width,
        "mention is never split at its space"
    );
    assert!(narrow.y > pill.y);
    assert!(k.node(1).unwrap().frame.height > before);
    k.apply(0, 3, &[style(3, &[(Width, n(40.0)), (Height, n(30.0))])])
        .unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert_eq!(k.node(3).unwrap().frame.width, 40.0);
    let mut fresh = k.rehydrate(Box::new(exact_kernel::MonospaceMeasurer::default()));
    fresh
        .compute_layout(1, Offer::definite(300.0, 500.0))
        .unwrap();
    for id in 1..=5 {
        assert_eq!(k.node(id).unwrap().frame, fresh.node(id).unwrap().frame);
    }
}
#[test]
fn hidden_boxes_and_own_text_do_not_leave_phantom_advance_or_frames() {
    use StyleId::*;
    let mut k = fixture();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    let runs = k.node(1).unwrap().text_runs();
    assert_eq!(
        runs.iter().map(|r| r.text.as_ref()).collect::<String>(),
        "Hello \u{fffc}\u{fffc} after the mention"
    );
    k.apply(0, 2, &[style(3, &[(Display, t("none"))])]).unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert_eq!(k.node(3).unwrap().frame.width, 0.0);
    assert_eq!(k.node(1).unwrap().text_runs().len(), 3);
    k.apply(0, 3, &[prop(1, "Replacement")]).unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert_eq!(k.node(4).unwrap().frame.width, 0.0);
}

#[test]
fn percentage_emoji_uses_the_paragraph_width_and_clamp_hides_later_boxes() {
    use StyleId::*;
    let mut k = fixture();
    k.apply(0, 2, &[style(3, &[(Width, t("50%"))])]).unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert_eq!(k.node(3).unwrap().frame.width, 90.0);
    k.apply(0, 3, &[style(1, &[(Width, n(40.0)), (LineClamp, n(1.0))])])
        .unwrap();
    k.compute_layout(1, Offer::definite(300.0, 500.0)).unwrap();
    assert_eq!(k.node(4).unwrap().frame.width, 0.0);
}
