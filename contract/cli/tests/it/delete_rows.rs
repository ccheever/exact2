//! LLP 1116.020 D1: `delete=action` on a list row is its swipe to delete.
//! Lowering expands it to the swipe form (`contract/lower/src/swipe_rows.rs`):
//! the row in a sideways snap scroll beside a destructive Delete button
//! that runs the action, disabled where the row is, the row styled as its
//! grouped list's row with its inset inside it.

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{DataError, DataSource, Event, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Notes `1` and the locked `2`, and `list` as the view under a 402 × 874 column.
fn app(list: &str) -> String {
    let list = list
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    format!(
        "shape Note\n  id: string\n  title: string\n  locked: bool\n\ncomponent App\n  state notes = [Note(id=\"1\", title=\"Groceries\", locked=false), Note(id=\"2\", title=\"Locked\", locked=true)]\n  state opened = \"\"\n  action remove(id: string)\n    notes = notes.filter(n => n.id != id)\n  action open(id: string)\n    opened = id\n  view\n    column testId=\"root\" width=402 height=874\n{list}"
    )
}

fn boot(list: &str) -> Runner<NoData> {
    let plan = contract::bake(
        contract::compile(&app(list)).unwrap_or_else(|e| panic!("{e}")),
        NoData,
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
    let k = r.kernel_mut();
    let root = k.node_by_key(k.find_by_test_id("root")[0]).unwrap().id;
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    r
}

fn id(r: &Runner<NoData>, test_id: &str) -> Option<u32> {
    let k = r.kernel();
    k.find_by_test_id(test_id)
        .first()
        .map(|key| k.node_by_key(*key).unwrap().id)
}

const GROUPED: &str = "list appearance=\"auto\" flex=1
  section
    for n in notes key=n.id
      button press=open(n.id) delete=remove(n.id) disabled=n.locked testId=`note-${n.id}`
        text n.title testId=`title-${n.id}`
        image \"symbol:forward-chevron\"
    button press=open(\"plain\")
      text \"Plain\" testId=\"title-plain\"";

#[test]
fn a_grouped_rows_delete_is_its_swipe_and_its_lock_keeps_it() {
    let mut r = boot(GROUPED);
    let row = id(&r, "note-1").unwrap();
    let k = r.kernel();
    let scroll = k.node(k.node(row).unwrap().parent.unwrap()).unwrap();
    assert_eq!(
        scroll.props.str(PropId::SwipeContent),
        Some("exact-swipe-row")
    );
    assert_eq!(
        scroll.props.str(PropId::SwipeTrailing),
        Some("exact-swipe-delete")
    );
    let delete = k.node(id(&r, "note-1-delete").unwrap()).unwrap();
    assert_eq!(delete.props.str(PropId::Id), Some("exact-swipe-delete"));
    assert_eq!(delete.props.bool(PropId::Destructive), Some(true));
    assert_eq!(delete.props.str(PropId::AccessibilityLabel), Some("Delete"));
    // The row is a row: its text where any row's starts, its height a row's.
    // The inset is inside it, so UIKit's cell and its long-press preview
    // hold it: the scroll stands at the card's edge.
    let x = |t: &str| k.node(id(&r, t).unwrap()).unwrap().frame.x;
    assert_eq!(
        x("title-1"),
        x("title-plain"),
        "the text at the row's inset"
    );
    assert_eq!(
        x("title-1") - scroll.frame.x,
        16.0,
        "the scroll at the card's edge"
    );
    assert_eq!(k.node(row).unwrap().frame.height, 52.0, "a row's height");
    // The lock is the delete's: the row still opens.
    let locked = id(&r, "note-2-delete").unwrap();
    assert_eq!(
        k.node(locked).unwrap().props.bool(PropId::Disabled),
        Some(true)
    );
    assert_eq!(
        k.node(id(&r, "note-2").unwrap())
            .unwrap()
            .props
            .bool(PropId::Disabled),
        None
    );
    r.dispatch(id(&r, "note-2").unwrap(), Event::Press).unwrap();
    assert_eq!(r.slot("opened").and_then(Value::as_str), Some("2"));
    // The gesture runs the action.
    r.dispatch(id(&r, "note-1-delete").unwrap(), Event::Press)
        .unwrap();
    assert!(id(&r, "note-1").is_none(), "the row is gone");
}

#[test]
fn a_cells_delete_is_its_swipe() {
    let mut r = boot("list flex=1\n  for n in notes key=n.id\n    row id=`own-${n.id}` delete=remove(n.id) testId=`note-${n.id}`\n      text n.title");
    let row = id(&r, "note-1").unwrap();
    let k = r.kernel();
    let scroll = k.node(k.node(row).unwrap().parent.unwrap()).unwrap();
    assert_eq!(
        scroll.props.str(PropId::SwipeContent),
        Some("own-1"),
        "the row's own id"
    );
    r.dispatch(id(&r, "note-1-delete").unwrap(), Event::Press)
        .unwrap();
    assert!(id(&r, "note-1").is_none());
}

#[test]
fn a_delete_no_list_row_holds_is_named() {
    let warned = |list: &str| -> Vec<String> {
        let (plan, warnings) = contract::compile_warned(&app(list));
        plan.unwrap_or_else(|e| panic!("{e}"));
        warnings
            .into_iter()
            .filter(|w| w.id == "lower-delete-row")
            .map(|w| w.message)
            .collect()
    };
    assert!(warned(GROUPED).is_empty());
    assert!(warned(
        "list flex=1\n  for n in notes key=n.id\n    row delete=remove(n.id)\n      text n.title"
    )
    .is_empty());
    let stray = warned("for n in notes key=n.id\n  row delete=remove(n.id)\n    text n.title");
    assert_eq!(stray.len(), 1, "{stray:?}");
    assert!(stray[0].contains("no row of a `list`"), "{}", stray[0]);
    // A section's header is no row.
    assert_eq!(warned("list appearance=\"auto\" flex=1\n  section\n    header delete=remove(\"1\")\n      text \"Notes\"\n    row\n      text \"One\"").len(), 1);
}
