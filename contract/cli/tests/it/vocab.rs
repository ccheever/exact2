//! LLP 1086 D3: `contract vocab` lists what the compiler admits, from its
//! own lookups, and refuses a name as the compiler would.

use serde_json::Value;
use std::process::{Command, Output};

fn vocab(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("vocab")
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn a_style_shows_its_rows_codec_and_default() {
    let out = stdout(&vocab(&["padding"]));
    assert!(out.contains("padding: style attribute"), "{out}");
    assert!(out.contains("padding_top"), "{out}");
    assert!(out.contains("dimension, default 0"), "{out}");
}

/// LLP 1116.001 D1.4: a name Exact invents starts with a dash, and is still
/// a name (`vocab -exact-fit`), not a flag; the platforms' spellings point at it.
#[test]
fn an_exact_name_is_looked_up_and_the_platforms_spellings_name_it() {
    let out = stdout(&vocab(&["-exact-fit"]));
    assert!(out.contains("-exact-fit: style attribute"), "{out}");
    assert!(out.contains("enum none|shrink, default none"), "{out}");
    assert!(out.contains("adjustsFontSizeToFitWidth"), "{out}");
    for old in ["adjustsFontSizeToFit", "minimumScaleFactor"] {
        let output = vocab(&[old]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let err = String::from_utf8(output.stderr).unwrap();
        assert!(err.contains("`-exact-fit`"), "{err}");
    }
    assert_eq!(
        vocab(&["-x"]).status.code(),
        Some(2),
        "a short dash is a flag"
    );
}

#[test]
fn a_handler_and_a_tag_say_what_they_are() {
    assert!(stdout(&vocab(&["press"])).contains("press: handler attribute"));
    let column = stdout(&vocab(&["column"]));
    assert!(column.contains("column: tag, View"), "{column}");
    assert!(column.contains("flex_direction: column"), "{column}");
}

#[test]
fn an_unknown_name_gets_the_compilers_suggestion_and_exit_1() {
    let output = vocab(&["paddin"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("did you mean `padding`?"), "{err}");
    let output = vocab(&["fontSize"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("`font-size`"));
}

#[test]
fn json_lists_everything_and_one_entry() {
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    let names = |key: &str| -> Vec<String> {
        doc[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(names("tags").contains(&"column".to_owned()));
    assert!(!names("tags").contains(&"flex".to_owned()));
    assert!(names("attributes").contains(&"padding".to_owned()));
    assert!(names("renamed").contains(&"fontSize".to_owned()));
    let one: Value = serde_json::from_str(&stdout(&vocab(&["--json", "align-items"]))).unwrap();
    assert_eq!(one["attribute"]["kind"], "style");
    assert_eq!(one["attribute"]["rows"][0]["default"], "normal");
    assert!(
        one["attribute"]["rows"][0]["values"]
            .as_array()
            .unwrap()
            .len()
            > 3
    );
}

/// The listed contextual restrictions are the compiler's: every entry of
/// `CONTEXTUAL` is refused on a tag outside its list, so a new entry needs a
/// case here, and a restriction the compiler drops fails this test.
#[test]
fn contextual_attributes_are_refused_elsewhere() {
    let cases = [
        ("sandbox", "view", "sandbox=\"allow-scripts\""),
        ("src", "view", "src=\"https://example.com\""),
        ("load", "view", "load=done"),
        ("message", "view", "message=got"),
        ("document", "view", "document=true"),
        ("reachstart", "view", "reachstart=done"),
        ("reachend", "view", "reachend=done"),
        ("text-transform", "input", "text-transform=\"uppercase\""),
        ("selectionchange", "view", "selectionchange=done"),
        ("name", "view", "name=\"station\""),
        ("markup", "view", "markup=\"markdown\""),
        (
            "metadata",
            "view",
            "metadata=MediaMetadata(title=\"a\", artist=\"\", album=\"\", artwork=\"\")",
        ),
        ("seekbackward", "view", "seekbackward=done"),
        ("seekforward", "view", "seekforward=done"),
        ("seekto", "view", "seekto=done"),
        ("previoustrack", "view", "previoustrack=done"),
        ("nexttrack", "view", "nexttrack=done"),
        ("stop", "view", "stop=done"),
        ("seekbackwardOffset", "view", "seekbackwardOffset=15"),
        ("seekforwardOffset", "view", "seekforwardOffset=15"),
    ];
    let listed: Vec<&str> = contract_lower::vocab::CONTEXTUAL
        .iter()
        .map(|(n, _)| *n)
        .collect();
    let tested: Vec<&str> = cases.iter().map(|(n, _, _)| *n).collect();
    assert_eq!(listed, tested, "every CONTEXTUAL entry needs a case");
    for (_, tag, attr) in cases.iter().chain([&("robots", "view", "robots=\"x\"")]) {
        let source =
            format!("component App\n  action done\n    let x = 1\n  action got(data: string)\n    let y = data\n  view\n    {tag} {attr}\n");
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "lower-attr-tag", "{tag} {attr}: {error}");
    }
}

#[test]
fn a_hyphenated_name_no_built_in_has_says_so_before_the_module_note() {
    let out = stdout(&vocab(&["outline-width"]));
    assert!(
        out.starts_with("outline-width: not a built-in tag or attribute, so a built-in tag refuses an attribute of that name; a hyphenated tag"),
        "{out}"
    );
    let data = stdout(&vocab(&["data-row"]));
    assert!(data.starts_with("data-row: `data-<word>`"), "{data}");
    assert!(!data.contains("built-in"), "{data}");
}

#[test]
fn the_host_commands_are_listed_and_answer_by_name() {
    let all = stdout(&vocab(&[]));
    assert!(all.contains("host commands ("), "{all}");
    assert!(all.contains("setRootFontSize"), "{all}");
    let one = stdout(&vocab(&["setRootFontSize"]));
    assert!(one.contains("setRootFontSize: host command"), "{one}");
    assert!(one.contains(":root { font-size }"), "{one}");
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    assert!(doc["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == "setScheme"));
}

/// LLP 1116.000 D5: one function's entry is its roster signature, the
/// grammar's rows for it and their examples.
#[test]
fn a_function_shows_its_signature_rules_and_example() {
    let out = stdout(&vocab(&["formatNumber"]));
    assert!(out.starts_with("formatNumber: standard function\n  signature: formatNumber(number, \"compact\" | \"decimal\" | \"currency\" | \"percent\"[, \"USD\" | "), "{out}");
    assert!(out.contains("Intl.NumberFormat(\"en-US\")"), "{out}");
    assert!(
        out.contains("example: `formatNumber(1481.47, \"currency\", \"USD\")` is `\"$1,481.47\"`"),
        "{out}"
    );
    let slice = stdout(&vocab(&["slice"]));
    assert!(
        slice.contains("  signature: slice(any, number[, number]) -> any\n"),
        "{slice}"
    );
    assert_eq!(
        slice.matches("example:").count(),
        2,
        "text and list: {slice}"
    );
    let now = stdout(&vocab(&["performanceNow"]));
    assert!(
        now.contains("performanceNow() -> number") && now.contains("epochAtZero"),
        "{now}"
    );
    let push = stdout(&vocab(&["push"]));
    assert!(
        push.contains("only in an app with a `routes` declaration"),
        "{push}"
    );
    assert!(
        push.contains("the web's `push`: a list is a value"),
        "{push}"
    );
    let path = stdout(&vocab(&["path"]));
    assert!(
        path.contains("path: compiler intrinsic\n"),
        "the tag, then the intrinsic: {path}"
    );
}

#[test]
fn a_name_that_is_an_attribute_and_a_function_shows_both() {
    let out = stdout(&vocab(&["filter"]));
    assert!(out.contains("filter: style attribute"), "{out}");
    assert!(
        out.contains("filter: standard function\n  signature: filter(any, any) -> any"),
        "{out}"
    );
}

#[test]
fn a_function_the_web_has_and_contract_refuses_says_what_to_write() {
    for (name, says) in [
        ("sort", "compute it in the data source"),
        ("sortBy", "compute it in the data source"),
        ("pow", "`x * x` is a square"),
        ("now", "performanceNow()"),
        ("padStart", "pad in the data module"),
    ] {
        let output = vocab(&[name]);
        assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
        let err = String::from_utf8(output.stderr).unwrap();
        assert!(err.contains(says), "{name}: {err}");
    }
    let output = vocab(&["formatNumbr"]);
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(
        err.contains("did you mean the function `formatNumber`?"),
        "{err}"
    );
}

#[test]
fn the_functions_are_listed_and_in_the_json() {
    let all = stdout(&vocab(&[]));
    let count = exact_plan::Stdlib::ALL.len();
    assert!(
        all.contains(&format!("standard functions ({count})")),
        "{all}"
    );
    assert!(
        all.contains("  split(string, string) -> list<string>\n"),
        "{all}"
    );
    assert!(
        all.contains("compiler intrinsics:\n  pending, failed, failure, path"),
        "{all}"
    );
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    assert_eq!(
        doc["functions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["kind"] == "standard function")
            .count(),
        count
    );
    let one: Value = serde_json::from_str(&stdout(&vocab(&["--json", "formatDate"]))).unwrap();
    assert_eq!(one["function"]["kind"], "standard function");
    assert!(one["function"]["rules"][0]["example"]
        .as_str()
        .unwrap()
        .contains("Sep 21, 2026"));
}

/// The grammar's table is the roster's: every standard function has a row
/// and every row an example, and every example written `` `call` is
/// `value` `` is true when a view evaluates it.
#[test]
fn every_function_has_rows_and_every_written_example_holds() {
    let functions = contract::functions::functions();
    let mut missing = Vec::new();
    let mut examples = Vec::new();
    for f in &functions {
        if f.rows.is_empty() {
            missing.push(format!("{}: no row", f.name));
        }
        for row in &f.rows {
            if row.example.is_empty() {
                missing.push(format!("{}: no example", row.call));
            }
            let parts: Vec<&str> = row.example.split('`').collect();
            if let [_, call, " is ", value, ""] = parts.as_slice() {
                if !examples.iter().any(|(c, _)| c == call) {
                    examples.push((call.to_string(), value.to_string()));
                }
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
    assert!(examples.len() > 40, "{examples:?}");
    let views: String = examples
        .iter()
        .enumerate()
        .map(|(i, (call, value))| {
            format!(
                "      text (({call}) == ({value}) ? \"holds\" : \"differs\") testId=\"e{i}\"\n"
            )
        })
        .collect();
    let src = format!(
        "routes nav\n  home \"/\"\n  item \"/item/:id\"\n\ncomponent App\n  view\n    main\n{views}"
    );
    let plan = contract::compile(&src).unwrap_or_else(|e| panic!("{e}\n{src}"));
    struct NoData;
    impl exact_runner::DataSource for NoData {
        fn query(
            &mut self,
            source: &str,
            _: &[exact_runner::Value],
        ) -> Result<exact_runner::Value, exact_runner::DataError> {
            Err(exact_runner::DataError::UnknownSource(source.into()))
        }
    }
    let r = exact_runner::Runner::boot(
        plan,
        NoData,
        exact_kernel::Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let wrong: Vec<String> = examples
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let key = k.find_by_test_id(&format!("e{i}"))[0];
            k.node_by_key(key)
                .unwrap()
                .props
                .str(exact_kernel::PropId::Text)
                != Some("holds")
        })
        .map(|(_, (call, value))| format!("{call} is not {value}"))
        .collect();
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// LLP 1116.001 D4.3: several names are one call, each entry in order, and a
/// value names the attributes that take it; an unknown name is refused in its
/// place, the others still printed, and the call exits 1.
#[test]
fn several_names_print_in_order_and_one_unknown_exits_1() {
    let out = stdout(&vocab(&["formatDate", "slice", "tabular-nums"]));
    let at = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{out}"))
    };
    assert!(at("formatDate: standard function") < at("slice: standard function"));
    assert!(at("slice: standard function") < at("tabular-nums: a value of `font-variant-numeric`"));
    let output = vocab(&["padding", "paddin", "press"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let (out, err) = (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    );
    assert!(
        out.contains("padding: style attribute") && out.contains("press: handler attribute"),
        "{out}"
    );
    assert!(err.contains("did you mean `padding`?"), "{err}");
    let output = vocab(&["--json", "slice", "paddin"]);
    assert_eq!(output.status.code(), Some(1));
    let docs: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(docs[0]["function"]["name"], "slice");
    assert_eq!(docs[1]["name"], "paddin");
    assert!(docs[1]["refused"]
        .as_str()
        .unwrap()
        .contains("did you mean"));
    let one: Value = serde_json::from_str(&stdout(&vocab(&["--json", "ellipsis"]))).unwrap();
    assert_eq!(one["valueOf"][0], "text-overflow");
}
