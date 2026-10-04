//! The evaluation budget's emission (LLP 1090 §4): what a body counts and
//! checks, where it resets, and that every check names the VM's pc.

use exact_plan::{Opcode, Stdlib};
use exact_runner::vm::instructions;

fn js(src: &str) -> (exact_plan::Plan, String) {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{src}\n{e}"));
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    (plan, js)
}

/// The statement `const <name>=…;` up to the next top-level declaration.
fn decl<'a>(js: &'a str, name: &str) -> &'a str {
    let at = js
        .find(&format!("const {name}="))
        .unwrap_or_else(|| panic!("{name} in {js}"));
    let rest = &js[at..];
    // The next declaration of the module's (a body's own are `const $…`).
    let end = rest
        .match_indices(';')
        .find(|(i, _)| {
            let next = &rest[i + 1..];
            next.starts_with("mount(") || next.starts_with("const ") && !next[6..].starts_with('$')
        })
        .map_or(rest.len(), |(i, _)| i + 1);
    &rest[..end]
}

const CARD: &str = "shape Card\n  id: string\n  n: number\n  label: string\n";

/// A body with no list step, construction, concatenation or checked call is
/// emitted as it was; one that builds or concatenates checks there, with no
/// counter.
#[test]
fn only_a_list_step_meters_a_body() {
    let (_, js) = js(&format!("{CARD}component App\n  state n = 1\n  state s = \"a\"\n  derive plain = n + 1\n  derive joined = s + \"b\"\n  derive card = Card(id=s, n=n, label=s)\n  view\n    text `${{plain}} ${{joined}} ${{card.n}}`\n"));
    assert_eq!(decl(&js, "d_0"), "const d_0=memo(()=>(s_0()+1),\"n\");");
    let joined = decl(&js, "d_1");
    assert!(
        joined.contains("cc(s_1(),\"b\",") && !joined.contains("$s"),
        "{joined}"
    );
    let card = decl(&js, "d_2");
    assert!(card.contains("K([") && !card.contains("$s"), "{card}");
    assert!(js.contains("from\"./budget.js\""), "{js}");
}

/// A metered body counts in one local of its own, nested callbacks
/// included; a `join` adds its list's length at its call; a binding that
/// meters is a function called in place, so each evaluation starts at 0.
#[test]
fn a_metered_body_declares_one_counter_and_join_steps_at_its_call() {
    let (_, js) = js(&format!("{CARD}component App\n  resource cards = cards(3) as shape list<Card>\n  derive nested = map(cards, a => map(cards, b => b.n))\n  derive labels = join(map(cards, c => c.label), \",\")\n  view\n    column\n      text labels\n      text `${{length(map(cards, c => c.n))}}`\n"));
    let nested = decl(&js, "d_0");
    assert_eq!(nested.matches("let $s=0").count(), 1, "{nested}");
    assert_eq!(nested.matches("++$s>65536").count(), 2, "{nested}");
    let labels = decl(&js, "d_1");
    assert!(
        labels.contains(".length)>65536)$T(\"IterationLimit\""),
        "{labels}"
    );
    assert!(labels.contains("x_join("), "{labels}");
    assert!(
        js.contains("(()=>{let $s=0;"),
        "a metered binding is called in place: {js}"
    );
}

/// Each resource argument, row-state initializer and handler argument is an
/// evaluation of its own; a handler's arguments are taken inside its
/// action's commit (`.t`).
#[test]
fn each_reset_point_is_its_own_evaluation() {
    let (_, js) = js(&format!("{CARD}component App\n  resource cards = cards(3, 0) as shape list<Card>\n  resource two = cards(length(map(cards, c => c)), length(map(cards, c => c))) as shape list<Card>\n  state n = 0\n  action set(a: number, b: number)\n    n = a + b\n  view\n    column\n      button press=set(length(map(cards, c => c)), length(map(cards, c => c))) testId=\"go\"\n        text \"go\"\n      each c in cards key=c.id\n        Row(c=c, all=cards)\ncomponent Row\n  props\n    c: Card\n    all: list<Card>\n  state a = length(map(all, x => x))\n  state b = length(map(all, x => x))\n  view\n    text `${{a}} ${{b}}`\n"));
    assert_eq!(decl(&js, "r_1").matches("let $s=0").count(), 2, "{js}");
    assert!(js.contains("on(e"), "{js}");
    let press = js.split("\"press\",").nth(1).expect("a press handler");
    assert!(press.starts_with("a_0.t(()=>["), "{press}");
    assert_eq!(
        press
            .split("]));")
            .next()
            .unwrap()
            .matches("let $s=0")
            .count(),
        2,
        "{press}"
    );
    let owned = js.split("const $r").nth(1).expect("the row's slots");
    let owned = owned.split("};const").next().unwrap();
    assert_eq!(owned.matches("let $s=0").count(), 2, "{owned}");
}

