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

#[test]
fn a_member_in_another_file_shadows_a_timeline_as_one_written_here_does() {
    let app = App::new("timeline");
    let root = "\
timeline Pending
keyframes p
  to opacity=0
component App
  members \"./more.contract\"
  view
    text \"x\" animation=\"p 1s\" animation-timeline=Pending
";
    app.write(
        "more.contract",
        "members App\n  state Pending = \"--drag\"\n",
    );
    let split = contract::compile_path(&app.write("app.contract", root)).unwrap();
    let flat = root.replace(
        "  members \"./more.contract\"\n",
        "  state Pending = \"--drag\"\n",
    );
    let flat = contract::compile(&flat).unwrap();
    assert_eq!(split.encode(), flat.encode());
}

#[test]
fn a_text_with_no_path_names_no_members_file() {
    let error = contract::compile(SMALL).unwrap_err();
    assert_eq!(error.id, "contract-use-unresolved", "{error}");
    let block = "members App\n  state step = 1\ncomponent App\n  view\n    text \"x\"\n";
    let error = contract::compile(block).unwrap_err();
    assert_eq!(error.id, "contract-members-unlisted", "{error}");
    let (result, _) = contract::compile_warned(SMALL);
    assert_eq!(result.unwrap_err().id, "contract-use-unresolved");
}

#[test]
fn a_param_in_a_members_files_state_is_refused_as_in_the_components_own() {
    let app = App::new("params");
    let root = "\
component App
  params
    fill: color = \"#ffffff\"
  members \"./more.contract\"
  view
    column background-color=\"param.fill\"
      text c
";
    app.write("more.contract", "members App\n  state c = \"param.fill\"\n");
    let error = app.refuse(root);
    assert_eq!(error.id, "tamber-param-scope", "{error}");
    assert_eq!(error.span.source_id, 1, "{error}");
}

#[test]
fn fmt_uses_writes_no_use_back_to_the_file_that_lists_a_members_file() {
    let app = App::new("uses");
    let root = app.write(
        "app.contract",
        "fn helper(): number = 1\ncomponent App\n  members \"./more.contract\"\n  view\n    text `${value}`\n",
    );
    let more = app.write("more.contract", "members App\n  derive value = helper()\n");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["fmt", "--uses"])
        .arg(&root)
        .output()
        .unwrap();
    let written = std::fs::read_to_string(&more).unwrap();
    assert!(
        !written.contains("use helper"),
        "{written}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let error = contract::compile_path(&root).unwrap_err();
    assert_eq!(error.id, "contract-use-missing", "{error}");
}

#[test]
fn an_initializer_sees_another_files_members_as_it_sees_its_own() {
    // A state's initializer sees props, injects and the states before it,
    // not actions: a call there of a name only another file's `fn` and a
    // members file's action declare is the `fn`'s, which needs its `use`.
    let app = App::new("initializer");
    app.write(
        "math.contract",
        "fn one(): number = 1\nfn helper(x: number): number = x\n",
    );
    app.write("more.contract", "members App\n  action helper\n    n = 1\n");
    let error = app.refuse(
        "use one from \"./math.contract\"\ncomponent App\n  state n = helper(one())\n  members \"./more.contract\"\n  view\n    text `${n}`\n",
    );
    assert_eq!(error.id, "contract-use-missing", "{error}");
}

#[test]
fn an_action_named_like_a_method_is_the_components_wherever_it_is_written() {
    let app = App::new("method");
    let root = "\
component App
  state count = 0
  members \"./more.contract\"
  view
    button press=trim(1) aria-label=\"Trim\"
      text count.toString()
";
    app.write(
        "more.contract",
        "members App\n  action trim(n: number)\n    count = n\n",
    );
    let split = contract::compile_path(&app.write("app.contract", root)).unwrap();
    let flat = root.replace(
        "  members \"./more.contract\"\n",
        "  action trim(n: number)\n    count = n\n",
    );
    assert_eq!(split.encode(), contract::compile(&flat).unwrap().encode());

    // A method call the component's action would capture is refused, as in
    // one file.
    let root = root.replace("text count.toString()", "text \" x \".trim()");
    let error = app.refuse(&root);
    assert_eq!(error.id, "syntax-method-shadowed", "{error}");
    assert_eq!(error.span.source_id, 0, "{error}");
    // And a free call of a method's name no file declares, as in one file.
    app.write(
        "more.contract",
        "members App\n  action other\n    count = 1\n",
    );
    let error = app.refuse(&root.replace("text \" x \".trim()", "text \"x\""));
    assert_eq!(error.id, "syntax-method-form", "{error}");
}

#[test]
fn a_member_shares_its_name_with_a_shape_as_in_one_file() {
    let app = App::new("shape");
    let root = "\
shape Item
  value: number
component App
  state Item = 0
  members \"./more.contract\"
  state item = Item(value=1)
  view
    text item.value.toString()
";
    app.write("more.contract", "members App\n  state extra = 0\n");
    let split = contract::compile_path(&app.write("app.contract", root)).unwrap();
    let flat = root.replace("  members \"./more.contract\"\n", "  state extra = 0\n");
    assert_eq!(split.encode(), contract::compile(&flat).unwrap().encode());
}
