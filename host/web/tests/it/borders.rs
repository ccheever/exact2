//! LLP 1053 G2: the parity page's per-side border colours reach the DOM as
//! CSS's own longhands — `currentcolor` a keyword, `light-dark()` a function
//! the browser resolves — so Chrome paints exactly what was authored. Its
//! pictures of the page (`scripts/fixtures/borders.web*.png`) are the other
//! hosts' oracle.

use exact_runner::{DataError, DataSource, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

#[test]
fn every_side_is_its_own_css_longhand() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/fixtures/borders.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (_, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    for expected in [
        // `border-color` with four values: one longhand a side.
        "border-top-color:rgba(229,57,53,1);border-right-color:rgba(67,160,71,1);border-bottom-color:rgba(30,136,229,1);border-left-color:rgba(253,216,53,1);",
        // Three values: the left side is the right's.
        "border-right-color:rgba(0,0,0,0);border-bottom-color:rgba(30,136,229,1);border-left-color:rgba(0,0,0,0);",
        // An explicit `currentcolor` stays the keyword.
        "border-bottom-color:currentcolor;border-left-color:currentcolor;",
        // `light-dark()` per side, for the browser to resolve.
        "border-top-color:light-dark(rgba(229,57,53,1), rgba(128,222,234,1));",
    ] {
        assert!(batch.contains(expected), "{expected} in {batch}");
    }
}

#[test]
fn patterned_border_styles_reach_css_on_their_own_sides() {
    let src = "component A\n  view\n    box border-width=\"1px 2px 4px 3px\" border-style=\"dotted dashed solid none\" border-radius=12 testId=\"pattern\"\n";
    let plan = contract::compile(src).unwrap();
    let (_, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    assert!(batch.contains("border-top-style:dotted;border-right-style:dashed;border-bottom-style:solid;border-left-style:none;"), "{batch}");
    assert!(batch.contains("border-top-width:1px;border-right-width:2px;border-bottom-width:4px;border-left-width:3px;"), "{batch}");
}
