//! A negative calculated row the presenter can't take, as sent.
use super::*;
use exact_kernel::{StyleId, StyleValue};

/// Presenters read padding from this dictionary.
#[test]
fn a_negative_calculated_padding_is_sent_as_zero() {
    let mut s = StyleProps::default();
    s.set_dynamic(
        StyleId::PaddingLeft,
        &StyleValue::Text("calc(0px - 10px)".into()),
    )
    .unwrap();
    let (json, _) = style_json(&s, &Env::default());
    assert!(json.contains("\"padding_left\":0"), "{json}");
}
