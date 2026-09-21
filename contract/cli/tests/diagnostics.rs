//! Exercise the real CLI protocol and original imported locations.

use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Output},
};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-json-{name}-{}-é\"", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .arg("build")
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn diagnostics(output: &Output, code: i32) -> Vec<Value> {
    assert_eq!(output.status.code(), Some(code), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("stdout is exactly one JSON array")
}
fn same_error(diagnostic: &Value, error: &contract::CompileError) {
    assert_eq!(
        *diagnostic,
        serde_json::from_str::<Value>(&error.to_json()).unwrap()
    );
    assert_eq!(diagnostic["id"], error.id);
    assert_eq!(diagnostic["message"], error.message);
    assert_eq!(diagnostic["line"], error.span.line);
    assert_eq!(diagnostic["col"], error.span.col);
    assert_eq!(diagnostic["end_col"], error.span.end_col);
    assert_eq!(diagnostic["related"], serde_json::json!([]));
}

#[test]
fn imported_refusals_are_json_with_original_files_and_byte_ranges() {
    let app = App::new("imports");
    let root = app.write(
        "app.contract",
        "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    Row()\n",
    );
    for source in [
        "component Row\n  view\n    text `é ${missingName}`\n",
        "component Row\n  view\n    text )\n",
        "routes nav\n  home \"/\"\ncomponent Row\n  view\n    text \"row\"\n",
        "component Row\n  view\n    view align-items=\"middle\"\n",
        "component Row\n  view\n    view widht=100\n",
        "use Absent from \"./absent.contract\"\ncomponent Row\n  view\n    text \"row\"\n",
    ] {
        let row = app
            .write("lib/row.contract", source)
            .canonicalize()
            .unwrap();
        let expected = contract::compile_path(&root).unwrap_err();
        let output = app.run(&[root.to_str().unwrap(), "--json", "-o", "out.plan"]);
        let errors = diagnostics(&output, 1);
        assert_eq!(errors.len(), 1);
        same_error(&errors[0], &expected);
        assert_eq!(errors[0]["file"], row.to_str().unwrap());
        assert!(!app.0.join("out.plan").exists());
    }
}

#[test]
fn success_is_empty_diagnostics_and_writes_the_identical_plan() {
    let app = App::new("success");
    let source = "component App\n  view\n    text \"hello\"\n";
    let root = app.write("app.contract", source);
    let expected = contract::compile_path(&root).unwrap().encode();
    assert!(diagnostics(&app.run(&["--json", "app.contract"]), 0).is_empty());
    assert!(!app.0.join("app.plan").exists());
    for args in [
        vec!["app.contract", "--json", "-o", "out.plan"],
        vec!["--json", "app.contract", "-o", "out.plan"],
        vec!["app.contract", "-o", "out.plan", "--json"],
    ] {
        assert!(diagnostics(&app.run(&args), 0).is_empty());
        assert_eq!(std::fs::read(app.0.join("out.plan")).unwrap(), expected);
    }
    let human = app.run(&["app.contract", "-o", "human.plan"]);
    assert!(human.status.success());
    assert!(human.stderr.is_empty());
    assert!(String::from_utf8_lossy(&human.stdout).contains("nodes,"));
    assert_eq!(std::fs::read(app.0.join("human.plan")).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(root).unwrap(), source);
}

#[test]
fn io_and_usage_failures_stay_in_the_structured_protocol() {
    let app = App::new("errors");
    app.write(
        "app.contract",
        "component App\n  view\n    text \"hello\"\n",
    );
    for args in [
        vec!["--json"],
        vec!["app.contract", "--json", "-o"],
        vec!["app.contract", "--json", "--unknown"],
        vec!["app.contract", "--json", "extra.contract"],
        vec!["app.contract", "--json", "--json"],
        vec!["app.contract", "--json", "-o", "--json"],
        vec![
            "app.contract",
            "--json",
            "-o",
            "out.plan",
            "-o",
            "other.plan",
        ],
    ] {
        let errors = diagnostics(&app.run(&args), 2);
        assert_eq!(errors[0]["id"], "contract-build-usage");
        assert_eq!(errors[0]["file"], Value::Null);
        assert_eq!(errors[0]["line"], 0);
        assert!(!app.0.join("out.plan").exists());
    }
    let errors = diagnostics(&app.run(&["absent.contract", "--json"]), 1);
    assert_eq!(errors[0]["id"], "contract-use-unreadable");
    assert_eq!(errors[0]["file"], "absent.contract");
    let errors = diagnostics(
        &app.run(&["app.contract", "--json", "-o", "absent/out.plan"]),
        1,
    );
    assert_eq!(errors[0]["id"], "contract-output-write");
    assert_eq!(errors[0]["file"], "absent/out.plan");
    assert_eq!(errors[0]["line"], 0);
    let human = app.run(&["absent.contract"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(human.stdout.is_empty());
    assert!(String::from_utf8_lossy(&human.stderr).contains("contract-use-unreadable"));
}

#[test]
fn compile_failure_preserves_an_existing_output() {
    let app = App::new("preserve");
    app.write("app.contract", "component App\n  view\n    text missing\n");
    let output = app.write("out.plan", "previous verified plan");
    diagnostics(&app.run(&["app.contract", "--json", "-o", "out.plan"]), 1);
    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        "previous verified plan"
    );
}

#[test]
fn standalone_source_json_has_no_invented_file() {
    let error = contract::compile("component App\n  view\n    text absent\n").unwrap_err();
    let diagnostic: Value = serde_json::from_str(&error.to_json()).unwrap();
    assert_eq!(diagnostic["file"], Value::Null);
    assert_eq!(diagnostic["line"], 3);
    assert_eq!(diagnostic["col"], 10);
    assert_eq!(diagnostic["end_col"], 16);
}

#[test]
fn manifest_failure_uses_the_same_json_protocol() {
    let app = App::new("manifest");
    app.write(
        "app.contract",
        "component App\n  view\n    text \"hello\"\n",
    );
    app.write("app.json", "{invalid");
    let errors = diagnostics(&app.run(&["app.contract", "--json"]), 1);
    assert_eq!(errors[0]["id"], "app-manifest");
    assert_eq!(errors[0]["file"], "app.contract");
    assert_eq!(errors[0]["line"], 0);
}

#[test]
fn every_command_help_succeeds_without_reading_or_changing_files() {
    let app = App::new("help");
    // A literal operand would try to parse these; help must not open them.
    for flag in ["--help", "-h"] {
        app.write(flag, "this is not a Contract source\n");
    }
    for command in ["", "build", "symbols", "fmt", "types", "test", "compat"] {
        for flag in ["--help", "-h"] {
            let mut process = Command::new(env!("CARGO_BIN_EXE_contract"));
            if !command.is_empty() {
                process.arg(command);
            }
            let output = process.arg(flag).current_dir(&app.0).output().unwrap();
            assert!(output.status.success(), "{command} {flag}: {output:?}");
            assert!(output.stderr.is_empty(), "{command} {flag}: {output:?}");
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.starts_with("usage:"), "{text}");
            assert!(text.contains(&format!("contract {command}")), "{text}");
        }
    }
    for flag in ["--help", "-h"] {
        assert_eq!(
            std::fs::read_to_string(app.0.join(flag)).unwrap(),
            "this is not a Contract source\n"
        );
    }
    assert_eq!(std::fs::read_dir(&app.0).unwrap().count(), 2);
}

#[test]
fn test_cli_requires_one_file_and_keeps_explicit_flag_named_paths() {
    let app = App::new("test-args");
    let source = "test \"probe\"\n  clock settle\n";
    for name in ["probe.test.contract", "--help", "-h"] {
        app.write(name, source);
    }
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .arg("test")
            .args(args)
            .current_dir(&app.0)
            .output()
            .unwrap()
    };
    for file in ["probe.test.contract", "./--help", "./-h"] {
        let result = run(&[file]);
        assert!(result.status.success(), "{result:?}");
        assert!(result.stderr.is_empty());
        let json: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(json[0]["name"], "probe");
        assert_eq!(json[0]["steps"][0]["op"], "clock");
    }
    for args in [
        vec![],
        vec![""],
        vec!["--unknown"],
        vec!["--"],
        vec!["probe.test.contract", "--unknown"],
        vec!["probe.test.contract", "other.test.contract"],
        vec!["probe.test.contract", "--help"],
        vec!["missing.test.contract", "--unknown"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}: {result:?}");
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).starts_with("usage: contract test "));
    }
    let missing = run(&["missing.test.contract"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing.test.contract"));
    for name in ["probe.test.contract", "--help", "-h"] {
        assert_eq!(std::fs::read_to_string(app.0.join(name)).unwrap(), source);
    }
    assert_eq!(std::fs::read_dir(&app.0).unwrap().count(), 3);
}

#[test]
fn compat_cli_validates_all_options_before_reading_the_manifest() {
    let app = App::new("compat-args");
    let manifest = r#"{"app":{"id":"com.exact.cli-args","name":"CLI args"},"deploy":{"store":{"ios":"0","macos":"0","linux":"0","web":"0"}}}"#;
    app.write("app.json", manifest);
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_contract"))
            .arg("compat")
            .args(args)
            .current_dir(&app.0)
            .env("EXACT_UPDATE_TRUST", "development")
            .env_remove("OUT_DIR")
            .output()
            .unwrap()
    };
    for (platform, target) in [
        ("web", "wasm32-unknown-unknown"),
        ("macos", "aarch64-apple-darwin"),
        ("ios", "aarch64-apple-ios"),
        ("linux", "x86_64-unknown-linux-gnu"),
    ] {
        let result = run(&[".", "--json", "--target", target, "--platform", platform]);
        assert!(result.status.success(), "{result:?}");
        assert!(result.stderr.is_empty());
        let json: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(json["inputs"]["platform"], platform);
        assert_eq!(json["target"], target);
        let plain = run(&[".", "--platform", platform, "--target", target]);
        assert!(plain.status.success(), "{plain:?}");
        assert_eq!(
            String::from_utf8(plain.stdout).unwrap().trim(),
            json["id"].as_str().unwrap()
        );
    }
    let default = run(&[".", "--platform", "web", "--json"]);
    assert!(default.status.success(), "{default:?}");
    let json: Value = serde_json::from_slice(&default.stdout).unwrap();
    assert_eq!(
        json["target"],
        format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
    );
    assert_eq!(
        std::fs::read_to_string(app.0.join("app.json")).unwrap(),
        manifest
    );

    app.write("app.json", "invalid JSON");
    for args in [
        vec![],
        vec![""],
        vec!["."],
        vec![".", "--platform"],
        vec![".", "--platform", "unknown"],
        vec![".", "--platform", "web", "--unknown"],
        vec![".", "--platform", "web", "extra"],
        vec![".", "--platform", "web", "--platform", "ios"],
        vec![".", "--platform", "web", "--target"],
        vec![".", "--platform", "web", "--target", "--json"],
        vec![".", "--platform", "web", "--target", ""],
        vec![".", "--platform", "web", "--target", "a", "--target", "b"],
        vec![".", "--platform", "web", "--json", "--json"],
    ] {
        let result = run(&args);
        assert_eq!(result.status.code(), Some(2), "{args:?}: {result:?}");
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).starts_with("usage: contract compat "));
    }
    assert_eq!(
        std::fs::read_to_string(app.0.join("app.json")).unwrap(),
        "invalid JSON"
    );
    assert_eq!(std::fs::read_dir(&app.0).unwrap().count(), 1);
}

