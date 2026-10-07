//! `includes`, `startsWith` and `endsWith` on strings: the web's searches
//! under the web's names, proven on the runner and refused as the web's
//! method spellings are.

use exact_kernel::{Color, Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};
use std::path::Path;

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Field order is `shape Session` in `search.contract`.
struct Sessions;

impl DataSource for Sessions {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "sessions" => Ok(Value::list(
                [
                    ("10", "notes", "todo.md"),
                    ("12", "build log", "out.txt"),
                    ("21", "plan", "README.md"),
                ]
                .into_iter()
                .map(|(code, name, file)| {
                    Value::record(vec![Value::str(code), Value::str(name), Value::str(file)])
                })
                .collect(),
            )),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn text(r: &Runner<Sessions>, id: &str) -> String {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap_or_default()
        .to_string()
}

fn background(r: &Runner<Sessions>, id: &str) -> Option<exact_kernel::ColorValue> {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(id)[0])
        .unwrap()
        .style
        .background_color
}

#[test]
fn a_launcher_highlights_the_rows_whose_code_starts_with_the_digit_typed() {
    let plan = contract::compile(&corpus("search.contract")).unwrap();
    let plan = contract::bake(plan, Sessions).unwrap();
    let mut r = Runner::boot(
        plan,
        Sessions,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let white = Some(Color::parse_hex("#ffffff").unwrap().into());
    let amber = Some(Color::parse_hex("#ffe08a").unwrap().into());
    // Nothing typed: every name includes "", every code starts with "",
    // and no row is highlighted.
    assert_eq!(text(&r, "named"), "3 named");
    for code in ["10", "12", "21"] {
        assert_eq!(text(&r, &format!("hit-{code}")), "*");
        assert_eq!(background(&r, &format!("row-{code}")), white);
    }
    assert_eq!(text(&r, "kind-10"), "markdown");
    assert_eq!(text(&r, "kind-12"), "plain");
    assert_eq!(text(&r, "kind-21"), "markdown");
    // The digit `1`: codes 10 and 12 start with it, 21 only includes it.
    r.act("edit", vec![Value::str("1")]).unwrap();
    assert_eq!(text(&r, "hit-10"), "*");
    assert_eq!(text(&r, "hit-12"), "*");
    assert_eq!(text(&r, "hit-21"), "");
    assert_eq!(background(&r, "row-10"), amber);
    assert_eq!(background(&r, "row-12"), amber);
    assert_eq!(background(&r, "row-21"), white);
    assert_eq!(text(&r, "named"), "0 named");
    // `includes` is a substring search, case-sensitive, as on the web.
    r.act("edit", vec![Value::str("o")]).unwrap();
    assert_eq!(text(&r, "named"), "2 named", "notes, build log");
    r.act("edit", vec![Value::str("O")]).unwrap();
    assert_eq!(text(&r, "named"), "0 named");
    for code in ["10", "12", "21"] {
        assert_eq!(background(&r, &format!("row-{code}")), white);
    }
}

#[test]
fn the_searches_take_two_strings() {
    let src = corpus("search.contract");
    for (call, replacement) in [
        ("includes(s.name, typed)", "includes(sessions, typed)"),
        ("startsWith(s.code, typed)", "startsWith(s.code, 1)"),
        ("endsWith(s.file, \".md\")", "endsWith(true, \".md\")"),
    ] {
        assert!(src.contains(call), "{call}");
        let e = contract::compile(&src.replace(call, replacement)).unwrap_err();
        assert_eq!(e.id, "type-argument", "{replacement}: {e}");
    }
    let e = contract::compile(&src.replace("endsWith(s.file, \".md\")", "endsWith(s.file)"))
        .unwrap_err();
    assert_eq!(e.id, "type-arity", "{e}");
}

#[test]
fn the_webs_method_spellings_name_the_function_form() {
    let src = corpus("search.contract");
    for (call, method, fix) in [
        (
            "includes(s.name, typed)",
            "s.name.includes(typed)",
            "write `includes(s, t)`",
        ),
        (
            "startsWith(s.code, typed)",
            "s.code.startsWith(typed)",
            "write `startsWith(s, t)`",
        ),
        (
            "endsWith(s.file, \".md\")",
            "s.file.endsWith(\".md\")",
            "write `endsWith(s, t)`",
        ),
    ] {
        let e = contract::compile(&src.replace(call, method)).unwrap_err();
        assert_eq!(e.id, "syntax-method-call", "{method}: {e}");
        assert!(e.message.contains(fix), "{method}: {e}");
    }
    // The predecessor's name is gone: the words are the web's. The refusal
    // names the spelling that works, so an app outside this repo that still
    // writes `contains` is told the fix, not sent to a `resource`.
    for spelling in ["contains(s.name, typed)", "s.name.contains(typed)"] {
        let e = contract::compile(&src.replace("includes(s.name, typed)", spelling)).unwrap_err();
        assert!(
            matches!(e.id.as_str(), "type-refused-idiom" | "syntax-method-call"),
            "{spelling}: {e}"
        );
        assert!(
            e.message.contains("write `includes(s, t)`"),
            "{spelling}: {e}"
        );
    }
}

/// LLP 1088 D1, D2: calendar's end-after-start and calc's Backspace in the
/// view — two strings compare in code-unit order, and `slice`,
/// `replaceAll` and `toLowerCase` are the web's (`end` may be left out).
#[test]
fn strings_compare_and_cut_as_the_web_does() {
    let plan = contract::compile(&corpus("strings.contract")).unwrap();
    let mut r = Runner::boot(
        plan,
        Sessions,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "valid"), "ok");
    assert_eq!(
        text(&r, "tail"),
        "2+3",
        "an omitted end is the string's end"
    );
    assert_eq!(text(&r, "lower"), "οδος paris", "a final sigma");
    r.act("setEnd", vec![Value::str("09:00")]).unwrap();
    assert_eq!(text(&r, "valid"), "end before start");
    r.act("backspace", vec![]).unwrap();
    assert_eq!(text(&r, "expr"), "12+");
    r.act("flip", vec![]).unwrap();
    assert_eq!(text(&r, "expr"), "12-");
}

