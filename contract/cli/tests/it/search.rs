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

fn background(r: &Runner<Sessions>, id: &str) -> exact_kernel::ColorValue {
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
    let white = Color::parse_hex("#ffffff").unwrap().into();
    let amber = Color::parse_hex("#ffe08a").unwrap().into();
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