#[test]
fn unknown_record_fields_name_declared_choices_at_the_original_import() {
    let app = App::new("record-fields");
    let root = app.write(
        "app.contract",
        "use Person from \"./lib/model.contract\"\nuse Row from \"./lib/row.contract\"\ncomponent App\n  resource person = load() as shape Person\n  view\n    Row(person=person)\n",
    );
    app.write(
        "lib/model.contract",
        "shape Person\n  id: string\n  name: string\n  unread: bool\n",
    );
    let source = "use Person from \"./model.contract\"\ncomponent Row\n  props\n    person: Person\n  view\n    text `${person.nmae}`\n";
    let row = app
        .write("lib/row.contract", source)
        .canonicalize()
        .unwrap();
    let expected = contract::compile_path(&root).unwrap_err();
    assert_eq!(expected.id, "type-unknown-field");
    assert_eq!(
        expected.message,
        "`Person` has no field `nmae`; available fields: `id`, `name`, `unread`"
    );
    let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 1);
    same_error(&errors[0], &expected);
    assert_eq!(errors[0]["file"], row.to_str().unwrap());
    assert_eq!(errors[0]["line"], 6);
    assert_eq!(errors[0]["col"], 20);
    assert_eq!(errors[0]["end_col"], 24);
    let human = app.run(&[root.to_str().unwrap()]);
    assert_eq!(human.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&human.stderr).contains(&expected.message));
    for field in ["id", "name", "unread"] {
        app.write(
            "lib/row.contract",
            &source.replace("person.nmae", &format!("person.{field}")),
        );
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
    app.write("lib/model.contract", "shape Person\n");
    app.write("lib/row.contract", source);
    let empty = contract::compile_path(&root).unwrap_err();
    assert_eq!(empty.id, "type-unknown-field");
    assert_eq!(
        empty.message,
        "`Person` has no field `nmae`; this shape declares no fields"
    );
}

