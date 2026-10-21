//! The base scale on the wasm web (LLP 1121 stage 11 B2): a boot says the
//! neutral look (the role table's switch, which is the metrics' column too),
//! and a base name reads the kernel's neutral column, as every other row of
//! the look does. (The scale has no web column: `Column::Web` reads the
//! neutral cell, so the test holds the switch itself, and a value iOS's cell
//! does not share.)
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
    // The neutral column's, not iOS's (which the old first look copied): they
    // differ here (4 and 5), so the batch tells them apart. (`Column::Web`
    // reads the neutral cell too, B1; the boot's switch is what this holds.)
    assert_ne!(
        neutral("space.xs"),
        scale::css(scale::of("space.xs", metrics::Column::Ios).unwrap())
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

/// Stage 12 B3: a shadow token whose lengths are over base names reads
/// through the kernel (`theme::measure`) on the wasm web too, each length
/// the neutral column's px, which the batch's `box-shadow` writes.
#[test]
fn a_web_boot_folds_a_shadow_over_base_names_in_the_neutral_column() {
    let dir = std::env::temp_dir().join(format!("exact-web-shadow-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\ntheme\n  card.shadow: box-shadow = 0 space.md calc(space.sm * 2) #000\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column testId=\"box\" box-shadow=\"card.shadow\"\n      text \"a\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract"))
        .unwrap()
        .encode();
    let _ = std::fs::remove_dir_all(&dir);
    let mut bridge: Bridge<NoData> = Bridge::new();
    let len = bridge.boot(&plan, NoData, 390.0, 844.0, "/");
    let batch = String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned();
    let px = |expr: &str| scale::evaluate(expr, metrics::Column::Neutral).unwrap();
    // The neutral column's, not the Mac's: they differ for space.md.
    assert_ne!(
        px("calc(space.md)"),
        scale::evaluate("calc(space.md)", metrics::Column::Mac).unwrap()
    );
    let at = batch
        .find("\"data-testid\":\"box\"")
        .unwrap_or_else(|| panic!("{batch}"));
    let start = batch[..at].rfind("{\"op\":").unwrap();
    let end = batch[at..].find("{\"op\":").map_or(batch.len(), |n| at + n);
    let op = &batch[start..end];
    assert!(
        op.contains(&format!(
            "box-shadow:0px {}px {}px 0px",
            px("calc(space.md)"),
            px("calc(space.sm * 2)")
        )),
        "{op}"
    );
}
