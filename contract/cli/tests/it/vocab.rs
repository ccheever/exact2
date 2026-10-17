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

/// LLP 1116.009 D6: a builder guessed a colour role (tip-splitter), so a colour property's entry lists the roles it takes:
/// Exact's, written `-exact-<role>`, and CSS's system colours by name; a property that is not a colour lists none.
#[test]
fn a_colour_lists_the_roles_it_takes() {
    for name in ["background-color", "color", "border-color", "accent-color"] {
        let out = stdout(&vocab(&[name]));
        assert!(
            out.contains("roles `-exact-<role>`")
                && out.contains("secondary-label")
                && out.contains("secondary-grouped-background")
                && out.contains("system-indigo")
                && out.contains("CSS system colours, by name: Canvas, CanvasText"),
            "{name}: {out}"
        );
    }
    let json: Value =
        serde_json::from_str(&stdout(&vocab(&["--json", "background-color"]))).unwrap();
    let roles = &json["attribute"]["roles"];
    assert!(roles["exact"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "secondary-background"));
    assert!(roles["css"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "Canvas"));
    for name in ["width", "gap", "font-size"] {
        assert!(!stdout(&vocab(&[name])).contains("roles `-exact"), "{name}");
    }
}

#[test]
fn a_style_shows_its_rows_codec_and_default() {
    let out = stdout(&vocab(&["padding"]));
    assert!(out.contains("padding: style attribute"), "{out}");
    assert!(out.contains("padding_top"), "{out}");
    assert!(out.contains("dimension, default 0"), "{out}");
}

#[test]
fn font_family_vocabulary_describes_runtime_native_lists() {
    let out = stdout(&vocab(&["font-family"]));
    assert!(out.contains("state or data"), "{out}");
    assert!(out.contains("macOS, iOS and Linux"), "{out}");
    assert!(!out.contains("pending"), "{out}");
}

#[test]
fn appearance_vocab_explains_authored_bars() {
    let out = stdout(&vocab(&["appearance"]));
    assert!(out.contains("root tablist"), "{out}");
    assert!(out.contains("header button"), "{out}");
    assert!(out.contains("only that button authored"), "{out}");
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["appearance", "--json"]))).unwrap();
    assert!(doc["attribute"]["note"]
        .as_str()
        .unwrap()
        .contains("root tablist"));
}

