//! Exercise the real CLI protocol and original imported locations.

use serde_json::Value;
use std::{
    path::PathBuf,
    process::{Command, Output},
};

#[test]
fn diagnostics_keep_the_by_value_error_below_the_large_result_threshold() {
    assert!(std::mem::size_of::<contract::CompileError>() < 128);
}

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        // Windows forbids quotes in filenames; backslashes still exercise JSON
        // path escaping, and both platforms retain Unicode and whitespace.
        let suffix = if cfg!(windows) {
            "é space"
        } else {
            "é space\""
        };
        let path =
            std::env::temp_dir().join(format!("exact-json-{name}-{}-{suffix}", std::process::id()));
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
    for command in ["", "build", "symbols", "fmt", "types", "test"] {
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
        "`Person` has no field `nmae`; available fields: `id`, `name`, `unread`; did you mean `name`?"
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
fn unknown_types_list_named_choices_at_the_original_import() {
    let app = App::new("type-choices");
    let root = app.write(
        "app.contract",
        "use Wrapper from \"./lib/model.contract\"\nshape Zulu\n  value: number\ncomponent App\n  view\n    text \"é\"\n",
    );
    app.write("lib/contact.contract", "shape Contact\n  name: string\n");
    let source =
        "use Contact from \"./contact.contract\"\nshape Wrapper\n  value: list<option<Contcat>>\n";
    let model = app
        .write("lib/model.contract", source)
        .canonicalize()
        .unwrap();
    let expected = contract::compile_path(&root).unwrap_err();
    assert_eq!(expected.id, "type-unknown");
    assert_eq!(expected.message, "unknown type `Contcat`; known named types: `number`, `string`, `bool`, `unit`, `action`, `ClipboardEvent`, `Contact`, `DOMRectReadOnly`, `DragEvent`, `Geometry`, `InputEvent`, `KeyboardEvent`, `MarkdownSelection`, `MediaMetadata`, `MediaSessionActionDetails`, `MouseEvent`, `Picked`, `PointerEvent`, `ReorderEvent`, `ScrollEvent`, `Selection`, `WheelEvent`, `Wrapper`, `Zulu`");
    let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 1);
    same_error(&errors[0], &expected);
    assert_eq!(errors[0]["file"], model.to_str().unwrap());
    assert_eq!(errors[0]["line"], 3);
    assert_eq!(errors[0]["col"], 22);
    assert_eq!(errors[0]["end_col"], 29);
    let human = app.run(&[root.to_str().unwrap()]);
    assert_eq!(human.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&human.stderr).contains(&expected.message));
    app.write("lib/model.contract", &source.replace("Contcat", "Contact"));
    assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
}

#[test]
fn type_choices_follow_the_resolver_without_duplicate_or_unavailable_names() {
    let app = App::new("type-choice-namespaces");
    let body = "component App\n  props\n    value: strng\n  view\n    text \"hello\"\n";
    let primitive_names = "`number`, `string`, `bool`, `unit`, `action`";
    for prefix in ["", "shape string\n", "shape Later\n  value: number\n"] {
        let root = app.write("app.contract", &format!("{prefix}{body}"));
        let error = contract::compile_path(&root).unwrap_err();
        // `ClipboardEvent`, `DOMRectReadOnly`, `DragEvent`, `KeyboardEvent`, `MarkdownSelection`,
        // `MouseEvent`, `Picked`, `PointerEvent`, `ReorderEvent`, `ScrollEvent`, `Selection` and
        // `WheelEvent` are the clipboard, `drop`, `key`, `select`, `press`,
        // file `change`, pointer, `reorderdrop`, scroll, `selectionchange` and wheel payloads
        // every file can name, and `Geometry` what `frame` and `measure` answer.
        let extra = if prefix.contains("Later") {
            ", `ClipboardEvent`, `DOMRectReadOnly`, `DragEvent`, `Geometry`, `InputEvent`, `KeyboardEvent`, `Later`, `MarkdownSelection`, `MediaMetadata`, `MediaSessionActionDetails`, `MouseEvent`, `Picked`, `PointerEvent`, `ReorderEvent`, `ScrollEvent`, `Selection`, `WheelEvent`"
        } else {
            ", `ClipboardEvent`, `DOMRectReadOnly`, `DragEvent`, `Geometry`, `InputEvent`, `KeyboardEvent`, `MarkdownSelection`, `MediaMetadata`, `MediaSessionActionDetails`, `MouseEvent`, `Picked`, `PointerEvent`, `ReorderEvent`, `ScrollEvent`, `Selection`, `WheelEvent`"
        };
        assert_eq!(error.id, "type-unknown");
        assert_eq!(
            error.message,
            format!("unknown type `strng`; known named types: {primitive_names}{extra}")
        );
    }
    let root = app.write("app.contract", &format!("routes nav\n  home \"/\"\n{body}"));
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "type-unknown");
    assert_eq!(error.message, format!("unknown type `strng`; known named types: {primitive_names}, `ClipboardEvent`, `DOMRectReadOnly`, `DragEvent`, `Entry`, `Geometry`, `InputEvent`, `KeyboardEvent`, `MarkdownSelection`, `MediaMetadata`, `MediaSessionActionDetails`, `MouseEvent`, `Params`, `Picked`, `PointerEvent`, `ReorderEvent`, `Router`, `ScrollEvent`, `Selection`, `Tab`, `WheelEvent`"));
    // Field resolution has already seen later declarations, even when it fails
    // while resolving the first shape's fields.
    let root = app.write("app.contract", "shape First\n  value: Ltaer\nshape Later\n  value: string\ncomponent App\n  view\n    text \"hello\"\n");
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "type-unknown");
    assert!(error
        .message
        .ends_with("`First`, `Geometry`, `InputEvent`, `KeyboardEvent`, `Later`, `MarkdownSelection`, `MediaMetadata`, `MediaSessionActionDetails`, `MouseEvent`, `Picked`, `PointerEvent`, `ReorderEvent`, `ScrollEvent`, `Selection`, `WheelEvent`"));
}

