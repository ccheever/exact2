//! A grouped row's value on iOS (`grouped/value.rs`): beside its title
//! through the largest standard text size, under it at every accessibility
//! size, as UIKit's value cell stacks its two texts there
//! (`prefersSideBySideTextAndSecondaryText` off), however short the pair.
//! One test, its own process: the column is the process's.
use exact_kernel::style::metrics::{set_column, Column};
use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"component App
  view
    column testId="root" width=402 height=874
      list appearance="auto" flex=1
        section
          row
            column
              text "Transport" testId="title"
            text "$12.50" testId="value"
"#;

#[test]
fn a_short_value_goes_under_its_title_at_the_accessibility_sizes() {
    set_column(Column::Ios);
    let plan = contract::bake(contract::compile(SOURCE).unwrap(), NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let at = |r: &mut Runner<NoData>, body: f64| {
        r.set_root_font_size(body).unwrap();
        let k = r.kernel_mut();
        let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
        k.compute_layout(root, Offer::definite(402.0, 874.0))
            .unwrap();
        let frame = |t: &str| {
            k.node(k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id)
                .unwrap()
                .frame
        };
        (frame("title"), frame("value"))
    };
    // The largest standard size (xxxLarge, a 23-point body): side by side.
    let (title, value) = at(&mut r, 23.0);
    assert!(
        value.y < title.y + title.height,
        "beside it: {title:?} {value:?}"
    );
    assert!(
        value.x > title.x + title.width,
        "at the far edge: {title:?} {value:?}"
    );
    // Each accessibility size (AX1 to AX5): under it, at its leading edge.
    for body in [28.0, 33.0, 40.0, 47.0, 53.0] {
        let (title, value) = at(&mut r, body);
        assert!(
            value.y >= title.y + title.height,
            "{body}: under it: {title:?} {value:?}"
        );
        assert_eq!(value.x, title.x, "{body}: at its leading edge");
    }
    set_column(Column::Web);
}
