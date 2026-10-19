//! The base scale on the wasm web (LLP 1121 stage 11 B2): a boot says the
//! neutral look (the role table's switch, which is the metrics' column too),
//! and a base name reads the kernel's neutral column, as every other row of
//! the look does.
use exact_kernel::style::{metrics, scale};
use exact_runner::{DataError, DataSource, Value};
use exact_web::abi::Bridge;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn a_web_boot_reads_the_base_scale_in_the_neutral_column() {
    let plan = contract::compile("component App\n  view\n    column testId=\"box\" padding=\"space.xs\" border-radius=\"radius.sm\"\n      text \"a\"\n")
        .unwrap()
        .encode();
    let mut bridge: Bridge<NoData> = Bridge::new();
    let len = bridge.boot(&plan, NoData, 390.0, 844.0, "/");
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned();
    assert_eq!(metrics::column(), metrics::Column::Neutral);
    let neutral = |n: &str| scale::css(scale::of(n, metrics::Column::Neutral).unwrap());
    // The neutral column's, not the web's first look's (iOS's, `@ios`): they differ here.
    assert_ne!(
        neutral("space.xs"),
        scale::css(scale::of("space.xs", metrics::Column::Web).unwrap())
    );
    let at = batch
        .find("\"data-testid\":\"box\"")
        .unwrap_or_else(|| panic!("{batch}"));
    let start = batch[..at].rfind("{\"op\":").unwrap();
    let end = batch[at..].find("{\"op\":").map_or(batch.len(), |n| at + n);
    let op = &batch[start..end];
    assert!(
        op.contains(&format!("padding-top:{}", neutral("space.xs"))),
        "{op}"
    );
    assert!(
        op.contains(&format!("border-top-left-radius:{}", neutral("radius.sm"))),
        "{op}"
    );
}
