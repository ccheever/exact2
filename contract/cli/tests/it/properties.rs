use contract_syntax::{PropertyForm, PropertyKind};

const EXAMPLE: &str = r#"always "the phone is on one of its five screens"
  expect state screen in "lock" "home" "settings" "display" "messages"
always "the counter is a whole number of at least one"
  expect state counter whole
  expect state counter >= 1
always "a probe's result is absent or an answer"
  expect state tsResult in none some
always "once the photo has loaded it stays loaded"
  expect state ready stays true
after "increment" "increment adds one"
  expect state counter == old + 1
after "reset" "reset puts the turns back and counts itself"
  expect state turns == 0
  expect state resets == old + 1
after "rotate" "rotating touches nothing but the turns"
  only writes turns
after "done" "done writes nothing and focuses the button"
  writes none
  command focus "done"
after clock "the probe never touches what the user set"
  only writes tsResult rustResult rustExecutorResult
never refuses "reset"
"#;

#[test]
fn the_rfc_example_parses_and_formats_without_losing_properties() {
    let p = contract::properties(EXAMPLE).unwrap();
    assert_eq!(p.len(), 10);
    assert_eq!(p[0].kind, PropertyKind::Always);
    assert_eq!(p[8].kind, PropertyKind::Clock);
    assert_eq!(p[9].kind, PropertyKind::NeverRefuses);
    assert!(matches!(&p[7].lines[0].form, PropertyForm::OnlyWrites(names) if names.is_empty()));
    let formatted = contract_syntax::fmt::format(EXAMPLE).unwrap();
    assert_eq!(contract::properties(&formatted).unwrap().len(), 10);
}

#[test]
fn refused_forms_name_the_authored_line() {
    for (source, line, why) in [
        (
            "component App\n  state n = 1\nalways \"p\"\n  expect state n whole\n",
            3,
            "a property lives in the test file beside the app",
        ),
        (
            "always \"p\"\n  expect state n whole\ncomponent App\n  state n = 1\n",
            1,
            "a property lives in the test file beside the app",
        ),
        (
            "always \"p\"\n  expect state n == old\n",
            2,
            "`old` is admitted only inside `after`",
        ),
        (
            "after \"a\" \"p\"\n  expect state n stays true\n",
            2,
            "`stays` is admitted only inside `always`",
        ),
        (
            "always \"p\"\n  writes none\n",
            2,
            "frame lines are admitted only inside `after`",
        ),
        (
            "always \"p\"\n  expect state n.field whole\n",
            2,
            "a property names a state or a derive by its bare name; fields are stage 2",
        ),
        (
            "always \"p\"\n  expect state n[0] whole\n",
            2,
            "fields are stage 2",
        ),
        (
            "always \"p\"\n  expect state n approximately 1\n",
            2,
            "a property tests",
        ),
    ] {
        let e = contract::properties(source).unwrap_err();
        assert_eq!(e.span.line, line, "{e}");
        assert!(e.message.contains(why), "{e}");
    }
}

#[test]
fn the_test_command_emits_structured_properties_beside_scripts() {
    let dir = std::env::temp_dir().join(format!("contract-properties-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("app.contract"), "component App\n  state screen = \"lock\"\n  state counter = 1\n  state tsResult = none\n  state rustResult = none\n  state rustExecutorResult = none\n  state ready = false\n  state turns = 0\n  state resets = 0\n").unwrap();
    let path = dir.join("app.test.contract");
    std::fs::write(
        &path,
        format!("{EXAMPLE}\ntest \"input\"\n  tap \"increment\"\n"),
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("test")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["tests"][0]["name"], "input");
    assert_eq!(json["properties"][0]["lines"][0]["test"], "in");
    assert_eq!(json["properties"][4]["lines"][0]["test"], "add");
    assert_eq!(json["properties"][4]["lines"][0]["number"], 1);
    assert_eq!(
        json["properties"][7]["lines"][1]["args"][0]["value"],
        "done"
    );
}

#[test]
fn the_cone_follows_derives_and_functions_but_respects_local_names() {
    let property = contract::properties("always \"p\"\n  expect state outer whole\n").unwrap();
    for (program, reason) in [
        ("shape Data\n  n: number\ncomponent App\n  resource data = read() as shape Data\n  derive inner = data.n\n  derive outer = inner + 1\n", "`outer` -> `inner` -> resource `data`"),
        ("fn time(): number = performanceNow()\nfn other(): number = time()\ncomponent App\n  derive outer = other()\n", "fn `other` -> fn `time` -> `performanceNow(…)`"),
        ("component App\n  derive outer = frame(\"box\").width\n", "`frame(…)`"),
        ("component App\n  derive outer = measure(\"box\").height\n", "`measure(…)`"),
        ("component App\n  derive outer = pending(saved) ? 1 : 0\n", "`pending(…)`"),
        ("component App\n  derive outer = failed(saved) ? 1 : 0\n", "`failed(…)`"),
        ("component App\n  derive outer = failure(saved)\n", "`failure(…)`"),
        ("component App\n  state n = 1\n", "not a root state or derive"),
    ] {
        let file = contract_syntax::parse(program).unwrap();
        let errors = contract_analyze::check_properties(&file.components[0], &file.fns, &property);
        assert_eq!(errors.len(), 1, "{program}");
        assert_eq!(errors[0].span.line, 2);
        assert!(errors[0].message.contains(reason), "{}", errors[0]);
    }
    let program = "fn identity(data: number): number = data\nshape Data\n  n: number\ncomponent App\n  resource data = read() as shape Data\n  state n = 1\n  derive inner = n * 2\n  derive outer = identity(inner)\n";
    let file = contract_syntax::parse(program).unwrap();
    assert!(
        contract_analyze::check_properties(&file.components[0], &file.fns, &property).is_empty()
    );
}

#[test]
fn device_json_refuses_the_same_imported_cone_as_verify() {
    let dir = std::env::temp_dir().join(format!("contract-property-cone-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(
        dir.join("clock.contract"),
        "fn reading(): number = performanceNow()\n",
    )
    .unwrap();
    let app = dir.join("app.contract");
    std::fs::write(&app, "use reading from \"./clock.contract\"\ncomponent App\n  derive outer = reading()\n  view\n    text \"clock\"\n").unwrap();
    let tests = dir.join("app.test.contract");
    std::fs::write(&tests, "always \"time\"\n  expect state outer whole\n").unwrap();
    let properties = contract::properties(&std::fs::read_to_string(&tests).unwrap()).unwrap();
    let errors = contract::check_properties_path(&app, &properties).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.to_string().contains("performanceNow")));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("test")
        .arg(&tests)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("performanceNow"), "{error}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn device_json_accepts_a_relative_test_path_whose_app_uses_a_sibling() {
    let dir =
        std::env::temp_dir().join(format!("contract-property-relative-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(
        dir.join("shapes.contract"),
        "fn twice(n: number): number = n * 2\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use twice from \"./shapes.contract\"\ncomponent App\n  state n = 1\n  derive outer = twice(n)\n  view\n    text \"n\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.test.contract"),
        "always \"whole\"\n  expect state outer whole\n",
    )
    .unwrap();
    // The device runner passes the test file as typed, often relative to the checkout.
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .arg("test")
        .arg("app.test.contract")
        .current_dir(&dir)
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{error}");
    let json = String::from_utf8_lossy(&output.stdout);
    assert!(json.contains("\"whole\""), "{json}");
    std::fs::remove_dir_all(dir).unwrap();
}