#[test]
fn missing_component_props_report_the_whole_call_interface() {
    let app = App::new("missing-props");
    let used_root = "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    provide theme = \"Light\"\n      Row()\n";
    let unused_root =
        "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    text \"No instance\"\n";
    let root = app.write("app.contract", used_root);
    app.write("lib/card.contract", "component Card\n  props\n    title: string\n    count: number\n    selected: bool\n    choose: action\n  inject\n    theme: string\n  view\n    button press=choose\n      text `${theme} ${title} ${count} ${selected}`\n");
    let row = "use Card from \"./card.contract\"\ncomponent Row\n  state clicks = 0\n  action choose() writes clicks\n    clicks = clicks + 1\n  view\n    Card(ARGS)\n";
    for (root_source, id) in [
        (used_root, "syntax-missing-prop"),
        (unused_root, "type-missing-prop"),
    ] {
        app.write("app.contract", root_source);
        for (args, missing) in [
            ("", "`title`, `count`, `selected`, `choose`"),
            ("title=\"Item\"", "`count`, `selected`, `choose`"),
            ("choose=choose, title=\"Item\"", "`count`, `selected`"),
            ("title=\"Item\", selected=true, choose=choose", "`count`"),
        ] {
            let path = app
                .write("lib/row.contract", &row.replace("ARGS", args))
                .canonicalize()
                .unwrap();
            let expected = contract::compile_path(&root).unwrap_err();
            assert_eq!(expected.id, id);
            assert_eq!(expected.message, format!("`Card` needs {missing}"));
            let errors = diagnostics(
                &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
                1,
            );
            same_error(&errors[0], &expected);
            assert_eq!(errors[0]["file"], path.to_str().unwrap());
            assert_eq!(errors[0]["line"], 7);
            assert_eq!(errors[0]["col"], 5);
            assert_eq!(errors[0]["end_col"], 9);
            assert!(!app.0.join("refused.plan").exists());
            let human = app.run(&[root.to_str().unwrap()]);
            assert_eq!(human.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&human.stderr).contains(&expected.message));
        }
        app.write(
            "lib/row.contract",
            &row.replace(
                "ARGS",
                "title=\"Item\", count=clicks, selected=true, choose=choose",
            ),
        );
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
}

