//! LLP 1017 P8: `use Name from "./file.contract"`, resolved by path.

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
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
        "use Button from \"./ui.contract\"\nshape Item\n  s: string\nfn clamp(x: number): number = x + 1\nstyle Pad\n  padding-top=20\nkeyframes pulse\n  to opacity=1\ncomponent App\n  view\n    column\n      Button()\n      Card()\ncomponent Card\n  view\n    column testId=\"app-card\" class=Pad width=clamp(1) animation=`pulse ${length(Item(s=\"x\").s)}s`\n",
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
