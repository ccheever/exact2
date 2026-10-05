//! LLP 1017 P4a/P4b: `provide`/`inject` and `slot`/`children`, proven on
//! the kernel after boot; `provide` is a component section (LLP
//! 1035.005.000 D9).

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
fn a_provide_section_fills_an_inject_and_the_nearest_component_wins() {
    let r = boot("provide.contract");
    let k = r.kernel();
    let color_of = |id: &str| {
        let key = k.find_by_test_id(id)[0];
        k.node_by_key(key).unwrap().style.text_color
    };
    // LLP 1035.005.000 D9: a section covers its component's whole view, an
    // inner component's overrides an outer one's, and a slot's fill keeps
    // its caller's context.
    for (id, hex) in [
        ("label-outer", "#112233"),
        ("label-through", "#112233"),
        ("label-inner", "#ff0000"),
        ("label-framed", "#00ff00"),
        ("label-fill", "#ff0000"),
    ] {
        assert_eq!(color_of(id), Color::parse_hex(hex).unwrap().into(), "{id}");
    }
}

#[test]
fn the_nested_provide_form_and_a_twice_provided_name_are_refused() {
    let nested = "component App\n  view\n    column\n      provide accent = \"#fff\"\n        Label()\ncomponent Label\n  inject\n    accent: string\n  view\n    text accent\n";
    let error = contract::compile(nested).unwrap_err();
    assert_eq!(
        (error.id.as_str(), error.span.line, error.span.col),
        ("syntax-provide-in-view", 4, 7)
    );
    assert!(
        error
            .message
            .contains("write `provide` beside `props` and `inject`, with `accent = …`"),
        "{error}"
    );
    let twice =
        "component App\n  provide\n    accent = \"#fff\"\n    accent\n  view\n    text \"a\"\n";
    let error = contract::compile(twice).unwrap_err();
    assert_eq!(
        (error.id.as_str(), error.span.line, error.span.col),
        ("syntax-duplicate-declaration", 4, 5)
    );
}

#[test]
fn a_provided_value_may_be_state_and_follows_it() {
    let src = "component App\n  state ink = \"#112233\"\n  action paint\n    ink = \"#00ff00\"\n  provide\n    accent = ink\n  view\n    column testId=\"root\"\n      Label(text=\"x\")\ncomponent Label\n  props\n    text: string\n  inject\n    accent: string\n  view\n    text text color=accent testId=`label-${text}`\n";
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

#[test]
fn a_declaration_continued_on_an_indented_line_is_told_to_join_it() {
    let src = "shape Task\n  id: string\ncomponent App\n  resource tasks = loadTasks() as shape list<Task>\n    else empty()\n  view\n    text \"a\"\n";
    let error = contract::compile(src).unwrap_err();
    assert_eq!(error.id, "syntax-expected-section");
    assert_eq!(
        error.message,
        "`else …` is indented under the line above, and a declaration is one line: join them (like `resource tasks = loadTasks() as shape list<Task> else empty()`)"
    );
    // An over-indented section is not a continuation: the plain message stands.
    let src = "component App\n  state n = 0\n    view\n      text \"a\"\n";
    let error = contract::compile(src).unwrap_err();
    assert_eq!(error.message, "expected a section, found an indented block");
}

#[test]
fn failed_names_why_a_mutation_is_not_its_argument() {
    let src = "shape Saved\n  ok: bool\ncomponent App\n  mutation save as shape Saved\n  view\n    text failed(save) ? \"x\" : \"y\"\n";
    let error = contract::compile(src).unwrap_err();
    assert_eq!(error.id, "type-failed-argument");
    assert!(error.message.starts_with("`save` is a mutation, and `failed` takes a resource: a mutation whose request fails without an answer keeps its previous value"), "{error}");
}

#[test]
fn an_annotated_state_or_derive_is_told_its_type_is_inferred() {
    let state = "shape Task\n  id: string\ncomponent App\n  state deleted: option<Task> = none\n  view\n    text \"a\"\n";
    let error = contract::compile(state).unwrap_err();
    assert_eq!(
        (error.id.as_str(), error.message.as_str()),
        ("syntax-expected", "a state's type is inferred, so `state deleted` takes no `: type`: write `state deleted = …`; an empty start is `none` or `[]`, and the writes give it its type")
    );
    let derive = "component App\n  derive n: number = 2\n  view\n    text \"a\"\n";
    let error = contract::compile(derive).unwrap_err();
    assert_eq!(
        error.message,
        "a derive's type is inferred, so `derive n` takes no `: type`: write `derive n = …`, which is its expression's"
    );
}

#[test]
fn an_html_label_names_the_text_and_the_field_name() {
    let src = "component App\n  view\n    label \"Name\"\n";
    let error = contract::compile(src).unwrap_err();
    assert!(
        error
            .message
            .contains("a label is `text` beside its field, and the field is named by `aria-label`"),
        "{error}"
    );
}

#[test]
fn a_view_if_and_a_state_as_name_the_contract_form() {
    let view_if = "component App\n  state on = false\n  view\n    column\n      if on\n        text \"a\"\n      else\n        text \"b\"\n";
    let error = contract::compile(view_if).unwrap_err();
    assert_eq!(
        (error.id.as_str(), error.message.as_str()),
        ("syntax-stray-keyword", "`if` is an action's statement: a view chooses with `when <condition>`, and an `else` under it")
    );
    let state_as =
        "component App\n  state draft = none as option<string>\n  view\n    text \"a\"\n";
    let error = contract::compile(state_as).unwrap_err();
    assert_eq!(
        (error.id.as_str(), error.message.as_str()),
        ("syntax-expected-newline", "a state takes no `as`: its type is inferred from its initializer and the writes to it (a `none` takes its type from a write such as `draft = some(…)`)")
    );
    let derive_as = "component App\n  derive n = 2 as number\n  view\n    text \"a\"\n";
    let error = contract::compile(derive_as).unwrap_err();
    assert_eq!(
        error.message,
        "a derive takes no `as`: its type is inferred from its expression"
    );
}

#[test]
fn position_fixed_says_how_to_pin_a_box() {
    let src =
        "component App\n  view\n    column\n      text \"toast\" position=\"fixed\" bottom=0\n";
    let error = contract::compile(src).unwrap_err();
    assert!(
        error
            .message
            .ends_with("; `fixed` is not a row (LLP 1001): pin a box to the viewport with `absolute` in a root that does not scroll"),
        "{error}"
    );
}
