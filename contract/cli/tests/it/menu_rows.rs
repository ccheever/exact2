//! exact2's menu rule's build note (LLP 1021; theme2 2026-10-21 19:47,
//! 22:05): a row of a menu whose `popovertarget` names a popover that is no
//! menu is noted, saying where it opens; a submenu, a row that hides its
//! menu and a button outside a menu are not.

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
      column id="more" popover="auto" role="menu"
        button
          text "Deep"
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
        vec![("lower-menu-row-popover".to_string(), 7)],
        "{warnings:?}"
    );
    let message = &warnings[0].message;
    assert!(
        message.starts_with("note: ") && message.contains("menu's own invoker"),
        "{message}"
    );
}
