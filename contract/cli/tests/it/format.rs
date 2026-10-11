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
    // An app's own `fn` shadows a roster entry of its name, so a roster
    // that gains a name never breaks an app that had it first (x2apps
    // files' `fn indexOf`, batch 6): every call in the program is the
    // app's, the built-in's arity and types no longer apply, and
    // `link-by-use` sees no roster call.
    for (src, shown) in [
        (
            "fn formatDate(t: number): string = `day ${t}`\ncomponent A\n  view\n    text formatDate(3) testId=\"x\"\n",
            "day 3",
        ),
        (
            "fn indexOf(xs: list<string>, p: string): number = 7\ncomponent A\n  state s = [\"a\", \"b\"]\n  view\n    text `${indexOf(s, \"b\")}` testId=\"x\"\n",
            "7",
        ),
        (
            "fn t(key: string): string = `[${key}]`\ncomponent A\n  view\n    text t(\"hi\") testId=\"x\"\n",
            "[hi]",
        ),
    ] {
        let plan = contract::compile(src).unwrap();
        assert_eq!(exact_runner::uses(&plan).to_string(), "", "{src}");
        let r = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        assert_eq!(text(&r, "x"), shown, "{src}");
    }
    let e = contract::compile("component A\n  view\n    text formatClockTime(0)\n").unwrap_err();
    assert_eq!(e.id, "type-unknown-function", "{e}");
    let e = contract::compile("component A\n  view\n    text formatDate(0, 0)\n").unwrap_err();
    assert_eq!(e.id, "type-arity", "{e}");
    assert!(
        e.message
            .contains(r#"formatDate(number, number, "medium" | "month-year" | "iso")"#),
        "{e}"
    );
}

/// LLP 1102 §3.1–§3.4: a birthday field's text read as a number, a total
/// rounded as JavaScript rounds it, a day as `YYYY-MM-DD`, and an age in
/// whole years, through the compiler and the runner. Only `iso` links the
/// `format` capability; the reads are the core's.
#[test]
fn a_field_s_number_an_age_and_an_iso_day() {
    let src = r#"component App
  state field = " 12.5 "
  state born = "2024-02-29"
  derive amount = match parseNumber(field) { case some(n) => n, case none => 0 }
  derive age = match calendarDiff(born, "2025-02-28", "years") { case some(n) => toString(n), case none => "?" }
  action type(s: string)
    field = s
  view
    column
      text `${round(amount)} ${ceil(amount)} ${round(-amount)}` testId="amount"
      text age testId="age"
"#;
    let plan = contract::compile(src).unwrap();
    assert_eq!(exact_runner::uses(&plan).to_string(), "");
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "amount"), "13 13 -12");
    assert_eq!(text(&r, "age"), "0", "Feb 29 completes a year on Mar 1");
    r.act("type", vec![Value::str("12px")]).unwrap();
    assert_eq!(text(&r, "amount"), "0 0 0");
    let iso =
        "component App\n  view\n    text formatDate(1790000000000, 0, \"iso\") testId=\"x\"\n";
    let plan = contract::compile(iso).unwrap();
    assert_eq!(exact_runner::uses(&plan).to_string(), "format");
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "x"), "2026-09-21");
    // A unit is a listed literal, as a style is.
    let e = contract::compile(
        "component A\n  view\n    text toString(calendarDiff(\"2024-01-01\", \"2025-01-01\", \"days\") == none)\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-format-style", "{e}");
    assert!(e.message.contains(r#"one of `"years"`, `"months"`"#), "{e}");
    let e =
        contract::compile("component A\n  view\n    text toString(round(\"2\"))\n").unwrap_err();
    assert_eq!(e.id, "type-argument", "{e}");
}

/// LLP 1102 §3.2 (decided (c)): `toFixed` and `formatDecimal` are linked
/// `format` entries; `digits` is a whole-number literal in each one's range,
/// written at the call, as a style is.
#[test]
fn money_is_format_decimal_and_a_measure_is_to_fixed() {
    let src = r#"component App
  state price = 19.995
  state km = 1.005
  view
    column
      text `$${formatDecimal(round(price * 100), 2)}` testId="price"
      text `${toFixed(km, 2)} km` testId="km"
"#;
    let plan = contract::compile(src).unwrap();
    assert_eq!(exact_runner::uses(&plan).to_string(), "format");
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "price"), "$20.00");
    assert_eq!(
        text(&r, "km"),
        "1.00 km",
        "the binary value of 1.005 is below it"
    );
    for (call, given) in [
        ("toFixed(1.5, 101)", "given `101`"),
        ("toFixed(1.5, 2.5)", "given `2.5`"),
        ("toFixed(1.5, 1 + 1)", "given an expression"),
        ("formatDecimal(5, 21)", "given `21`"),
    ] {
        let e = contract::compile(&format!("component A\n  view\n    text {call}\n")).unwrap_err();
        assert_eq!(e.id, "type-literal-digits", "{call}: {e}");
        assert!(e.message.contains(given), "{call}: {e}");
    }
    let e = contract::compile("component A\n  view\n    text toFixed(1.5, -1)\n").unwrap_err();
    assert_eq!(e.id, "type-literal-digits", "{e}");
    // A wrapper can't forward the digits: its parameter is an expression.
    let e = contract::compile(
        "fn money(v: number, d: number): string = toFixed(v, d)\ncomponent A\n  view\n    text money(1, 2)\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-literal-digits", "{e}");
    let e = contract::compile("component A\n  view\n    text toFixed(\"1.5\", 2)\n").unwrap_err();
    assert_eq!(e.id, "type-argument", "{e}");
}

