//! LLP 1102 §3.10–§3.11: CSS's own spellings reach the DOM as CSS. An unbounded
//! maximum is `none` (`max-width: auto` is no value), whether it was written
//! `none` or `auto`, and a pixel row's `14px` is its number in pixels.

use exact_runner::{DataError, DataSource, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn an_unbounded_maximum_is_none_and_a_px_text_is_pixels() {
    let src = "component App\n  view\n    column max-height=\"none\" max-width=\"auto\"\n      text \"a\" font-size=\"14px\" letter-spacing=\"-0.5px\"\n";
    let plan = contract::compile(src).unwrap();
    let (_, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    for expected in [
        "max-height:none",
        "max-width:none",
        "font-size:14px",
        "letter-spacing:-0.5px",
    ] {
        assert!(batch.contains(expected), "{expected} in {batch}");
    }
    assert!(
        !batch.contains("max-height:auto") && !batch.contains("max-width:auto"),
        "{batch}"
    );
}
