use super::warnings::warned;

const ID: &str = "lower-list-beside-fixed";

/// A route with an inline title over `body`, its lines at the route's depth.
fn route(body: &str) -> String {
    let body: String = body.lines().map(|l| format!("          {l}\n")).collect();
    format!("routes nav\n  home \"/\"\ncomponent App\n  state q = \"\"\n  state shown = true\n  action go\n    q = \"x\"\n  action edit(value: string)\n    q = value\n  view\n    main navigationKey=`${{top(nav).id}}` width=\"100%\" height=\"100%\"\n      for e in stack(nav) key=e.id\n        column navigationKey=`${{e.id}}` position=\"absolute\" inset=0\n          header\n            text \"Inbox\" role=\"heading\" aria-level=2\n{body}")
}

fn warnings(src: &str) -> Vec<(String, u32)> {
    warned(src).into_iter().filter(|(id, _)| id == ID).collect()
}

const LIST: &str = "list flex=1 min-height=0\n  text \"row\"";

/// Inbox's footer (bake-off r24): a count under the list, and a filter row
/// with a fill and a count over a list in a box.
#[test]
fn a_caption_or_a_filled_row_beside_the_list_is_named_with_the_repair() {
    let footer = route(&format!("{LIST}\nrow\n  text \"320 messages\""));
    assert_eq!(warnings(&footer), [(ID.into(), 18)]);
    let (_, notes) = contract::compile_warned(&footer);
    let note = notes.iter().find(|n| n.id == ID).unwrap();
    for phrase in [
        "below the list",
        "hard line",
        "section's `header` or `footer`",
        "list-rows.md",
    ] {
        assert!(note.message.contains(phrase), "{phrase}: {}", note.message);
    }
    let filled = route("row background-color=\"-exact-background\"\n  button press=go\n    text \"All\"\ncolumn flex=1 min-height=0\n  list flex=1 min-height=0\n    text \"row\"");
    assert_eq!(warnings(&filled), [(ID.into(), 16)]);
}

/// What the list runs under (LLP 1131 D1), and what may not be beside it.
#[test]
fn controls_a_conditional_box_a_side_or_no_bar_is_not_named() {
    for body in [
        // A composer of a field and a button with a border; a switcher.
        format!("{LIST}\nrow border=\"1px solid -exact-separator\" margin=8\n  textarea value=q input=edit aria-label=\"Message\"\n  button press=go\n    text \"Send\""),
        format!("row role=\"tablist\"\n  button role=\"tab\" press=go\n    text \"A\"\n{LIST}"),
        // A banner that comes and goes, a floating button, a list's sibling across.
        format!("if shown\n  text \"Offline\"\n{LIST}\nbutton press=go position=\"absolute\" bottom=8 right=8\n  text \"Up\""),
        // A list sized by its author, not taking the room.
        "text \"Recent\"\nlist height=200\n  text \"row\"".to_string(),
    ] {
        assert_eq!(warnings(&route(&body)), [], "{body}");
    }
    // A route with no bar: its first child is no header.
    let bare = route(&format!("{LIST}\ntext \"320 messages\"")).replace(
        "          header\n            text \"Inbox\" role=\"heading\" aria-level=2\n",
        "",
    );
    assert_eq!(warnings(&bare), []);
}

/// Contacts' own section index beside its list (bake-off r28), shown or
/// shown sometimes: the row is named with the platform's index as the repair.
#[test]
fn a_list_in_a_row_beside_another_box_is_named() {
    for side in ["text \"side\"", "if shown\n    text \"A–Z\" width=24"] {
        let body = format!(
            "row flex=1 min-height=0\n  list flex=1 min-height=0\n    text \"row\"\n  {side}"
        );
        assert_eq!(warnings(&route(&body)), [(ID.into(), 16)], "{body}");
        let (_, notes) = contract::compile_warned(&route(&body));
        let note = notes.iter().find(|n| n.id == ID).unwrap();
        for phrase in ["large title never collapses", "headed by one letter"] {
            assert!(note.message.contains(phrase), "{phrase}: {}", note.message);
        }
    }
    // A pane a wide window adds; a floating button over the list is in no row beside it.
    let pane = route("row flex=1 min-height=0\n  list flex=1 min-height=0\n    text \"row\"\n  if shown\n    column width=280\n      text \"Members\"");
    assert_eq!(warnings(&pane), []);
    let floating = route("row flex=1 min-height=0\n  list flex=1 min-height=0\n    text \"row\"\n  button press=go position=\"absolute\" bottom=8 right=8\n    text \"Up\"");
    assert_eq!(warnings(&floating), []);
}
