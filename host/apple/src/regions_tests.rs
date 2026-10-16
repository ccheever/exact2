//! The layout regions as the Apple host sets them (LLP 1123 D10): a node
//! that reads one is re-sent and laid out again when the host reports them.

use super::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource};

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

/// The x of the child inside `test_id`: its parent's left padding.
fn padding_left(h: &Host<NoData>, test_id: &str) -> f32 {
    let kernel = h.runner().kernel();
    let child = kernel
        .node_by_key(kernel.find_by_test_id(&format!("{test_id}-child"))[0])
        .unwrap();
    child.frame.x
}

#[test]
fn the_host_sets_the_regions_and_the_readers_lay_out_again() {
    let source = r#"component App
  view
    column width="100%"
      column testId="margins" width="100%" padding-left="env(-exact-layout-margins-inset-left)"
        column testId="margins-child" height=10
      column testId="plain" width="100%" padding-left=4
        column testId="plain-child" height=10
"#;
    let plan = contract::compile(source).unwrap().encode();
    let (mut h, _) = Host::boot(
        &plan,
        NoData,
        Box::new(MonospaceMeasurer::default()),
        400.,
        800.,
    )
    .unwrap();
    // No regions: the safe area's
    assert!(!h.set_insets(0., 0., 34., 6.).is_empty());
    assert_eq!(padding_left(&h, "margins"), 6.);
    // The host's: margins (region 1) left is 20
    let mut flat = [0f32; 20];
    flat[4 + 3] = 20.;
    let regions = Host::<NoData>::regions_from(&flat);
    assert_eq!(regions.map(|r| r[1][3]), Some(20.));
    assert!(
        !h.set_regions(regions).is_empty(),
        "a batch moves the reader"
    );
    assert_eq!(padding_left(&h, "margins"), 20.);
    assert_eq!(padding_left(&h, "plain"), 4.);
    assert_eq!(h.runner().kernel().env().regions, regions);
    // None again: the safe area's
    assert!(!h.set_regions(None).is_empty());
    assert_eq!(padding_left(&h, "margins"), 6.);
    assert_eq!(
        Host::<NoData>::regions_from(&flat[..19]),
        None,
        "only twenty is a report"
    );
}
