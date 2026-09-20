//! Original file/range diagnostics across imports and compiler passes.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("exact-source-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
    fn root(&self) -> PathBuf {
        self.write(
            "app.contract",
            "use Row from \"./lib/row.contract\"\ncomponent App\n  view\n    Row()\n",
        )
    }
    fn refuse(&self, source: &str, pass: &str, token: &str) -> contract::CompileError {
        let row = self
            .write("lib/row.contract", source)
            .canonicalize()
            .unwrap();
        let error = contract::compile_path(&self.root()).unwrap_err();
        assert_eq!(error.pass, pass, "{error}");
        assert_eq!(error.file.as_deref(), Some(row.as_path()), "{error}");
        let (line, text) = source
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains(token))
            .unwrap();
        let col = text.find(token).unwrap() as u32 + 1;
        assert_eq!(
            (error.span.line, error.span.col),
            (line as u32 + 1, col),
            "{error}"
        );
        error
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn imported_type_analyze_and_lower_errors_keep_the_offending_token_range() {
    let app = App::new("passes");
    for (source, pass, token) in [
        (
            "component Row\n  view\n    text missingName\n",
            "types",
            "missingName",
        ),
        (
            "component Row\n  view\n    view align-items=\"middle\"\n",
            "lower",
            "align-items",
        ),
        (
            "component Row\n  state on = true\n  view\n    view align-items=(on ? \"middle\" : \"center\")\n",
            "lower",
            "\"middle\"",
        ),
        (
            "routes nav\n  home \"/\"\ncomponent Row\n  view\n    text \"row\"\n",
            "analyze",
            "routes",
        ),
    ] {
        let error = app.refuse(source, pass, token);
        assert_eq!(error.span.end_col, error.span.col + token.len() as u32);
    }
}

#[test]
fn imported_lexer_parser_and_import_errors_name_their_own_file() {
    let app = App::new("syntax");
    app.refuse(
        "component Row\n  view\n    text \"unterminated\n",
        "syntax",
        "\"unterminated",
    );
    app.refuse("component Row\n  view\n    text )\n", "syntax", ")");
    app.refuse("component Row\n  view\n    text `é ${}`\n", "syntax", "}");
    app.refuse("component Row\n  view\n    text `é ${@}`\n", "syntax", "@");
    app.refuse(
        "use Missing from \"./missing.contract\"\ncomponent Row\n  view\n    text \"row\"\n",
        "use",
        "use",
    );
}

#[test]
fn transitive_imports_and_repeated_template_interpolations_keep_byte_columns() {
    let app = App::new("templates");
    let root = app.write(
        "app.contract",
        "use Outer from \"./lib/outer.contract\"\ncomponent App\n  view\n    Outer()\n",
    );
    app.write(
        "lib/outer.contract",
        "use Row from \"./row.contract\"\ncomponent Outer\n  view\n    Row()\n",
    );
    for line in [
        "    text `é ${1} xx ${missingName}`",
        "    text `é ${`nested ${missingName}`}`",
    ] {
        let row = app
            .write(
                "lib/row.contract",
                &format!("component Row\n  view\n{line}\n"),
            )
            .canonicalize()
            .unwrap();
        let error = contract::compile_path(&root).unwrap_err();
        assert_eq!(error.file.as_deref(), Some(row.as_path()), "{error}");
        assert_eq!(error.span.line, 3);
        let col = line.find("missingName").unwrap() as u32 + 1;
        assert_eq!(
            (error.span.col, error.span.end_col),
            (col, col + 11),
            "{error}"
        );
    }
}

#[test]
fn provided_root_source_is_not_reread_and_root_errors_keep_the_requested_path() {
    let app = App::new("snapshot");
    let root = app.root();
    app.write(
        "lib/row.contract",
        "component Row\n  view\n    text \"row\"\n",
    );
    let snapshot = "component App\n  view\n    text missingSnapshot\n";
    let error = contract::compile_path_source(&root, snapshot).unwrap_err();
    assert_eq!(error.file.as_deref(), Some(root.as_path()));
    assert!(error.message.contains("missingSnapshot"), "{error}");
    let error = contract::compile(snapshot).unwrap_err();
    assert_eq!(error.file, None);
    assert!(!error.to_string().contains("app.contract"));
    let missing = app.0.join("absent.contract");
    let error = contract::compile_path(&missing).unwrap_err();
    assert_eq!(error.file.as_deref(), Some(missing.as_path()));
    assert_eq!(error.to_string().matches("absent.contract").count(), 1);
}

