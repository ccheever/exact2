//! Both ingress forms are one write path: a frame and the same ops applied
//! in-process produce identical trees, and a bad frame applies nothing.

use exact_kernel::{
    wire, DecodeError, Dimension, Kernel, KernelError, NodeType, Offer, Op, PropId, StyleId,
    StyleProps,
};

fn ops() -> Vec<Op> {
    let mut root_style = StyleProps::default();
    root_style.width = Dimension::Points(120.0);
    root_style.mask.set(StyleId::Width);
    root_style.height = Dimension::Percent(50.0);
    root_style.mask.set(StyleId::Height);
    let mut text_style = StyleProps::default();
    text_style.font_size = 10.0;
    text_style.mask.set(StyleId::FontSize);
    vec![
        Op::CreateView {
            id: 1,
            node_type: NodeType::ScrollView,
        },
        Op::CreateView {
            id: 2,
            node_type: NodeType::Text,
        },
        Op::CreateView {
            id: 3,
            node_type: NodeType::Pressable,
        },
        Op::SetStyle {
            id: 1,
            patch: Box::new(root_style),
        },
        Op::SetStyle {
            id: 2,
            patch: Box::new(text_style),
        },
        Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: "Caltrain".into(),
        },
        Op::SetProp {
            id: 3,
            prop: PropId::TestId,
            value: "cta".into(),
        },
        Op::SetProp {
            id: 3,
            prop: PropId::Disabled,
            value: true.into(),
        },
        Op::SetProp {
            id: 3,
            prop: PropId::TabIndex,
            value: 2i64.into(),
        },
        Op::SetProp {
            id: 3,
            prop: PropId::HitSlop,
            value: 4.0f64.into(),
        },
        Op::SetChildren {
            id: 1,
            children: vec![2, 3],
        },
        Op::AttachRoot { id: 1 },
    ]
}

#[test]
fn a_frame_and_structured_apply_produce_the_same_tree() {
    let ops = ops();
    let bytes = wire::encode(5, 77, &ops);

    let mut via_frame = Kernel::with_monospace();
    let r1 = via_frame.apply_frame(&bytes).unwrap();
    let mut via_ops = Kernel::with_monospace();
    let r2 = via_ops.apply(5, 77, &ops).unwrap();

    assert_eq!(r1, r2, "identical receipts");
    assert_eq!(r1.batch, 77);
    assert_eq!(r1.root_id, 5);
    via_frame
        .compute_layout(1, Offer::definite(300.0, 400.0))
        .unwrap();
    via_ops
        .compute_layout(1, Offer::definite(300.0, 400.0))
        .unwrap();
    assert_eq!(
        via_frame.export(None).unwrap(),
        via_ops.export(None).unwrap(),
        "identical exports"
    );
    assert_eq!(
        via_frame.node(1).unwrap().frame.height,
        200.0,
        "50% of the 400pt offer"
    );
    assert_eq!(
        via_frame.node(3).unwrap().props.bool(PropId::Disabled),
        Some(true)
    );
}

#[test]
fn a_malformed_frame_applies_nothing() {
    let mut k = Kernel::with_monospace();
    k.apply(0, 1, &ops()).unwrap();
    let before = k.export(None).unwrap();
    let epoch = k.epoch();

    let mut bytes = wire::encode(
        0,
        2,
        &[
            Op::SetProp {
                id: 2,
                prop: PropId::Text,
                value: "changed".into(),
            },
            Op::DestroyView { id: 3 },
        ],
    );
    // Corrupt the second op's opcode: it starts right after the first op's padded bytes.
    let first_op_len = wire::encode(
        0,
        2,
        &[Op::SetProp {
            id: 2,
            prop: PropId::Text,
            value: "changed".into(),
        }],
    )
    .len()
        - wire::frame::HEADER_LEN;
    let second = wire::frame::HEADER_LEN + first_op_len;
    bytes[second] = 0xee;
    let err = k.apply_frame(&bytes).unwrap_err();
    assert_eq!(err, KernelError::Decode(DecodeError::UnknownOpcode(0xee)));
    assert_eq!(
        k.export(None).unwrap(),
        before,
        "the valid first op was not applied either"
    );
    assert_eq!(k.epoch(), epoch);
    assert_eq!(k.node(2).unwrap().props.str(PropId::Text), Some("Caltrain"));
}

#[test]
fn a_frame_from_another_schema_is_refused() {
    let mut k = Kernel::with_monospace();
    let mut bytes = wire::encode(0, 1, &ops());
    bytes[24..32].copy_from_slice(&0x1234_5678_9abc_def0u64.to_le_bytes());
    assert!(matches!(
        k.apply_frame(&bytes),
        Err(KernelError::Decode(
            DecodeError::SchemaDigestMismatch { .. }
        ))
    ));
    assert_eq!(k.live_count(), 0);
}

#[test]
fn frame_builder_streams_ops() {
    let ops = ops();
    let mut b = wire::FrameBuilder::new(1, 3);
    for op in &ops {
        b.op(op);
    }
    let bytes = b.build();
    assert_eq!(bytes, wire::encode(1, 3, &ops));
    let frame = wire::decode(&bytes).unwrap();
    assert_eq!(frame.ops, ops);
}

#[test]
fn every_nonempty_payload_padding_width_requires_zero_bytes() {
    for remainder in 1..=7usize {
        // SetProp(string) is 7 + string length bytes, so choose a length
        // that realizes each possible nonzero payload remainder.
        let string_len = (remainder + 1) % 8;
        let op = Op::SetProp {
            id: 1,
            prop: PropId::Text,
            value: "x".repeat(string_len).into(),
        };
        let mut bytes = wire::encode(0, 1, &[op]);
        let header = wire::frame::HEADER_LEN;
        let payload_len =
            u32::from_le_bytes(bytes[header + 8..header + 12].try_into().unwrap()) as usize;
        assert_eq!(payload_len % 8, remainder);
        let padding = header + wire::frame::OP_HEADER_LEN + payload_len;
        bytes[padding] = 0x80;
        assert_eq!(
            wire::decode(&bytes),
            Err(DecodeError::NonZeroPadding { offset: padding })
        );
    }
}