#[test]
fn navigation_detent_vocab_names_its_validated_forms() {
    let out = stdout(&vocab(&["navigationDetent"]));
    for form in [
        "large",
        "medium",
        "positive point height",
        "percentages of the sheet's maximum",
        "fit-content large",
    ] {
        assert!(out.contains(form), "{out}");
    }
    let doc: Value =
        serde_json::from_str(&stdout(&vocab(&["navigationDetent", "--json"]))).unwrap();
    let accepts = doc["attribute"]["accepts"].as_str().unwrap();
    assert!(accepts.contains("fit-content large"), "{accepts}");
    assert!(accepts.contains("positive point height"), "{accepts}");
    let note = doc["attribute"]["note"].as_str().unwrap();
    assert!(note.contains("journaled"), "{note}");
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

/// LLP 1116.003 D2.2: when a route's `header` becomes the iOS navigation
/// bar is one stated rule, under the tag.
#[test]
fn header_states_when_it_is_the_navigation_bar() {
    let header = stdout(&vocab(&["header"]));
    for part in [
        "the route's first child",
        "exactly one heading",
        "lower-header-no-bar",
        "lower-header-unplaced",
    ] {
        assert!(header.contains(part), "{part}: {header}");
    }
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json", "header"]))).unwrap();
    assert!(
        doc["tag"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("navigation bar")),
        "{doc}"
    );
    // LLP 1115.000 D5.2: what a route with no bar of its own gets, and the
    // word that says it has none, under the tag and under the word.
    for part in ["system Back button alone", "`navigationBar=\"none\"`"] {
        assert!(header.contains(part), "{part}: {header}");
    }
    let bar = stdout(&vocab(&["navigationBar"]));
    for part in [
        "\"none\"",
        "\"auto\"",
        "the edge swipe still goes back",
        "system Back button alone",
    ] {
        assert!(bar.contains(part), "{part}: {bar}");
    }
}

/// LLP 1115.000 D15, D14.5: a button nobody styled is the platform's, and
/// `vocab` says which that is on each host, under the tag and under the
/// word an author writes to choose another.
#[test]
fn button_states_the_platforms_default_style_per_host() {
    for name in ["button", "buttonStyle"] {
        let out = stdout(&vocab(&[name]));
        for part in [
            "plain tinted text on iOS",
            "a push button on macOS",
            "bordered on the web",
        ] {
            assert!(out.contains(part), "{name}: {part}: {out}");
        }
    }
    let button = stdout(&vocab(&["button"]));
    for part in ["`buttonStyle=\"filled\"`", "`destructive=true`"] {
        assert!(button.contains(part), "{part}: {button}");
    }
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json", "buttonStyle"]))).unwrap();
    assert!(
        doc["attribute"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("plain tinted text on iOS")),
        "{doc}"
    );
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
        ("transform", "view", "transform=\"scale(0.8)\""),
        ("selectionchange", "view", "selectionchange=done"),
        ("beforeinput", "view", "beforeinput=done"),
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
    for (name, tag, attr) in cases.iter().chain([&("robots", "view", "robots=\"x\"")]) {
        let source =
            format!("component App\n  action done\n    let x = 1\n  action got(data: string)\n    let y = data\n  view\n    {tag} {attr}\n");
        let error = contract::compile(&source).unwrap_err();
        let id = if matches!(*name, "reachstart" | "reachend") {
            "lower-collection-edge"
        } else {
            "lower-attr-tag"
        };
        assert_eq!(error.id, id, "{tag} {attr}: {error}");
    }
}

#[test]
fn collection_edges_name_the_required_shape_and_the_plain_feed_alternative() {
    for event in ["reachstart", "reachend"] {
        let output = stdout(&vocab(&[event]));
        assert!(
            output.contains("only on `list virtualized=true` with one direct `each`"),
            "{output}"
        );
        assert!(
            output.contains(&contract::collection_edge_refusal(event)),
            "{output}"
        );
        let json: Value = serde_json::from_str(&stdout(&vocab(&["--json", event]))).unwrap();
        assert_eq!(
            json["attribute"]["note"],
            contract::collection_edge_refusal(event)
        );
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
    let all = stdout(&vocab(&["--all"]));
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
        slice.contains("  signature: any.slice(number[, number]) -> any\n"),
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
        out.contains("filter: standard function\n  signature: any.filter(any) -> any"),
        "{out}"
    );
}

#[test]
fn a_function_the_web_has_and_contract_refuses_says_what_to_write() {
    for (name, says) in [
        ("sort", "name the order where the view reads it"),
        ("sortBy", "name the order where the view reads it"),
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
    let all = stdout(&vocab(&["--all"]));
    let count = exact_plan::Stdlib::ALL.len();
    assert!(
        all.contains(&format!("standard functions ({count})")),
        "{all}"
    );
    assert!(
        all.contains("  string.split(string) -> string[]\n"),
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

/// LLP 1116.003 D4: a built-in library's part answers too, with its props
/// and what it draws, read from its source.
#[test]
fn a_content_part_says_its_props_and_what_it_draws() {
    let out = stdout(&vocab(&["RoundButton", "DoneMark"]));
    assert!(
        out.contains(
            "RoundButton: a part of `exact:content` (`use RoundButton from \"exact:content\"`)"
        ),
        "{out}"
    );
    assert!(
        out.contains("props label: string, tint: option<string>, press: action, disabled: bool, testId: string"),
        "{out}"
    );
    assert!(out.contains("80-point circle"), "{out}");
    assert!(out.contains("props caption: string, done: bool"), "{out}");
    let json: Value = serde_json::from_str(&stdout(&vocab(&["--json", "AnswerTile"]))).unwrap();
    assert_eq!(json["part"]["props"][1], "status: string");
}

/// LLP 1116.010 D1: `contract vocab` with no name is the card, at most 12 KB (LLP 1116.011 D1 added the how-to),
/// with every function, the tags, each part's call, the test steps, the agent's
/// operations and the rules builders grepped for; `--all` is the full listing.
#[test]
fn no_name_prints_the_card_and_all_prints_every_table() {
    let card = stdout(&vocab(&[]));
    assert!(card.len() <= 12 * 1024, "the card is {} bytes", card.len());
    for want in [
        "functions: call -> result",
        "parseNumber(text) -> option<number>",
        "formatNumber(n,",
        "tags: attributes; on events",
        "input: value type inputmode min max step",
        "parts: use X from \"exact:content\"",
        "EmptyState(symbol=\"symbol:sparkles\"",
        "test steps",
        "expect text \"id\"",
        "ops: a target is a testId",
        "tap <target>",
        "persist",
        "negative `min` gives the pad its ± key",
        "symbol roles: back close",
        "idioms, whole with their code",
    ] {
        assert!(card.contains(want), "{want} missing:\n{card}");
    }
    // No SVG filter primitive: the listing has them, the card is what an app writes.
    assert!(!card.contains("feGaussianBlur"), "{card}");
    let all = stdout(&vocab(&["--all"]));
    assert!(all.len() > 20 * 1024, "{} bytes", all.len());
    assert!(all.contains("feGaussianBlur"), "{all}");
    assert!(all.contains("tags ("), "{all}");
    // The usage names both.
    let help = stdout(&vocab(&["--help"]));
    assert!(
        help.contains("[--all]") && help.contains("the card"),
        "{help}"
    );
    // A name still answers alone, and the JSON is unchanged.
    assert!(stdout(&vocab(&["EmptyState"])).contains("call EmptyState("));
    let json: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    assert!(json["tags"].is_array());
}

/// LLP 1116.010 D3: `min`, `max` and `step` say a number of either sign is
/// their text on an `input`, and where the decimal pad's ± key comes from.
#[test]
fn min_max_and_step_say_a_negative_number_is_written_bare() {
    for name in ["min", "max", "step"] {
        let out = stdout(&vocab(&[name]));
        assert!(
            out.contains("`min=-40`") && out.contains("± key"),
            "{name}: {out}"
        );
    }
}

/// LLP 1116.010 D3: a part's entry ends with one call as a builder would
/// paste it, written beside the component (`// Call: …`), and every part's
/// compiles with no diagnostic: a prop added to a component fails here until
/// its call passes it, so the call cannot drift from the props.
#[test]
fn every_content_part_ends_with_one_working_call() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lib/content.contract"
    ))
    .unwrap();
    let parts: Vec<&str> = source
        .lines()
        .filter_map(|l| l.strip_prefix("component "))
        .collect();
    assert_eq!(parts.len(), 9, "{parts:?}");
    let dir = std::env::temp_dir().join(format!("exact-parts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    let mut failures = Vec::new();
    for part in &parts {
        let json: Value = serde_json::from_str(&stdout(&vocab(&["--json", part]))).unwrap();
        let Some(call) = json["part"]["call"].as_str() else {
            failures.push(format!("{part}: no `// Call:` line above its component"));
            continue;
        };
        assert!(call.starts_with(&format!("{part}(")), "{part}: {call}");
        // The text entry ends with it, the call's own line.
        let text = stdout(&vocab(&[part]));
        assert_eq!(
            text.trim_end().lines().last().unwrap().trim(),
            format!("call {call}"),
            "{part}"
        );
        assert!(
            !json["part"]["about"].as_str().unwrap().contains("Call:"),
            "{part}"
        );
        // Every prop is passed, an option as `none` or `some(…)`, and it builds.
        let props: Vec<&str> = json["part"]["props"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().split(':').next().unwrap())
            .collect();
        for prop in &props {
            assert!(
                call.contains(&format!("{prop}=")),
                "{part}: {prop} in {call}"
            );
        }
        for (prop, ty) in json["part"]["props"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap().split_once(": ").unwrap())
        {
            if ty.starts_with("option<") {
                assert!(
                    call.contains(&format!("{prop}=none"))
                        || call.contains(&format!("{prop}=some(")),
                    "{part}: {prop} is an option: {call}"
                );
            }
        }
        // `press=toggle` and `press=pick(…)` name the author's own actions.
        let app = format!(
            "use {part} from \"exact:content\"\n\ncomponent App\n  state running = false\n  state picked = \"\"\n  action toggle\n    running = not running\n  action pick(a: string)\n    picked = a\n  view\n    column\n      {call}\n"
        );
        std::fs::write(&path, &app).unwrap();
        let (compiled, warnings) = contract::compile_path_all_warned(&path, false);
        if let Err(all) = compiled {
            failures.extend(
                all.iter()
                    .map(|e| format!("{part}: [{}] {}", e.id, e.message)),
            );
        }
        failures.extend(
            warnings
                .iter()
                .map(|w| format!("{part}: warning [{}] {}", w.id, w.message)),
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(failures.is_empty(), "{failures:#?}");
}

/// LLP 1116.004 D5.1: a concept a builder searches for answers with its whole
/// idiom, several in one call, after the entries of a tag of the same name.
#[test]
fn concepts_answer_together_after_the_tags_of_their_name() {
    let out = stdout(&vocab(&["scroll", "plural", "dialog"]));
    let at = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{out}"))
    };
    assert!(at("scroll: tag, ScrollView") < at("scroll (a concept)"));
    assert!(at("scroll (a concept)") < at("scrollIntoView(\"top\""));
    assert!(at("scrollIntoView(\"top\"") < at("plural (a concept)"));
    assert!(at("plural (a concept)") < at("dialog: tag, View"));
    assert!(at("dialog: tag, View") < at("modal (a concept)"));
    assert!(out.contains("navigationPresentation=\"modal\""), "{out}");
    // A concept two names share prints once.
    let twice = stdout(&vocab(&["swipe", "swipe-to-delete"]));
    assert_eq!(twice.matches("swipe (a concept)").count(), 1, "{twice}");
    assert!(
        twice.contains("swipe-to-delete: the concept `swipe`, above"),
        "{twice}"
    );
    // A real attribute keeps its entry first; a refused web name says so.
    let top = stdout(&vocab(&["scrollTop"]));
    assert!(top.starts_with("scrollTop: prop attribute"), "{top}");
    assert!(top.contains("scroll (a concept)"), "{top}");
    let every = stdout(&vocab(&["every"]));
    assert!(every.starts_with("`every` is refused"), "{every}");
    assert!(every.contains("every(1000, tick)"), "{every}");
    let all = stdout(&vocab(&["--all"]));
    assert!(
        all.contains("concepts (") && all.contains("  choice "),
        "{all}"
    );
}

/// LLP 1116.007 D1: the grammar's own words answer in one lookup, each in
/// under a kilobyte, so a builder does not grep `contract-grammar.md` for
/// `for` or get a 404 for `task`.
#[test]
fn the_grammars_words_each_answer_in_under_a_kilobyte() {
    for word in [
        "task", "every", "after", "send", "match", "for", "if", "let", "fn", "shape", "persist",
        "props", "derive", "action", "resource", "mutation", "stepper", "notfound",
    ] {
        let one: Value = serde_json::from_str(&stdout(&vocab(&["--json", word]))).unwrap();
        let concept = &one["concept"];
        assert!(concept.is_object(), "`{word}` has no answer: {one}");
        let answer = concept["answer"].as_str().unwrap();
        assert!(
            answer.len() < 1024,
            "`{word}` answers in {} bytes",
            answer.len()
        );
    }
    let stepper = stdout(&vocab(&["stepper"]));
    assert!(
        stepper.contains("UIStepper") && stepper.contains("NSStepper"),
        "{stepper}"
    );
    assert!(stepper.contains("appearance=\"textfield\""), "{stepper}");
}

/// Each concept's answer holds the repair whole: the concepts D5.1 names
/// each carry code, every `contract` block they print compiles with no
/// diagnostic, and no name answers for two concepts.
#[test]
fn every_concepts_code_compiles_and_its_names_are_its_own() {
    let doc: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    let concepts = doc["concepts"].as_array().unwrap();
    let mut names: Vec<String> = Vec::new();
    for c in concepts {
        for n in std::iter::once(&c["name"]).chain(c["names"].as_array().unwrap()) {
            let n = n.as_str().unwrap().to_owned();
            assert!(!names.contains(&n), "`{n}` answers for two concepts");
            names.push(n);
        }
    }
    let dir = std::env::temp_dir().join(format!("exact-concepts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    let mut failures = Vec::new();
    for name in [
        "scroll",
        "loading",
        "plural",
        "choice",
        "screen",
        "modal",
        "confirm",
        "date",
        "persist",
        "swipe",
        "task",
        "mutation",
        "resource",
        "match",
        "for",
        "if",
        "let",
        "fn",
        "shape",
        "props",
        "derive",
        "action",
        "stepper",
        "menu",
        "menubar",
        "::placeholder",
    ] {
        let one: Value = serde_json::from_str(&stdout(&vocab(&["--json", name]))).unwrap();
        let blocks = one["concept"]["contract"].as_array().unwrap();
        if blocks.is_empty() {
            failures.push(format!("{name}: no contract block"));
        }
        for body in blocks {
            if let Err(all) =
                contract::compile_path_source_all(&path, body.as_str().unwrap(), false)
            {
                for e in all {
                    failures.push(format!("{name}:{}: [{}] {}", e.span.line, e.id, e.message));
                }
            }
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn menu_bar_attributes_explain_the_fixed_removal_ids() {
    for name in ["-exact-menu", "-exact-menu-remove"] {
        let out = stdout(&vocab(&[name]));
        for part in [
            "menubar (a concept)",
            "close-all",
            "merge-all-windows",
            "lower-menu-remove",
        ] {
            assert!(out.contains(part), "{name}: {out}");
        }
    }
}

/// LLP 1104 D3: `::placeholder` answers with its block, and so does the
/// name an author guesses for it; the part's declaration is no attribute.
#[test]
fn placeholder_answers_with_its_block() {
    for name in ["::placeholder", "placeholder-color"] {
        let out = stdout(&vocab(&[name]));
        assert!(out.contains("::placeholder (a concept)"), "{name}: {out}");
        assert!(out.contains("admits `color` alone"), "{name}: {out}");
        assert!(!out.contains("native module"), "{name}: {out}");
    }
    let all: Value = serde_json::from_str(&stdout(&vocab(&["--json"]))).unwrap();
    let attrs = all["attributes"].as_array().unwrap();
    assert!(
        !attrs
            .iter()
            .any(|a| a["name"].as_str().unwrap().starts_with("::")),
        "a part's declaration is listed as an attribute"
    );
}

#[test]
fn transform_vocab_names_the_box_refusal_and_individual_properties() {
    let text = stdout(&vocab(&["transform"]));
    let json: Value = serde_json::from_str(&stdout(&vocab(&["--json", "transform"]))).unwrap();
    assert!(text.contains("only on SVG elements"), "{text}");
    assert!(json["attribute"]["only"]
        .as_str()
        .unwrap()
        .contains("SVG elements"));
    for note in [text.as_str(), json["attribute"]["note"].as_str().unwrap()] {
        for expected in [
            "`transform`",
            "deferred",
            "LLP 1055.000 D5",
            "`rotate`",
            "translate=\"0px 25px\"",
            "scale=0.8",
        ] {
            assert!(note.contains(expected), "{expected}: {note}");
        }
    }
}

#[test]
fn text_width_vocabulary_names_the_accessibility_warning_and_rem_repair() {
    for name in ["width", "font-size"] {
        let out = stdout(&vocab(&[name]));
        assert!(
            out.contains("lower-text-px-width")
                && out.contains("fixed ceiling")
                && out.contains("max-width=\"3rem\"")
                && out.contains("width and its bounds")
                && out.contains("rem")
                && out.contains("Apple")
                && out.contains("web controls inherit")
                && !out.contains("native controls start with their own scalable font"),
            "{out}"
        );
        let doc: Value = serde_json::from_str(&stdout(&vocab(&[name, "--json"]))).unwrap();
        assert!(doc["attribute"]["note"]
            .as_str()
            .unwrap()
            .contains("lower-text-px-width"));
    }
}

#[test]
fn wakelock_says_its_two_words_and_where_it_holds() {
    let out = vocab(&["wakelock"]);
    assert!(out.status.success());
    let text = stdout(&out);
    for want in [
        "prop",
        "\"screen\"",
        "\"none\"",
        "display: none",
        "state.wakelock",
    ] {
        assert!(text.contains(want), "{want}: {text}");
    }
}

/// LLP 1116.021 D1: a recipe's name answers with the whole recipe, in its
/// place among the other names, so a screen's lookups are one call; a recipe
/// two names reach prints once, and a pattern's name says what it has and
/// how to write it.
#[test]
fn a_recipe_name_prints_the_recipe_in_its_place() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs");
    let form = std::fs::read_to_string(format!("{root}/recipes/grouped-form.md")).unwrap();
    let out = stdout(&vocab(&[
        "swipe",
        "grouped-form",
        "persist",
        "recipes/grouped-form.md",
    ]));
    let at = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle} missing:\n{out}"))
    };
    assert!(at("swipe (a concept)") < at(&form) && at(&form) < at("persist (a concept)"));
    assert_eq!(out.matches(form.as_str()).count(), 1, "{out}");
    assert!(
        out.contains("recipes/grouped-form.md: the recipe `grouped-form`, above"),
        "{out}"
    );
    assert!(!out.contains("native module"), "{out}");
    let out = stdout(&vocab(&["tabs"]));
    assert!(
        out.contains("# Recipe: tabs") && out.contains("bun exact.mjs pattern tabs"),
        "{out}"
    );
    let docs: Value =
        serde_json::from_str(&stdout(&vocab(&["--json", "grouped-form", "one-screen"]))).unwrap();
    assert_eq!(docs[0]["recipe"]["text"], form.as_str());
    assert_eq!(
        docs[1]["pattern"]["write"],
        "bun exact.mjs pattern one-screen"
    );
    let output = vocab(&["tabz"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("try `contract vocab tabs` (a recipe)"));
}

/// LLP 1116.021, round 20: a builder's missed lookup sent them to the long
/// guides, so a miss names what it is written as and the recipes, patterns and
/// concepts named like it, and the grammar's `state` and the segmented
/// control's `segment` answer outright.
#[test]
fn a_miss_points_at_what_it_means() {
    let refused = |name: &str| {
        let output = vocab(&[name]);
        assert_eq!(output.status.code(), Some(1), "{name}");
        String::from_utf8(output.stderr).unwrap()
    };
    for name in ["random", "shuffle"] {
        let err = refused(name);
        assert!(
            err.contains("try `contract vocab random-and-shuffle` (a recipe)"),
            "{err}"
        );
    }
    assert!(refused("dates").contains("`contract vocab date` (a concept)"));
    assert!(refused("storage").contains("`contract vocab persist` (a concept)"));
    assert!(refused("checkbox").contains("`input type=\"checkbox\""));
    assert!(refused("strikethrough").contains("`text-decoration=\"line-through\"`"));
    assert!(!refused("month").contains("try"));
    assert!(stdout(&vocab(&["segment"])).contains("choice (a concept)"));
    assert!(stdout(&vocab(&["state"])).contains("action (a concept)"));
    // What a miss names is itself an answer.
    let named = refused("checkbox") + &refused("strikethrough") + &refused("random");
    for name in ["input", "text-decoration", "random-and-shuffle"] {
        assert!(named.contains(&format!("contract vocab {name}`")));
        stdout(&vocab(&[name]));
    }
}

/// LLP 1116.021 D3: a symbol the compiler refuses points to `contract vocab
/// symbol`, which lists the roles.
#[test]
fn symbol_lists_the_roles() {
    let out = stdout(&vocab(&["symbol"]));
    assert!(
        out.contains("symbol: tag")
            && out.contains("`symbol:<role>`")
            && out.contains("settings")
            && out.contains("symbol:sf/<name>"),
        "{out}"
    );
    let json: Value = serde_json::from_str(&stdout(&vocab(&["--json", "symbol"]))).unwrap();
    assert!(json["roles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "settings"));
    let refused = |source: &str| {
        contract::compile(&format!("component App\n  view\n    image \"{source}\"\n"))
            .unwrap_err()
            .to_string()
    };
    let error = refused("symbol:gearsh");
    assert!(
        error.contains("symbol `gearsh` is not a role; `contract vocab symbol` lists the roles")
            && !error.contains("microphone"),
        "{error}"
    );
    let error = refused("symbol:settins");
    assert!(error.contains("did you mean `symbol:settings`?"), "{error}");
}

/// Every recipe and pattern under `docs/` answers by its name: the lists
/// `contract vocab` embeds are the directories'.
#[test]
fn every_recipe_and_pattern_answers_by_name() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs");
    let mut recipes = 0;
    for entry in std::fs::read_dir(format!("{root}/recipes")).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_str().unwrap().to_owned();
        let Some(name) = file.strip_suffix(".md") else {
            continue;
        };
        let out = stdout(&vocab(&[name]));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains(&text), "{name}: add it to vocab/recipes.rs");
        recipes += 1;
    }
    assert!(recipes > 20, "{recipes} recipes");
    for entry in std::fs::read_dir(format!("{root}/patterns")).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let out = stdout(&vocab(&[&name]));
        assert!(
            out.contains(&format!("`bun exact.mjs pattern {name}`")),
            "{name}: add it to vocab/recipes.rs"
        );
    }
}
