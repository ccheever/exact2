//! Stage 14 C2: an image node showing an even-odd icon (its path's
//! `evenodd:` marker, read by `exact_kernel::Icon`) is sent its path, the
//! marker gone, and `symbolFillRule`; a nonzero icon's props are as before.
use super::*;
use exact_kernel::MonospaceMeasurer;
use exact_plan::Value;
use exact_runner::DataError;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = r#"component A
  state even = "symbol:icon||0 0 24 24|evenodd:M2 2 L22 2 L22 22 L2 22 Z M8 8 L16 8 L16 16 L8 16 Z|icons/frame.svg"
  state plain = "symbol:icon||0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z M8 8 L16 8 L16 16 L8 16 Z|icons/block.svg"
  view
    column
      image even testId="even"
      image plain testId="plain"
"#;

#[test]
fn an_even_odd_icons_image_is_sent_its_rule_and_a_nonzero_ones_is_not() {
    let plan = contract::compile(APP).unwrap().encode();
    let (host, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let k = host.runner.kernel();
    let props = |t: &str| host.props_of(&k.node_by_key(k.find_by_test_id(t)[0]).unwrap());
    let even = props("even");
    let d = "M 2 2 L 22 2 L 22 22 L 2 22 Z M 8 8 L 16 8 L 16 16 L 8 16 Z";
    assert_eq!(
        even.get("symbolPath").map(String::as_str),
        Some(d),
        "the marker is not a command"
    );
    assert_eq!(
        even.get("symbolFillRule").map(String::as_str),
        Some("evenodd")
    );
    let plain = props("plain");
    assert_eq!(plain.get("symbolPath").map(String::as_str), Some(d));
    assert_eq!(
        plain.get("symbolFillRule"),
        None,
        "nonzero: what it always sent"
    );
}