#[test]
fn missing_component_props_report_the_whole_call_interface() {
    let app = App::new("missing-props");
    let used_root = "use Row from \"./lib/row.contract\"\ncomponent App\n  provide\n    theme = \"Light\"\n  view\n    Row()\n";
    let unused_root =
        "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    text \"No instance\"\n";
    let root = app.write("app.contract", used_root);
    app.write("lib/card.contract", "component Card\n  props\n    title: string\n    count: number\n    selected: bool\n    choose: action\n  inject\n    theme: string\n  view\n    button press=choose\n      text `${theme} ${title} ${count} ${selected}`\n");
    let row = "use Card from \"./card.contract\"\ncomponent Row\n  state clicks = 0\n  action choose()\n    clicks = clicks + 1\n  view\n    Card(ARGS)\n";
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
fn unknown_import_lists_only_target_exports_and_repairs_through_the_cli() {
    let app = App::new("import-choices");
    app.write("leaf.contract", "shape Item\n  name: string\nstyle Line\n  padding-top=10\nfn label(): string = \"label\"\ncomponent Row\n  view\n    text \"row\"\ncomponent Badge\n  view\n    text \"badge\"\n");
    for name in ["Left", "Right"] {
        app.write(
            &format!("{name}.contract"),
            &format!("use Row from \"./leaf.contract\"\ncomponent {name}\n  view\n    Row()\n"),
        );
    }
    app.write("barrel.contract", "use Left from \"./Left.contract\"\nuse Right from \"./Right.contract\"\ncomponent Wrapper\n  view\n    text \"wrapper\"\n");
    app.write(
        "other.contract",
        "component Unrelated\n  view\n    text \"other\"\n",
    );
    // The first import populates the cache. Its declaration and the root's own
    // declaration cannot satisfy a name in a different referenced file.
    let source = "use Unrelated from \"./other.contract\"\nuse Wrapper from \"./barrel.contract\"\nuse Rwo from \"./barrel.contract\"\ncomponent App\n  view\n    Row()\n";
    let root = app.write("app.contract", source);
    let expected = contract::compile_path(&root).unwrap_err();
    assert_eq!(expected.id, "contract-use-unknown");
    assert_eq!(expected.span.line, 3);
    assert_eq!(expected.file.as_deref(), Some(root.as_path()));
    // Only what barrel declares or itself names: Row, Item, Line and label
    // reach it through Left and Right, which do not pass them on (LLP 1091 D1).
    assert_eq!(expected.message, "`./barrel.contract` declares no component, shape, style, function, keyframes, or timeline `Rwo`; available components: `Left`, `Right`, `Wrapper`");
    let output = app.run(&[root.to_str().unwrap(), "--json", "-o", "out.plan"]);
    let errors = diagnostics(&output, 1);
    assert_eq!(errors.len(), 1);
    same_error(&errors[0], &expected);
    assert!(!app.0.join("out.plan").exists());
    let human = app.run(&[root.to_str().unwrap()]);
    assert_eq!(human.status.code(), Some(1));
    assert!(String::from_utf8(human.stderr)
        .unwrap()
        .contains(&expected.message));
    // Every offered declaration is actually admitted by this use syntax.
    let source = source.replace("    Row()\n", "    Wrapper()\n");
    for name in ["Wrapper", "Left", "Right"] {
        app.write(
            "app.contract",
            &source.replace("use Rwo from", &format!("use {name} as Chosen from")),
        );
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
}

#[test]
fn empty_import_choices_exclude_fonts_and_keep_nested_locations() {
    let app = App::new("empty-import-choices");
    let root = app.write(
        "app.contract",
        "use Wrap from \"./lib/wrap.contract\"\ncomponent App\n  view\n    Wrap()\n",
    );
    let nested = app
        .write(
            "lib/wrap.contract",
            "use Brand from \"./empty.contract\"\ncomponent Wrap\n  view\n    text \"wrapped\"\n",
        )
        .canonicalize()
        .unwrap();
    for source in ["", "font \"Brand\" = \"assets/not-loaded.ttf\"\n"] {
        app.write("lib/empty.contract", source);
        let expected = contract::compile_path(&root).unwrap_err();
        assert_eq!(expected.id, "contract-use-unknown");
        assert_eq!(expected.file.as_deref(), Some(nested.as_path()));
        assert_eq!(expected.span.line, 1);
        assert_eq!(expected.message, "`./empty.contract` declares no component, shape, style, function, keyframes, or timeline `Brand`; this file declares nothing that can be used");
        let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 1);
        same_error(&errors[0], &expected);
    }
}

#[test]
fn unknown_component_props_report_all_names_and_declared_choices() {
    let app = App::new("unknown-props");
    let root = app.write("app.contract", "");
    app.write("lib/card.contract", "component Card\n  props\n    title: string\n    count: number\n  inject\n    theme: string\n  view\n    text `${theme} ${title} ${count}`\n");
    let row = "use Card from \"./card.contract\"\ncomponent Row\n  provide\n    theme = \"Light\"\n  view\n    Card(ARGS)\n";
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
            let col = source.lines().nth(5).unwrap().find(first).unwrap() + 1;
            let errors = diagnostics(
                &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
                1,
            );
            assert_eq!(errors.len(), 1);
            same_error(&errors[0], &expected);
            assert_eq!(errors[0]["file"], path.to_str().unwrap());
            assert_eq!(errors[0]["line"], 6);
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

/// Each action's allowlist in the plan, as slot names, in plan order.
fn plan_writes(plan: &exact_plan::Plan) -> Vec<(String, Vec<String>)> {
    let name = |id: exact_plan::StrId| plan.strings[id.0 as usize].clone();
    plan.actions
        .iter()
        .map(|a| {
            let slots = (a.writes.start..a.writes.start + a.writes.len)
                .map(|w| name(plan.slots[plan.writes[w as usize].slot.0 as usize].name))
                .collect();
            (name(a.name), slots)
        })
        .collect()
}

// @ref LLP 1035.005.000 D1 — effects are inferred; the clause is refused.
#[test]
fn an_imported_writes_clause_is_refused_at_its_token_and_symbols_show_the_inferred_effects() {
    let app = App::new("action-effects");
    let used = "use Toggle from \"./lib/toggle.contract\"\ncomponent App\n  view\n    column\n      Toggle()\n      Toggle()\n";
    let unused =
        "use Toggle from \"./lib/toggle.contract\"\ncomponent App\n  view\n    text \"unused\"\n";
    let root = app.write("app.contract", used);
    let source = "component Toggle\n  state enabled = false\n  state clicks = 0\n  state touched = false\n  state idle = 0\n  action choose()WRITES\n    if enabled\n      clicks = clicks + 1\n    else\n      enabled = true\n    match some(clicks)\n      case some(value)\n        touched = true\n        clicks = value + 1\n      case none\n        touched = false\n  view\n    button press=choose\n      text \"Choose\"\n";
    for root_source in [used, unused] {
        app.write("app.contract", root_source);
        let path = app
            .write(
                "lib/toggle.contract",
                &source.replace("WRITES", " writes clicks"),
            )
            .canonicalize()
            .unwrap();
        let expected = contract::compile_path(&root).unwrap_err();
        assert_eq!(expected.id, "syntax-writes-clause");
        assert_eq!(
            expected.message,
            "`choose`'s effects are inferred from its body: delete the `writes` clause"
        );
        let errors = diagnostics(
            &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
            1,
        );
        same_error(&errors[0], &expected);
        assert_eq!(errors[0]["file"], path.to_str().unwrap());
        assert_eq!(
            (
                errors[0]["line"].as_u64(),
                errors[0]["col"].as_u64(),
                errors[0]["end_col"].as_u64()
            ),
            (Some(6), Some(19), Some(25))
        );
        assert!(!app.0.join("refused.plan").exists());
        app.write("lib/toggle.contract", &source.replace("WRITES", ""));
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
        let symbols: Value =
            serde_json::from_str(&contract::symbols_json(&root, Some("choose")).unwrap()).unwrap();
        let definitions = symbols["definitions"].as_array().unwrap();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0]["kind"], "action");
        assert_eq!(
            definitions[0]["writes"],
            serde_json::json!(["enabled", "clicks", "touched"])
        );
        assert!(symbols["references"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r.get("writes").is_none()));
    }
}

