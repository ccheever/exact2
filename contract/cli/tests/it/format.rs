//! `formatTime`, `formatDate` and `formatNumber` (LLP 1054.000.003): a
//! post's date line and its like count, formatted in the view from numbers
//! the view holds, so an optimistic `+1` is a view change.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SRC: &str = r#"component App
  state createdAt = 1790000000000
  state offset = 0
  state likes = 1249
  action like
    likes = likes + 1
  action move(minutes: number)
    offset = minutes
  view
    column
      text `${formatDate(createdAt, offset, "medium")} at ${formatTime(createdAt, offset, "short")}` testId="stamp"
      text `Joined ${formatDate(createdAt, offset, "month-year")}` testId="joined"
      text (likes > 0 ? formatNumber(likes, "compact") : "") testId="likes"
"#;

fn text(r: &Runner<NoData>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    let node = k.node_by_key(key).unwrap();
    node.props.str(PropId::Text).unwrap_or("").to_string()
}

#[test]
fn a_post_s_date_and_count_are_formatted_in_the_view() {
    let plan = contract::compile(SRC).unwrap();
    assert_eq!(exact_runner::uses(&plan).to_string(), "format");
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // 1790000000000 is 2026-09-21T14:13:20Z.
    assert_eq!(text(&r, "stamp"), "Sep 21, 2026 at 2:13 PM");
    assert_eq!(text(&r, "joined"), "Joined September 2026");
    assert_eq!(text(&r, "likes"), "1.2K");
    r.act("like", vec![]).unwrap();
    assert_eq!(text(&r, "likes"), "1.2K", "1,250 truncates");
    // UTC+14 is past midnight: the date moves with the time.
    r.act("move", vec![Value::Number(840.0)]).unwrap();
    assert_eq!(text(&r, "stamp"), "Sep 22, 2026 at 4:13 AM");
    // An offset past ±18 h is invalid, and invalid is blank.
    r.act("move", vec![Value::Number(1200.0)]).unwrap();
    assert_eq!(text(&r, "stamp"), " at ");
}

/// D9: a style is a string literal the roster lists, written at the call.
#[test]
fn a_style_is_a_listed_literal() {
    let refused = |from: &str, to: &str| {
        let src = SRC.replace(from, to);
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, "type-format-style", "{to}: {e}");
        e.message
    };
    let m = refused(
        r#"formatDate(createdAt, offset, "medium")"#,
        r#"formatDate(createdAt, offset, "long")"#,
    );
    assert!(
        m.contains(r#"one of `"medium"`, `"month-year"`"#) && m.ends_with(r#"given `"long"`"#),
        "{m}"
    );
    refused(r#""compact")"#, r#"(likes > 1 ? "compact" : "compact"))"#);
    refused(r#"offset, "short")"#, r#"offset, `short`)"#);
    // A wrapper can't forward a style: its parameter is an expression.
    let e = contract::compile(
        "fn day(t: number, s: string): string = formatDate(t, 0, s)\ncomponent A\n  view\n    text day(0, \"medium\")\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-format-style", "{e}");
    // A wrapper that writes its own literal is fine.
    contract::compile(
        "fn count(n: number): string = formatNumber(n, \"compact\")\ncomponent A\n  view\n    text count(3)\n",
    )
    .unwrap();
}

#[test]
fn the_roster_names_are_the_roster_s() {
    let e = contract::compile(
        "fn formatDate(t: number, o: number, s: string): string = \"x\"\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "contract-fn-shadows-roster", "{e}");
    let e = contract::compile("component A\n  view\n    text formatClockTime(0)\n").unwrap_err();
    assert_eq!(e.id, "type-unknown-function", "{e}");
    let e = contract::compile("component A\n  view\n    text formatDate(0, 0)\n").unwrap_err();
    assert_eq!(e.id, "type-arity", "{e}");
    assert!(
        e.message
            .contains(r#"formatDate(number, number, "medium" | "month-year")"#),
        "{e}"
    );
}
