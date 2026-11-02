use exact_runner::Event;

#[test]
fn length_prefixed_terminal_distinguishes_empty_unicode_none_and_some() {
    for item in ["", "🦀,\n\0", "None"] {
        for before in [None, Some(""), Some("東京,\n\0")] {
            let e = Event::ReorderDrop {
                item: item.into(),
                before: before.map(str::to_owned),
            };
            let bytes = e.reorder_drop_bytes().unwrap();
            assert_eq!(Event::reorder_drop_payload(&bytes), Some(e));
            for n in 0..bytes.len() {
                assert_eq!(Event::reorder_drop_payload(&bytes[..n]), None);
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert_eq!(Event::reorder_drop_payload(&trailing), None);
        }
    }
}
#[test]
fn hostile_lengths_utf8_versions_and_option_tags_are_refused() {
    for bytes in [
        vec![],
        vec![2, 0, 0, 0],
        vec![1, 0, 0, 0, 255, 255, 255, 255],
        vec![1, 0, 0, 0, 1, 0, 0, 0, 255, 0],
        vec![1, 0, 0, 0, 0, 0, 0, 0, 2],
    ] {
        assert_eq!(Event::reorder_drop_payload(&bytes), None);
    }
}

#[test]
fn numeric_geometry_codec_keeps_full_u64_and_rejects_malformed_or_nonfinite_facts() {
    use exact_kernel::NodeKey;
    use exact_runner::ReorderGeometry;
    let g = ReorderGeometry {
        list: NodeKey {
            index: u32::MAX,
            generation: u32::MAX,
        },
        revision: u64::MAX,
        scroll_sequence: 9_007_199_254_740_993,
        scroll_top: 12.5,
        port_width: 320.,
        port_height: 100.,
        row_width: 318.,
        total_extent: 1e6,
    };
    let wire = g.encode().unwrap();
    assert_eq!(wire.len(), 68);
    assert_eq!(ReorderGeometry::decode(&wire).unwrap(), g);
    for n in 0..wire.len() {
        assert!(ReorderGeometry::decode(&wire[..n]).is_err());
    }
    let mut extra = wire.clone();
    extra.push(0);
    assert!(ReorderGeometry::decode(&extra).is_err());
    let mut nan = wire;
    nan[28..36].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(ReorderGeometry::decode(&nan).is_err());
    let mut bad = g;
    bad.row_width = -1.;
    assert!(bad.encode().is_err());
}