#[test]
fn a_child_action_cannot_write_its_parents_state() {
    // What `writes` once caught is scope: a child names only its own slots.
    let source = "component App\n  state parent = 0\n  view\n    column\n      Row()\ncomponent Row\n  state count = 0\n  action add\n    parent = count\n  view\n    button \"Add\" press=add\n";
    let error = contract::compile(source).unwrap_err();
    assert_eq!(error.id, "type-assign-not-state");
    assert_eq!(error.message, "`parent` is not a state or a mutation");
    assert!(contract::compile(&source.replace("parent = count", "count = count + 1")).is_ok());
}

#[test]
fn the_plans_allowlist_is_every_slot_the_body_writes_or_sends_in_slot_order() {
    // Router state, a send, a branch, a repeat; a lifted child's slots stay its own.
    let source = "shape Reply\n  ok: bool\nroutes nav\n  home \"/\"\ncomponent App\n  state count = 0\n  state waiting = false\n  derive total = count + 1\n  resource data = load() as shape Reply\n  mutation result as shape Reply\n  action submit(value: number)\n    count = value\n    if value > 1\n      nav = push(nav, \"/\")\n      send result = save()\n    else\n      count = 0\n      send result = save()\n  action idle\n    focus(\"save\")\n  view\n    column\n      Child()\n      Child()\n      button \"Save\" press=submit(1) testId=\"save\"\ncomponent Child\n  state hidden = false\n  action hide\n    hidden = true\n  view\n    button \"child\" press=hide\n";
    let writes = plan_writes(&contract::compile(source).unwrap());
    let of = |name: &str| writes.iter().find(|(n, _)| n == name).unwrap().1.clone();
    assert_eq!(of("submit"), ["nav", "count", "result"]);
    assert_eq!(of("idle"), Vec::<String>::new());
    let hides: Vec<_> = writes
        .iter()
        .filter(|(n, _)| n != "submit" && n != "idle")
        .collect();
    assert_eq!(hides.len(), 2, "{writes:?}");
    for (_, slots) in hides {
        assert_eq!(slots.len(), 1, "{writes:?}");
        assert!(slots[0].contains("hidden"), "{writes:?}");
    }
}