/// LLP 1116 D8: a bill's grouped total, its currency and a share as a
/// percent, through the compiler and the runner: Intl's `en-US` forms, the
/// code written with `"currency"` only, and listed.
#[test]
fn a_total_its_currency_and_a_share() {
    let src = r#"component App
  state total = 1481.4666
  state share = 0.256
  action refund
    total = -5
  view
    column
      text formatNumber(total, "decimal") testId="grouped"
      text formatNumber(total, "currency", "USD") testId="usd"
      text formatNumber(total, "currency", "JPY") testId="yen"
      text formatNumber(share, "percent") testId="share"
"#;
    let plan = contract::compile(src).unwrap();
    assert_eq!(exact_runner::uses(&plan).to_string(), "format");
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "grouped"), "1,481.467");
    assert_eq!(text(&r, "usd"), "$1,481.47");
    assert_eq!(text(&r, "yen"), "¥1,481");
    assert_eq!(text(&r, "share"), "26%");
    r.act("refund", vec![]).unwrap();
    assert_eq!(text(&r, "usd"), "-$5.00");
    for (call, says) in [
        (
            r#"formatNumber(total, "currency")"#,
            "names its currency: an ISO 4217 code",
        ),
        (
            r#"formatNumber(total, "decimal", "USD")"#,
            r#"written only with `"currency"`"#,
        ),
        (
            r#"formatNumber(total, "currency", "XYZ")"#,
            r#"given `"XYZ"`"#,
        ),
        (r#"formatNumber(total, "money")"#, r#"given `"money"`"#),
    ] {
        let e =
            contract::compile(&src.replace(r#"formatNumber(total, "decimal")"#, call)).unwrap_err();
        assert_eq!(e.id, "type-format-style", "{call}: {e}");
        assert!(e.message.contains(says), "{call}: {e}");
    }
    let e = contract::compile("component A\n  view\n    text formatNumber(1)\n").unwrap_err();
    assert_eq!(e.id, "type-arity", "{e}");
    assert!(
        e.message.contains(
            r#"formatNumber(number, "compact" | "decimal" | "currency" | "percent", "USD" | "EUR" | …?)"#
        ),
        "{e}"
    );
}