#[test]
fn unknown_components_report_merged_declarations_at_the_original_use() {
    let app = App::new("unknown-components");
    let root = app.write("app.contract", "");
    app.write("lib/badge.contract", "shape BadgeInfo\n  title: string\nfn badgeText(): string = \"badge\"\ncomponent Badge\n  view\n    text badgeText()\ncomponent Zulu\n  view\n    text \"zulu\"\n");
    let source = "use Badge from \"./badge.contract\"\ncomponent Row\n  view\n    Badg()\n";
    for (view, id) in [
        ("Row()", "syntax-unknown-component"),
        ("text \"Unused import\"", "type-unknown-component"),
    ] {
        app.write(
            "app.contract",
            &format!("use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    {view}\n"),
        );
        let path = app
            .write("lib/row.contract", source)
            .canonicalize()
            .unwrap();
        let expected = contract::compile_path(&root).unwrap_err();
        let message =
            "unknown component `Badg`; declared components: `App`, `Row`, `Badge`, `Zulu`";
        assert_eq!(expected.id, id);
        assert_eq!(expected.message, message);
        let errors = diagnostics(
            &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
            1,
        );
        assert_eq!(errors.len(), 1);
        same_error(&errors[0], &expected);
        assert_eq!(errors[0]["file"], path.to_str().unwrap());
        assert_eq!(errors[0]["line"], 4);
        assert_eq!(errors[0]["col"], 5);
        assert_eq!(errors[0]["end_col"], 9);
        assert!(!app.0.join("refused.plan").exists());
        let human = app.run(&[root.to_str().unwrap()]);
        assert_eq!(human.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        app.write("lib/row.contract", &source.replace("Badg()", "Badge()"));
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
    // A single declaration is still useful context; the root is named as a
    // declaration, not promised as a non-recursive replacement at this use.
    app.write("app.contract", "component App\n  view\n    Absent()\n");
    let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 1);
    assert_eq!(
        errors[0]["message"],
        "unknown component `Absent`; declared components: `App`"
    );
    // A missing import keeps the loader's earlier refusal, not this use error.
    app.write(
        "app.contract",
        "use Absent from \"./lib/badge.contract\"\ncomponent App\n  view\n    Absent()\n",
    );
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "contract-use-unknown");
}

#[test]
fn unknown_component_props_report_all_names_and_declared_choices() {
    let app = App::new("unknown-props");
    let root = app.write("app.contract", "");
    app.write("lib/card.contract", "component Card\n  props\n    title: string\n    count: number\n  inject\n    theme: string\n  view\n    text `${theme} ${title} ${count}`\n");
    let row = "use Card from \"./card.contract\"\ncomponent Row\n  view\n    provide theme = \"Light\"\n      Card(ARGS)\n";
    for view in ["Row()", "text \"Unused import\""] {
        app.write(
            "app.contract",
            &format!("use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    {view}\n"),
        );
        for (extra, message) in [
            (
                "titel=\"Typo\"",
                "`Card` has no prop `titel`; available props: `title`, `count`",
            ),
            (
                "colour=\"red\", titel=\"Typo\", theme=\"Wrong\"",
                "`Card` has no props `colour`, `titel`, `theme`; available props: `title`, `count`",
            ),
            (
                "titel=\"First\", titel=\"Second\"",
                "`Card` has no prop `titel`; available props: `title`, `count`",
            ),
        ] {
            let source = row.replace("ARGS", &format!("title=\"Item\", count=1, {extra}"));
            let path = app
                .write("lib/row.contract", &source)
                .canonicalize()
                .unwrap();
            let expected = contract::compile_path(&root).unwrap_err();
            assert_eq!(expected.id, "type-unknown-prop");
            assert_eq!(expected.message, message);
            let first = extra.split('=').next().unwrap();
            let col = source.lines().nth(4).unwrap().find(first).unwrap() + 1;
            let errors = diagnostics(
                &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
                1,
            );
            assert_eq!(errors.len(), 1);
            same_error(&errors[0], &expected);
            assert_eq!(errors[0]["file"], path.to_str().unwrap());
            assert_eq!(errors[0]["line"], 5);
            assert_eq!(errors[0]["col"], col);
            assert_eq!(errors[0]["end_col"], col + first.len());
            assert!(!app.0.join("refused.plan").exists());
            let human = app.run(&[root.to_str().unwrap()]);
            assert_eq!(human.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        }
        app.write(
            "lib/row.contract",
            &row.replace("ARGS", "title=\"Item\", count=1"),
        );
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
    app.write("app.contract", "component Empty\n  view\n    text \"No props\"\ncomponent App\n  view\n    Empty(extra=true)\n");
    let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 1);
    assert_eq!(errors[0]["id"], "type-unknown-prop");
    assert_eq!(
        errors[0]["message"],
        "`Empty` has no prop `extra`; this component declares no props"
    );
    app.write(
        "app.contract",
        "component Empty\n  view\n    text \"No props\"\ncomponent App\n  view\n    Empty()\n",
    );
    assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
}

#[test]
fn imported_action_effects_use_authored_names_and_report_all_missing_slots() {
    let app = App::new("action-effects");
    let used = "use Toggle from \"./lib/toggle.contract\"\ncomponent App\n  view\n    column\n      Toggle()\n      Toggle()\n";
    let unused =
        "use Toggle from \"./lib/toggle.contract\"\ncomponent App\n  view\n    text \"unused\"\n";
    let root = app.write("app.contract", used);
    let source = "component Toggle\n  state enabled = false\n  state clicks = 0\n  state touched = false\n  action choose()WRITES\n    if enabled\n      clicks = clicks + 1\n    else\n      enabled = true\n    match some(clicks)\n      case some(value)\n        clicks = value + 1\n        touched = true\n      case none\n        touched = false\n  view\n    button press=choose\n      text \"Choose\"\n";
    for root_source in [used, unused] {
        app.write("app.contract", root_source);
        for (writes, message, line, col, end_col) in [
            ("", "`choose` has undeclared effects on `clicks`, `enabled`, `touched`; add these names to its `writes` declaration", 7, 7, 13),
            (" writes clicks", "`choose` has undeclared effects on `enabled`, `touched`; add these names to its `writes` declaration", 9, 7, 14),
            (" writes clicks, enabled", "`choose` writes `touched` but does not declare it: add `writes touched`", 13, 9, 16),
        ] {
            let path = app.write("lib/toggle.contract", &source.replace("WRITES", writes)).canonicalize().unwrap();
            let expected = contract::compile_path(&root).unwrap_err();
            assert_eq!(expected.id, "analyze-write-not-declared");
            assert_eq!(expected.message, message);
            let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]), 1);
            same_error(&errors[0], &expected);
            assert_eq!(errors[0]["file"], path.to_str().unwrap());
            assert_eq!(errors[0]["line"], line);
            assert_eq!(errors[0]["col"], col);
            assert_eq!(errors[0]["end_col"], end_col);
            assert!(!app.0.join("refused.plan").exists());
            let human = app.run(&[root.to_str().unwrap()]);
            assert_eq!(human.status.code(), Some(1));
            assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        }
        for (writes, id, message) in [
            (
                " writes enabled, enabled",
                "analyze-writes-duplicate",
                "`enabled` listed twice in `writes`",
            ),
            (
                " writes absent",
                "analyze-writes-unknown-state",
                "`absent` in `writes` is not a state or a mutation",
            ),
        ] {
            app.write("lib/toggle.contract", &source.replace("WRITES", writes));
            let error = contract::compile_path(&root).unwrap_err();
            assert_eq!(error.id, id);
            assert_eq!(error.message, message);
        }
        app.write(
            "lib/toggle.contract",
            &source.replace("WRITES", " writes enabled, clicks, touched"),
        );
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
}

