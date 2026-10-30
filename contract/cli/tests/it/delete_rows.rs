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

const WRITTEN: &str = "list appearance=\"auto\" flex=1
  section
    for n in notes key=n.id
      scroll swipeContent=`row-${n.id}` swipeTrailing=`flag-${n.id}` testId=`swipe-${n.id}` overflow-x=\"scroll\" overflow-y=\"hidden\" scrollbar-width=\"none\" scroll-snap-type=\"x mandatory\"
        button press=open(n.id) id=`row-${n.id}` testId=`note-${n.id}` width=\"100%\" flex-shrink=0 scroll-snap-align=\"start\"
          text n.title testId=`title-${n.id}`
          image \"symbol:forward-chevron\" testId=`chevron-${n.id}`
        button press=remove(n.id) id=`flag-${n.id}` testId=`flag-${n.id}` flex-shrink=0 scroll-snap-align=\"start\"
          text \"Flag\"
    button press=open(\"plain\")
      text \"Plain\" testId=\"title-plain\"
      image \"symbol:forward-chevron\" testId=\"chevron-plain\"";

/// A swipe written by hand in a grouped list is styled as `delete=`'s is:
/// the row it names is the grouped row, with its inset and its parts'
/// rules, and the scroll takes the row's box in the card, its content and
/// actions side by side.
#[test]
fn a_hand_written_swipe_rows_content_is_the_grouped_row() {
    let r = boot(WRITTEN);
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t).unwrap()).unwrap().frame;
    let scroll = frame("swipe-1");
    assert_eq!(
        frame("title-1").x,
        frame("title-plain").x,
        "the text at the row's inset"
    );
    assert_eq!(
        frame("title-1").x - scroll.x,
        16.0,
        "the scroll at the card's edge"
    );
    assert_eq!(
        frame("chevron-1").x + frame("chevron-1").width,
        frame("chevron-plain").x + frame("chevron-plain").width,
        "the chevron at the row's trailing edge, the accessory"
    );
    assert_eq!(frame("note-1").height, 52.0, "a row's height");
    assert_eq!(
        frame("note-1").width,
        scroll.width,
        "the row as wide as its scroll"
    );
    assert!(
        frame("flag-1").x >= frame("note-1").x + frame("note-1").width,
        "the action beside it"
    );
}

const NESTED: &str = "list appearance=\"auto\" flex=1
  section
    for n in notes key=n.id
      scroll swipeContent=`row-${n.id}` swipeLeading=`pin-${n.id}` testId=`swipe-${n.id}` overflow-x=\"scroll\" overflow-y=\"hidden\" scrollbar-width=\"none\" scroll-snap-type=\"x mandatory\"
        row testId=`wrap-${n.id}` width=\"100%\"
          button press=open(n.id) id=`pin-${n.id}` flex-shrink=0 scroll-snap-align=\"start\"
            text \"Pin\"
          button press=open(n.id) id=`row-${n.id}` testId=`note-${n.id}` width=\"100%\" flex-shrink=0 scroll-snap-align=\"start\"
            text n.title testId=`title-${n.id}`
            image \"symbol:forward-chevron\" testId=`chevron-${n.id}`
    button press=open(\"plain\") testId=\"plain\"
      text \"Plain\" testId=\"title-plain\"
      image \"symbol:forward-chevron\" testId=\"chevron-plain\"";

/// A hand-written swipe whose row stands in a wrapper beside its leading
/// actions (Messages' inbox) is the grouped row too.
#[test]
fn a_hand_written_swipes_nested_content_is_the_grouped_row() {
    let r = boot(NESTED);
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t).unwrap()).unwrap().frame;
    assert_eq!(frame("note-1").height, 52.0, "a row's height");
    let inset = |row: &str, chevron: &str| {
        frame(row).x + frame(row).width - frame(chevron).x - frame(chevron).width
    };
    assert_eq!(
        inset("note-1", "chevron-1"),
        inset("plain", "chevron-plain"),
        "the chevron at the row's trailing inset, the accessory"
    );
    assert_eq!(
        frame("title-1").x - frame("note-1").x,
        frame("title-plain").x - frame("plain").x,
        "the text at the row's leading inset"
    );
}

