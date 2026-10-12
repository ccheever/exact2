//! The web host under the neutral look (LLP 1115.000 D4 part B). A binary of
//! its own: the look is the process's (the role table's column,
//! `roles::set_column`), and every other test of this host reads Apple's
//! pairs beside it. One test, so nothing here boots while the switch moves.
use exact_runner::{DataError, DataSource, Value};
use exact_web::abi::Bridge;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A page whose stylesheet is the neutral one says so in the facts word it
/// boots with (bit 25, navigation.js `foldBits`). From then a role falls
/// back to the table's neutral column, and each element is told what the
/// neutral sheet has to know and CSS cannot say: which controls it draws
/// (their appearance is the platform's), and which rows the author wrote
/// where it would draw over an implied or inherited one. Before that, as
/// every build is until the switch is thrown, the page is told none of it.
#[test]
fn a_page_in_the_neutral_look_boots_the_neutral_column_and_is_told_what_its_sheet_draws() {
    // A file control's picker is a capability the app links (LLP 1047).
    exact_web::link(exact_web_capabilities::ALL);
    let plan = contract::compile(
        r##"component App
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
      list appearance="auto" testId="list"
        section
          row
            text "A row"
      list appearance="auto" background-color="-exact-background" testId="ground"
        section
          row
            text "A row"
"##,
    )
    .unwrap()
    .encode();
    let boot = |facts: u32| {
        let mut bridge: Bridge<NoData> = Bridge::new();
        bridge.set_preferences(facts);
        let len = bridge.boot(&plan, NoData, 390.0, 844.0, "/");
        String::from_utf8_lossy(bridge.output_bytes(len as usize)).into_owned()
    };
    // As every page boots today: Apple's pairs, and no mark.
    let apple = boot(0);
    assert!(
        apple.contains("var(--exact-separator, light-dark(#3c3c431f, #54545880))"),
        "{apple}"
    );
    assert!(
        !apple.contains("data-exact-control") && !apple.contains("data-exact-said"),
        "{apple}"
    );

    // The neutral page's facts: the fold's bits and the look's.
    let neutral = boot(1 << 25);
    assert!(
        neutral.contains("var(--exact-separator, light-dark(#0000001f, #ffffff29))"),
        "a role's fallback is the neutral column's: {neutral}"
    );
}
