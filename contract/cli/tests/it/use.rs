//! LLP 1017 P8: `use Name from "./file.contract"`, resolved by path.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
#[cfg(unix)]
use std::os::unix::fs::symlink as symlink_file;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Stations;

impl DataSource for Stations {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "stations" => Ok(Value::list(vec![
                Value::record(vec![Value::str("mv"), Value::str("Mountain View")]),
                Value::record(vec![Value::str("pa"), Value::str("Palo Alto")]),
            ])),
            other => Err(DataError::UnknownSource(other.into())),
        }
    }
}

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name)
}

#[test]
fn a_used_component_brings_its_shape_and_style_and_the_using_file_stays_the_root() {
    let plan = contract::compile_path(&corpus("use/app.contract")).unwrap();
    let plan = contract::bake(plan, Stations).unwrap();
    let mut r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    assert_eq!(k.find_by_test_id("station-mv").len(), 1);
    assert_eq!(k.find_by_test_id("station-pa").len(), 1);
    let line = k.find_by_test_id("line-mv")[0];
    assert_eq!(
        k.node_by_key(line).unwrap().style.padding_top,
        exact_kernel::Dimension::Points(10.0)
    );
    r.act("pick", vec![Value::str("pa")]).unwrap();
    assert_eq!(r.slot("picked"), Some(&Value::str("pa")));
}

#[test]
fn provided_root_bytes_keep_their_path_for_uses() {
    let path = corpus("use/app.contract");
    let src = std::fs::read_to_string(&path).unwrap();
    let plan = contract::compile_path_source(&path, &src).unwrap();
    let plan = contract::bake(plan, Stations).unwrap();
    let r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(r.kernel().find_by_test_id("station-mv").len(), 1);
}

