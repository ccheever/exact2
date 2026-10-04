//! LLP 1017 P2: `if`/`else` and `match` as statements in an action, proven
//! on the runner (LLP 1004 D6: a construct exists at both ends).

use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{DataError, DataSource, Event, Runner};
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

#[test]
fn an_action_branches_on_a_key_and_matches_an_option() {
    let plan = contract::compile(&corpus("branch.contract")).unwrap();
    let plan = contract::bake(plan, NoData).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("typed", vec![Value::str("a")]).unwrap();
    r.act("typed", vec![Value::str("b")]).unwrap();
    assert_eq!(r.slot("query"), Some(&Value::str("ab")));
    assert_eq!(r.slot("submitted"), Some(&Value::Number(0.0)));
    r.act("typed", vec![Value::str("Enter")]).unwrap();
    assert_eq!(r.slot("query"), Some(&Value::str("")));
    assert_eq!(r.slot("submitted"), Some(&Value::Number(1.0)));
    r.act("show", vec![]).unwrap();
    assert_eq!(r.slot("seen"), Some(&Value::str("nothing")));
    r.act("pick", vec![Value::str("mv")]).unwrap();
    r.act("show", vec![]).unwrap();
    assert_eq!(r.slot("seen"), Some(&Value::str("mv")));
}

/// A `key` action claims its key with `preventDefault()`, a host command
/// taking no arguments (docs/contract-grammar.md#keys; minesweeper F7).
#[test]
fn a_key_action_prevents_its_default_by_a_command_without_arguments() {
    let src = |call: &str| {
        format!("component A\n  state n = 0\n  action k(name: string)\n    if name == \"ArrowDown\"\n      n = n + 1\n      {call}\n  view\n    column key=k testId=\"grid\"\n")
    };
    let plan = contract::compile(&src("preventDefault()")).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("k", vec![Value::str("a")]).unwrap();
    assert!(r.take_commands().is_empty());
    r.act("k", vec![Value::str("ArrowDown")]).unwrap();
    assert!(r.take_commands().iter().any(|c| c.name == "preventDefault"));
    let e = contract::compile(&src("preventDefault(1)")).unwrap_err();
    assert!(e.to_string().contains("takes no arguments"), "{e}");
    // Its sibling keeps the key from the ancestors' handlers (files diary F8).
    contract::compile(&src("stopPropagation()")).unwrap();
    let e = contract::compile(&src("stopPropagation(1)")).unwrap_err();
    assert_eq!(e.id, "type-stop-propagation");
}

/// A `key` action taking one more parameter hears the `KeyboardEvent`
/// with its modifiers; one that does not hears the key alone, and the
/// event is typed (chat F2: Enter sends, Shift+Enter is a newline).
#[test]
fn a_key_action_may_take_the_keyboard_event_with_its_modifiers() {
    let src = "component A\n  state seen = \"\"\n  state sent = 0\n  action k(name: string, e: KeyboardEvent)\n    if name == \"Enter\" && !e.shiftKey\n      sent = sent + 1\n      preventDefault()\n    seen = `${e.key} ${e.shiftKey} ${e.ctrlKey} ${e.altKey} ${e.metaKey}`\n  action bare(name: string)\n    seen = name\n  view\n    column key=k testId=\"grid\"\n      textarea key=bare testId=\"area\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let id = |r: &Runner<NoData>, t: &str| {
        let k = r.kernel();
        k.node_by_key(k.find_by_test_id(t)[0]).unwrap().id
    };
    let grid = id(&r, "grid");
    r.dispatch(grid, Event::key("Shift+Enter")).unwrap();
    assert_eq!(r.slot("sent"), Some(&Value::Number(0.0)));
    assert_eq!(
        r.slot("seen"),
        Some(&Value::str("Enter true false false false"))
    );
    r.dispatch(grid, Event::key("Enter")).unwrap();
    assert_eq!(r.slot("sent"), Some(&Value::Number(1.0)));
    assert!(r.take_commands().iter().any(|c| c.name == "preventDefault"));
    r.dispatch(grid, Event::key("Control+Alt+Meta++")).unwrap();
    assert_eq!(r.slot("seen"), Some(&Value::str("+ false true true true")));
    r.dispatch(id(&r, "area"), Event::key("Meta+s")).unwrap();
    assert_eq!(r.slot("seen"), Some(&Value::str("s")));
    let e = contract::compile(&src.replace("e: KeyboardEvent", "e: KeyboardEvent, f: bool"))
        .unwrap_err();
    assert!(e.to_string().contains("key"), "{e}");
    // Another type is no `KeyboardEvent`: the body's field reads are refused.
    contract::compile(&src.replace("e: KeyboardEvent", "e: string")).unwrap_err();
    // A textarea's Enter breaks the line, as HTML's: no implicit `submit`.
    let e = contract::compile(&src.replace("key=bare", "submit=bare(\"x\")")).unwrap_err();
    assert!(e.to_string().contains("has no `submit`"), "{e}");
}