/// Two strings or two numbers: mixed operands and bools stay refused, and
/// `slice` takes two or three arguments; an action named `slice` keeps its
/// own arity (a scoped action is found before the roster), and `slice` of a
/// list is a list (LLP 1088 §9.1).
#[test]
fn string_order_and_slice_are_typed() {
    let src = corpus("strings.contract");
    for (from, to, id, says) in [
        (
            "end > start",
            "end > 1",
            "type-operand",
            "comparison needs two numbers or two strings, given `string` and `number`",
        ),
        (
            "end > start",
            "true < false",
            "type-operand",
            "given `bool` and `bool`",
        ),
        (
            "slice(expr, 1)",
            "slice(expr)",
            "type-arity",
            "`slice` takes 2 to 3 argument(s), given 1; expected `slice(string | list, number, number?)`",
        ),
        (
            "slice(expr, 1)",
            "slice(expr, 1, 2, 3)",
            "type-arity",
            "given 4",
        ),
        (
            "toLowerCase(query)",
            "toLowerCase(1)",
            "type-argument",
            "expects `string`",
        ),
    ] {
        assert!(src.contains(from), "{from}");
        let e = contract::compile(&src.replace(from, to)).unwrap_err();
        assert_eq!(e.id, id, "{to}: {e}");
        assert!(e.message.contains(says), "{to}: {e}");
    }
    contract::compile("component App\n  state n = 0\n  action slice(by: number)\n    n = n + by\n  view\n    button \"s\" press=slice(1)\n")
        .unwrap();
    contract::compile(
        "component App\n  state xs = [\"a\", \"b\"]\n  derive ys = slice(xs, 1)\n  view\n    text join(ys, \",\")\n",
    )
    .unwrap();
}