const ARMS: &str = "list appearance=\"auto\" flex=1
  section
    for n in notes key=n.id
      scroll swipeContent=`row-${n.id}` swipeTrailing=`flag-${n.id}` testId=`swipe-${n.id}` overflow-x=\"scroll\" overflow-y=\"hidden\" scrollbar-width=\"none\" scroll-snap-type=\"x mandatory\"
        if n.locked
          button press=open(n.id) id=`row-${n.id}` testId=`note-${n.id}` width=\"100%\" flex-shrink=0 scroll-snap-align=\"start\"
            text n.title testId=`title-${n.id}`
            image \"symbol:lock\"
        else
          button press=open(n.id) id=`row-${n.id}` testId=`note-${n.id}` width=\"100%\" flex-shrink=0 scroll-snap-align=\"start\"
            text n.title testId=`title-${n.id}`
            image \"symbol:forward-chevron\" testId=`chevron-${n.id}`
        button press=remove(n.id) id=`flag-${n.id}` flex-shrink=0 scroll-snap-align=\"start\"
          text \"Flag\"
    button press=open(\"plain\") testId=\"plain\"
      text \"Plain\" testId=\"title-plain\"
      image \"symbol:forward-chevron\" testId=\"chevron-plain\"";

/// A hand-written swipe whose content is written in each arm of an `if` is
/// the grouped row in either arm, as a direct child is.
#[test]
fn a_hand_written_swipes_content_in_an_if_is_the_grouped_row() {
    let r = boot(ARMS);
    let k = r.kernel();
    let frame = |t: &str| k.node(id(&r, t).unwrap()).unwrap().frame;
    for n in ["1", "2"] {
        let (row, title, scroll) = (
            frame(&format!("note-{n}")),
            frame(&format!("title-{n}")),
            frame(&format!("swipe-{n}")),
        );
        assert_eq!(row.height, 52.0, "{n}: a row's height");
        assert_eq!(
            title.x,
            frame("title-plain").x,
            "{n}: the text at the row's inset"
        );
        assert_eq!(
            title.x - scroll.x,
            16.0,
            "{n}: the scroll at the card's edge"
        );
    }
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

/// The element lint, which runs beside an earlier refusal, takes `delete=`
/// as lowering does: the one error is the type error.
#[test]
fn a_delete_beside_another_error_is_not_refused() {
    let src = app(
        "list flex=1\n  for n in notes key=n.id\n    row delete=remove(n.id)\n      text n.titel",
    );
    let Err(errors) =
        contract::compile_path_source_all(std::path::Path::new("app.contract"), &src, false)
    else {
        panic!("the misspelt field refuses this app")
    };
    let ids: Vec<&str> = errors.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["type-unknown-field"], "{errors:?}");
}

/// LLP 1116.021 D5: a destructive button inside the row a `for` repeats,
/// with no `delete=` on the row, is advice to write one; a detail screen's
/// section, a row that says `delete=`, and a dialog or menu in the row are
/// quiet.
#[test]
fn a_destructive_button_in_a_repeated_row_asks_for_delete() {
    let warned = |list: &str| -> Vec<u32> {
        let (plan, warnings) = contract::compile_warned(&app(list));
        plan.unwrap_or_else(|e| panic!("{e}"));
        warnings
            .into_iter()
            .filter(|w| w.id == "lower-row-delete-button")
            .map(|w| w.span.line)
            .collect()
    };
    let row = |section: &str| format!("list appearance=\"auto\" flex=1\n  section\n{section}");
    // The grouped row and a cell list's row.
    assert_eq!(
        warned(&row("    for n in notes key=n.id\n      row\n        text n.title\n        if ! n.locked\n          button press=remove(n.id) destructive=true\n            text \"Delete\"")),
        [21]
    );
    assert_eq!(
        warned("list flex=1\n  for n in notes key=n.id\n    row\n      text n.title\n      button press=remove(n.id) destructive=true aria-label=\"Delete\"\n        image \"symbol:delete\""),
        [19]
    );
    // A row that says `delete=`, and a section that repeats no row.
    assert!(warned(&row("    for n in notes key=n.id\n      row delete=remove(n.id)\n        text n.title\n        button press=remove(n.id) destructive=true\n          text \"Delete\"")).is_empty());
    assert!(warned(&row("    row\n      text \"Groceries\"\n    button press=remove(\"1\") destructive=true\n      text \"Delete Note\"")).is_empty());
    assert!(warned("for n in notes key=n.id\n  list appearance=\"auto\" flex=1\n    section\n      button press=remove(n.id) destructive=true\n        text \"Delete Note\"").is_empty());
    // A row's dialog and menu hold their own actions.
    assert!(warned(&row("    for n in notes key=n.id\n      row\n        text n.title\n        dialog id=`ask-${n.id}` role=\"alertdialog\" aria-label=\"Delete?\"\n          button press=remove(n.id) destructive=true\n            text \"Delete\"\n        column popover=\"auto\" id=`menu-${n.id}` role=\"menu\"\n          button press=remove(n.id) destructive=true role=\"menuitem\"\n            text \"Delete\"")).is_empty());
}

/// A `list` holding a `section` with no `appearance` is neither form, grouped
/// or cells, and draws its rows as written: Duo Notes' list. Each form, an
/// opt-out, a class that says `appearance`, a windowed list of section rows
/// (LLP 1083 D5) and a list that is no cell list for another reason
/// (sideways) are quiet; a component used twice is named once.
#[test]
fn a_sectioned_list_with_no_appearance_is_named() {
    let warned = |list: &str| -> Vec<(u32, String)> {
        let (plan, warnings) = contract::compile_warned(&app(list));
        plan.unwrap_or_else(|e| panic!("{e}"));
        warnings
            .into_iter()
            .filter(|w| w.id == "lower-list-section")
            .map(|w| (w.span.line, w.message))
            .collect()
    };
    let section = "  section\n    for n in notes key=n.id\n      row\n        text n.title";
    let found = warned(&format!("list flex=1\n{section}"));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, 15);
    assert!(
        found[0].1.contains("write `appearance=\"auto\"`")
            && found[0].1.contains("drop the `section`")
            && found[0].1.contains("say `appearance=\"none\"`")
            && found[0]
                .1
                .contains("put each of its children in a `section`"),
        "{}",
        found[0].1
    );
    // Through a choice, as `cells::rows` reads sections.
    let chosen = "list flex=1\n  if notes.length > 0\n    section\n      for n in notes key=n.id\n        row\n          text n.title";
    assert_eq!(warned(chosen).len(), 1);
    for list in [
        "list appearance=\"auto\" flex=1",
        "list appearance=\"auto\" listStyle=\"plain\" flex=1",
        "list appearance=\"none\" flex=1",
    ] {
        assert!(warned(&format!("{list}\n{section}")).is_empty(), "{list}");
    }
    // A class is read as lowering reads it: one that says `appearance` is that form,
    // and a `delete=` in its section is a row's; one of layout alone says nothing.
    // `class` is the list's class and `own` its other attributes, over `style Look`
    // and a `style Plain` of padding alone.
    let listed = |class: &str, own: &str, style: &str| {
        let src = format!(
            "{}\nstyle Look\n  {style}\nstyle Plain\n  padding=4\n",
            app(&format!("list class={class} {own} flex=1\n  section\n    for n in notes key=n.id\n      row delete=remove(n.id)\n        text n.title"))
        );
        let (plan, warnings) = contract::compile_warned(&src);
        plan.unwrap_or_else(|e| panic!("{e}"));
        warnings
            .into_iter()
            .map(|w| w.id)
            .filter(|id| id.contains("list") || id.contains("delete"))
            .collect::<Vec<_>>()
    };
    let classed = |style: &str| listed("Look", "", style);
    assert_eq!(classed("appearance=\"auto\""), Vec::<String>::new());
    // In rank order (`contract_lower::warning_rank`): the row that cannot delete first.
    assert_eq!(
        classed("padding=8"),
        ["lower-delete-row", "lower-list-section"]
    );
    // A class chosen between two styles is read as lowering reads it (`class::rows`):
    // a row both set alike is that row, and one they set differently is a choice,
    // which says no layout the cell rule can read.
    let chosen = "(opened == \"\" ? Look : Plain)";
    assert_eq!(
        listed(chosen, "", "padding=8"),
        ["lower-delete-row", "lower-list-section"]
    );
    assert_eq!(
        listed(chosen, "appearance=\"none\"", "padding=8"),
        ["lower-delete-row"]
    );
    assert_eq!(
        listed(chosen, "", "display=\"flex\"\n  flex-direction=\"row\""),
        ["lower-delete-row"]
    );
    // Two styles that agree on a cell list's layout and differ on the rest: its rows
    // are cells and their `delete=` swipes; two that differ on `display` make no cell
    // list, so no section is named and a row's `delete=` is.
    let pair = |a: &str, b: &str, rows: &str| {
        let src = format!(
            "{}\nstyle A\n  {a}\nstyle B\n  {b}\n",
            app(&format!(
                "list class=(opened == \"\" ? A : B) flex=1\n{rows}"
            ))
        );
        let (plan, warnings) = contract::compile_warned(&src);
        plan.unwrap_or_else(|e| panic!("{e}"));
        warnings
            .into_iter()
            .map(|w| w.id)
            .filter(|id| id.contains("list") || id.contains("delete"))
            .collect::<Vec<_>>()
    };
    let cell_rows = "  for n in notes key=n.id\n    row delete=remove(n.id)\n      text n.title";
    assert_eq!(
        pair(
            "overflow-x=\"auto\"\n  overflow-y=\"hidden\"",
            "overflow-x=\"scroll\"\n  overflow-y=\"hidden\"",
            cell_rows
        ),
        Vec::<String>::new()
    );
    let section_rows = "  section\n    for n in notes key=n.id\n      row delete=remove(n.id)\n        text n.title";
    assert_eq!(
        pair(
            "display=\"block\"\n  flex-direction=\"column\"",
            "display=\"flex\"\n  flex-direction=\"column\"",
            section_rows
        ),
        ["lower-delete-row"]
    );
    assert!(
        warned("list flex=1\n  for n in notes key=n.id\n    row\n      text n.title").is_empty()
    );
    let windowed = "list virtualized=true flex=1\n  for n in notes key=n.id\n    section\n      header position=\"sticky\" top=0\n        text n.title\n      row\n        text n.title";
    assert!(
        warned(windowed).is_empty(),
        "section rows of a windowed list"
    );
    let sideways = "list display=\"flex\" flex-direction=\"row\" overflow-x=\"scroll\" overflow-y=\"hidden\" height=120\n  for n in notes key=n.id\n    section width=200\n      text n.title";
    assert!(warned(sideways).is_empty(), "a carousel is no cell list");
    // A component used twice is inlined twice, and named once for its source.
    let twice = "shape Note\n  id: string\n  title: string\n\ncomponent App\n  state notes = [Note(id=\"1\", title=\"Groceries\")]\n  view\n    column\n      Notes(notes=notes)\n      Notes(notes=notes)\n\ncomponent Notes\n  props\n    notes: Note[]\n  view\n    list flex=1\n      section\n        for n in notes key=n.id\n          row\n            text n.title\n";
    let (plan, warnings) = contract::compile_warned(twice);
    plan.unwrap_or_else(|e| panic!("{e}"));
    let lines: Vec<u32> = warnings
        .iter()
        .filter(|w| w.id == "lower-list-section")
        .map(|w| w.span.line)
        .collect();
    assert_eq!(lines, [16]);
}
