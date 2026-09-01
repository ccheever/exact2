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

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5u32, |hash, byte| {
        (hash ^ *byte as u32).wrapping_mul(0x0100_0193)
    })
}

fn section_info(bytes: &[u8], kind: u32) -> (usize, usize, usize) {
    let count = read_u32(bytes, 28) as usize;
    for index in 0..count {
        let entry = export::HEADER_LEN + index * export::DIR_ENTRY_LEN;
        if read_u32(bytes, entry) == kind {
            return (
                entry,
                read_u32(bytes, entry + 4) as usize,
                read_u32(bytes, entry + 8) as usize,
            );
        }
    }
    panic!("section {kind} not found");
}

fn refresh_checksum(bytes: &mut [u8], kind: u32) {
    let (entry, start, len) = section_info(bytes, kind);
    let checksum = fnv1a32(&bytes[start..start + len]);
    bytes[entry + 12..entry + 16].copy_from_slice(&checksum.to_le_bytes());
}

fn prop_entry(bytes: &[u8], row: usize, wanted: PropId) -> (usize, usize) {
    let (_, start, _) = section_info(bytes, export::SECTION_PROPS);
    let mut offset = start;
    for current_row in 0..=row {
        let count = read_u16(bytes, offset) as usize;
        offset += 2;
        for _ in 0..count {
            let id_offset = offset;
            let id = PropId::from_wire(read_u16(bytes, offset)).unwrap();
            offset += 2;
            let kind = bytes[offset];
            offset += 1;
            let value_offset = offset;
            offset += match kind {
                0 => 4 + read_u32(bytes, offset) as usize,
                1 => 1,
                2 | 3 => 8,
                _ => panic!("fixture carried unknown prop kind {kind}"),
            };
            if current_row == row && id == wanted {
                return (id_offset, value_offset);
            }
        }
    }
    panic!("prop {wanted:?} not found on row {row}");
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

#[test]
fn malformed_envelope_scalars_and_topology_are_refused_exactly() {
    let bytes = kernel().export(Some(10)).unwrap();

    let mut reserved = bytes.clone();
    reserved[32] = 1;
    assert_eq!(export::decode(&reserved), Err(DecodeError::ReservedFlags));

    let mut header = bytes.clone();
    header[6..8].copy_from_slice(&48u16.to_le_bytes());
    assert_eq!(
        export::decode(&header),
        Err(DecodeError::BadHeaderLength(48))
    );

    let mut flags = bytes.clone();
    let (_, nodes, _) = section_info(&flags, export::SECTION_NODES);
    flags[nodes + 13] |= 0x80;
    let bad_flags = flags[nodes + 13];
    refresh_checksum(&mut flags, export::SECTION_NODES);
    assert_eq!(
        export::decode(&flags),
        Err(DecodeError::UnknownRowFlags {
            row: 0,
            flags: bad_flags
        })
    );

    let mut frame = bytes.clone();
    let (_, nodes, _) = section_info(&frame, export::SECTION_NODES);
    frame[nodes + 16..nodes + 20].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
    refresh_checksum(&mut frame, export::SECTION_NODES);
    assert_eq!(
        export::decode(&frame),
        Err(DecodeError::NonFiniteFrame { row: 0 })
    );

    let mut topology = bytes.clone();
    let (_, nodes, _) = section_info(&topology, export::SECTION_NODES);
    topology[nodes + 2 * export::ROW_LEN + 4..nodes + 2 * export::ROW_LEN + 8]
        .copy_from_slice(&11u32.to_le_bytes());
    refresh_checksum(&mut topology, export::SECTION_NODES);
    assert_eq!(
        export::decode(&topology),
        Err(DecodeError::InvalidTopology { row: 2 })
    );

    let mut boolean = bytes.clone();
    let (_, value) = prop_entry(&boolean, 2, PropId::ToggleValue);
    boolean[value] = 2;
    refresh_checksum(&mut boolean, export::SECTION_PROPS);
    assert_eq!(
        export::decode(&boolean),
        Err(DecodeError::NonCanonicalBool(2))
    );

    let mut duplicate = bytes.clone();
    let (text_id, _) = prop_entry(&duplicate, 1, PropId::Text);
    let (test_id, _) = prop_entry(&duplicate, 1, PropId::TestId);
    let text = duplicate[text_id..text_id + 2].to_vec();
    duplicate[test_id..test_id + 2].copy_from_slice(&text);
    refresh_checksum(&mut duplicate, export::SECTION_PROPS);
    assert_eq!(
        export::decode(&duplicate),
        Err(DecodeError::DuplicateProp {
            row: 1,
            prop: PropId::Text
        })
    );

    let mut float = bytes;
    let (_, value) = prop_entry(&float, 2, PropId::HitSlop);
    float[value..value + 8].copy_from_slice(&f64::NAN.to_bits().to_le_bytes());
    refresh_checksum(&mut float, export::SECTION_PROPS);
    assert_eq!(
        export::decode(&float),
        Err(DecodeError::NonFiniteProp {
            row: 2,
            prop: PropId::HitSlop
        })
    );
}

#[test]
fn malformed_section_directories_and_trailing_payload_are_refused() {
    let bytes = kernel().export(Some(10)).unwrap();
    let nodes_entry = export::HEADER_LEN;
    let styles_entry = nodes_entry + export::DIR_ENTRY_LEN;

    let mut duplicate = bytes.clone();
    duplicate[styles_entry..styles_entry + 4].copy_from_slice(&export::SECTION_NODES.to_le_bytes());
    assert_eq!(
        export::decode(&duplicate),
        Err(DecodeError::UnexpectedSection {
            index: 1,
            expected: export::SECTION_STYLES,
            actual: export::SECTION_NODES
        })
    );

    let mut overlap = bytes.clone();
    let nodes_offset = overlap[nodes_entry + 4..nodes_entry + 8].to_vec();
    overlap[styles_entry + 4..styles_entry + 8].copy_from_slice(&nodes_offset);
    assert_eq!(
        export::decode(&overlap),
        Err(DecodeError::InvalidSectionLayout {
            section: export::SECTION_STYLES
        })
    );

    let mut header_pointing = bytes.clone();
    header_pointing[nodes_entry + 4..nodes_entry + 8]
        .copy_from_slice(&(export::HEADER_LEN as u32).to_le_bytes());
    assert_eq!(
        export::decode(&header_pointing),
        Err(DecodeError::InvalidSectionLayout {
            section: export::SECTION_NODES
        })
    );

    let mut trailing = bytes;
    let (props_entry, _, props_len) = section_info(&trailing, export::SECTION_PROPS);
    trailing.extend_from_slice(&[0; 8]);
    let total = trailing.len() as u32;
    trailing[8..12].copy_from_slice(&total.to_le_bytes());
    trailing[props_entry + 8..props_entry + 12]
        .copy_from_slice(&(props_len as u32 + 8).to_le_bytes());
    refresh_checksum(&mut trailing, export::SECTION_PROPS);
    assert_eq!(
        export::decode(&trailing),
        Err(DecodeError::TrailingSection {
            section: export::SECTION_PROPS,
            remaining: 8
        })
    );
}
