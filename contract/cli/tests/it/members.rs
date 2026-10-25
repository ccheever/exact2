//! A component's members across files (LLP 1091.002).

use std::path::PathBuf;

struct App(PathBuf);
impl App {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-members-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, source).unwrap();
        path
    }
    fn refuse(&self, root: &str) -> contract::CompileError {
        contract::compile_path(&self.write("app.contract", root)).unwrap_err()
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

const ROOT: &str = "\
use label from \"./label.contract\"
component App
  state count = 1
  members \"./features/counting.contract\"
  state shown = true
  action reset
    count = 0
    step = 0
  view
    column
      text label(count)
      text `${doubled} ${step}`
      button press=bump aria-label=\"Bump\"
        text \"Bump\"
";

const COUNTING: &str = "\
use label from \"./label.contract\"
members App
  state step = count + 1
  derive doubled = count * 2
  action bump
    count = count + step
    shown = label(count) != \"\"
  action tick
    reset()
  task ticker mount
    every(1000, tick)
";

// The same members written where the line stands, and one `fn` for the two
// files' (a `fn`'s name never reaches the plan, LLP 1091).
const FLAT: &str = "\
fn label(n: number): string = `${n}`
fn label2(n: number): string = `${n}!`
component App
  state count = 1
  state step = count + 1
  derive doubled = count * 2
  action bump
    count = count + step
    shown = label2(count) != \"\"
  action tick
    reset()
  task ticker mount
    every(1000, tick)
  state shown = true
  action reset
    count = 0
    step = 0
  view
    column
      text label(count)
      text `${doubled} ${step}`
      button press=bump aria-label=\"Bump\"
        text \"Bump\"
";

#[test]
fn component_members_across_files_compile_to_the_one_file_plan() {
    let app = App::new("plan");
    app.write("label.contract", "fn label(n: number): string = `${n}`\n");
    // The members file's `label` is its own file's, not the root's (D5).
    app.write(
        "features/label.contract",
        "fn label(n: number): string = `${n}!`\n",
    );
    app.write("features/counting.contract", COUNTING);
    let root = app.write("app.contract", ROOT);
    let split = contract::compile_path(&root).unwrap();
    let flat = contract::compile(FLAT).unwrap();
    assert_eq!(split.encode(), flat.encode());
}

const SMALL: &str = "\
component App
  state count = 1
  members \"./more.contract\"
  view
    text `${count}`
";

#[test]
fn a_members_file_is_its_listing_components_once() {
    let app = App::new("owner");
    let more = app.write("more.contract", "members Other\n  state step = 1\n");
    let error = app.refuse(SMALL);
    assert_eq!(error.id, "contract-members-owner", "{error}");
    assert_eq!(
        error.file.as_deref(),
        Some(more.canonicalize().unwrap().as_path())
    );
    assert!(error.message.contains("`members App`"), "{}", error.message);

    app.write("more.contract", "fn step(): number = 1\n");
    let error = app.refuse(SMALL);
    assert_eq!(error.id, "contract-members-owner", "{error}");
    assert_eq!((error.span.line, error.span.col), (3, 3), "{error}");

    app.write("more.contract", "members App\n  state step = 1\n");
    let twice = SMALL.replace("  view", "  members \"./more.contract\"\n  view");
    let error = app.refuse(&twice);
    assert_eq!(error.id, "contract-members-twice", "{error}");
    assert_eq!(error.span.line, 4, "{error}");

    // Reached by a `use` and listed by no line, its members are no one's.
    app.write(
        "lib.contract",
        "fn one(): number = 1\nmembers App\n  state step = 1\n",
    );
    let error =
        app.refuse("use one from \"./lib.contract\"\ncomponent App\n  view\n    text `${one()}`\n");
    assert_eq!(error.id, "contract-members-unlisted", "{error}");
}

#[test]
fn a_member_declared_in_two_of_a_components_files_names_both() {
    let app = App::new("duplicate");
    app.write(
        "more.contract",
        "members App\n  derive step = 2\n  state count = 2\n",
    );
    let error = app.refuse(SMALL);
    assert_eq!(error.id, "contract-members-duplicate", "{error}");
    assert_eq!(error.span.line, 3, "{error}");
    assert!(
        error.message.contains("app.contract") && error.message.contains("more.contract"),
        "{}",
        error.message
    );
    assert_eq!(error.related.len(), 1);
    assert_eq!(error.related[0].span.line, 2);
}

#[test]
fn a_members_block_holds_members_only() {
    let app = App::new("sections");
    for section in [
        "view\n    text \"x\"",
        "props\n    x: number",
        "members \"./other.contract\"",
    ] {
        app.write(
            "more.contract",
            &format!("members App\n  state step = 1\n  {section}\n"),
        );
        let error = app.refuse(SMALL);
        assert_eq!(error.id, "syntax-members-section", "{section}: {error}");
    }
}

#[test]
fn a_members_file_is_checked_where_it_is_written() {
    let app = App::new("checked");
    // A refusal in a member points at its own file and line.
    let more = app.write(
        "more.contract",
        "members App\n  action bump\n    count = \"many\"\n",
    );
    let error = app.refuse(SMALL);
    assert_eq!(
        error.file.as_deref(),
        Some(more.canonicalize().unwrap().as_path()),
        "{error}"
    );
    assert_eq!(error.span.line, 3, "{error}");

    // A child's members file sees its props and is held to a child's rules.
    let root = "\
component App
  view
    Row(start=1)
component Row
  props
    start: number
  members \"./row.contract\"
  view
    text `${shown}`
";
    let row = app.write("row.contract", "members Row\n  state shown = start + 1\n");
    contract::compile_path(&app.write("app.contract", root)).unwrap();
    app.write(
        "row.contract",
        "shape Item\n  name: string\nmembers Row\n  state shown = start + 1\n  resource items = loadItems() as shape Item[]\n",
    );
    let error = app.refuse(root);
    assert_eq!(error.id, "type-child-resource", "{error}");
    assert_eq!(
        error.file.as_deref(),
        Some(row.canonicalize().unwrap().as_path()),
        "{error}"
    );
}

#[test]
fn a_members_file_formats_as_a_component_does() {
    let src = "members App\n  state picked = 0\n  action pick(x: option<string>)\n    picked = 1\n";
    let formatted = contract_syntax::fmt::format(src).unwrap();
    assert_eq!(formatted, src);
    let root = "component App\n  members \"./more.contract\"\n  view\n    text \"x\"\n";
    assert_eq!(contract_syntax::fmt::format(root).unwrap(), root);
}