#[test]
fn a_branch_may_nest_and_an_omitted_else_is_fine() {
    let src = "component A\n  state n = 0\n  state s = \"\"\n  action go(k)\n    if k == \"a\"\n      if n > 0\n        s = \"again\"\n      else\n        s = \"first\"\n      n = n + 1\n  view\n    column testId=\"root\"\n      input value=s change=go testId=\"i\"\n";
    let plan = contract::compile(src).unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    r.act("go", vec![Value::str("x")]).unwrap();
    assert_eq!(r.slot("n"), Some(&Value::Number(0.0)));
    r.act("go", vec![Value::str("a")]).unwrap();
    assert_eq!(r.slot("s"), Some(&Value::str("first")));
    r.act("go", vec![Value::str("a")]).unwrap();
    assert_eq!(r.slot("s"), Some(&Value::str("again")));
    assert_eq!(r.slot("n"), Some(&Value::Number(2.0)));
}

#[test]
fn cloned_scopes_keep_shadowing_and_region_depth_independent() {
    use contract_types::{Ref, Scope, Ty};

    fn send_sync<T: Send + Sync>() {}
    send_sync::<Scope>();
    let mut outer = Scope::default();
    let nested = Ty::Option(Box::new(Ty::List(Box::new(Ty::Record("Row".into())))));
    outer.push(vec![("value".into(), Ref::Slot(0), nested.clone())]);
    outer.push_region(Some(("item".into(), Ref::Item(0), Ty::Number)));
    let mut left = outer.clone();
    let mut right = outer.clone();
    left.push(vec![("value".into(), Ref::Param(0), Ty::String)]);
    left.push_region(Some(("bound".into(), Ref::Bound(0), nested.clone())));
    right.push_region(None);
    right.push_region(Some(("item".into(), Ref::Item(0), Ty::Bool)));

    assert_eq!(outer.lookup("value"), Some((Ref::Slot(0), &nested)));
    assert_eq!(left.lookup("value"), Some((Ref::Param(0), &Ty::String)));
    assert_eq!(left.lookup("item"), Some((Ref::Item(1), &Ty::Number)));
    assert_eq!(left.lookup("bound"), Some((Ref::Bound(0), &nested)));
    assert_eq!(right.lookup("item"), Some((Ref::Item(0), &Ty::Bool)));
    assert_eq!(right.region_depth(), 3);
    assert_eq!(outer.region_depth(), 1);
    assert_eq!(outer.lookup("bound"), None);

    outer.pop();
    left.pop();
    left.pop();
    right.pop();
    assert_eq!(outer.lookup("item"), None);
    assert_eq!(left.lookup("item"), Some((Ref::Item(0), &Ty::Number)));
    assert_eq!(right.lookup("item"), Some((Ref::Item(1), &Ty::Number)));
    assert_eq!(left.lookup("value"), Some((Ref::Slot(0), &nested)));
}
