//! LLP 1035.005.000 D4a: closed choices of strings and `match` over them,
//! proven on the runner over `contract/corpus/choices.contract`; the seam
//! holds an answer to the literals; the generated TypeScript and Rust spell
//! a choice as their own closed types.

use exact_kernel::{Kernel, PropId};
use exact_plan::{Plan, Value};
use exact_runner::{DataError, DataSource, Event, Runner};
use std::path::Path;

/// `blocks()` answers one block of each kind but `item`, `one()` the
/// record `B` with the kind it holds, and `echo(k)` the record `B` of `k`.
struct Blocks(&'static str);

fn block(id: &str, kind: &str, text: &str) -> Value {
    Value::record(vec![Value::str(id), Value::str(kind), Value::str(text)])
}

impl DataSource for Blocks {
    fn query(&mut self, source: &str, args: &[Value]) -> Result<Value, DataError> {
        match source {
            "blocks" => Ok(Value::list(vec![
                block("1", "heading", "Title"),
                block("2", "paragraph", "Body"),
                block("3", self.0, "-"),
            ])),
            "one" => Ok(Value::record(vec![Value::str(self.0)])),
            "echo" => Ok(Value::record(vec![args[0].clone()])),
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

fn boot(src: &str, third: &'static str) -> Runner<Blocks> {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let plan = Plan::decode(&plan.encode()).unwrap();
    Runner::boot(
        plan,
        Blocks(third),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn text(r: &Runner<Blocks>, id: &str) -> Option<String> {
    let k = r.kernel();
    let key = *k.find_by_test_id(id).first()?;
    Some(k.node_by_key(key)?.props.str(PropId::Text)?.to_string())
}

fn press(r: &mut Runner<Blocks>, id: &str) {
    let key = r.kernel().find_by_test_id(id)[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    r.dispatch(view, Event::Press).unwrap();
}

#[test]
fn a_match_over_a_choice_takes_the_arm_its_literal_names() {
    let mut r = boot(&corpus("choices.contract"), "rule");
    // A choice reads as a string; an expression's `match` in a `fn`.
    assert_eq!(
        text(&r, "summary").unwrap(),
        "heading:24 paragraph:16 rule:0"
    );
    assert_eq!(text(&r, "state").unwrap(), "1 heading rule:0 rule ");
    // The view's arms: a heading's text, a paragraph's badge, a rule's line.
    assert_eq!(text(&r, "block-1").unwrap(), "Title");
    assert_eq!(text(&r, "badge-paragraph-Body").unwrap(), "Body");
    assert!(r.kernel().find_by_test_id("block-2").is_empty());
    assert_eq!(r.kernel().find_by_test_id("block-3").len(), 1);
    // A literal passed to a choice prop, matched in the component.
    assert_eq!(text(&r, "badge-rule-fixed").unwrap(), "fixed");
    // An action's `match`, a choice parameter from a row and from a literal.
    press(&mut r, "block-1");
    assert_eq!(
        text(&r, "state").unwrap(),
        "1 heading heading:24 paragraph a title"
    );
    press(&mut r, "block-3");
    assert_eq!(
        text(&r, "state").unwrap(),
        "1 heading rule:0 heading a line"
    );
}

#[test]
fn a_multi_literal_arm_takes_each_of_its_literals() {
    let r = boot(&corpus("choices.contract"), "item");
    assert_eq!(
        text(&r, "summary").unwrap(),
        "heading:24 paragraph:16 item:16"
    );
    assert_eq!(text(&r, "badge-item--").unwrap(), "• -");
}

const SEAM: &str = "shape B\n  kind: \"a\" | \"b\"\ncomponent App\n  resource one = one() as shape B\n  view\n    text one.kind testId=\"t\"\n";

#[test]
fn an_answer_outside_the_choice_is_refused_at_the_seam() {
    assert_eq!(text(&boot(SEAM, "b"), "t").unwrap(), "b");
    // `"c"` is a string, and not one of `B.kind`'s: the answer is refused
    // as one of another shape is.
    let plan = contract::compile(SEAM).unwrap();
    let refused = Runner::boot(
        plan,
        Blocks("c"),
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .err()
    .expect("refused");
    assert_eq!(format!("{refused:?}"), "Shape { resource: \"one\" }");
    // After boot, a commit whose answer is outside the choice is refused
    // the same way, and what it wrote does not land.
    let src = "shape B\n  kind: \"a\" | \"b\"\ncomponent App\n  state k = \"b\"\n  resource one = echo(k) as shape B\n  action ask(to: string)\n    k = to\n  view\n    column\n      text `${k} ${one.kind}` testId=\"t\"\n      button \"a\" press=ask(\"a\") testId=\"a\"\n      button \"c\" press=ask(\"c\") testId=\"c\"\n";
    let mut r = boot(src, "b");
    press(&mut r, "a");
    assert_eq!(text(&r, "t").unwrap(), "a a");
    let key = r.kernel().find_by_test_id("c")[0];
    let view = r.kernel().node_by_key(key).unwrap().id;
    let refused = r.dispatch(view, Event::Press).unwrap_err();
    assert_eq!(format!("{refused:?}"), "Shape { resource: \"one\" }");
    assert_eq!(text(&r, "t").unwrap(), "a a");
}

#[test]
fn the_generated_types_spell_a_choice_as_their_own_closed_type() {
    let plan = contract::compile(&corpus("choices.contract")).unwrap();
    let ts = contract::typescript(&plan).unwrap();
    assert!(
        ts.contains("= \"heading\" | \"item\" | \"paragraph\" | \"rule\";"),
        "{ts}"
    );
    let rust = contract::rust(&plan).unwrap();
    // One enum per choice, named for the first field that holds it, its
    // variants sorted as the plan holds them and its default the first.
    assert!(rust.contains("pub enum BlockKind {\n    #[default] Heading,\n    Item,\n    Paragraph,\n    Rule,\n}"), "{rust}");
    assert!(!rust.contains("enum PickKind"), "{rust}");
    assert!(rust.contains("pub kind: BlockKind,"), "{rust}");
    assert!(rust.contains("\"rule\" => Self::Rule,"), "{rust}");
}
