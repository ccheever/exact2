//! A popover's default surface (LLP 1021; theme2 2026-10-21 19:55): the
//! compiler marks a popover whose author wrote no surface, and the host
//! sends its corner from this platform's metrics, iOS's 30, the Mac none.
use super::*;
use exact_kernel::style::metrics::Column;
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
  view
    column
      column id="bare" popover="auto" padding=12 testId="bare"
        text "Popover text"
      column id="filled" popover="auto" background-color="white" testId="filled"
        text "Filled"
"#;

#[test]
fn a_popover_with_no_surface_of_its_authors_is_sent_the_platforms_corner() {
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
    let bare = props("bare");
    assert_eq!(bare.get("defaultSurface").map(String::as_str), Some("true"));
    // On the Mac, whose column has no corner, none is sent.
    #[cfg(target_os = "macos")]
    assert_eq!(
        bare.get("defaultSurfaceRadius"),
        None,
        "the Mac's system popover draws its own"
    );
    #[cfg(target_os = "macos")]
    assert_eq!(bare.get("defaultSurfaceShadow"), None);
    let filled = props("filled");
    assert_eq!(filled.get("defaultSurface"), None, "its author's surface");
    assert_eq!(
        default_surface(Column::Ios),
        Some(("30".into(), "8 24 0.015".into())),
        "iOS's corner and shadow"
    );
    assert_eq!(default_surface(Column::Mac), None);
}
