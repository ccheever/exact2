//! `data-*` (LLP 1075.003 §3.3, Q2): the app's own words on any element,
//! one `dataset` row of sorted strings; a module tag's words are its
//! dataset, not module props; a word a host writes, or one HTML does not
//! spell, is refused; and a word `app.json` does not declare fails the bake.

use exact_kernel::{Kernel, NodeType, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};
use std::path::PathBuf;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn dataset(r: &Runner<NoData>, test_id: &str) -> Option<String> {
    let key = r.kernel().find_by_test_id(test_id)[0];
    let node = r.kernel().node_by_key(key).unwrap();
    node.props.str(PropId::Dataset).map(str::to_owned)
}

#[test]
fn words_lower_to_one_sorted_row_that_follows_state() {
    let src = r#"component A
  state count = 1
  state wide = false
  action bump
    count = count + 1
    wide = true
  view
    column testId="route" data-trailing="compose" data-count=count data-wide=wide data-mode=(wide ? some("wide") : none)
      button testId="bump" press=bump
        text "bump"
      text "plain" testId="plain"
"#;
    let mut r = boot(src);
    assert_eq!(
        dataset(&r, "route").as_deref(),
        Some(r#"{"count":"1","trailing":"compose","wide":"false"}"#),
        "an absent option drops its word"
    );
    assert_eq!(dataset(&r, "plain"), None, "no words, no row");
    let key = r.kernel().find_by_test_id("bump")[0];
    let id = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(id, Event::Press).unwrap();
    assert_eq!(
        dataset(&r, "route").as_deref(),
        Some(r#"{"count":"2","mode":"wide","trailing":"compose","wide":"true"}"#)
    );
}

#[test]
fn a_module_tags_words_are_its_dataset_not_its_props() {
    let r = boot("component A\n  view\n    ghostty-terminal testId=\"term\" cwd=\"/tmp\" data-title=\"Shell\"\n");
    let key = r.kernel().find_by_test_id("term")[0];
    let node = r.kernel().node_by_key(key).unwrap();
    assert_eq!(node.node_type, NodeType::NativeView);
    assert_eq!(
        node.props.str(PropId::NativeViewProps),
        Some(r#"{"cwd":"/tmp"}"#)
    );
    assert_eq!(
        node.props.str(PropId::Dataset),
        Some(r#"{"title":"Shell"}"#)
    );
}

#[test]
fn a_word_a_host_writes_or_html_cannot_spell_is_refused() {
    for (attr, id, says) in [
        (
            "data-testid=\"x\"",
            "lower-data-word",
            "written by the web host",
        ),
        (
            "data-scrollstart=\"x\"",
            "lower-data-word",
            "written by the web host",
        ),
        (
            "data-liststyle=\"x\"",
            "lower-data-word",
            "written by the web host",
        ),
        (
            "data-exact-id=\"x\"",
            "lower-data-word",
            "written by the web host",
        ),
        (
            "data-Title=\"x\"",
            "lower-data-word",
            "not a data attribute name",
        ),
        ("data-list=[]", "lower-data-value", "string, number or bool"),
    ] {
        let src = format!("component A\n  view\n    column {attr}\n");
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, id, "{attr}: {e}");
        assert!(e.message.contains(says), "{attr}: {e}");
    }
    let e = contract::compile("component A\n  view\n    head data-title=\"x\"\n").unwrap_err();
    assert_eq!(e.id, "lower-attr-tag", "{e}");
}

struct App(PathBuf);
impl App {
    fn new(name: &str, data: &str, view: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exact-dataset-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("app.json"),
            format!(r#"{{"name":"D","app":{{"id":"com.example.d","name":"D"}}{data}}}"#),
        )
        .unwrap();
        std::fs::write(
            path.join("app.contract"),
            format!("component App\n  view\n    {view}\n"),
        )
        .unwrap();
        Self(path)
    }
    fn compile(&self) -> Result<(), Vec<contract::CompileError>> {
        contract::compile_path_all(&self.0.join("app.contract"), false).map(|_| ())
    }
}
impl Drop for App {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn a_word_the_manifest_does_not_declare_fails_the_bake_by_name() {
    let ok = App::new(
        "ok",
        r#","data":["title","trailing"]"#,
        r#"column data-title="Inbox" data-trailing="compose""#,
    );
    ok.compile().unwrap_or_else(|e| panic!("{e:?}"));
    let typo = App::new(
        "typo",
        r#","data":["title","trailing"]"#,
        r#"column data-trailng="compose""#,
    );
    let errors = typo.compile().unwrap_err();
    assert_eq!(errors[0].id, "bake-undeclared-data", "{errors:?}");
    assert!(
        errors[0].message.contains("`data-trailng` is not declared"),
        "{}",
        errors[0].message
    );
    assert!(
        errors[0].message.contains("did you mean `data-trailing`?"),
        "{}",
        errors[0].message
    );
    let none = App::new("none", "", r#"column data-title="Inbox""#);
    let errors = none.compile().unwrap_err();
    assert_eq!(errors[0].id, "bake-undeclared-data");
    assert!(
        errors[0].message.contains("declares no data-* words"),
        "{}",
        errors[0].message
    );
    let bad = App::new(
        "bad",
        r#","data":["testid"]"#,
        r#"column data-title="Inbox""#,
    );
    let errors = bad.compile().unwrap_err();
    assert_eq!(errors[0].id, "app-manifest", "{errors:?}");
    // Each word is one Swift key: a word listed twice would be two.
    let twice = App::new(
        "twice",
        r#","data":["title","title"]"#,
        r#"column data-title="Inbox""#,
    );
    let errors = twice.compile().unwrap_err();
    assert!(errors[0].message.contains("listed twice"), "{errors:?}");
}

/// LLP 1075.003.000 §3.1: `hook` is one literal word on a node that is not a
/// module view, lowered to its own row.
#[test]
fn a_hook_is_one_literal_word_on_a_node_lowered_to_its_row() {
    let r = boot("component A\n  view\n    column\n      column testId=\"a\" hook=\"avatar\" data-size=\"small\"\n      text \"x\" testId=\"t\"\n");
    let row = |id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        let node = r.kernel().node_by_key(key).unwrap();
        node.props.str(PropId::Hook).map(str::to_owned)
    };
    assert_eq!(row("a").as_deref(), Some("avatar"));
    assert_eq!(row("t"), None);
    assert_eq!(dataset(&r, "a").as_deref(), Some(r#"{"size":"small"}"#));
    for (tag, attr, id) in [
        ("column", "hook=name", "lower-hook-value"),
        ("column", "hook=\"Avatar\"", "lower-hook-word"),
        ("ghostty-terminal", "hook=\"term\"", "lower-hook-module"),
    ] {
        let src = format!("component A\n  state name = \"a\"\n  view\n    {tag} {attr}\n");
        let e = contract::compile(&src).unwrap_err();
        assert_eq!(e.id, id, "{tag} {attr}: {e}");
    }
}

#[test]
fn a_hook_word_the_manifest_does_not_declare_fails_the_bake_by_name() {
    let ok = App::new(
        "hooked",
        r#","hooks":["avatar"]"#,
        r#"column hook="avatar""#,
    );
    ok.compile().unwrap_or_else(|e| panic!("{e:?}"));
    let typo = App::new(
        "hooktypo",
        r#","hooks":["avatar"]"#,
        r#"column hook="avatr""#,
    );
    let errors = typo.compile().unwrap_err();
    assert_eq!(errors[0].id, "bake-undeclared-hook", "{errors:?}");
    assert!(
        errors[0]
            .message
            .contains("did you mean `hook=\"avatar\"`?"),
        "{}",
        errors[0].message
    );
    let none = App::new("hooknone", "", r#"column hook="avatar""#);
    let errors = none.compile().unwrap_err();
    assert!(
        errors[0].message.contains("declares no hook words"),
        "{errors:?}"
    );
}
