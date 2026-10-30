//! An alert's parts (`alertPart`, contract/lower/src/alert_parts.rs): the
//! compiler groups an alert-shaped `dialog role="alertdialog"` into its title,
//! a `message` box of its texts and an `actions` box of its buttons, a
//! prompt's field between them, so a host that draws the alert can scroll a
//! long message while the rest stays in view. A shape it cannot prove stays
//! as written.

use exact_kernel::{Kernel, PropId, ViewId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// A component whose view is `dialog`, indented under one `column`.
fn app(dialog: &str) -> String {
    let body: String = dialog.lines().map(|l| format!("      {l}\n")).collect();
    format!("component App\n  state busy = false\n  state name = \"\"\n  action go\n    busy = ! busy\n  action edit(v: string)\n    name = v\n  action pick(v: string)\n    name = v\n  view\n    column\n{body}")
}

/// The dialog `c`'s children by their test ids: a part as `message(…)` or
/// `actions(…)` with its rows, a marked title as `title:…`, any other row
/// by itself.
fn parts(dialog: &str) -> String {
    let plan = contract::compile(&app(dialog)).unwrap_or_else(|e| panic!("{e}"));
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let name = |id: ViewId| {
        k.node(id)
            .unwrap()
            .props
            .str(PropId::TestId)
            .unwrap_or("?")
            .to_owned()
    };
    let dialog = k.node_by_key(k.find_by_test_id("c")[0]).unwrap();
    dialog
        .children()
        .into_iter()
        .map(|id| {
            let row = k.node(id).unwrap();
            match row.props.str(PropId::AlertPart) {
                Some("title") => format!("title:{}", name(id)),
                Some(part) => format!(
                    "{part}({})",
                    row.children()
                        .into_iter()
                        .map(name)
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                None => name(id),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

const LABELLED: &str =
    "dialog id=\"c\" role=\"alertdialog\" aria-label=\"Couldn’t Save\" testId=\"c\"";
const UNLABELLED: &str = "dialog id=\"c\" role=\"alertdialog\" testId=\"c\"";
const OK: &str = "  button commandfor=\"c\" command=\"close\" testId=\"ok\"\n    text \"OK\"";
const SAVE: &str =
    "  button press=go commandfor=\"c\" command=\"close\" testId=\"save\"\n    text \"Save\"";

fn text(id: &str) -> String {
    format!("  text \"Words\" testId=\"{id}\"")
}

fn dialog(open: &str, rows: &[&str]) -> String {
    format!("{open}\n{}", rows.join("\n"))
}

#[test]
fn an_alert_is_grouped_into_its_message_and_its_actions() {
    // The recipe's one-button alert: its `aria-label` is the title
    assert_eq!(
        parts(&dialog(LABELLED, &[&text("m"), OK])),
        "message(m) actions(ok)"
    );
    // Its first line is the title when it has no label, outside the message
    assert_eq!(
        parts(&dialog(
            UNLABELLED,
            &[&text("t"), &text("m"), &text("n"), SAVE, OK]
        )),
        "title:t message(m,n) actions(save,ok)"
    );
    // A title alone, with no message
    assert_eq!(
        parts(&dialog(UNLABELLED, &[&text("t"), OK])),
        "title:t actions(ok)"
    );
    // A prompt's field stays between the message and the actions
    let field = "  input value=name input=edit aria-label=\"Name\" testId=\"f\"";
    assert_eq!(
        parts(&dialog(LABELLED, &[&text("m"), field, SAVE, OK])),
        "message(m) f actions(save,ok)"
    );
}

#[test]
fn control_flow_of_one_part_moves_into_it_whole() {
    let sometimes =
        "  if busy\n    text \"Busy\" testId=\"b\"\n  else\n    text \"Idle\" testId=\"i\"";
    assert_eq!(
        parts(&dialog(LABELLED, &[&text("m"), sometimes, OK])),
        "message(m,i) actions(ok)"
    );
    let action = "  if busy\n    button press=go commandfor=\"c\" command=\"close\" testId=\"stop\"\n      text \"Stop\"";
    assert_eq!(
        parts(&dialog(LABELLED, &[&text("m"), action, OK])),
        "message(m) actions(ok)"
    );
}

#[test]
fn a_shape_the_source_does_not_prove_stays_as_written() {
    // A chooser: four buttons
    let pick = |v: &str| {
        format!("  button press=pick(\"{v}\") commandfor=\"c\" command=\"close\" testId=\"{v}\"\n    text \"{v}\"")
    };
    let (a, b, c) = (pick("a"), pick("b"), pick("c"));
    assert_eq!(
        parts(&dialog(LABELLED, &[&text("m"), &a, &b, &c, OK])),
        "m a b c ok"
    );
    // The author lays the dialog out, or places a row in it
    let row = "dialog id=\"c\" role=\"alertdialog\" aria-label=\"Couldn’t Save\" testId=\"c\" flex-direction=\"row\"";
    assert_eq!(parts(&dialog(row, &[&text("m"), OK])), "m ok");
    let ordered = "  text \"Words\" testId=\"m\" order=2";
    assert_eq!(parts(&dialog(LABELLED, &[ordered, OK])), "m ok");
    // A choice whose arms hold a text and a button belongs in no one part
    let mixed = "  if busy\n    text \"Busy\" testId=\"b\"\n  else\n    button press=go commandfor=\"c\" command=\"close\" testId=\"go\"\n      text \"Go\"";
    assert_eq!(
        parts(&dialog(LABELLED, &[&text("m"), mixed, OK])),
        "m go ok"
    );
    // A row after the buttons would move
    assert_eq!(parts(&dialog(LABELLED, &[OK, &text("m")])), "ok m");
    // With no label, the title must be a text outside control flow
    let first = "  if busy\n    text \"Busy\" testId=\"b\"";
    assert_eq!(parts(&dialog(UNLABELLED, &[first, &text("m"), OK])), "m ok");
    // whose literal words show it is never empty, as a label's must
    let computed = "  text name testId=\"t\"";
    assert_eq!(
        parts(&dialog(UNLABELLED, &[computed, &text("m"), OK])),
        "t m ok"
    );
    let empty = "  text \" \" testId=\"t\"";
    assert_eq!(
        parts(&dialog(UNLABELLED, &[empty, &text("m"), OK])),
        "t m ok"
    );
    let named = "dialog id=\"c\" role=\"alertdialog\" aria-label=name testId=\"c\"";
    assert_eq!(parts(&dialog(named, &[&text("m"), OK])), "m ok");
    // A row whose size or place is its parent's (a percentage, an offset)
    let sized =
        "  button commandfor=\"c\" command=\"close\" testId=\"ok\" height=\"50%\"\n    text \"OK\"";
    assert_eq!(parts(&dialog(LABELLED, &[&text("m"), sized])), "m ok");
    let placed = "  text \"Words\" testId=\"m\" position=\"absolute\" top=0";
    assert_eq!(parts(&dialog(LABELLED, &[placed, OK])), "m ok");
}

#[test]
fn alert_part_is_the_compilers() {
    let e = contract::compile(&app(&dialog(
        LABELLED,
        &["  text \"Words\" alertPart=\"message\"", OK],
    )))
    .unwrap_err();
    assert_eq!(e.id, "lower-alert-part", "{}", e.message);
}

/// What moves into a part keeps working there: a keyed `for` reorders its
/// rows, a `match` binds its value, and a button in the actions box runs its
/// action.
#[test]
fn rows_in_a_part_keep_their_keys_bindings_and_actions() {
    let src = r#"component App
  state order = ["a", "b", "c"]
  state picked = some("b")
  action flip
    order = ["c", "b", "a"]
    picked = none
  view
    column
      dialog id="c" role="alertdialog" aria-label="Reorder" testId="c"
        for x in order key=x
          text x testId=x
        match picked
          case some(p)
            text `Picked ${p}` testId="picked"
          case none
            text "None picked" testId="none"
        button press=flip commandfor="c" command="close" testId="flip"
          text "Flip"
        button commandfor="c" command="close" testId="ok"
          text "OK"
"#;
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let rows = |r: &Runner<NoData>| {
        let k = r.kernel();
        let dialog = k.node_by_key(k.find_by_test_id("c")[0]).unwrap();
        let message = k.node(dialog.children()[0]).unwrap();
        assert_eq!(message.props.str(PropId::AlertPart), Some("message"));
        let words = |id| {
            let row = k.node(id).unwrap();
            format!(
                "{}:{}",
                row.props.str(PropId::TestId).unwrap_or("?"),
                row.props.str(PropId::Text).unwrap_or("")
            )
        };
        message
            .children()
            .into_iter()
            .map(words)
            .collect::<Vec<_>>()
    };
    assert_eq!(rows(&r), ["a:a", "b:b", "c:c", "picked:Picked b"]);
    let a = r.kernel().find_by_test_id("a")[0];
    let flip = r
        .kernel()
        .node_by_key(r.kernel().find_by_test_id("flip")[0])
        .unwrap()
        .id;
    r.dispatch(flip, exact_runner::Event::Press).unwrap();
    assert_eq!(rows(&r), ["c:c", "b:b", "a:a", "none:None picked"]);
    assert_eq!(
        r.kernel().find_by_test_id("a")[0],
        a,
        "the keyed row moved, not rebuilt"
    );
}

/// The message box is a group named by the alert's title, in the page as
/// compiled, so a page rendered ahead has its name before its runtime loads.
#[test]
fn the_message_box_is_a_group_named_by_the_title() {
    let named = |dialog: &str| {
        let plan = contract::compile(&app(dialog)).unwrap_or_else(|e| panic!("{e}"));
        let r = Runner::boot(
            plan,
            NoData,
            Kernel::with_monospace(),
            Default::default(),
            "/",
        )
        .unwrap();
        let k = r.kernel();
        let dialog = k.node_by_key(k.find_by_test_id("c")[0]).unwrap();
        let message = dialog
            .children()
            .into_iter()
            .map(|id| k.node(id).unwrap())
            .find(|n| n.props.str(PropId::AlertPart) == Some("message"))
            .unwrap();
        let prop = |p| message.props.str(p).map(str::to_owned);
        (
            prop(PropId::AccessibilityRole),
            prop(PropId::AccessibilityLabel),
        )
    };
    let group = |title: &str| (Some("group".to_owned()), Some(title.to_owned()));
    assert_eq!(
        named(&dialog(LABELLED, &[&text("m"), OK])),
        group("Couldn’t Save")
    );
    let title = "  text \"Remove Book?\" testId=\"t\"";
    assert_eq!(
        named(&dialog(UNLABELLED, &[title, &text("m"), OK])),
        group("Remove Book?")
    );
}
