//! exact2's menu rule's build note (LLP 1021; theme2 2026-10-21 19:47,
//! 22:05): a row of a menu whose `popovertarget` names a popover that is no
//! menu is noted, saying where it opens (`commandfor` too); a submenu, a
//! hide (any case), a confirmation, a role the source doesn't settle and a
//! button outside a menu are not.

const APP: &str = r#"component App
  view
    column
      button popovertarget="message-menu" testId="opener"
        text "Message"
      column id="message-menu" popover="auto" role="menu"
        button popovertarget="picker"
          text "Add Reaction"
        button popovertarget="more"
          text "More"
        button popovertarget="message-menu" popovertargetaction="hide"
          text "Close"
        button commandfor="picker" command="show-popover"
          text "Pick (command)"
        button popovertarget="picker" popovertargetaction="HIDE"
          text "Hide the picker"
        button popovertarget="confirm"
          text "Delete…"
        button popovertarget="unsure"
          text "Unsure"
        button commandfor="" popovertarget="picker"
          text "Pick (no command target)"
      column id="more" popover="auto" role="menu"
        button
          text "Deep"
      column id="confirm" popover="auto" role="alertdialog" aria-label="Delete?"
        text "Delete?"
        button popovertarget="confirm" popovertargetaction="hide"
          text "Cancel"
      column id="unsure" popover="auto" role=(1 > 0 ? "menu" : "list")
        button
          text "x"
      column id="picker" popover="auto"
        button popovertarget="picker" popovertargetaction="hide"
          text "Done"
      button popovertarget="picker"
        text "Pick"
"#;

#[test]
fn a_menu_rows_popover_that_is_no_menu_is_noted_where_it_opens() {
    let (result, warnings) = contract::compile_warned(APP);
    result.unwrap_or_else(|e| panic!("{e}"));
    let notes: Vec<(String, u32)> = warnings
        .iter()
        .map(|w| (w.id.clone(), w.span.line))
        .collect();
    assert_eq!(
        notes,
        vec![
            ("lower-menu-row-popover".to_string(), 7),
            ("lower-menu-row-popover".to_string(), 13),
            ("lower-menu-row-popover".to_string(), 21)
        ],
        "the picker's three rows, by popovertarget, by commandfor, and by popovertarget past an empty commandfor: {warnings:?}"
    );
    let message = &warnings[0].message;
    assert!(
        message.starts_with("note: ") && message.contains("menu's own invoker"),
        "{message}"
    );
}

/// A target two arms declare differently (a menu in one, no menu in the
/// other) is unsettled: no note.
#[test]
fn a_target_two_arms_declare_differently_earns_no_note() {
    let (result, warnings) = contract::compile_warned(
        "component App\n  state a = true\n  view\n    column\n      column id=\"menu\" popover=\"auto\" role=\"menu\"\n        button popovertarget=\"next\"\n          text \"Next\"\n      if a\n        column id=\"next\" popover=\"auto\" role=\"menu\"\n          button\n            text \"x\"\n      else\n        column id=\"next\" popover=\"auto\"\n          text \"y\"\n",
    );
    result.unwrap_or_else(|e| panic!("{e}"));
    assert!(
        warnings.iter().all(|w| w.id != "lower-menu-row-popover"),
        "{warnings:?}"
    );
}
