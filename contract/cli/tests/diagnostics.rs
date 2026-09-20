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
