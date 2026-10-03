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

/// Each source's compile: `"ok"`, or the id it is refused with.
fn compiled(src: &str) -> String {
    contract::compile(src).map_or_else(|e| e.id.to_string(), |_| "ok".into())
}

#[test]
fn a_branch_a_choice_does_not_accept_is_refused_in_either_order() {
    for body in ["c ? \"a\" : 123", "c ? 123 : \"a\"", "c ? \"a\" : \"z\""] {
        let src = format!(
            "fn bad(c: bool): \"a\" | \"b\" = {body}\ncomponent App\n  view\n    text \"x\"\n"
        );
        let want = if body.contains('z') {
            "type-choice-unknown"
        } else {
            "type-fn-return"
        };
        assert_eq!(compiled(&src), want, "{body}");
    }
    // Literals reach a choice through `some`, `?:` and an option's `match`.
    for f in [
        "fn f(c: bool): option<\"a\" | \"b\"> = c ? some(\"a\") : none",
        "fn f(o: option<string>): \"a\" | \"b\" = match o { case some(x) => \"a\", case none => \"b\" }",
    ] {
        let src = format!("{f}\ncomponent App\n  view\n    text \"x\"\n");
        assert_eq!(compiled(&src), "ok", "{f}");
    }
}

#[test]
fn a_components_match_holds_to_its_declared_choice_whatever_its_use_passes() {
    let child = "component Child\n  props\n    kind: \"a\" | \"b\"\n  state n = 0\n  derive d = kind\n  view\n    column\n      text d\n      match d\n        case \"a\"\n          text \"A\"\n        case \"b\"\n          text \"B\"\n";
    // A field of a narrower choice, a `?:` of a field and a literal, a literal.
    for arg in ["s.k", "flag ? s.w : \"a\"", "\"b\""] {
        let src = format!("shape S\n  k: \"a\"\n  w: \"a\" | \"b\"\ncomponent App\n  state flag = true\n  resource s = s() as shape S\n  view\n    column\n      Child(kind={arg})\n{child}");
        assert_eq!(compiled(&src), "ok", "{arg}");
    }
    // An authored literal subject is not a choice: no arm would be checked.
    let src = "component App\n  view\n    text match \"c\" { case \"a\" => \"A\", case \"b\" => \"B\" }\n";
    assert_eq!(compiled(src), "type-match-subject");
}

#[test]
fn inferred_parameters_sources_and_providers_agree_on_a_string() {
    // An untyped parameter called with a choice and a string is a string,
    // in either order; so is a source's argument.
    for (x, y) in [("firstKind()", "\"b\""), ("\"b\"", "firstKind()")] {
        let src = format!("fn firstKind(): \"a\" = \"a\"\ncomponent App\n  state s = \"\"\n  resource r = echo({x}) as shape string\n  resource q = echo({y}) as shape string\n  action pick(k)\n    s = k\n  view\n    column\n      button \"x\" press=pick({x})\n      button \"y\" press=pick({y})\n");
        assert_eq!(compiled(&src), "ok", "{x}, {y}");
    }
    // A provided literal fills a choice inject, and not a narrower one.
    for (provided, ok) in [("\"a\"", true), ("\"z\"", false)] {
        let src = format!("component App\n  provide\n    kind = {provided}\n  view\n    column\n      Child()\ncomponent Child\n  inject\n    kind: \"a\" | \"b\"\n  view\n    text kind\n");
        assert_eq!(compiled(&src) == "ok", ok, "{provided}");
    }
}

#[test]
fn a_keyframe_constant_folds_a_match_and_a_wide_arm_stays_shallow() {
    let src = "fn alpha(k: \"a\" | \"b\"): number = match k { case \"a\" => 0, case \"b\" => 1 }\nkeyframes fade\n  from opacity=alpha(\"a\")\n  to opacity=1\ncomponent App\n  view\n    text \"x\" animation-name=\"fade\" animation-duration=\"1s\"\n";
    assert_eq!(compiled(src), "ok");
    // 600 literals: 63 arms of one and a first arm of the rest, in a view and
    // an action. The wide arm's test pairs its comparisons off.
    let lits: Vec<String> = (0..600).map(|i| format!("\"k{i}\"")).collect();
    let wide = lits[63..].join(" | ");
    let mut src = format!("shape S\n  k: {}\ncomponent App\n  resource s = s() as shape S\n  state n = 0\n  action go\n    match s.k\n      case {wide}\n        n = 1\n", lits.join(" | "));
    for l in &lits[..63] {
        src += &format!("      case {l}\n        n = 2\n");
    }
    src += &format!("  view\n    column\n      text \"x\" press=go\n      match s.k\n        case {wide}\n          text \"rest\"\n");
    for l in &lits[..63] {
        src += &format!("        case {l}\n          text {l}\n");
    }
    assert_eq!(compiled(&src), "ok");
}

#[test]
fn every_literal_names_a_rust_variant() {
    let src = "shape S\n  k: \"---\" | \"ok\" | \"self\"\ncomponent App\n  resource s = s() as shape S\n  view\n    text s.k\n";
    let rust = contract::rust(&contract::compile(src).unwrap()).unwrap();
    assert!(
        rust.contains("pub enum SK {\n    #[default] V0,\n    Ok,\n    VSelf,\n}"),
        "{rust}"
    );
}

