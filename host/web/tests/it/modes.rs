//! LLP 1121 D7: the wasm host writes a mode boundary's implied rows in its
//! CSS, as any row: inherited text inside a pin is the pin's tuple's, and a
//! Light card on a dark page writes its text again, so the browser resolves
//! the `light-dark()` there.

use exact_runner::{DataError, DataSource, Value};
use exact_web::Host;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

/// The op that creates the element with `test_id`, to the next op.
fn created<'a>(batch: &'a str, test_id: &str) -> &'a str {
    let at = batch
        .find(&format!("\"data-testid\":\"{test_id}\""))
        .unwrap_or_else(|| panic!("no {test_id} in {batch}"));
    let start = batch[..at].rfind("{\"op\":").unwrap();
    let end = batch[at..].find("{\"op\":").map_or(batch.len(), |n| at + n);
    &batch[start..end]
}

#[test]
fn a_boundary_writes_its_inherited_token_text_in_its_own_tuple() {
    let dir = std::env::temp_dir().join(format!("exact-web-modes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("app.tamber"),
        "tamber t\n\naxes\n  dark-style = black | dim  under scheme=dark\n\ntheme\n  text.primary = light-dark(#0f1419, #e7e9ea)\n    when dark-style=dim: light-dark(#0f1419, #f7f9f9)\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "component App\n  view\n    column color=\"text.primary\" color-scheme=\"dark\" testId=\"page\"\n      column -exact-mode=\"dark-style=dim\" testId=\"dim\"\n        column -exact-mode=\"dark-style=black\" testId=\"black\"\n          text \"a\"\n      column color-scheme=\"light\" testId=\"light\"\n        text \"b\"\n      column color=\"#123456\" testId=\"own\"\n        column color-scheme=\"light\" testId=\"plain\"\n          text \"c\"\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let (_, batch) = Host::boot(&plan.encode(), NoData, Default::default(), "/").unwrap();
    let black = "color:light-dark(rgba(15,20,25,1), rgba(231,233,234,1));";
    let dim = "color:light-dark(rgba(15,20,25,1), rgba(247,249,249,1));";
    assert!(created(&batch, "page").contains(black), "{batch}");
    assert!(
        created(&batch, "dim").contains(dim),
        "{}",
        created(&batch, "dim")
    );
    assert!(
        created(&batch, "black").contains(black),
        "{}",
        created(&batch, "black")
    );
    assert!(
        created(&batch, "light").contains(black),
        "{}",
        created(&batch, "light")
    );
    // A colour from no token is inherited as the browser computed it.
    assert!(
        !created(&batch, "plain").contains("color:"),
        "{}",
        created(&batch, "plain")
    );
}
