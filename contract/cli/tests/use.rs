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
    let mut r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
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
    let r = Runner::boot(plan, Stations, Kernel::with_monospace()).unwrap();
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
fn a_name_declared_differently_in_both_files_is_a_duplicate() {
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
    assert_eq!(e.id, "contract-use-duplicate", "{e}");
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
            format!("use Outside from \"{path}\"\ncomponent App\n  view\n    Outside()\n"),
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