#[test]
fn missing_effects_include_sends_and_do_not_repeat_targets() {
    let source = "shape Reply\n  ok: bool\ncomponent App\n  state waiting = false\n  mutation result as shape Reply\n  action submitWRITES\n    send result = save()\n    waiting = true\n    if waiting\n      send result = save()\n  view\n    button press=submit\n      text \"Save\"\n";
    for (writes, message) in [
        ("", "`submit` has undeclared effects on `result`, `waiting`; add these names to its `writes` declaration"),
        (" writes waiting", "`submit` sends `result` but does not declare it: add `writes result`"),
        (" writes result", "`submit` writes `waiting` but does not declare it: add `writes waiting`"),
    ] {
        let error = contract::compile(&source.replace("WRITES", writes)).unwrap_err();
        assert_eq!(error.id, "analyze-write-not-declared");
        assert_eq!(error.message, message);
    }
    assert!(contract::compile(&source.replace("WRITES", " writes result, waiting")).is_ok());
}

#[test]
fn missing_providers_report_every_absent_inject_on_the_use_path() {
    let app = App::new("missing-provides");
    // A provider in a sibling branch cannot satisfy the component use.
    let root = app.write("app.contract", "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    view\n      provide locale = \"Sibling\"\n        text \"Other branch\"\n      Row()\n");
    app.write("lib/card.contract", "component Card\n  inject\n    theme: string\n    locale: string\n    density: number\n  view\n    text `${theme} ${locale} ${density}`\n");
    for (providers, missing) in [
        (vec![], vec!["theme", "locale", "density"]),
        (vec!["density = 2"], vec!["theme", "locale"]),
        (
            vec!["locale = \"en\"", "theme = \"Light\""],
            vec!["density"],
        ),
        (
            vec!["theme = 1", "theme = \"Night\""],
            vec!["locale", "density"],
        ),
    ] {
        let mut source = "use Card from \"./card.contract\"\ncomponent Row\n  view\n".to_owned();
        let mut indent = "    ".to_owned();
        for provider in &providers {
            source.push_str(&format!("{indent}provide {provider}\n"));
            indent.push_str("  ");
        }
        source.push_str(&format!("{indent}Card()\n"));
        let path = app
            .write("lib/row.contract", &source)
            .canonicalize()
            .unwrap();
        let names = missing
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let scopes = missing
            .iter()
            .map(|name| format!("`provide {name} = …`"))
            .collect::<Vec<_>>()
            .join(", ");
        let message = if missing.len() == 1 {
            format!("`Card` injects {names}, and nothing above this use provides it: wrap the use in {scopes}")
        } else {
            format!("`Card` injects {names}, and nothing above this use provides them: wrap the use in nested {scopes} scopes")
        };
        let expected = contract::compile_path(&root).unwrap_err();
        assert_eq!(expected.id, "syntax-missing-provide");
        assert_eq!(expected.message, message);
        let errors = diagnostics(
            &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
            1,
        );
        assert_eq!(errors.len(), 1);
        same_error(&errors[0], &expected);
        assert_eq!(errors[0]["file"], path.to_str().unwrap());
        assert_eq!(errors[0]["line"], 4 + providers.len());
        assert_eq!(errors[0]["col"], indent.len() + 1);
        assert_eq!(errors[0]["end_col"], indent.len() + 5);
        assert!(!app.0.join("refused.plan").exists());
        let human = app.run(&[root.to_str().unwrap()]);
        assert_eq!(human.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&human.stderr).contains(&message));
        // Repair all reported names at the caller. Nearer providers in Row
        // remain authoritative, including the correctly typed inner shadow.
        app.write("app.contract", "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    provide theme = \"Outer\"\n      provide locale = \"en\"\n        provide density = 1\n          Row()\n");
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
        app.write("app.contract", "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    view\n      provide locale = \"Sibling\"\n        text \"Other branch\"\n      Row()\n");
    }
    // An unused component can still require providers from its future caller.
    app.write(
        "app.contract",
        "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    text \"No instance\"\n",
    );
    assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
}

