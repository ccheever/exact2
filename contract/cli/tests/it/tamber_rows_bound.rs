//! Stage 14 E: the write check judges a computed row under a `for` or
//! `match` in the arm's scope, the names pushed as lowering pushes them
//! (`Scope::push_each_of`, `push_some_of`, `push_none`), and through a `let`
//! with its binding typed (`Scope::push_let`); a record's field is typed as
//! `expr::compile` types it, at the root too. A subject, a `let`'s value or a
//! field whose type isn't known by its form leaves the row to the build.
#![cfg(unix)]

use super::tamber_package_params::reading;
use super::tamber_rows_check::{same_words, transaction};
use super::tamber_serve::Serve;
use serde_json::{json, Value};

const DESIGN: &str = "tamber t\n\ntheme\n  ink = #111111\n  one = 1\n";
const HEAD: &str = "use Card from \"@acme/reading\"\n";

fn zero() -> Value {
    json!({"op": "set-token", "name": "one", "value": "0"})
}

fn zeroed() -> String {
    DESIGN.replace("one = 1", "one = 0")
}

/// The server accepts `op` on `contract`, which builds; and `contract build`
/// of the design the write leaves refuses it (`refused`: the stated limit, the
/// build's catch) or accepts it.
fn accepted(tag: &str, contract: &str, op: Value, written: &str, refused: bool) {
    let dir = reading(tag);
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), contract).unwrap();
    contract::compile_path(&dir.join("app.contract")).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, op);
    assert!(w.get("ok").is_some(), "{tag}: {w}");
    drop(s);
    std::fs::write(dir.join("app.tamber"), written).unwrap();
    assert_eq!(
        contract::compile_path(&dir.join("app.contract")).is_err(),
        refused,
        "{tag}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A `for`'s item, typed from the list's type (`string[]` gives `string`): a
/// number row reading it may name any number token, so a write making one
/// unfit is refused as the build refuses it.
#[test]
fn a_for_items_computed_row_is_judged() {
    let contract = format!("{HEAD}component App\n  state picks = [\"2\"]\n  view\n    column\n      for p in picks key=p\n        button \"Go\" -exact-press-scale=p\n      Card()\n");
    same_words(
        &reading("rows-bound-each"),
        DESIGN,
        &contract,
        zero(),
        &zeroed(),
    );
}

/// A `for`'s position is a number, as lowering types it: a number row
/// reading it names no token, and the write is accepted as the build accepts
/// it.
#[test]
fn a_for_position_is_a_number() {
    let contract = format!("{HEAD}component App\n  state picks = [\"2\"]\n  view\n    column\n      for p, i in picks key=p\n        button \"Go\" -exact-press-scale=i\n      Card()\n");
    accepted("rows-bound-index", &contract, zero(), &zeroed(), false);
}

/// A `match`'s binding in its `some` arm, typed from the subject's `t?`; and
/// in the `none` arm, which binds nothing, a root name is judged as at the
/// root.
#[test]
fn a_match_arms_computed_rows_are_judged() {
    let some = format!("{HEAD}component App\n  state picked = some(\"2\")\n  view\n    column\n      match picked\n        case some(p)\n          button \"Go\" -exact-press-scale=p\n        case none\n          text \"none\"\n      Card()\n");
    same_words(
        &reading("rows-bound-some"),
        DESIGN,
        &some,
        zero(),
        &zeroed(),
    );
    let none = format!("{HEAD}component App\n  state picked = some(\"2\")\n  state pick = \"2\"\n  view\n    column\n      match picked\n        case some(p)\n          text p\n        case none\n          button \"Go\" -exact-press-scale=pick\n      Card()\n");
    same_words(
        &reading("rows-bound-none"),
        DESIGN,
        &none,
        zero(),
        &zeroed(),
    );
}

/// An inner binding shadows an outer one of the same name, as lowering's
/// scope does: the `for`'s item (a number) inside a `match` binding the
/// name to a string names no token.
#[test]
fn an_inner_binding_shadows_the_outer() {
    let contract = format!("{HEAD}component App\n  state picked = some(\"2\")\n  state counts = [2]\n  view\n    column\n      match picked\n        case some(p)\n          for p in counts key=p\n            button \"Go\" -exact-press-scale=p\n        case none\n          text \"none\"\n      Card()\n");
    accepted("rows-bound-shadow", &contract, zero(), &zeroed(), false);
}

/// A subject whose type isn't known by its form (a call) leaves the region
/// to the build: not refused here, the build refuses it.
#[test]
fn a_region_over_a_call_is_the_builds() {
    let contract = format!("{HEAD}fn picksOf(x: string): string[] = [x]\ncomponent App\n  state pick = \"2\"\n  view\n    column\n      for p in picksOf(pick) key=p\n        button \"Go\" -exact-press-scale=p\n      Card()\n");
    accepted("rows-bound-call", &contract, zero(), &zeroed(), true);
}

/// A row through a `let` (a child's derive read twice is bound once,
/// `inline::derives`) is typed with its binding and judged.
#[test]
fn a_row_through_a_let_is_judged() {
    let contract = format!("{HEAD}component App\n  state pick = \"2\"\n  view\n    column\n      Pad(pick=pick)\n      Card()\ncomponent Pad\n  props\n    pick: string\n  derive a = pick\n  derive b = `${{a}}${{a}}`\n  view\n    button \"Go\" -exact-press-scale=b\n");
    same_words(
        &reading("rows-bound-let"),
        DESIGN,
        &contract,
        zero(),
        &zeroed(),
    );
}

/// A `let` whose value's type isn't known by its form (a call) is the
/// build's.
#[test]
fn a_let_over_a_call_is_the_builds() {
    let contract = format!("{HEAD}fn same(x: string): string = x\ncomponent App\n  state pick = \"2\"\n  view\n    column\n      Pad(pick=pick)\n      Card()\ncomponent Pad\n  props\n    pick: string\n  derive a = same(pick)\n  derive b = `${{a}}${{a}}`\n  view\n    button \"Go\" -exact-press-scale=b\n");
    accepted("rows-bound-let-call", &contract, zero(), &zeroed(), true);
}

/// A record's field is typed as `expr::compile` types it, at the root too
/// (theme2's Q1): judged; a field of a value whose type isn't a record by its
/// form (a call's) is the build's.
#[test]
fn a_records_field_is_typed_at_the_root() {
    let shape = "shape Pick\n  scale: string\n";
    let contract = format!("{HEAD}{shape}component App\n  state card = Pick(scale=\"2\")\n  view\n    column\n      button \"Go\" -exact-press-scale=card.scale\n      Card()\n");
    same_words(
        &reading("rows-bound-member"),
        DESIGN,
        &contract,
        zero(),
        &zeroed(),
    );
    let call = format!("{HEAD}{shape}fn pickOf(s: string): Pick = Pick(scale=s)\ncomponent App\n  state s = \"2\"\n  view\n    column\n      button \"Go\" -exact-press-scale=pickOf(s).scale\n      Card()\n");
    accepted("rows-bound-member-call", &call, zero(), &zeroed(), true);
}

/// A `match` whose subject isn't typed by its form (a call) leaves both arms
/// to the build, the `none` arm too: the build lowers the `some` arm first,
/// and a refusal judged only in the `none` arm would not be its first
/// (Grok's round 1).
#[test]
fn a_match_over_an_untyped_subject_is_the_builds() {
    let contract = format!("{HEAD}component App\n  state picks = [\"2\"]\n  state other = \"2\"\n  view\n    column\n      for s in picks key=s\n        match first(picks)\n          case some(p)\n            button \"A\" -exact-press-scale=p\n          case none\n            button \"B\" -exact-press-scale=other\n      Card()\n");
    accepted("rows-bound-match-call", &contract, zero(), &zeroed(), true);
}

/// A record's field through a `for`'s item, typed by the field's own type
/// (Astra's round 1): its string field in a number row is judged, its number
/// field names no token and the write is accepted as the build accepts it.
#[test]
fn a_bound_records_field_has_its_own_type() {
    let shape = "shape Pick\n  n: number\n  scale: string\n";
    let row = |field: &str| {
        format!("{HEAD}{shape}component App\n  state cards = [Pick(n=2, scale=\"2\")]\n  view\n    column\n      for c in cards key=c.scale\n        button \"Go\" -exact-press-scale=c.{field}\n      Card()\n")
    };
    same_words(
        &reading("rows-bound-field-string"),
        DESIGN,
        &row("scale"),
        zero(),
        &zeroed(),
    );
    accepted(
        "rows-bound-field-number",
        &row("n"),
        zero(),
        &zeroed(),
        false,
    );
}

/// An `if` inside a region keeps the region's names (its arms push a frame
/// binding nothing, as lowering's do), and a region inside an inlined
/// component's instance is typed in that instance: both judged.
#[test]
fn an_if_in_a_region_and_a_region_in_an_instance_are_judged() {
    let when = format!("{HEAD}component App\n  state on = true\n  state picks = [\"2\"]\n  view\n    column\n      for p in picks key=p\n        if on\n          button \"Go\" -exact-press-scale=p\n      Card()\n");
    same_words(
        &reading("rows-bound-when"),
        DESIGN,
        &when,
        zero(),
        &zeroed(),
    );
    let instance = format!("{HEAD}component App\n  state picks = [\"2\"]\n  view\n    column\n      Picks(picks=picks)\n      Card()\ncomponent Picks\n  props\n    picks: string[]\n  view\n    column\n      for p in picks key=p\n        button \"Go\" -exact-press-scale=p\n");
    same_words(
        &reading("rows-bound-instance"),
        DESIGN,
        &instance,
        zero(),
        &zeroed(),
    );
}

/// The stated limit (round 2, both reviewers): a row the check can't type (a
/// call) that the build refuses first leaves the check's first refusal at a
/// later row, which the build refuses too, in the same words. Never a false
/// refusal: the build of the written design fails.
#[test]
fn a_row_left_to_the_build_before_a_judged_one_is_the_stated_limit() {
    let contract = format!("{HEAD}fn same(x: string): string = x\ncomponent App\n  state pick = \"2\"\n  view\n    column\n      button \"A\" -exact-press-scale=same(pick)\n      button \"B\" -exact-press-scale=pick\n      Card()\n");
    let dir = reading("rows-bound-limit");
    std::fs::write(dir.join("app.tamber"), DESIGN).unwrap();
    std::fs::write(dir.join("app.contract"), &contract).unwrap();
    let mut s = Serve::start(&dir, &[]);
    s.ask(json!({"method": "propose", "ops": [{"op": "set-token", "name": "ink", "value": "#444444"}]}));
    let w = transaction(&mut s, &dir, zero());
    drop(s);
    let d = &w["error"]["diagnostics"][0];
    assert!(d.is_object(), "refused: {w}");
    std::fs::write(dir.join("app.tamber"), zeroed()).unwrap();
    let built = contract::compile_path(&dir.join("app.contract")).unwrap_err();
    // The build's first is A's call (line 7); the check's is B's name (line 8).
    assert_eq!(
        (built.span.line, d["line"].as_u64().unwrap()),
        (7, 8),
        "{w}"
    );
    assert_eq!(d["id"], built.id);
    let _ = std::fs::remove_dir_all(&dir);
}
