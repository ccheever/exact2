//! What the emitted program says about a plan's resources.
use exact_runner::{DataError, DataSource, Value};

struct Answers;
impl DataSource for Answers {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        match source {
            "preview" => Ok(Value::Number(1.0)),
            "full" => Ok(Value::Number(2.0)),
            _ => Err(DataError::UnknownSource(source.into())),
        }
    }
}

/// Review B4: an `else` row's build-time answer is the bake's for every
/// launch (the runner never asks it again), so the JS target receives it
/// settled (`res`'s last argument), while the resource it stands in for
/// keeps its build-time answer as a first frame to ask again at launch.
#[test]
fn an_else_row_is_settled_and_its_owner_is_a_bake() {
    let plan = contract::compile(
        "component App\n  resource full = full() as shape number else preview()\n  view\n    text `${full}`\n",
    )
    .unwrap();
    let plan = contract::bake(plan, Answers).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    let rows: Vec<&str> = js
        .split("const r_")
        .skip(1)
        .map(|s| s.split(';').next().unwrap())
        .collect();
    let owner = rows
        .iter()
        .find(|r| r.contains("\"full\""))
        .expect("full's row");
    let other = rows
        .iter()
        .find(|r| !r.contains("\"full\""))
        .expect("the else row");
    assert!(other.ends_with(",1)"), "the else row is settled: {other}");
    assert!(
        !owner.ends_with(",1)"),
        "the owner is a bake to ask again: {owner}"
    );
}

/// The runner answers its own sources: the program marks their resources, so
/// the overlay never lays a write over one (overlay.js).
#[test]
fn a_runner_owned_resource_is_marked() {
    let plan = contract::compile(
        "shape Time\n  epochAtZero: number\ncomponent App\n  resource time = exactTime() as shape Time\n  resource full = full() as shape number\n  view\n    text `${full}/${time.epochAtZero}`\n",
    )
    .unwrap();
    let plan = contract::bake(plan, Answers).unwrap();
    let js = crate::emit::emit(&plan, false, false).unwrap().js;
    let rows: Vec<&str> = js.split("const r_").skip(1).collect();
    let time = rows
        .iter()
        .find(|r| r.contains("\"exactTime\""))
        .expect("time's row");
    let full = rows
        .iter()
        .find(|r| r.contains("\"full\""))
        .expect("full's row");
    assert!(time.contains(".r.owned=true;"), "{time}");
    assert!(!full.contains(".r.owned"), "{full}");
}