#[test]
fn missing_providers_report_every_absent_inject_on_the_use_path() {
    let app = App::new("missing-provides");
    // A sibling component's `provide` section cannot satisfy the use
    // (LLP 1035.005.000 D9: a section covers its own component's view).
    let sibling = "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    view\n      Other()\n      Row()\ncomponent Other\n  provide\n    locale = \"Sibling\"\n  view\n    text \"Other branch\"\n";
    let root = app.write("app.contract", sibling);
    app.write("lib/card.contract", "component Card\n  inject\n    theme: string\n    locale: string\n    density: number\n  view\n    text `${theme} ${locale} ${density}`\n");
    for (providers, missing) in [
        (vec![], vec!["theme", "locale", "density"]),
        (vec!["density = 2"], vec!["theme", "locale"]),
        (
            vec!["locale = \"en\"", "theme = \"Light\""],
            vec!["density"],
        ),
        (vec!["theme = \"Night\""], vec!["locale", "density"]),
    ] {
        let mut source = "use Card from \"./card.contract\"\ncomponent Row\n".to_owned();
        if !providers.is_empty() {
            source.push_str("  provide\n");
            for provider in &providers {
                source.push_str(&format!("    {provider}\n"));
            }
        }
        source.push_str("  view\n    Card()\n");
        let path = app
            .write("lib/row.contract", &source)
            .canonicalize()
            .unwrap();
        let names = missing
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let bindings = missing
            .iter()
            .map(|name| format!("`{name} = …`"))
            .collect::<Vec<_>>()
            .join(", ");
        let them = if missing.len() == 1 { "it" } else { "them" };
        let message = format!("`Card` injects {names}, and no component above this use provides {them}: in this component or one that uses it, write a `provide` section with {bindings} indented under it");
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
        let section = if providers.is_empty() {
            0
        } else {
            1 + providers.len()
        };
        assert_eq!(errors[0]["line"], 4 + section);
        assert_eq!(errors[0]["col"], 5);
        assert_eq!(errors[0]["end_col"], 9);
        assert!(!app.0.join("refused.plan").exists());
        let human = app.run(&[root.to_str().unwrap()]);
        assert_eq!(human.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&human.stderr).contains(&message));
        // Repair all reported names at the caller. Row's own section stays
        // authoritative for what it provides.
        app.write("app.contract", "use Row from \"./lib/row.contract\"\ncomponent App\n  provide\n    theme = \"Outer\"\n    locale = \"en\"\n    density = 1\n  view\n    Row()\n");
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
        app.write("app.contract", sibling);
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
        ("formatTiem", "formatTime", "0, 0, \"short\""),
        ("formatTme", "formatTime", "0, 0, \"short\""),
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
        "fn paints(n: number): number = n\ncomponent App\n  state count = 0\n  action points(n: number)\n    count = n\n  view\n    text `${pints(1)}`\n".to_owned(),
        "component App\n  props\n    points: action\n  view\n    text `${pints(1)}`\n".to_owned(),
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
    let lifted_ambiguity = "fn paints(n: number): number = n\ncomponent App\n  view\n    Row()\ncomponent Row\n  state count = 0\n  action points(n: number)\n    count = n\n  view\n    text `${pints(1)}`\n";
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
    let source = "fn paints(n: number): number = n\ncomponent App\n  state count = 0\n  action points(n: number)\n    count = n\n  action invoke(points: number)\n    count = pints(points)\n  view\n    button \"Run\" press=invoke(1)\n";
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
    let action = "  state count = 0\n  action save\n    count = count + 1\n";
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
        (format!("component App\n{action}  provide\n    commit = save\n  view\n    Row()\ncomponent Row\n  inject\n    commit: action\n  view\n    button \"Save\" press=comimt()\n"), "comimt", "commit"),
        (format!("component App\n{action}  view\n    Row()\n      button \"Save\" press=svae\ncomponent Row\n  slot\n  view\n    column\n      children\n"), "svae", "save"),
        (format!("component App\n{action}  task timer mount\n    every(1000, svae)\n  view\n    text toString(count)\n"), "svae", "save"),
    ] {
        let error = contract::compile(&source).unwrap_err();
        assert!(error.message.ends_with(&format!("; did you mean `{correct}`?")), "{error}");
        contract::compile(&source.replace(typo, correct)).unwrap();
    }
    // Never offer another component's action, a shadowed action, or an
    // ambiguous correction; a generated spelling cannot be written. Global
    // functions are not handlers.
    for source in [
        format!("component App\n  view\n    button \"Save\" press=svae\n    Row()\ncomponent Row\n{action}  view\n    text toString(count)\n"),
        format!("component App\n{action}  view\n    Row()\ncomponent Row\n  view\n    button \"Save\" press=svae\n"),
        format!("component App\n{action}  action sale\n    count = 1\n  view\n    button \"Save\" press=sace\n"),
        format!("component App\n{action}  resource items = items() as shape list<number>\n  view\n    each save in items key=toString(save)\n      button \"Save\" press=svae\n"),
        format!("component App\n{action}  state chosen = some(1)\n  view\n    match chosen\n      case some(save)\n        button \"Save\" press=svae\n      case none\n        text \"None\"\n"),
        "fn save(): number = 1\ncomponent App\n  view\n    button \"Save\" press=svae()\n".into(),
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
    let action = "  state count = 0\n  action save\n    count = count + 1\n";
    for (source, typo) in [
        (format!("component App\n{action}  view\n    Row(commit=svae)\ncomponent Row\n  props\n    commit: action\n  view\n    Leaf(submit=commit)\ncomponent Leaf\n  props\n    submit: action\n  view\n    button \"Save\" press=submit\n"), "svae"),
        (format!("component App\n{action}  provide\n    commit = svae\n  view\n    Row()\ncomponent Row\n  view\n    Leaf()\ncomponent Leaf\n  inject\n    commit: action\n  view\n    button \"Save\" press=commit()\n"), "svae"),
        ("component App\n  state count = 0\n  action save(value: number)\n    count = value\n  view\n    Row(commit=svae)\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=commit(1)\n".into(), "svae"),
        (format!("component App\n{action}  view\n    Row(sace=sace)\ncomponent Row\n  props\n    sace: action\n  state n = 0\n  action sale\n    n = 1\n  view\n    button \"Save\" press=sace\n"), "sace"),
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
    let source = "component App\n  view\n    First()\n    Second()\ncomponent First\n  state n = 0\n  action save\n    n = 1\n  view\n    Leaf(submit=sace)\ncomponent Second\n  state n = 0\n  action sale\n    n = 1\n  view\n    Leaf(submit=sace)\ncomponent Leaf\n  props\n    submit: action\n  view\n    button \"Go\" press=submit\n";
    // Each child's call site is checked in its own scope, so each gets the
    // correction its own actions support, first `First`'s, then `Second`'s.
    let error = contract::compile(source).unwrap_err();
    assert_eq!(error.id, "type-unknown-name");
    assert_eq!(error.span.line, 10);
    assert!(error.message.ends_with("; did you mean `save`?"), "{error}");
    assert!(error.related.is_empty());
    let source = source.replacen("submit=sace", "submit=save", 1);
    let error = contract::compile(&source).unwrap_err();
    assert_eq!(error.span.line, 16);
    assert!(error.message.ends_with("; did you mean `sale`?"), "{error}");
    // Repaired, only the fixture's two-node root remains to refuse.
    let error = contract::compile(&source.replace("submit=sace", "submit=sale")).unwrap_err();
    assert_eq!(error.id, "lower-one-root");
}

#[test]
fn action_hints_respect_call_intrinsics_and_function_precedence() {
    for (name, typo, global) in [
        ("pending", "pendign", ""),
        ("failed", "fialed", ""),
        ("path", "paht", ""),
        ("save", "svae", "fn save(): number = 1\n"),
    ] {
        let declarations = format!(
            "{global}component App\n  state count = 0\n  action {name}\n    count = count + 1\n"
        );
        for view in [
            format!("  view\n    button \"Save\" press={typo}()\n"),
            format!("  view\n    Row(commit={typo})\ncomponent Row\n  props\n    commit: action\n  view\n    button \"Save\" press=commit()\n"),
            format!("  provide\n    commit = {typo}\n  view\n    Row()\ncomponent Row\n  inject\n    commit: action\n  view\n    button \"Save\" press=commit()\n"),
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

#[test]
fn path_guidance_uses_the_route_table_and_original_imported_call_location() {
    let app = App::new("path-guidance");
    let root = app.write("app.contract", "use Row from \"./lib/row.contract\"\nroutes nav\n  home \"/\"\n    post \"/post/:id\"\n    archive \"/archive/:year/:id\"\n  notfound\ncomponent App\n  view\n    column\n      Row(id=\"a\")\n      Row(id=\"b\")\n");
    for (expr, message, fixed) in [
        (
            "path(\"psot\", id)",
            "unknown path route `psot`; available routes: `home`, `post`, `archive`",
            "path(\"post\", id)",
        ),
        (
            "path(\"notfound\")",
            "unknown path route `notfound`; available routes: `home`, `post`, `archive`",
            "path(\"home\")",
        ),
        (
            "path(\"post\")",
            "`post` expects 1 path parameter (`id`), received 0",
            "path(\"post\", id)",
        ),
        (
            "path(\"post\", 2026, id)",
            "`post` expects 1 path parameter (`id`), received 2",
            "path(\"post\", id)",
        ),
        (
            "path(\"archive\", id)",
            "`archive` expects 2 path parameters (`year`, `id`), received 1",
            "path(\"archive\", 2026, id)",
        ),
        (
            "path(\"home\", id)",
            "`home` expects 0 path parameters, received 1",
            "path(\"home\")",
        ),
    ] {
        let source =
            format!("component Row\n  props\n    id: string\n  view\n    text `é ${{{expr}}}`\n");
        let path = app
            .write("lib/row.contract", &source)
            .canonicalize()
            .unwrap();
        let error = contract::compile_path(&root).unwrap_err();
        assert_eq!(error.id, "route-unknown");
        assert_eq!(error.message, message);
        let errors = diagnostics(
            &app.run(&[root.to_str().unwrap(), "--json", "-o", "refused.plan"]),
            1,
        );
        same_error(&errors[0], &error);
        assert_eq!(errors[0]["file"], path.to_str().unwrap());
        assert_eq!(errors[0]["line"], 5);
        let col = source.lines().last().unwrap().find("path").unwrap() + 1;
        assert_eq!(errors[0]["col"], col);
        assert_eq!(errors[0]["end_col"], col + 4);
        assert!(!app.0.join("refused.plan").exists());
        let human = app.run(&[root.to_str().unwrap()]);
        assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        app.write("lib/row.contract", &source.replace(expr, fixed));
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
}

#[test]
fn path_guidance_keeps_missing_table_literal_name_and_segment_errors_distinct() {
    let base = "routes nav\n  home \"/\"\n    post \"/post/:id\"\ncomponent App\n  state routeName = \"home\"\n  view\n    text EXPR\n";
    for expr in ["path()", "path(routeName)", "path(1)"] {
        let error = contract::compile(&base.replace("EXPR", expr)).unwrap_err();
        assert_eq!(error.id, "route-unknown");
        assert_eq!(
            error.message,
            "`path()` needs a string-literal route name as its first argument"
        );
    }
    let missing =
        contract::compile("component App\n  view\n    text path(\"home\")\n").unwrap_err();
    assert_eq!(missing.id, "route-unknown");
    assert_eq!(missing.message, "declare `routes` to use `path()`");
    for value in ["", ".", ".."] {
        let expr = format!("path(\"post\", \"{value}\")");
        let error = contract::compile(&base.replace("EXPR", &expr)).unwrap_err();
        assert_eq!(error.id, "route-unknown");
        assert_eq!(
            error.message,
            "a path parameter cannot be empty, `.` or `..`"
        );
    }
    let bad_type = contract::compile(&base.replace("EXPR", "path(\"post\", true)")).unwrap_err();
    assert_eq!(bad_type.id, "type-argument");
    assert_eq!(
        bad_type.message,
        "`path()` expects a string or number, given `bool`"
    );
    assert!(contract::compile(
        "fn path(name: string): string = name\ncomponent App\n  view\n    text path(\"home\")\n"
    )
    .is_ok());
}

#[test]
fn function_calls_report_ordered_types_and_argument_positions_at_imported_sites() {
    let app = App::new("call-signatures");
    let root = app.write(
        "app.contract",
        "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    Row()\n",
    );
    app.write(
        "lib/fns.contract",
        "fn mix(n: number, s: string, b: bool): string = b ? s : \"\"\nfn hello(): string = \"ok\"\nfn sizes(values: option<list<number>>): number = 0\nfn path(n: number): string = toString(n)\n",
    );
    let imports = "use mix from \"./fns.contract\"\nuse hello from \"./fns.contract\"\nuse sizes from \"./fns.contract\"\nuse path from \"./fns.contract\"\n";
    for (expr, id, message, marker, fixed) in [
        (
            "mix()",
            "type-arity",
            "`mix` takes 3 argument(s), given 0; expected `mix(number, string, bool)`",
            "mix",
            "mix(1, \"ok\", true)",
        ),
        (
            "mix(unknown)",
            "type-arity",
            "`mix` takes 3 argument(s), given 1; expected `mix(number, string, bool)`",
            "mix",
            "mix(1, \"ok\", true)",
        ),
        (
            "mix(1, \"ok\", true, false)",
            "type-arity",
            "`mix` takes 3 argument(s), given 4; expected `mix(number, string, bool)`",
            "mix",
            "mix(1, \"ok\", true)",
        ),
        (
            "hello(1)",
            "type-arity",
            "`hello` takes 0 argument(s), given 1; expected `hello()`",
            "hello",
            "hello()",
        ),
        (
            "sizes()",
            "type-arity",
            "`sizes` takes 1 argument(s), given 0; expected `sizes(option<list<number>>)`",
            "sizes",
            "sizes(none)",
        ),
        (
            "path()",
            "type-arity",
            "`path` takes 1 argument(s), given 0; expected `path(number)`",
            "path",
            "path(1)",
        ),
        (
            "max(1)",
            "type-arity",
            "`max` takes 2 argument(s), given 1; expected `max(number, number)`",
            "max",
            "max(1, 2)",
        ),
        (
            "length()",
            "type-arity",
            "`length` takes 1 argument(s), given 0; expected `length(string | list)`",
            "length",
            "length(\"abc\")",
        ),
        (
            "mix(1, false, true)",
            "type-argument",
            "argument 2 of `mix` expects `string`, given `bool`",
            "false",
            "mix(1, \"ok\", true)",
        ),
        (
            "mix(\"bad\", unknown, true)",
            "type-argument",
            "argument 1 of `mix` expects `number`, given `string`",
            "\"bad\"",
            "mix(1, \"ok\", true)",
        ),
        (
            "max(1, \"bad\")",
            "type-argument",
            "argument 2 of `max` expects `number`, given `string`",
            "\"bad\"",
            "max(1, 2)",
        ),
    ] {
        let source = format!("{imports}component Row\n  view\n    text `é ${{{expr}}}`\n");
        let path = app
            .write("lib/row.contract", &source)
            .canonicalize()
            .unwrap();
        let error = contract::compile_path(&root).unwrap_err();
        assert_eq!(error.id, id, "{expr}: {error}");
        assert_eq!(error.message, message, "{expr}");
        let errors = diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 1);
        assert_eq!(errors.len(), 1);
        same_error(&errors[0], &error);
        assert_eq!(errors[0]["file"], path.to_str().unwrap());
        assert_eq!(errors[0]["line"], 7);
        let col = source.lines().last().unwrap().find(marker).unwrap() + 1;
        assert_eq!(errors[0]["col"], col, "{expr}");
        assert_eq!(errors[0]["end_col"], col + marker.len(), "{expr}");
        let human = app.run(&[root.to_str().unwrap()]);
        assert!(String::from_utf8_lossy(&human.stderr).contains(message));
        app.write("lib/row.contract", &source.replace(expr, fixed));
        assert!(diagnostics(&app.run(&[root.to_str().unwrap(), "--json"]), 0).is_empty());
    }
}

#[test]
fn deep_views_are_refused_without_aborting_the_compiler() {
    let app = App::new("view-depth");
    for depth in [256, 400, 800] {
        let mut source = String::from("component App\n  view\n");
        for level in 0..depth {
            source.push_str(&format!("{}column\n", " ".repeat(4 + level * 2)));
        }
        let path = app.write("app.contract", &source);
        let output = app.run(&[path.to_str().unwrap(), "--json", "-o", "out.plan"]);
        if depth == 256 {
            assert!(output.status.success(), "{output:?}");
        } else {
            let errors = diagnostics(&output, 1);
            assert_eq!(errors[0]["id"], "syntax-view-depth");
        }
    }
}

#[test]
fn scroll_payload_arity_is_a_diagnostic() {
    for params in ["x: number", "x: number, y: number"] {
        let source = format!("component App\n  state n = 0\n  action onScroll({params})\n    n = x\n  view\n    scroll scroll=onScroll height=100\n      text \"hi\"\n");
        let result = contract::compile(&source);
        if params.contains(',') {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().id, "analyze-handler-arity");
        }
    }
}

/// LLP 1088 D4, D7.1, D7.4: refusals that name the fix, each asserted whole.
#[test]
fn refusals_from_the_app_diaries_name_the_fix() {
    let refused = |src: &str| {
        let e = contract::compile(src).unwrap_err();
        (e.id, e.message)
    };
    let says = |src: &str, id: &str, message: &str| {
        assert_eq!(refused(src), (id.to_string(), message.to_string()), "{src}");
    };
    // D4 (kanban F5): an initializer's scope is said, not guessed at.
    let reads = "a state's initializer runs before any resource answers, and reads only props, injects and earlier states";
    says(
        "shape Col\n  id: string\nshape Board\n  cols: list<Col>\ncomponent App\n  resource board = loadBoard() as shape Board\n  resource boardX = loadBoard() as shape Board\n  state scrolls = map(board.cols, c => c.id)\n  view\n    text \"a\"\n",
        "type-initializer-scope",
        &format!("`board` is a resource; {reads}. Derive it from `board`, or keep per-row state in a component used inside `each … in board.cols`"),
    );
    says(
        "component App\n  derive two = 2\n  state n = two\n  view\n    text \"a\"\n",
        "type-initializer-scope",
        &format!("`two` is a derive; {reads}. Make `n` a derive too, or start it from a value and write it in an action"),
    );
    says(
        "component App\n  state a = b\n  state b = 1\n  view\n    text \"a\"\n",
        "type-initializer-scope",
        &format!("`b` is declared after `a`; {reads}: declare `b` above `a`"),
    );
    // A row's state is held to the same scope in its component.
    says(
        "component App\n  view\n    Row(n=1)\ncomponent Row\n  props\n    n: number\n  derive twice = n * 2\n  state x = twice\n  view\n    text \"a\"\n",
        "type-initializer-scope",
        &format!("`twice` is a derive; {reads}. Make `x` a derive too, or start it from a value and write it in an action"),
    );
    // D7.1 (pomodoro F1): the declaration that fits, payload named and typed.
    says(
        "shape Task\n  id: string\n  done: bool\ncomponent App\n  resource tasks = loadTasks() as shape list<Task>\n  action flipTask(id: string)\n    refresh tasks\n  view\n    column\n      each t in tasks key=t.id\n        input type=\"checkbox\" checked=t.done change=flipTask(t.id)\n",
        "analyze-handler-arity",
        "`change=flipTask(t.id)` calls `flipTask` with `t.id` and then the checkbox's new `checked` (bool); declare `action flipTask(id: string, checked: bool)`, or `action flipTask(id: string, checked: bool, event: InputEvent)` for its `InputEvent`",
    );
    says(
        "component App\n  state q = \"\"\n  action setQ\n    q = \"\"\n  view\n    input value=q input=setQ\n",
        "analyze-handler-arity",
        "`input=setQ` calls `setQ` with the field's new `value` (string); declare `action setQ(value: string)`, or `action setQ(value: string, event: InputEvent)` for its `InputEvent`",
    );
    says(
        "component App\n  state n = 0\n  action save(id: string, extra: number)\n    n = 1\n  view\n    button \"s\" press=save(\"a\")\n",
        "analyze-handler-arity",
        "`press=save(\"a\")` calls `save` with `\"a\"` and nothing more; declare `action save(id: string)`, or `action save(id: string, event: MouseEvent)` for its `MouseEvent`",
    );
    // D7.4 (ledger F3, hn-reader F2): the web's spellings, rewritten.
    says(
        "style Card\n  background-color: \"#fff\"\ncomponent App\n  view\n    text \"a\" class=Card\n",
        "syntax-expected-attr",
        "`background-color:` is CSS's declaration; a line of `style Card` is `attr=literal`: write `background-color=\"#fff\"`",
    );
    says(
        "component App\n  state s = \"ab\"\n  view\n    text toString(len(s))\n",
        "type-refused-idiom",
        "write `length(x)`: Contract spells the web's `.length`, of text or of a list, as a roster function",
    );
    let idiom = |call: &str| {
        refused(&format!("component App\n  state s = \"a\"\n  state xs = []\n  action go\n    xs = {call}\n  view\n    text s\n"))
    };
    for (call, says) in [
        ("push(xs, s)", "as in `xs = concat(xs, [x])`"),
        (
            "replace(s, \"a\", \"b\")",
            "write `replaceAll(s, find, with)`",
        ),
        ("substring(s, 1)", "write `slice(s, start, end)`"),
        ("padStart(s, 2, \"0\")", "LLP 1088 D2 defers it"),
        ("toUpperCase(s)", "`text-transform=\"uppercase\"`"),
        ("parseInt(s)", "Contract does not parse numbers from text"),
        ("Number(s)", "Contract does not parse numbers from text"),
    ] {
        let (id, message) = idiom(call);
        assert_eq!(id, "type-refused-idiom", "{call}: {message}");
        assert!(message.contains(says), "{call}: {message}");
    }
}
