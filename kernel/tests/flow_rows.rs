//! @ref LLP 1043.000 §3 D1 — generated rows, masks, and wire validation.
use exact_kernel::{
    wire::codec::{Reader, Writer},
    DecodeError, ShapeOutside, StyleId, StyleMask, StyleProps, StyleValue, WrapFlow,
};

#[test]
fn defaults_masks_and_wire_vocabulary() {
    let defaults = StyleProps::default();
    assert_eq!(defaults.wrap_flow, WrapFlow::Auto);
    assert_eq!(defaults.shape_outside, ShapeOutside::default());
    assert_eq!(defaults.shape_margin.to_bits(), 0f32.to_bits());
    for (id, bit) in [
        (StyleId::WrapFlow, 95),
        (StyleId::ShapeOutside, 96),
        (StyleId::ShapeMargin, 97),
    ] {
        assert_eq!(id as u32, bit);
        assert!(!id.inherited());
        assert!(id.affects_layout());
        assert!(!StyleMask::TEXT.has(id));
    }
    assert_eq!(
        WrapFlow::ALL.map(|v| v.name()),
        ["auto", "both", "start", "end", "minimum", "maximum", "clear"]
    );
    for wrap in WrapFlow::ALL {
        let mut s = StyleProps::default();
        s.set_dynamic(StyleId::WrapFlow, &StyleValue::Text(wrap.name().into()))
            .unwrap();
        s.set_dynamic(
            StyleId::ShapeOutside,
            &StyleValue::Text("circle(40% at 20px 60%)".into()),
        )
        .unwrap();
        s.set_dynamic(StyleId::ShapeMargin, &StyleValue::Number(8.))
            .unwrap();
        let mut w = Writer::new();
        s.encode_patch(&mut w);
        assert_eq!(
            StyleProps::decode_patch(&mut Reader::new(w.as_slice())).unwrap(),
            s
        );
    }
}

#[test]
fn bad_shape_outside_is_a_typed_decode_rejection() {
    let mut w = Writer::new();
    w.style_mask(StyleMask::of(StyleId::ShapeOutside));
    w.string("url(dancer.png)");
    assert_eq!(
        StyleProps::decode_patch(&mut Reader::new(w.as_slice())),
        Err(DecodeError::BadShapeOutside)
    );
}