/// An action knows its string parameters' names, and whether it takes the
/// row first, so an argument past MAX_STRING is refused naming it.
#[test]
fn act_names_its_string_parameters() {
    let (_, js) = js("component App\n  state t = \"\"\n  action put(v: string, n: number)\n    t = v\n  resource ids = ids() as shape list<string>\n  view\n    column\n      button press=put(\"a\", 1) testId=\"go\"\n        text \"go\"\n      each i in ids key=i\n        Row()\ncomponent Row\n  state got = \"\"\n  action take(w: string)\n    got = w\n  view\n    input value=got input=take testId=\"in\"\n");
    assert!(
        decl(&js, "a_0").ends_with(",[\"s\",\"n\"],0,[\"v\",0]);"),
        "{js}"
    );
    assert!(decl(&js, "a_1").ends_with(",[\"s\"],1,[\"w\"]);"), "{js}");
    assert!(
        js.contains("sig(\"\",\"s\",\"t\")"),
        "a string slot is named: {js}"
    );
}

/// Every check names the pc of the instruction the VM checks at.
#[test]
fn every_check_names_its_instructions_pc() {
    let (plan, js) = js(&format!("{CARD}component App\n  resource cards = cards(3) as shape list<Card>\n  state s = \"a\"\n  derive all = join(filter(map(cards, c => c.label + s), l => l != \"\"), \",\")\n  derive card = Card(id=encodeURIComponent(s), n=1, label=s)\n  view\n    text `${{all}} ${{card.id}}`\n"));
    for (i, d) in plan.derives.iter().enumerate() {
        let body = decl(&js, &format!("d_{i}"));
        for x in instructions(plan.code(d.body)).flatten() {
            let want = match x.op {
                Opcode::Map | Opcode::Filter => format!("\"IterationLimit\",{})", x.pc),
                Opcode::Concat => format!(",{})", x.pc),
                Opcode::Record | Opcode::List => format!("],{})", x.pc),
                Opcode::Call if x.args[0] == Stdlib::Join as u64 => {
                    format!("x_join(s0,s1,{})", x.pc)
                }
                Opcode::Call if x.args[0] == Stdlib::EncodeURIComponent as u64 => {
                    format!(",{})", x.pc)
                }
                _ => continue,
            };
            assert!(
                body.contains(&want),
                "{:?} at {}: {want} in {body}",
                x.op,
                x.pc
            );
        }
    }
}

/// LLP 1088 §9.1: `concat`, and `slice` and `includes` over a list, take
/// their steps on the caller's budget, as `join` does: each gets the body's
/// `$s` at its call and its pc, and the body adds the steps it took (`ST`);
/// a body that calls one is metered, text's included.
#[test]
fn the_list_builders_step_on_the_callers_counter_at_their_call() {
    let (plan, js) = js(&format!("{CARD}component App\n  resource cards = cards(3) as shape list<Card>\n  state s = \"ab\"\n  derive both = length(concat(cards, cards))\n  derive kept = length(slice(cards, 1))\n  derive found = includes(map(cards, c => c.label), s)\n  derive text = includes(s, \"a\")\n  view\n    text `${{both}} ${{kept}} ${{found}} ${{text}}`\n"));
    for (i, d) in plan.derives.iter().enumerate() {
        let body = decl(&js, &format!("d_{i}"));
        assert_eq!(body.matches("let $s=0").count(), 1, "{body}");
        let calls: Vec<_> = instructions(plan.code(d.body))
            .flatten()
            .filter(|x| x.op == Opcode::Call)
            .filter(|x| {
                [Stdlib::Concat, Stdlib::Slice, Stdlib::Includes]
                    .iter()
                    .any(|&f| x.args[0] == f as u64)
            })
            .collect();
        assert_eq!(calls.len(), 1, "{body}");
        let f = Stdlib::from_wire(calls[0].args[0] as u8).unwrap();
        let want = format!("$s,{});$s+=ST;", calls[0].pc);
        assert!(
            body.contains(&format!("x_{}(", f.name())) && body.contains(&want),
            "{want} in {body}"
        );
    }
    assert!(
        js.contains("ST,") || js.contains(",ST}"),
        "ST imported: {js}"
    );
}
