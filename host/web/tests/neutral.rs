//! Web boots use neutral roles and metrics without a page-supplied selector.
use exact_kernel::style::roles::{self, Column};
use exact_runner::{DataError, DataSource, Value};
use exact_web::abi::Bridge;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// The sheet receives control and authored-row metadata on every web boot,
/// including callers that supply no preferences word.
#[test]
fn a_web_boot_uses_neutral_roles_and_marks_without_a_look_fact() {
    // A file control's picker is a capability the app links (LLP 1047).
    exact_web::link(exact_web_capabilities::ALL);
    let plan = contract::compile(
        r##"component App
  state inset = 0
  view
    column border-color="-exact-separator"
      input type="checkbox" testId="check"
      input type="checkbox" appearance="none" testId="own"
      input type="radio" testId="radio"
      input type="range" testId="range"
      input type="file" testId="file"
      input type="checkbox" switch testId="switch"
      progress testId="spinner"
      progress color="-exact-system-red" testId="red"
      progress value=40 max=100 color="-exact-system-red" testId="bar"
      button testId="unsized"
        text "Go"
      button width=84 height=24 testId="sized"
        text "Go"
      button height=28 testId="short"
        text "Go"
      button padding-top=0 padding-bottom=0 testId="padded"
        text "Go"
      button padding=8 testId="inset"
        text "Go"
      button padding=inset testId="bound"
        text "Go"
      list appearance="auto" testId="list"
        section
          row
            text "A row"
      list appearance="auto" background-color="-exact-background" testId="ground"
        section
          row
            text "A row"
      list testId="rows"
        text "A row" testId="plain-row"
        text "Its own line" border-bottom="1px solid -exact-separator" testId="lined-row"
        text "No line above" border-top="none" testId="bare-row"
      list appearance="none" testId="own-rows"
        text "A row"
      button disabled=true opacity=0.5 testId="own-off"
        text "Off"
      button disabled=true testId="off"
        text "Off"
"##,
    )
    .unwrap()
    .encode();
    let boot = |facts: Option<u32>| {
        let mut bridge: Bridge<NoData> = Bridge::new();
        if let Some(facts) = facts {
            bridge.set_preferences(facts);
        }
        let len = bridge.boot(&plan, NoData, 390.0, 844.0, "/");
        String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned()
    };
    // The element a batch creates for a test id: its op, to the next one.
    let created = |batch: &str, test_id: &str| {
        let at = batch
            .find(&format!("\"data-testid\":\"{test_id}\""))
            .unwrap_or_else(|| panic!("no {test_id} in {batch}"));
        let start = batch[..at].rfind("{\"op\":").unwrap();
        let end = batch[at..].find("{\"op\":").map_or(batch.len(), |n| at + n);
        batch[start..end].to_string()
    };

    roles::set_column(Column::Apple);
    let neutral = boot(None);
    assert_eq!(roles::column(), Column::Neutral);
    roles::set_column(Column::Apple);
    assert_eq!(boot(Some(0)), neutral);
    assert!(
        neutral.contains("var(--exact-separator, light-dark(#0000001f, #ffffff29))"),
        "a role's fallback is the neutral column's: {neutral}"
    );
    for drawn in ["check", "radio", "range", "file"] {
        let op = created(&neutral, drawn);
        assert!(op.contains("\"data-exact-control\":\"\""), "{drawn}: {op}");
    }
    // `appearance="none"` hands the box to the author; the switch has its
    // own mark; a control the sheet draws nothing over has none.
    for (other, why) in [
        ("own", "the author's box"),
        ("switch", "its own mark"),
        ("spinner", "a spinner is no drawn control"),
    ] {
        let op = created(&neutral, other);
        assert!(!op.contains("data-exact-control"), "{other}, {why}: {op}");
    }
    assert!(created(&neutral, "switch").contains("\"data-exact-switch\":\"\""));
    // The rows the author wrote where the sheet would draw over them.
    assert!(!created(&neutral, "spinner").contains("data-exact-said"));
    assert!(
        created(&neutral, "red").contains("\"data-exact-said\":\"color\""),
        "{}",
        created(&neutral, "red")
    );
    assert!(
        !created(&neutral, "bar").contains("data-exact-said"),
        "a determinate bar's colour is no ring's"
    );
    // A platform button's written size or padding, which the look's floor
    // yields to on that axis; an unsized button keeps the floor.
    assert!(!created(&neutral, "unsized").contains("data-exact-said"));
    assert!(
        created(&neutral, "sized").contains("\"data-exact-said\":\"width height\""),
        "{}",
        created(&neutral, "sized")
    );
    assert!(
        created(&neutral, "short").contains("\"data-exact-said\":\"height\""),
        "{}",
        created(&neutral, "short")
    );
    assert!(
        created(&neutral, "padded").contains("\"data-exact-said\":\"padding-block\""),
        "{}",
        created(&neutral, "padded")
    );
    assert!(
        created(&neutral, "inset").contains("\"data-exact-said\":\"padding-inline padding-block\""),
        "{}",
        created(&neutral, "inset")
    );
    assert!(
        created(&neutral, "bound").contains("\"data-exact-said\":\"padding-inline padding-block\"")
    );
    // LLP 1115.000 D16: a list whose rows are cells carries Contract's mark
    // on every boot, a list whose author opted out does not, and a row is
    // told which of its edges its author wrote (`none` is written too), so
    // the sheet draws no separator of its own there.
    // LLP 1115.000 D6: Contract's mark on a button that styles its own disabled state.
    assert!(
        created(&neutral, "own-off").contains("\"data-disabledstyled\":\"true\""),
        "{}",
        created(&neutral, "own-off")
    );
    assert!(!created(&neutral, "off").contains("data-disabledstyled"));
    let rows = created(&neutral, "rows");
    assert!(rows.contains("\"data-listrows\":\"true\""), "{rows}");
    assert!(!created(&neutral, "own-rows").contains("data-listrows"));
    assert!(!created(&neutral, "list").contains("data-listrows"));
    assert!(!created(&neutral, "plain-row").contains("data-exact-said"));
    assert!(
        created(&neutral, "lined-row").contains("\"data-exact-said\":\"border-bottom\""),
        "{}",
        created(&neutral, "lined-row")
    );
    assert!(
        created(&neutral, "bare-row").contains("\"data-exact-said\":\"border-top\""),
        "{}",
        created(&neutral, "bare-row")
    );
    assert!(!created(&neutral, "list").contains("data-exact-said"));
    assert!(
        created(&neutral, "ground").contains("\"data-exact-said\":\"background-color\""),
        "{}",
        created(&neutral, "ground")
    );
}
