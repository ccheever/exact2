//! LLP 1017 P4a/P4b: `provide`/`inject` and `slot`/`children`, proven on
//! the kernel after boot.

use exact_kernel::{Color, Kernel, PropValue};
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Runner};
use std::path::Path;

#[derive(Default)]
struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn boot(name: &str) -> Runner<NoData> {
    let plan = contract::compile(&corpus(name)).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

#[test]
fn a_slot_takes_the_nodes_under_a_use_in_the_use_sites_scope() {
    let r = boot("slot.contract");
    let k = r.kernel();
    let body = k.find_by_test_id("body")[0];
    let body = k.node_by_key(body).unwrap();
    let text = body.props.iter().find_map(|(id, v)| match v {
        PropValue::Str(s) if id.name() == "text" => Some(s.clone()),
        _ => None,
    });
    assert_eq!(text.as_deref(), Some("2 trains"));
    // The body sits inside the shell's content column, under its title.
    let content = k.find_by_test_id("content-Stations")[0];
    let content = k.node_by_key(content).unwrap();
    assert_eq!(content.children(), vec![body.id]);
    assert_eq!(k.find_by_test_id("title-Stations").len(), 1);
    // An empty fill is fine.
    let empty = k.find_by_test_id("content-Empty")[0];
    assert!(k.node_by_key(empty).unwrap().children().is_empty());
}

#[test]
fn a_provide_fills_an_inject_and_the_innermost_wins() {
    let r = boot("provide.contract");
    let k = r.kernel();
    let color_of = |id: &str| {
        let key = k.find_by_test_id(id)[0];
        k.node_by_key(key).unwrap().style.text_color
    };
    assert_eq!(
        color_of("label-outer"),
        Color::parse_hex("#112233").unwrap().into()
    );
    assert_eq!(
        color_of("label-inner"),
        Color::parse_hex("#ff0000").unwrap().into()
    );
}

#[test]
fn a_provided_value_may_be_state_and_follows_it() {
    let src = "component App\n  state ink = \"#112233\"\n  action paint\n    ink = \"#00ff00\"\n  view\n    column testId=\"root\"\n      provide accent = ink\n        Label(text=\"x\")\ncomponent Label\n  props\n    text: string\n  inject\n    accent: string\n  view\n    text text color=accent testId=`label-${text}`\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("paint", vec![]).unwrap();
    let k = r.kernel();
    let key = k.find_by_test_id("label-x")[0];
    assert_eq!(
        k.node_by_key(key).unwrap().style.text_color,
        Color::parse_hex("#00ff00").unwrap().into()
    );
}

#[test]
fn a_misspelled_or_mistyped_prop_is_refused_where_it_is_written() {
    let source = |args: &str| {
        format!("shape Todo\n  title: string\ncomponent App\n  resource todo = todo() as shape Todo\n  state count = 0\n  action pick\n    count = 1\n  view\n    Row({args})\ncomponent Row\n  props\n    todo: Todo\n    onPick: action\n  view\n    button press=onPick\n      text todo.title\n")
    };
    let error = contract::compile(&source("todo=todo, onPik=pick")).unwrap_err();
    assert_eq!(
        (error.id.as_str(), error.span.line, error.span.col),
        ("type-unknown-prop", 9, 20)
    );
    assert!(
        error.message.ends_with("; did you mean `onPick`?"),
        "{error}"
    );
    let error = contract::compile(&source("todo=\"hello\", onPick=pick")).unwrap_err();
    assert_eq!((error.id.as_str(), error.span.line), ("type-prop", 9));
    assert_eq!(error.message, "`todo` expects `Todo`, given `string`");
    contract::compile(&source("todo=todo, onPick=pick")).unwrap();
}
