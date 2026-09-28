//! @ref LLP 1069.000 D3 — `rem` and `em` from a Contract source to the
//! kernel, and the root font size a host sets through the runner.
use exact_kernel::{Dimension, Kernel, RowValue, StyleId};
use exact_runner::{DataError, DataSource, Runner, RunnerError, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("no data: {source}")
    }
}

const SOURCE: &str = "component App\n  view\n    column testId=\"box\" padding-top=\"1rem\" font-size=\"0.875rem\"\n      text \"Offline\" testId=\"label\" letter-spacing=\"0.1em\" margin-top=\"8px\"\n";

fn node<'a>(r: &'a Runner<NoData>, id: &str) -> exact_kernel::NodeRef<'a> {
    let key = r.kernel().find_by_test_id(id)[0];
    r.kernel().node_by_key(key).unwrap()
}

fn font(r: &Runner<NoData>, id: &str) -> f64 {
    match node(r, id).computed(StyleId::FontSize) {
        RowValue::Number(n) => n,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_root_font_size_rescales_rem_and_em_in_one_commit() {
    let plan = contract::bake(contract::compile(SOURCE).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.root_font_size(), 16.0);
    assert_eq!(font(&r, "label"), 14.0);
    assert_eq!(node(&r, "box").style.padding_top, Dimension::Points(16.0));
    assert!(r.set_root_font_size(24.0).unwrap().is_some());
    assert_eq!(font(&r, "label"), 21.0);
    assert_eq!(node(&r, "box").style.padding_top, Dimension::Points(24.0));
    assert!((node(&r, "label").style.letter_spacing - 2.1).abs() < 1e-5);
    assert_eq!(node(&r, "label").style.margin_top, Dimension::Points(8.0));
    assert!(
        r.set_root_font_size(24.0).unwrap().is_none(),
        "the same size"
    );
    assert!(matches!(
        r.set_root_font_size(f64::NAN),
        Err(RunnerError::Kernel(_))
    ));
    assert_eq!(r.root_font_size(), 24.0);
}