#[test]
fn a_components_match_carried_into_its_use_takes_only_its_arms_literals() {
    // Passed a bare `action` prop, the `match` is checked in the component.
    let src = "shape S\n  k: \"a\" | \"b\" | \"c\"\ncomponent App\n  state out = \"\"\n  resource s = s() as shape S\n  action set(x: string)\n    out = x\n  view\n    Child(kind=s.k, picked=set)\ncomponent Child\n  props\n    kind: \"a\" | \"b\" | \"c\"\n    picked: action\n  view\n    button \"go\" press=picked(match kind { case \"a\" => \"A\", case \"b\" => \"B\" })\n";
    assert_eq!(compiled(src), "type-match-missing");
    // A number in a component whose `match` is over a bare-action argument.
    let src = "component App\n  state out = \"\"\n  action set(s: string)\n    out = s\n  view\n    Child(kind=123, picked=set)\ncomponent Child\n  props\n    kind: number\n    picked: action\n  view\n    button \"go\" press=picked(match kind { case \"a\" => \"A\", case \"b\" => \"B\" })\n";
    assert_eq!(compiled(src), "type-match-subject");
    // A same-named narrower value passes through.
    let src = "fn a(): \"a\" = \"a\"\ncomponent App\n  state kind = a()\n  view\n    Child(kind=kind)\ncomponent Child\n  props\n    kind: \"a\" | \"b\"\n  view\n    text match kind { case \"a\" => \"A\", case \"b\" => \"B\" }\n";
    assert_eq!(compiled(src), "ok");
    // A state a choice prop initializes is a `string` at the use: refused,
    // and the refusal says to match the prop.
    let src = "component App\n  view\n    column\n      Child(k=\"a\")\ncomponent Child\n  props\n    k: \"a\" | \"b\"\n  state mode = k\n  view\n    column\n      match mode\n        case \"a\"\n          text \"A\"\n        case \"b\"\n          text \"B\"\n";
    let e = contract::compile(src).unwrap_err();
    assert_eq!(e.id, "type-match-subject", "{e}");
    assert!(e.message.contains("match the choice prop itself"), "{e}");
}

#[test]
fn a_payload_and_a_provided_option_agree_as_arguments_do() {
    for (a, b) in [
        (
            "button \"c\" press=pick(firstKind())",
            "input type=\"text\" input=pick",
        ),
        (
            "input type=\"text\" input=pick",
            "button \"c\" press=pick(firstKind())",
        ),
    ] {
        let src = format!("fn firstKind(): \"a\" = \"a\"\ncomponent App\n  state s = \"\"\n  action pick(k)\n    s = k\n  view\n    column\n      {a}\n      {b}\n");
        assert_eq!(compiled(&src), "ok", "{a}");
    }
    let src = "component App\n  state c = true\n  provide\n    kind = c ? some(\"a\") : none\n  view\n    column\n      Child()\ncomponent Child\n  inject\n    kind: option<\"a\" | \"b\">\n  view\n    text \"x\"\n";
    assert_eq!(compiled(src), "ok");
}

#[test]
fn nested_action_matches_are_refused_past_256_levels() {
    let lits: Vec<String> = (0..63).map(|i| format!("\"k{i}\"")).collect();
    let mut src = format!(
        "fn k(): {} = \"k62\"\ncomponent App\n  state n = 0\n  action go\n",
        lits.join(" | ")
    );
    let mut indent = 4;
    for _ in 0..5 {
        src += &format!("{}match k()\n", " ".repeat(indent));
        for l in &lits {
            src += &format!("{}case {l}\n", " ".repeat(indent + 2));
        }
        indent += 4;
    }
    src += &format!(
        "{}n = 2\n  view\n    text \"x\" press=go\n",
        " ".repeat(indent)
    );
    assert_eq!(compiled(&src), "syntax-action-depth");
}

#[test]
fn a_subject_fits_through_an_option_match_a_shared_derive_and_never_while_unknown() {
    let child = "component Child\n  props\n    k: \"a\" | \"b\"\n  derive mode = k\n  derive line = match mode { case \"a\" => mode + \"!\", case \"b\" => mode + \"?\" }\n  view\n    text line\n";
    for arg in [
        "\"a\"",
        "match o { case some(x) => \"a\", case none => \"b\" }",
    ] {
        let src =
            format!("component App\n  state o = some(\"x\")\n  view\n    Child(k={arg})\n{child}");
        assert_eq!(compiled(&src), "ok", "{arg}");
    }
    // A derive is typed before the action that writes `o`: its `match` on
    // what `o` holds is not taken on trust while that is `?`.
    let src = "fn third(): \"a\" | \"b\" | \"c\" = \"c\"\ncomponent App\n  state o = none\n  derive d = match o { case some(k) => match k { case \"a\" => \"A\", case \"b\" => \"B\" }, case none => \"none\" }\n  action go\n    o = some(third())\n  view\n    column\n      text d\n      button \"go\" press=go\n";
    assert_eq!(compiled(src), "type-match-subject");
    // Empty arms nest as deep as full ones.
    let lits: Vec<String> = (0..63).map(|i| format!("\"k{i}\"")).collect();
    let mut src = format!(
        "fn k(): {} = \"k62\"\ncomponent App\n  state n = 0\n  action go\n",
        lits.join(" | ")
    );
    let mut indent = 4;
    for _ in 0..5 {
        src += &format!("{}match k()\n", " ".repeat(indent));
        for l in &lits {
            src += &format!("{}case {l}\n", " ".repeat(indent + 2));
        }
        indent += 4;
    }
    src += "  view\n    text \"x\" press=go\n";
    assert_eq!(compiled(&src), "syntax-action-depth");
}
