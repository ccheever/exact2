//! LLP 1035.005.000 D3: records built in expressions, `Shape(field=…)` and
//! `Shape(base, field=…)`, lowered to the plan's `Record` and proven on the
//! runner over `contract/corpus/records.contract`.

use exact_kernel::{Kernel, PropId};
use exact_plan::{Opcode, Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

/// Answers `echo(x)` with `x`: a record passed as a source's argument
/// arrives as the record it was built as.
struct Echo;

impl DataSource for Echo {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match (source, args) {
            ("echo", [x]) => Ok(x.clone()),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

fn corpus(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../corpus")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn text(r: &Runner<Echo>, id: &str) -> String {
    let k = r.kernel();
    let key = k.find_by_test_id(id)[0];
    k.node_by_key(key)
        .unwrap()
        .props
        .str(PropId::Text)
        .unwrap()
        .to_string()
}

fn press(r: &mut Runner<Echo>, id: &str) {
    let key = r.kernel().find_by_test_id(id)[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

/// How many of `op` the named action's body holds.
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
fn a_record_is_built_and_copied_wherever_an_expression_goes() {
    let plan = contract::compile(&corpus("records.contract")).unwrap();
    let plan = Plan::decode(&plan.encode()).unwrap();
    // A copy binds its base once and builds one record.
    assert_eq!(ops(&plan, "retitle", Opcode::Record), 1);
    assert_eq!(ops(&plan, "retitle", Opcode::BindLocal), 1);
    assert_eq!(ops(&plan, "retitle", Opcode::DropLocal), 1);
    let mut r = Runner::boot(
        plan,
        Echo,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    // A state initializer, derives, the view, and a source's argument.
    assert_eq!(text(&r, "title"), "Groceries");
    assert_eq!(text(&r, "body"), "eggs");
    assert_eq!(text(&r, "pinned"), "loose");
    assert_eq!(text(&r, "dirty"), "clean");
    assert_eq!(text(&r, "note"), "n1 Groceries eggs.");
    assert_eq!(text(&r, "preview"), "preview");
    assert_eq!(text(&r, "echoed"), "Groceries pinned");
    // An action copies with one field replaced; equality is structural.
    press(&mut r, "retitle");
    assert_eq!(text(&r, "title"), "Errands");
    assert_eq!(text(&r, "body"), "eggs");
    assert_eq!(text(&r, "dirty"), "dirty");
    press(&mut r, "pin");
    assert_eq!(text(&r, "pinned"), "pinned");
    assert_eq!(text(&r, "title"), "Errands");
    // A `fn` body and a handler's argument build one.
    press(&mut r, "blank");
    assert_eq!(text(&r, "title"), "Fresh");
    assert_eq!(text(&r, "body"), "");
    assert_eq!(text(&r, "pinned"), "loose");
    press(&mut r, "inline");
    assert_eq!(text(&r, "title"), "Inline");
    assert_eq!(text(&r, "pinned"), "pinned");
    assert_eq!(
        r.derive("fields"),
        Some(&Value::record(vec![
            Value::str("Inline"),
            Value::str("b"),
            Value::Bool(true)
        ]))
    );
    // Saved, the draft equals what was loaded; the source asks again.
    press(&mut r, "save");
    assert_eq!(text(&r, "dirty"), "clean");
    assert_eq!(text(&r, "echoed"), "Inline pinned");
    press(&mut r, "retitle");
    press(&mut r, "retitle");
    assert_eq!(text(&r, "dirty"), "dirty");
    // The same fields, built again, compare equal.
    press(&mut r, "inline");
    assert_eq!(text(&r, "dirty"), "clean");
    press(&mut r, "discard");
    assert_eq!(text(&r, "title"), "Inline");
}

const SHAPES: &str = "shape F\n  a: string\n  b: number\n  c: bool\ncomponent App\n  state f = F(a=\"q\", b=1, c=true)\n  state s = \"x\"\n";

fn refused(derive: &str) -> contract::CompileError {
    let src = format!("{SHAPES}  derive d = {derive}\n  view\n    text \"a\"\n");
    contract::compile(&src).unwrap_err()
}

#[test]
fn a_record_names_every_field_once_and_nothing_else() {
    let e = refused("F(a=\"x\")");
    assert_eq!(e.id, "type-record-missing", "{e}");
    assert!(e.message.contains("`b`, `c` are missing"), "{e}");
    assert!(e.message.contains("never defaulted"), "{e}");
    assert_eq!((e.span.line, e.span.col), (8, 14));
    let e = refused("F(a=\"x\", b=1, c=true, d=2)");
    assert_eq!(e.id, "type-record-unknown-field", "{e}");
    assert!(
        e.message
            .contains("`F` has no field `d`; its fields are a, b, c"),
        "{e}"
    );
    assert_eq!((e.span.line, e.span.col), (8, 36));
    let e = refused("F(a=\"x\", b=1, a=\"y\", c=true)");
    assert_eq!(e.id, "type-record-duplicate", "{e}");
    assert_eq!((e.span.line, e.span.col), (8, 28));
    let e = refused("F(a=1, b=1, c=true)");
    assert_eq!(e.id, "type-argument", "{e}");
    assert!(
        e.message
            .contains("field `a` of `F` is `string`, given `number`"),
        "{e}"
    );
}

#[test]
fn a_copy_takes_one_base_of_its_shape_first() {
    for derive in ["F(a=\"x\", f)", "F(f, f, a=\"x\")"] {
        let e = refused(derive);
        assert_eq!(e.id, "type-record-base", "{derive}: {e}");
    }
    let e = refused("F(s, a=\"x\")");
    assert_eq!(e.id, "type-record-base", "{e}");
    assert!(e.message.contains("copies a `F`, given `string`"), "{e}");
    // A copy with every field named, or none, is still a copy.
    for derive in ["F(f)", "F(f, a=\"x\", b=2, c=false)"] {
        let src = format!("{SHAPES}  derive d = {derive}\n  view\n    text d.a\n");
        contract::compile(&src).unwrap_or_else(|e| panic!("{derive}: {e}"));
    }
}

#[test]
fn only_a_declared_shape_is_built_and_a_fn_may_not_take_its_name() {
    // The compiler's own shapes are not built by hand.
    let e = contract::compile(
        "component App\n  action go\n    let g = Geometry(x=0, y=0, width=0, height=0, provisional=false, unavailable=false)\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-unknown-function", "{e}");
    let e = contract::compile(
        "shape F\n  a: string\nfn F(a: string): string = a\ncomponent App\n  view\n    text \"a\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "type-fn-shape-name", "{e}");
}
