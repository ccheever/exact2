//! A symbol's authored `-exact-tint-color` is no paint of it (theme2 2026-10-23, after `lower-symbol-tint`): a symbol
//! is drawn in its `color` on every host (LLP 1081 stage 2), so lowering answers the tint with `currentcolor`, each
//! state's row too, which Apple already drew and the web now draws. A native button's face keeps its tint (LLP 1115
//! D4), and a raster's tint is still its silhouette's paint.

use exact_kernel::{Color, ColorValue, Kernel, StyleId};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn a_symbols_tint_is_its_colour_and_a_native_face_or_a_raster_keeps_its_own() {
    let src = "component A\n  action go\n  view\n    column color=\"#0000ff\"\n      image \"symbol:heart\" testId=\"tinted\" -exact-tint-color=\"#ff0000\" alt=\"Heart\"\n      image \"symbol:heart\" testId=\"stated\" -exact-tint-color=\"#ff0000\" alt=\"Heart\"\n        :disabled\n          -exact-tint-color=\"#00ff00\"\n      image \"https://example.com/a.png\" testId=\"raster\" -exact-tint-color=\"#ff0000\" alt=\"A\"\n      button press=go testId=\"lock\"\n        image \"symbol:lock\" testId=\"face\" -exact-tint-color=\"#ff0000\"\n        text \"Lock\"\n";
    let plan = contract::bake(contract::compile(src).unwrap(), NoData).unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let tint = |id: &str| {
        let n = k.node_by_key(k.find_by_test_id(id)[0]).unwrap();
        (n.style.mask.has(StyleId::TintColor), n.style.tint_color)
    };
    let red: ColorValue = Color::parse_hex("#ff0000").unwrap().into();
    // `currentcolor`: the keyword, which each host paints in `color` (styles_rows.rs).
    assert_eq!(tint("tinted"), (true, None));
    assert_eq!(tint("stated"), (true, None));
    assert_eq!(tint("raster"), (true, Some(red)));
    assert_eq!(tint("face"), (true, Some(red)));
}
