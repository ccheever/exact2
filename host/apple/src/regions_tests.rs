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

/// The window, the insets, the regions and the viewport told at once leave
/// the page as the four calls one by one do, in one batch.
#[test]
fn the_view_told_at_once_is_laid_out_as_told_one_by_one() {
    let source = r#"component App
  view
    column width="100%" height="100%"
      column testId="margins" width="50vw" padding-left="env(-exact-layout-margins-inset-left)" padding-top="env(safe-area-inset-top)"
        column testId="margins-child" height=10
      column testId="rest" flex=1
"#;
    let plan = contract::compile(source).unwrap().encode();
    let boot = || {
        let measurer = Box::new(MonospaceMeasurer::default());
        Host::boot(&plan, NoData, measurer, 400., 700.).unwrap().0
    };
    let mut flat = [0f32; 20];
    flat[4 + 3] = 20.;
    let regions = Host::<NoData>::regions_from(&flat);
    let frames = |h: &Host<NoData>| {
        let kernel = h.runner().kernel();
        ["margins", "margins-child", "rest"].map(|t| {
            let f = kernel
                .node_by_key(kernel.find_by_test_id(t)[0])
                .unwrap()
                .frame;
            (f.x, f.y, f.width, f.height)
        })
    };
    let mut apart = boot();
    apart.set_screen(Some((402., 874.)));
    apart.set_insets(62., 0., 34., 0.);
    apart.set_regions(regions);
    apart.resize(402., 874.);
    let mut together = boot();
    let batch = together.set_view(
        Some(Some((402., 874.))),
        Some([62., 0., 34., 0.]),
        Some(regions),
        Some((402., 874.)),
    );
    assert!(!batch.contains("\"error\":\""), "{batch}");
    assert_eq!(frames(&together), frames(&apart));
    assert_eq!(
        together.runner().kernel().env(),
        apart.runner().kernel().env()
    );
    assert_eq!(padding_left(&together, "margins"), 20.);
    // Only what is given changes: the insets alone.
    together.set_view(None, Some([0., 0., 0., 0.]), None, None);
    apart.set_insets(0., 0., 0., 0.);
    assert_eq!(frames(&together), frames(&apart));
}