#[test]
fn cli_build_and_types_print_the_imported_filename_once() {
    let app = App::new("cli");
    let root = app.root();
    let row = app
        .write(
            "lib/row.contract",
            "component Row\n  view\n    text missingName\n",
        )
        .canonicalize()
        .unwrap();
    for subcommand in ["build", "types"] {
        let result = Command::new(env!("CARGO_BIN_EXE_contract"))
            .arg(subcommand)
            .arg(&root)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1));
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(
            stderr.starts_with(&format!("{}:3:10 [", row.display())),
            "{stderr}"
        );
        assert!(!stderr.contains("app.contract"), "{stderr}");
    }
}

fn diamond(app: &App) -> PathBuf {
    app.write("row.contract", "component Row\n  view\n    text \"row\"\n");
    app.write(
        "left.contract",
        "use Row from \"./row.contract\"\ncomponent Left\n  view\n    Row()\n",
    );
    app.write(
        "right.contract",
        "use Row from \"./row.contract\"\ncomponent Right\n  view\n    Row()\n",
    );
    app.write("app.contract", "use Left from \"./left.contract\"\nuse Right from \"./right.contract\"\ncomponent App\n  view\n    view\n      Left()\n      Right()\n")
}

#[test]
fn diamonds_reuse_files_without_losing_name_checks_or_accepting_cycles() {
    let app = App::new("diamond");
    let root = diamond(&app);
    contract::compile_path(&root).unwrap();
    // This lookup reaches an already completed file, but its requested name is still checked.
    app.write(
        "right.contract",
        "use Absent from \"./row.contract\"\ncomponent Right\n  view\n    text \"right\"\n",
    );
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "contract-use-unknown");
    assert_eq!(
        error.file.as_deref(),
        Some(
            app.0
                .join("right.contract")
                .canonicalize()
                .unwrap()
                .as_path()
        )
    );
    app.write(
        "row.contract",
        "use Left from \"./left.contract\"\ncomponent Row\n  view\n    text \"row\"\n",
    );
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "contract-use-cycle");
    assert_eq!(
        error.file.as_deref(),
        Some(app.0.join("row.contract").canonicalize().unwrap().as_path())
    );
}

#[test]
fn equivalent_duplicate_declarations_do_not_become_conflicts_due_to_file_ids_or_end_columns() {
    let app = App::new("duplicate");
    let root = app.write("app.contract", "use Row from \"./one.contract\"\nuse Row from \"./two.contract\"\ncomponent App\n  view\n    Row()\n");
    app.write("one.contract", "component Row\n  view\n    view width=1\n");
    app.write(
        "two.contract",
        "component Row\n  view\n    view width=1.0\n",
    );
    contract::compile_path(&root).unwrap();
    app.write("two.contract", "component Row\n  view\n    view width=2\n");
    assert_eq!(
        contract::compile_path(&root).unwrap_err().id,
        "contract-use-duplicate"
    );
    // Existing duplicate semantics include the original line/column positions.
    app.write(
        "two.contract",
        "\ncomponent Row\n  view\n    view width=1\n",
    );
    assert_eq!(
        contract::compile_path(&root).unwrap_err().id,
        "contract-use-duplicate"
    );
}

#[cfg(unix)]
#[test]
fn symlink_aliases_share_identity_and_still_detect_cycles() {
    let app = App::new("alias");
    let root = diamond(&app);
    std::os::unix::fs::symlink(Path::new("row.contract"), app.0.join("alias.contract")).unwrap();
    app.write(
        "right.contract",
        "use Row from \"./alias.contract\"\ncomponent Right\n  view\n    Row()\n",
    );
    contract::compile_path(&root).unwrap();
    app.write(
        "row.contract",
        "use Row from \"./alias.contract\"\ncomponent Row\n  view\n    text \"row\"\n",
    );
    assert_eq!(
        contract::compile_path(&root).unwrap_err().id,
        "contract-use-cycle"
    );
}
