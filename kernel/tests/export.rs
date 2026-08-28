//! EXNODE: the envelope round-trips the typed rows, and a corrupted envelope
//! is refused whole.

use exact_kernel::{
    export, Color, DecodeError, Dimension, Kernel, NodeType, Offer, Op, PropId, StyleId, StyleProps,
};

fn kernel() -> Kernel {
    let mut root = StyleProps::default();
    root.width = Dimension::Points(100.0);
    root.mask.set(StyleId::Width);
    root.height = Dimension::Points(100.0);
    root.mask.set(StyleId::Height);
    root.background_color = Color::rgba(1, 2, 3, 4);
    root.mask.set(StyleId::BackgroundColor);
    let mut leaf = StyleProps::default();
    leaf.font_size = 10.0;
    leaf.mask.set(StyleId::FontSize);
    leaf.opacity = 0.5;
    leaf.mask.set(StyleId::Opacity);
    let mut k = Kernel::with_monospace();
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 10,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 11,
                node_type: NodeType::Text,
            },
            Op::CreateView {
                id: 12,
                node_type: NodeType::Toggle,
            },
            Op::SetStyle {
                id: 10,
                patch: Box::new(root),
            },
            Op::SetStyle {
                id: 11,
                patch: Box::new(leaf),
            },
            Op::SetProp {
                id: 11,
                prop: PropId::Text,
                value: "hi".into(),
            },
            Op::SetProp {
                id: 11,
                prop: PropId::TestId,
                value: "greeting".into(),
            },
            Op::SetProp {
                id: 12,
                prop: PropId::ToggleValue,
                value: true.into(),
            },
            Op::SetProp {
                id: 12,
                prop: PropId::TabIndex,
                value: (-1i64).into(),
            },
            Op::SetProp {
                id: 12,
                prop: PropId::HitSlop,
                value: 2.5f64.into(),
            },
            Op::SetChildren {
                id: 10,
                children: vec![11, 12],
            },
            Op::AttachRoot { id: 10 },
        ],
    )
    .unwrap();
    k.compute_layout(10, Offer::definite(200.0, 200.0)).unwrap();
    k
}

#[test]
fn envelope_round_trips_rows_styles_and_props() {
    let k = kernel();
    let bytes = k.export(Some(10)).unwrap();
    assert_eq!(bytes.len() % 8, 0);
    let snap = export::decode(&bytes).unwrap();
    assert_eq!(snap.root_id, 10);
    assert_eq!(snap.epoch, k.epoch());
    let rows = k.rows(Some(10)).unwrap();
    assert_eq!(snap.rows.len(), 3);
    for (decoded, live) in snap.rows.iter().zip(rows.iter()) {
        assert_eq!(decoded.id, live.id);
        assert_eq!(decoded.parent, live.parent);
        assert_eq!(decoded.node_type, live.node_type);
        assert_eq!(decoded.depth, live.depth);
        assert_eq!(decoded.flags, live.flags);
        assert!(decoded.frame.bits_eq(live.frame));
        assert_eq!(decoded.key.generation, live.key.generation);
    }
    assert_eq!(snap.rows[0].flags & export::ROW_ROOT, export::ROW_ROOT);
    assert_eq!(snap.rows[1].frame.height, 12.0);

    assert_eq!(snap.styles[0].width, Dimension::Points(100.0));
    assert_eq!(snap.styles[0].background_color, Color::rgba(1, 2, 3, 4));
    assert!(snap.styles[0].mask.has(StyleId::BackgroundColor));
    assert_eq!(snap.styles[1].opacity, 0.5);
    assert_eq!(snap.styles[2].mask, exact_kernel::StyleMask::EMPTY);

    assert_eq!(snap.props[1].str(PropId::Text), Some("hi"));
    assert_eq!(snap.props[1].str(PropId::TestId), Some("greeting"));
    assert_eq!(snap.props[2].bool(PropId::ToggleValue), Some(true));
    assert_eq!(
        snap.props[2].get(PropId::TabIndex).and_then(|v| v.as_int()),
        Some(-1)
    );
    assert_eq!(
        snap.props[2]
            .get(PropId::HitSlop)
            .and_then(|v| v.as_float()),
        Some(2.5)
    );
}

#[test]
fn every_root_export_covers_multiple_roots() {
    let mut k = kernel();
    k.apply(
        0,
        2,
        &[
            Op::CreateView {
                id: 20,
                node_type: NodeType::View,
            },
            Op::AttachRoot { id: 20 },
        ],
    )
    .unwrap();
    let snap = export::decode(&k.export(None).unwrap()).unwrap();
    assert_eq!(snap.root_id, 0);
    assert_eq!(
        snap.rows.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![10, 11, 12, 20]
    );
}

#[test]
fn a_corrupted_section_is_refused_whole() {
    let k = kernel();
    let bytes = k.export(Some(10)).unwrap();
    // Flip a byte inside the NODES section payload.
    let mut corrupted = bytes.clone();
    let offset = export::HEADER_LEN + 3 * export::DIR_ENTRY_LEN + 4;
    corrupted[offset] ^= 0x01;
    assert!(export::decode(&corrupted).is_err());
    // Truncation is a length mismatch, not a partial adoption.
    assert!(matches!(
        export::decode(&bytes[..bytes.len() - 8]),
        Err(DecodeError::FrameLengthMismatch { .. })
    ));
    // Wrong magic.
    let mut bad = bytes.clone();
    bad[0] = b'X';
    assert_eq!(export::decode(&bad), Err(DecodeError::BadMagic));
}
