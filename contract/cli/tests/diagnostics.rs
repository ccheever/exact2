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