#[test]
fn unknown_functions_suggest_only_one_available_global_spelling() {
    let app = App::new("function-hints");
    let root = app.write(
        "app.contract",
        "use Row from \"./row.contract\"\ncomponent App\n  view\n    Row()\n",
    );
    for (typo, correct, argument) in [
        ("lenght", "length", "\"hello\""),
        ("toStrng", "toString", "42"),
        ("toStringg", "toString", "42"),
        ("formatClokTime", "formatClockTime", "0"),
        ("formatClocXTime", "formatClockTime", "0"),
    ] {
        let source = format!("component Row\n  view\n    text `é ${{{typo}({argument})}}`\n");
        let path = app.write("row.contract", &source).canonicalize().unwrap();
        let error = contract::compile_path(&root).unwrap_err();
        assert_eq!(error.id, "type-unknown-function", "{error}");
        assert!(
            error
                .message
                .ends_with(&format!("; did you mean `{correct}`?")),
            "{error}"
        );
        assert_eq!(error.file.as_deref(), Some(path.as_path()));
        assert_eq!(error.span.line, 3);
        assert_eq!(
            error.span.col as usize,
            source.lines().nth(2).unwrap().find(typo).unwrap() + 1
        );
        let output = app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]);
        same_error(&diagnostics(&output, 1)[0], &error);
        assert!(!app.0.join("refused.plan").exists());
        let human = app.run(&[root.to_str().unwrap()]);
        assert_eq!(human.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&human.stderr).contains(&error.message));
        app.write("row.contract", &source.replace(typo, correct));
        let repaired = contract::compile_path(&root).unwrap().encode();
        let output = app.run(&[root.to_str().unwrap(), "--json", "-o", "repaired.plan"]);
        assert!(diagnostics(&output, 0).is_empty());
        assert_eq!(
            std::fs::read(app.0.join("repaired.plan")).unwrap(),
            repaired
        );
    }
    let library = "fn price(n: number): number = n\n";
    app.write("helpers.contract", library);
    app.write(
        "row.contract",
        "use price from \"./helpers.contract\"\ncomponent Row\n  view\n    text `${prcie(1)}`\n",
    );
    let error = contract::compile_path(&root).unwrap_err();
    assert!(
        error.message.ends_with("; did you mean `price`?"),
        "{error}"
    );
    app.write(
        "row.contract",
        "use price from \"./helpers.contract\"\ncomponent Row\n  view\n    text `${price(1)}`\n",
    );
    contract::compile_path(&root).unwrap();

    for source in [
        "fn paints(n: number): number = n\nfn points(n: number): number = n\ncomponent App\n  view\n    text `${pints(1)}`\n".to_owned(),
        "fn paints(n: number): number = n\ncomponent App\n  state count = 0\n  action points(n: number) writes count\n    count = n\n  view\n    text `${pints(1)}`\n".to_owned(),
        "component App\n  props\n    points: action\n  view\n    text `${pints(1)}`\n".to_owned(),
        "fn foo__1(n: number): number = n\ncomponent App\n  view\n    text `${foo__2(1)}`\n".to_owned(),
        "component App\n  view\n    text `${puch(1)}`\n".to_owned(), // push requires routes
        "component App\n  view\n    text `${zzz(1)}`\n".to_owned(),
        "component App\n  view\n    text `${fl(1)}`\n".to_owned(),
        format!("component App\n  view\n    text `${{{}(1)}}`\n", "x".repeat(65)),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert_eq!(error.id, "type-unknown-function", "{error}");
        assert!(!error.message.contains("did you mean"), "{error}");
    }
    for (source, typo, correct) in [
        ("routes nav\n  home \"/\"\ncomponent App\n  derive next = puch(nav, \"/\")\n  view\n    text \"ok\"\n", "puch", "push"),
        ("shape Datum\n  value: string\ncomponent App\n  resource data = read() as shape Datum\n  view\n    text `${pendng(data)}`\n", "pendng", "pending"),
    ] {
        let error = contract::compile(source).unwrap_err();
        assert_eq!(error.id, "type-unknown-function", "{error}");
        assert!(error.message.ends_with(&format!("; did you mean `{correct}`?")), "{error}");
        contract::compile(&source.replace(typo, correct)).unwrap();
    }
    let lifted_ambiguity = "fn paints(n: number): number = n\ncomponent App\n  view\n    Row()\ncomponent Row\n  state count = 0\n  action points(n: number) writes count\n    count = n\n  view\n    text `${pints(1)}`\n";
    let error = contract::compile(lifted_ambiguity).unwrap_err();
    assert_eq!(error.id, "type-unknown-function");
    assert!(!error.message.contains("did you mean"), "{error}");
    // path() is an intrinsic outside the roster: it must participate in
    // ambiguity with push(), and only becomes available with routes or a fn.
    for source in [
        "routes nav\n  home \"/\"\ncomponent App\n  view\n    text `${pash(\"home\")}`\n",
        "component App\n  view\n    text `${pth(\"home\")}`\n",
    ] {
        let error = contract::compile(source).unwrap_err();
        assert_eq!(error.id, "type-unknown-function");
        assert!(!error.message.contains("did you mean"), "{error}");
    }
    for source in [
        "routes nav\n  home \"/\"\ncomponent App\n  view\n    text pth(\"home\")\n",
        "fn path(value: string): string = value\ncomponent App\n  view\n    text pth(\"home\")\n",
    ] {
        let error = contract::compile(source).unwrap_err();
        assert!(error.message.ends_with("; did you mean `path`?"), "{error}");
        contract::compile(&source.replace("pth(", "path(")).unwrap();
    }
    // A parameter shadows the similarly named action; it is not callable.
    let source = "fn paints(n: number): number = n\ncomponent App\n  state count = 0\n  action points(n: number) writes count\n    count = n\n  action invoke(points: number) writes count\n    count = pints(points)\n  view\n    button \"Run\" press=invoke(1)\n";
    let error = contract::compile(source).unwrap_err();
    assert!(
        error.message.ends_with("; did you mean `paints`?"),
        "{error}"
    );
    contract::compile(&source.replace("pints(points)", "paints(points)")).unwrap();
    // Existing precise errors retain priority; a hint never admits a typo.
    for (expression, id) in [
        ("length()", "type-arity"),
        ("floor(\"x\")", "type-argument"),
        ("length(missing)", "type-unknown-name"),
    ] {
        let error = contract::compile(&format!(
            "component App\n  view\n    text `${{{expression}}}`\n"
        ))
        .unwrap_err();
        assert_eq!(error.id, id);
        assert!(!error.message.contains("did you mean"));
    }
}

