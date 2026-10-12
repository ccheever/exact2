//! LLP 1115.000 D10 on the Linux host (and so on Android's and Windows's,
//! which lay out and paint through this presenter): a control's spacing and
//! size are the plan's, the same on every host.
use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

fn boot(app: &str, scale: f32) -> Presenter<Empty> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(app)
            .unwrap_or_else(|e| panic!("{e}"))
            .encode(),
        Empty,
        (400., 400.),
        scale,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn rect(p: &mut Presenter<Empty>, test_id: &str) -> (f32, f32, f32, f32) {
    let k = p.host().kernel();
    let id = k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id;
    p.rect_of(id).unwrap()
}

/// D10.1: a checkbox, a radio and a range sit where their row puts them,
/// with no margin of their own (Chrome's UA margins put the checkbox at 4,
/// 3 and each next control 7, 8 and 5 points further on).
#[test]
fn a_checkbox_a_radio_and_a_range_have_no_margin_of_their_own() {
    let mut p = boot(
        r#"component App
  view
    row width=400 testId="row"
      input type="checkbox" testId="check"
      input type="radio" testId="radio"
      input type="range" width=90 testId="range"
      box width=10 height=10 testId="after"
"#,
        1.,
    );
    let [check, radio, range, after] =
        ["check", "radio", "range", "after"].map(|id| rect(&mut p, id));
    assert_eq!((check.0, check.1), (0.0, 0.0), "the checkbox");
    assert_eq!((radio.0, radio.1), (check.2, 0.0), "the radio touches it");
    assert_eq!((range.0, range.1), (check.2 + radio.2, 0.0), "the range");
    assert_eq!(after.0, check.2 + radio.2 + 90.0, "and what follows");
}
