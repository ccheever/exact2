//! LLP 1035.005.000 D2: `let` in actions — immutable, sequentially scoped
//! locals, lowered to `BindLocal`/`LoadLocal`/`DropLocal` and proven on the
//! runner over `contract/corpus/let.contract`.

use exact_kernel::{Kernel, Offer, PropId};
use exact_plan::{Opcode, Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

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

fn text(r: &Runner<NoData>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

fn press(r: &mut Runner<NoData>, id: &str) {
    let key = r.kernel().find_by_test_id(id)[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

fn ops(plan: &Plan, action: &str, op: Opcode) -> usize {
    let a = plan
        .actions
        .iter()
        .find(|a| plan.str(a.name) == action)
        .unwrap();
    exact_runner::vm::instructions(plan.code(a.body))
        .map(Result::unwrap)
        .filter(|i| i.op == op)
        .count()
}

#[test]
fn a_local_is_read_by_the_statements_after_it_and_sees_the_starting_state() {
    let plan = contract::compile(&corpus("let.contract")).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    // A geometry read named once is read once.
    assert_eq!(ops(&plan, "read", Opcode::Call), 1);
    assert_eq!(ops(&plan, "read", Opcode::BindLocal), 1);
    assert_eq!(ops(&plan, "read", Opcode::DropLocal), 1);
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    assert_eq!(text(&r, "counts"), "5 0 0");
    // `seen` is read after `count` is written: writes land together, so it
    // is still the starting count.
    press(&mut r, "step");
    assert_eq!(text(&r, "counts"), "6 5 5");
    // Each branch is a block of its own; a name may be declared in both.
    press(&mut r, "small");
    assert_eq!(text(&r, "label"), "small 3");
    press(&mut r, "big");
    assert_eq!(text(&r, "label"), "big 30");
    // A local in a `match` arm reads the binding and an earlier local.
    press(&mut r, "place");
    assert_eq!(text(&r, "spot"), "2,8");
    press(&mut r, "clear");
    assert_eq!(text(&r, "spot"), "0,0");
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
    press(&mut r, "read");
    assert_eq!(text(&r, "room"), "300 600");
}

#[test]
fn a_childs_local_never_captures_what_its_parent_passes_in() {
    let mut r = Runner::boot(
        contract::compile(&corpus("let.contract")).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // `amount` is the root's `count` (5); the child's own `count` is 2.
    press(&mut r, "double");
    assert_eq!(text(&r, "doubled"), "10");
    press(&mut r, "step");
    press(&mut r, "double");
    assert_eq!(text(&r, "doubled"), "12");
    // Lifted into the root, the child's `label` is its own local, not the
    // root's state of that name.
    press(&mut r, "small");
    press(&mut r, "say");
    assert_eq!(text(&r, "said"), "child");
    assert_eq!(text(&r, "label"), "small 3");
}

fn refused(body: &str) -> contract::CompileError {
    let src = format!(
        "component App\n  state s = 0\n  action go(p: number)\n{body}  view\n    text \"a\"\n"
    );
    contract::compile(&src).unwrap_err()
}

#[test]
fn a_local_never_shadows_and_is_declared_once_per_block() {
    for (body, what) in [
        ("    let s = 1\n", "a state"),
        ("    let p = 1\n", "a parameter"),
        ("    let go = 1\n", "an action"),
        ("    let a = 1\n    if true\n      let a = 2\n", "a local"),
        (
            "    match some(1)\n      case some(v)\n        let v = 2\n      case none\n        s = 0\n",
            "a local",
        ),
    ] {
        let e = refused(body);
        assert_eq!(e.id, "type-let-shadow", "{body}: {e}");
        assert!(e.message.contains(&format!("is already {what} here")), "{e}");
    }
    let e = refused("    let a = 1\n    let a = 2\n");
    assert_eq!(e.id, "type-let-duplicate", "{e}");
    assert!(e.message.contains("the `let` on line 4"), "{e}");
    assert_eq!((e.span.line, e.span.col), (5, 9));
    // Sibling blocks and blocks after one another each scope their own.
    for body in [
        "    if true\n      let a = 1\n      s = a\n    else\n      let a = 2\n      s = a\n",
        "    if true\n      let a = 1\n      s = a\n    let a = 2\n    s = a\n",
    ] {
        let src = format!(
            "component App\n  state s = 0\n  action go(p: number)\n{body}  view\n    text \"a\"\n"
        );
        contract::compile(&src).unwrap_or_else(|e| panic!("{body}: {e}"));
    }
}

#[test]
fn a_local_is_never_reassigned_or_read_before_its_line_or_outside_its_block() {
    let e = refused("    let a = 1\n    a = 2\n");
    assert_eq!(e.id, "type-let-reassign", "{e}");
    assert!(e.message.contains("`a` is the `let` on line 4"), "{e}");
    assert_eq!((e.span.line, e.span.col), (5, 5));
    let e = refused("    s = a\n    let a = 1\n");
    assert_eq!(e.id, "type-let-before-declaration", "{e}");
    assert!(e.message.contains("before the `let` on line 5"), "{e}");
    assert_eq!((e.span.line, e.span.col), (4, 9));
    // A nested block reads the enclosing block's later `let` too early.
    let e = refused("    if true\n      s = a\n    let a = 1\n");
    assert_eq!(e.id, "type-let-before-declaration", "{e}");
    // A local's own value cannot read it.
    let e = refused("    let a = a + 1\n");
    assert_eq!(e.id, "type-unknown-name", "{e}");
    let e = refused("    if true\n      let a = 1\n    s = a\n");
    assert_eq!(e.id, "type-unknown-name", "{e}");
}

#[test]
fn a_childs_local_is_refused_in_the_childs_terms() {
    let e = contract::compile(
        "component App\n  view\n    Child(n=1)\ncomponent Child\n  props\n    n: number\n  state t = 0\n  action go\n    let n = 2\n    t = n\n  view\n    button press=go\n      text \"go\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-let-shadow", "{e}");
    assert!(e.message.contains("`n` is already a prop here"), "{e}");
    assert_eq!((e.span.line, e.span.col), (9, 9));
}

#[test]
fn let_is_a_name_where_no_name_follows_it() {
    // `let = …` assigns a state named `let`, as `send = …` does.
    let plan = contract::compile(
        "component App\n  state let = 0\n  action go\n    let = let + 1\n  view\n    button press=go testId=\"go\"\n      text `${let}` testId=\"n\"\n",
    )
    .unwrap();
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    press(&mut r, "go");
    assert_eq!(text(&r, "n"), "1");
}