#[test]
fn action_hints_use_authored_scopes_and_preserve_refusal_locations() {
    let app = App::new("action-hints");
    let action = "  state count = 0\n  action save writes count\n    count = count + 1\n";
    for (typo, id) in [
        ("svae", "type-unknown-name"),
        ("sav()", "type-unknown-function"),
        ("savee", "type-unknown-name"),
        ("saxe", "type-unknown-name"),
    ] {
        let source = format!("component Row\n{action}  view\n    button \"é\" press={typo}\n");
        let child = app.write("row.contract", &source).canonicalize().unwrap();
        let root = app.write(
            "app.contract",
            "use Row from \"./row.contract\"\ncomponent App\n  view\n    Row()\n",
        );
        let error = contract::compile_path(&root).unwrap_err();
        assert_eq!(error.id, id, "{error}");
        assert!(error.message.ends_with("; did you mean `save`?"), "{error}");
        assert_eq!(error.file.as_deref(), Some(child.as_path()));
        assert_eq!(error.span.line, 6);
        assert_eq!(
            error.span.col as usize,
            source.lines().nth(5).unwrap().find(typo).unwrap() + 1
        );
        let mapped = contract::compile_path_mapped(&root).err().unwrap();
        assert_eq!(mapped, error);
        same_error(
            &diagnostics(
                &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
                1,
            )[0],
            &error,
        );
        assert!(!app.0.join("refused.plan").exists());
        let human = app.run(&[root.to_str().unwrap()]);
        assert!(String::from_utf8_lossy(&human.stderr).contains(&error.message));
        app.write("row.contract", &source.replace(typo, "save"));
        contract::compile_path(&root).unwrap();
    }
    // Props and injections are actions in the child; slot children retain the
    // caller's scope. Passing an action prop is also an action-valued position.
    for (source, typo, correct) in [
        (format!("component App\n{action}  view\n    Row(commit=svae)\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=commit\n"), "svae", "save"),
        (format!("component App\n{action}  view\n    Row(commit=save)\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=comimt\n"), "comimt", "commit"),
        (format!("component App\n{action}  view\n    provide commit = save\n      Row()\ncomponent Row\n  inject\n    commit: action\n  view\n    button \"Save\" press=comimt()\n"), "comimt", "commit"),
        (format!("component App\n{action}  view\n    Row()\n      button \"Save\" press=svae\ncomponent Row\n  slot\n  view\n    column\n      children\n"), "svae", "save"),
        (format!("component App\n{action}  task timer mount\n    every(1000, svae)\n  view\n    text toString(count)\n"), "svae", "save"),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert!(error.message.ends_with(&format!("; did you mean `{correct}`?")), "{error}");
        contract::compile(&source.replace(typo, correct)).unwrap();
    }
    // Never offer another component's action, a shadowed action, a generated
    // spelling, or an ambiguous correction. Global functions are not handlers.
    for source in [
        format!("component App\n  view\n    button \"Save\" press=svae\n    Row()\ncomponent Row\n{action}  view\n    text toString(count)\n"),
        format!("component App\n{action}  view\n    Row()\ncomponent Row\n  view\n    button \"Save\" press=svae\n"),
        format!("component App\n{action}  action sale writes count\n    count = 1\n  view\n    button \"Save\" press=sace\n"),
        format!("component App\n{action}  resource items = items() as shape list<number>\n  view\n    each save in items key=toString(save)\n      button \"Save\" press=svae\n"),
        format!("component App\n{action}  state chosen = some(1)\n  view\n    match chosen\n      case some(save)\n        button \"Save\" press=svae\n      case none\n        text \"None\"\n"),
        "fn save(): number = 1\ncomponent App\n  view\n    button \"Save\" press=svae()\n".into(),
        "component App\n  state count = 0\n  action save__1 writes count\n    count = 1\n  view\n    button \"Save\" press=save__2\n".into(),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert!(matches!(error.id.as_str(), "type-unknown-name" | "type-unknown-function"), "{error}");
        assert!(!error.message.contains("did you mean"), "{error}");
    }
    // A local in the opposite branch must not hide this arm's action.
    let source = format!("component App\n{action}  state chosen = some(1)\n  view\n    column\n      match chosen\n        case some(save)\n          text toString(save)\n        case none\n          button \"Save\" press=svae\n");
    let error = contract::compile(&source).unwrap_err();
    assert!(error.message.ends_with("; did you mean `save`?"), "{error}");
    contract::compile(&source.replace("svae", "save")).unwrap();
}

