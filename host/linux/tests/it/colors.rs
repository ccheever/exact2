//! LLP 1100 D10: this host draws sRGB only. A colour outside it, set at run
//! time, is refused as an invalid colour is (the bake refuses a literal one).
use exact_kernel::{ColorValue, MonospaceMeasurer};
use exact_linux::Host;
use exact_runner::{DataError, DataSource, Event};

struct NoData;
impl DataSource for NoData {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

#[test]
fn a_colour_outside_srgb_is_refused_as_it_is_set() {
    // The wide colour is state, not a literal: another test's boot limits
    // this process to sRGB, and the compiler checks literals against it.
    let src = "component A\n  state wide = false\n  state p3 = \"color(display-p3 1 0 0)\"\n  action go()\n    wide = not wide\n  view\n    column\n      button press=go testId=\"go\"\n        text \"Go\"\n      box testId=\"in\" width=4 height=4 background-color=\"oklch(0.7 0.05 200)\"\n      box testId=\"b\" width=4 height=4 background-color=(wide ? p3 : \"#00ff00\")\n";
    let plan = contract::compile(src).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let node = |h: &Host<NoData>, t: &str| {
        let k = h.kernel();
        k.node_by_key(k.find_by_test_id(t)[0])
            .unwrap()
            .style
            .background_color
    };
    assert!(
        matches!(node(&h, "in"), Some(ColorValue::Wide(_))),
        "inside sRGB: drawn, in any CSS form"
    );
    assert_ne!(
        node(&h, "b"),
        Some(ColorValue::Fixed(exact_kernel::Color(0)))
    );
    let go = {
        let k = h.kernel();
        k.node_by_key(k.find_by_test_id("go")[0]).unwrap().id
    };
    h.dispatch_at(go, Event::Press, 100.);
    assert_eq!(
        node(&h, "b"),
        Some(ColorValue::Fixed(exact_kernel::Color(0))),
        "outside sRGB: refused, as an invalid colour is"
    );
}