#[test]
fn a_use_in_a_text_has_no_path_to_resolve_from() {
    let src = std::fs::read_to_string(corpus("use/app.contract")).unwrap();
    let e = contract::compile(&src).unwrap_err();
    assert_eq!(e.id, "contract-use-unresolved");
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact2-use-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn an_unknown_name_an_unreadable_file_and_a_cycle_are_refused_by_name() {
    let dir = temp_dir("refusals");
    std::fs::write(
        dir.join("row.contract"),
        "component Row\n  props\n    n: number\n  view\n    text `${n}`\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("unknown.contract"),
        "use Column from \"./row.contract\"\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("unknown.contract")).unwrap_err();
    assert_eq!(e.id, "contract-use-unknown", "{e}");
    std::fs::write(
        dir.join("missing.contract"),
        "use Row from \"./nowhere.contract\"\ncomponent A\n  view\n    text \"a\"\n",
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("missing.contract")).unwrap_err();
    assert_eq!(e.id, "contract-use-unreadable", "{e}");
    std::fs::write(
        dir.join("a.contract"),
        "use B from \"./b.contract\"\ncomponent A\n  view\n    B()\ncomponent Root\n  view\n    text \"a\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("b.contract"),
        "use A from \"./a.contract\"\ncomponent B\n  view\n    text \"b\"\n",
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("a.contract")).unwrap_err();
    assert_eq!(e.id, "contract-use-cycle", "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The euxy port: `Port` here and an identical `PortRow` in a used file
/// met `expects PortRow[], given Port[]` alone. A shape is its name; the
/// refusal says so and keeps the shared one, and that edit compiles.
#[test]
fn two_shapes_with_the_same_fields_are_told_a_shape_is_its_name() {
    let dir = temp_dir("nominal");
    std::fs::write(
        dir.join("lib.contract"),
        "shape PortRow\n  name: string\n  on: bool\n\ncomponent PortList\n  props\n    rows: PortRow[]\n  view\n    column\n      for r in rows key=r.name\n        text r.name\n",
    )
    .unwrap();
    let app = |shape: &str, uses: &str| {
        format!("use PortList{uses} from \"./lib.contract\"\n{shape}component App\n  state ports = [{}(name=\"a\", on=true)]\n  view\n    PortList(rows=ports)\n", if shape.is_empty() { "PortRow" } else { "Port" })
    };
    std::fs::write(
        dir.join("app.contract"),
        app("shape Port\n  name: string\n  on: bool\n\n", ""),
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(
        e.message,
        "`rows` expects `PortRow[]`, given `Port[]`; `PortRow` and `Port` have the same fields, but a shape matches only itself, by name: write `PortRow` for `Port`, naming `PortRow` in the `use` of the file that declares it, and delete `shape Port`"
    );
    std::fs::write(dir.join("app.contract"), app("", ", PortRow")).unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap_or_else(|e| panic!("{e}"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_this_file_declares_and_uses_is_refused_and_as_renames_it() {
    let dir = temp_dir("dup");
    std::fs::write(
        dir.join("row.contract"),
        "component Row\n  props\n    n: number\n  view\n    text `${n}`\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Row from \"./row.contract\"\ncomponent App\n  view\n    Row(n=1)\ncomponent Row\n  props\n    n: number\n  view\n    text \"mine\"\n",
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(e.id, "contract-use-shadows", "{e}");
    std::fs::write(
        dir.join("app.contract"),
        "use Row as TheirRow from \"./row.contract\"\ncomponent App\n  view\n    column\n      TheirRow(n=1)\n      Row(n=2)\ncomponent Row\n  props\n    n: number\n  view\n    text \"mine\"\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn uses_are_dot_relative_contained_and_relative_to_the_using_file() {
    let base = temp_dir("containment");
    let app = base.join("app");
    std::fs::create_dir_all(app.join("lib")).unwrap();
    let outside = base.join("outside.contract");
    std::fs::write(
        &outside,
        "component Outside\n  view\n    text \"outside\"\n",
    )
    .unwrap();
    let entry = app.join("app.contract");

    for path in [
        "../outside.contract".to_string(),
        outside.display().to_string(),
        "outside.contract".to_string(),
    ] {
        std::fs::write(
            &entry,
            format!(
                "use Outside from {}\ncomponent App\n  view\n    Outside()\n",
                serde_json::to_string(&path).unwrap()
            ),
        )
        .unwrap();
        let error = contract::compile_path(&entry).unwrap_err();
        assert_eq!(error.id, "contract-use-path", "{path}: {error}");
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, app.join("linked.contract")).unwrap();
        std::fs::write(
            &entry,
            "use Outside from \"./linked.contract\"\ncomponent App\n  view\n    Outside()\n",
        )
        .unwrap();
        let error = contract::compile_path(&entry).unwrap_err();
        assert_eq!(error.id, "contract-use-path", "{error}");

        let text_target = app.join("row.txt");
        std::fs::write(
            &text_target,
            "component Outside\n  view\n    text \"not contract\"\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&text_target, app.join("alias.contract")).unwrap();
        std::fs::write(
            &entry,
            "use Outside from \"./alias.contract\"\ncomponent App\n  view\n    Outside()\n",
        )
        .unwrap();
        let error = contract::compile_path(&entry).unwrap_err();
        assert_eq!(error.id, "contract-use-path", "{error}");
    }

    std::fs::write(
        app.join("lib/row.contract"),
        "component Row\n  view\n    text \"row\"\n",
    )
    .unwrap();
    std::fs::write(
        app.join("lib/outer.contract"),
        "use Row from \"./row.contract\"\ncomponent Outer\n  view\n    Row()\n",
    )
    .unwrap();
    std::fs::write(
        &entry,
        "use Outer from \"./lib/outer.contract\"\ncomponent App\n  view\n    Outer()\n",
    )
    .unwrap();
    contract::compile_path(&entry).unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn two_files_of_the_same_names_load_together_and_each_reads_its_own() {
    // LLP 1091 D1/D4/D5: a shape, a fn, a style, keyframes and a component
    // of one name in each file. The root keeps its names; the library's are
    // renamed where they are written.
    let dir = temp_dir("same-names");
    std::fs::write(
        dir.join("ui.contract"),
        "shape Item\n  n: number\nfn clamp(x: number): number = x + 100\nstyle Pad\n  padding-top=10\nkeyframes pulse\n  to opacity=0\ncomponent Card\n  view\n    column testId=\"ui-card\" class=Pad width=clamp(Item(n=1).n) animation=\"pulse 1s\"\ncomponent Button\n  view\n    Card()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Button from \"./ui.contract\"\nshape Item\n  s: string\nfn clamp(x: number): number = x + 1\nstyle Pad\n  padding-top=20\nkeyframes pulse\n  to opacity=1\ncomponent App\n  view\n    column\n      Button()\n      Card()\ncomponent Card\n  view\n    column testId=\"app-card\" class=Pad width=clamp(1) animation=`pulse ${Item(s=\"x\").s.length}s`\n",
    )
    .unwrap();
    let plan = contract::compile_path(&dir.join("app.contract")).unwrap();
    let text = format!("{plan:?}");
    assert!(text.contains("\"pulse__ui 1s\""), "{text}");
    let plan = contract::bake(plan, Stations).unwrap();
    let r = Runner::boot(
        plan,
        Stations,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let style = |id: &str| {
        k.node_by_key(k.find_by_test_id(id)[0])
            .unwrap()
            .style
            .clone()
    };
    let (ui, app) = (style("ui-card"), style("app-card"));
    assert_eq!(ui.padding_top, exact_kernel::Dimension::Points(10.0));
    assert_eq!(app.padding_top, exact_kernel::Dimension::Points(20.0));
    assert_eq!(ui.width, exact_kernel::Dimension::Points(101.0));
    assert_eq!(app.width, exact_kernel::Dimension::Points(2.0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_reached_only_through_another_file_is_refused_with_its_use() {
    let dir = temp_dir("transitive");
    std::fs::write(
        dir.join("icons.contract"),
        "component Icon\n  view\n    text \"i\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("ui.contract"),
        "use Icon from \"./icons.contract\"\ncomponent Card\n  view\n    Icon()\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("app.contract"),
        "use Card from \"./ui.contract\"\ncomponent App\n  view\n    column\n      Card()\n      Icon()\n",
    )
    .unwrap();
    let e = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    assert_eq!(e.id, "contract-use-missing", "{e}");
    assert!(e.message.contains("`icons.contract`"), "{}", e.message);
    assert_eq!(e.span.line, 6);
    // Named through the file that uses it, it is the same declaration.
    std::fs::write(
        dir.join("app.contract"),
        "use Card, Icon from \"./ui.contract\"\ncomponent App\n  view\n    column\n      Card()\n      Icon()\n",
    )
    .unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// LLP 1091 D1 for an app written before module scope (19 x2apps apps):
/// every file's references to names it does not see are refused in one
/// compile, one refusal a file naming each `use` line it lacks — the one
/// it has for that file, extended, or a new one — and `fix_uses` writes
/// exactly those, after which the program compiles to the bytes the lines
/// written by hand give. What no rule decides is said, never written.
#[test]
fn the_missing_use_lines_are_named_and_written() {
    let dir = temp_dir("missing");
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    write("icons.contract", "component Icon\n  view\n    text \"i\"\n");
    write(
        "shapes.contract",
        "shape Row\n  id: string\nfn twice(n: number): number = n * 2\nstyle Muted\n  opacity=0.5\nkeyframes rise\n  from opacity=0\n  to opacity=1\n",
    );
    write(
        "ui.contract",
        "// The card.\nuse Icon from \"./icons.contract\"\nuse Row from \"./shapes.contract\" // its rows\ncomponent Card\n  props\n    row: Row\n  view\n    text `${twice(1)}` class=Muted animation=\"rise 1s\"\n",
    );
    let app = "// The app.\nuse Card from \"./ui.contract\"\ncomponent App\n  view\n    column\n      Card(row=Row(id=\"a\"))\n      Icon()\n";
    write("app.contract", app);
    let root = dir.join("app.contract");
    let Err(all) = contract::compile_path_all(&root, false) else {
        panic!("compiled")
    };
    assert_eq!(all.len(), 2, "one refusal a file: {all:?}");
    assert!(all.iter().all(|e| e.id == "contract-use-missing"));
    let message = |file: &str| {
        all.iter()
            .find(|e| {
                e.file.as_deref() == Some(dir.join(file).as_path())
                    || e.file.as_deref().is_some_and(|f| f.ends_with(file))
            })
            .unwrap_or_else(|| panic!("no refusal in {file}: {all:?}"))
            .message
            .clone()
    };
    let at_app = message("app.contract");
    assert!(
        at_app.contains("`Row` is a shape or function declared in `shapes.contract`"),
        "{at_app}"
    );
    assert!(
        at_app.contains("a new line: use Row from \"./shapes.contract\""),
        "{at_app}"
    );
    assert!(
        at_app.contains("a new line: use Icon from \"./icons.contract\""),
        "{at_app}"
    );
    assert!(
        at_app.contains("`contract fmt --uses app.contract`"),
        "{at_app}"
    );
    let at_ui = message("ui.contract");
    assert!(
        at_ui.contains("change line 3 to `use Row, twice, Muted, rise from \"./shapes.contract\"`"),
        "{at_ui}"
    );
    let (written, refused) = contract::fix_uses(&root).unwrap();
    assert!(refused.is_empty(), "{refused:?}");
    assert_eq!(written.len(), 2, "{written:?}");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    assert_eq!(
        read("ui.contract"),
        "// The card.\nuse Icon from \"./icons.contract\"\nuse Row, twice, Muted, rise from \"./shapes.contract\" // its rows\ncomponent Card\n  props\n    row: Row\n  view\n    text `${twice(1)}` class=Muted animation=\"rise 1s\"\n"
    );
    assert!(read("app.contract").starts_with("// The app.\nuse Card from \"./ui.contract\"\nuse Row from \"./shapes.contract\"\nuse Icon from \"./icons.contract\"\ncomponent App\n"));
    let fixed = contract::compile_path(&root).unwrap().encode();
    // Nothing more to write, and the bytes are the hand-written lines'.
    assert_eq!(contract::fix_uses(&root).unwrap().0.len(), 0);
    write(
        "app.contract",
        "// The app.\nuse Card from \"./ui.contract\"\nuse Icon from \"./icons.contract\"\nuse Row from \"./shapes.contract\"\ncomponent App\n  view\n    column\n      Card(row=Row(id=\"a\"))\n      Icon()\n",
    );
    assert_eq!(contract::compile_path(&root).unwrap().encode(), fixed);
    // A name two other files declare is the author's to choose; a
    // generated name is written as the declaration's own.
    write("more.contract", "component Icon\n  view\n    text \"j\"\n");
    write(
        "app.contract",
        "use Spare from \"./spare.contract\"\nuse Card from \"./ui.contract\"\ncomponent App\n  view\n    column\n      Card(row=Row(id=\"a\"))\n      Icon()\n      text `${twice__shapes(1)}`\n",
    );
    write("spare.contract", "use Icon from \"./more.contract\"\nfn twice(n: number): number = n\ncomponent Spare\n  view\n    Icon()\n");
    let (written, refused) = contract::fix_uses(&root).unwrap();
    assert_eq!(written.len(), 1, "Row still is: {written:?}");
    let left = &refused[0].message;
    assert!(
        left.contains("by hand: `Icon` is declared in `more.contract` and `icons.contract`"),
        "{left}"
    );
    assert!(
        left.contains(
            "by hand: `twice__shapes` is a name the compiler gave `twice` of `shapes.contract`"
        ),
        "{left}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A root split into files that names none of them (the euxy port): its
/// neighbours are read too, so `fix_uses` writes the lines the root lacks,
/// and what stays unreached (another entry) is named, not passed over.
#[test]
fn a_root_that_names_no_file_has_its_lines_written() {
    let dir = temp_dir("unnamed");
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    write("model.contract", "shape Pattern\n  name: string\n");
    write(
        "ui.contract",
        "component Row\n  props\n    p: Pattern\n  view\n    text p.name\n",
    );
    // Another entry that lacks its own lines, and a link to a file outside
    // the app that lacks them too: candidates at most, never written.
    let widget = "component Widget\n  view\n    Row(p=Pattern(name=\"w\"))\n";
    write("widget.contract", widget);
    write("broken.contract", "component\n");
    let shared_dir = temp_dir("unnamed-shared");
    let shared = "component Shared\n  view\n    Row(p=Pattern(name=\"s\"))\n";
    std::fs::write(shared_dir.join("x.contract"), shared).unwrap();
    symlink_file(shared_dir.join("x.contract"), dir.join("shared.contract")).unwrap();
    write("app.test.contract", "test \"a\"\n  clock settle\n");
    write(
        "app.contract",
        "component App\n  view\n    column\n      Row(p=Pattern(name=\"a\"))\n",
    );
    let root = dir.join("app.contract");
    assert!(contract::compile_path(&root).is_err());
    let (written, refused) = contract::fix_uses(&root).unwrap();
    assert!(refused.is_empty(), "{refused:?}");
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    let app = read("app.contract");
    assert!(
        app.contains("use Pattern from \"./model.contract\"\n")
            && app.contains("use Row from \"./ui.contract\"\n"),
        "{written:?}"
    );
    assert!(read("ui.contract").starts_with("use Pattern from \"./model.contract\"\n"));
    contract::compile_path(&root).unwrap();
    let unreached: Vec<String> = contract::unreached(&root)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(unreached, ["broken.contract", "widget.contract"]);
    assert_eq!(read("widget.contract"), widget);
    assert_eq!(read("broken.contract"), "component\n");
    assert_eq!(
        std::fs::read_to_string(shared_dir.join("x.contract")).unwrap(),
        shared
    );
    assert!(
        written
            .iter()
            .all(|(p, _)| p.starts_with(dir.canonicalize().unwrap())),
        "{written:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&shared_dir);
}

/// Review round 2's case: app → Panel, panel → Widget, widget → Panel and
/// Icon. The Panel/Widget lines would make a cycle and are not written, so
/// the root never reaches widget.contract, and its `use Icon` is not
/// written either: only an edge a fix writes makes a file the root's.
#[test]
fn a_file_reached_only_through_a_refused_cycle_is_not_written() {
    let dir = temp_dir("cycle-reach");
    let write = |name: &str, text: &str| std::fs::write(dir.join(name), text).unwrap();
    write("icon.contract", "component Icon\n  view\n    text \"i\"\n");
    let widget = "component Widget\n  view\n    column\n      Panel()\n      Icon()\n";
    write("widget.contract", widget);
    let panel = "component Panel\n  view\n    Widget()\n";
    write("panel.contract", panel);
    write(
        "app.contract",
        "component App\n  view\n    column\n      Panel()\n",
    );
    let root = dir.join("app.contract");
    let (written, refused) = contract::fix_uses(&root).unwrap();
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    assert_eq!(read("widget.contract"), widget, "{written:?}");
    assert_eq!(read("panel.contract"), panel, "{written:?}");
    assert_eq!(
        read("icon.contract"),
        "component Icon\n  view\n    text \"i\"\n"
    );
    assert!(
        read("app.contract").starts_with("use Panel from \"./panel.contract\"\n"),
        "{written:?}"
    );
    assert!(
        refused
            .iter()
            .any(|e| e.message.contains("would make a cycle")),
        "{refused:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