#[test]
fn forwarded_action_hints_link_the_supplied_argument_through_props_and_providers() {
    let app = App::new("forwarded-action-hints");
    let action = "  state count = 0\n  action save writes count\n    count = count + 1\n";
    for (source, typo) in [
        (format!("component App\n{action}  view\n    Row(commit=svae)\ncomponent Row\n  props\n    commit: action\n  view\n    Leaf(submit=commit)\ncomponent Leaf\n  props\n    submit: action\n  view\n    button \"Save\" press=submit\n"), "svae"),
        (format!("component App\n{action}  view\n    provide commit = svae\n      Row()\ncomponent Row\n  view\n    Leaf()\ncomponent Leaf\n  inject\n    commit: action\n  view\n    button \"Save\" press=commit()\n"), "svae"),
        ("component App\n  state count = 0\n  action save(value: number) writes count\n    count = value\n  view\n    Row(commit=svae)\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=commit(1)\n".into(), "svae"),
        (format!("component App\n{action}  view\n    Row(sace=sace)\ncomponent Row\n  props\n    sace: action\n  state n = 0\n  action sale writes n\n    n = 1\n  view\n    button \"Save\" press=sace\n"), "sace"),
    ] {
        let path = app.write("app.contract", &source).canonicalize().unwrap();
        let error = contract::compile_path(&path).unwrap_err();
        assert!(error.message.ends_with("; did you mean `save`?"), "{error}");
        assert_eq!(error.related.len(), 1, "{error}");
        let related = &error.related[0];
        assert_eq!(related.file.as_deref(), Some(path.as_path()));
        assert_eq!(related.span.line, 6);
        assert!(related.note.contains(typo));
        assert_eq!(source.lines().nth(5).unwrap()[related.span.col as usize-1..related.span.end_col as usize-1], *typo);
        assert_eq!(contract::compile_path_mapped(&path).err().unwrap(), error);
        let errors = diagnostics(&app.run(&[path.to_str().unwrap(), "--json"]), 1);
        assert_eq!(errors[0], serde_json::from_str::<Value>(&error.to_json()).unwrap());
        // Only the indicated caller expression is repaired; child names stay.
        let mut repaired = source.lines().map(str::to_owned).collect::<Vec<_>>();
        let line = &mut repaired[related.span.line as usize-1];
        line.replace_range(related.span.col as usize-1..related.span.end_col as usize-1, "save");
        contract::compile(&repaired.join("\n")).unwrap();
    }
    let source = "component App\n  view\n    First()\n    Second()\ncomponent First\n  state n = 0\n  action save writes n\n    n = 1\n  view\n    Leaf(submit=sace)\ncomponent Second\n  state n = 0\n  action sale writes n\n    n = 1\n  view\n    Leaf(submit=sace)\ncomponent Leaf\n  props\n    submit: action\n  view\n    button \"Go\" press=submit\n";
    let error = contract::compile(source).unwrap_err();
    assert_eq!(error.id, "type-unknown-name");
    assert!(!error.message.contains("did you mean"), "{error}");
    assert!(error.related.is_empty());
}

#[test]
fn action_hints_respect_call_intrinsics_and_function_precedence() {
    for (name, typo, global) in [
        ("pending", "pendign", ""),
        ("path", "paht", ""),
        ("save", "svae", "fn save(): number = 1\n"),
    ] {
        let declarations = format!("{global}component App\n  state count = 0\n  action {name} writes count\n    count = count + 1\n");
        for view in [
            format!("  view\n    button \"Save\" press={typo}()\n"),
            format!("  view\n    Row(commit={typo})\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=commit()\n"),
            format!("  view\n    provide commit = {typo}\n      Row()\ncomponent Row\n  inject\n    commit: action\n  view\n    button \"Save\" press=commit()\n"),
        ] {
            let error = contract::compile(&format!("{declarations}{view}")).unwrap_err();
            assert_eq!(error.id, "type-unknown-function", "{error}");
            assert!(!error.message.contains("did you mean"), "{error}");
            assert!(error.related.is_empty());
        }
        // Bare action references and timers do not invoke the expression
        // intrinsic/function resolver, even when forwarded through a prop.
        for view in [
            format!("  view\n    button \"Save\" press={typo}\n"),
            format!("  task timer mount\n    every(1000, {typo})\n  view\n    text toString(count)\n"),
            format!("  view\n    Row(commit={typo})\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=commit\n"),
        ] {
            let source = format!("{declarations}{view}");
            let error = contract::compile(&source).unwrap_err();
            assert!(error.message.ends_with(&format!("; did you mean `{name}`?")), "{error}");
            contract::compile(&source.replace(typo, name)).unwrap();
        }
    }
}
