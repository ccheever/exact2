use super::warnings::warned;

const ID: &str = "lower-header-routeless";
const HEADER: &str = r#"header display="flex"
  text "Household" role="heading" aria-level=1
  text "3 items"
  button disabled=true
    text "Persona""#;

fn app(view: &str) -> String {
    let view: String = view.lines().map(|line| format!("    {line}\n")).collect();
    format!("component App\n  state shown = true\n  view\n{view}")
}

fn screen(header: &str) -> String {
    let header: String = header.lines().map(|line| format!("  {line}\n")).collect();
    format!("column\n{header}")
}

fn warnings(src: &str) -> Vec<(String, u32)> {
    warned(src).into_iter().filter(|(id, _)| id == ID).collect()
}

#[test]
fn a_routeless_header_names_the_navigation_root_and_screen_it_needs() {
    let src = app(&screen(HEADER));
    assert_eq!(warnings(&src), [(ID.into(), 5)]);
    let (_, notes) = contract::compile_warned(&src);
    let note = notes.iter().find(|note| note.id == ID).unwrap();
    for phrase in [
        "on iOS",
        "navigation root",
        "separately keyed screen",
        "first child",
        "navigationKey",
        "appearance=\"none\"",
        "docs/recipes/navigation-stack.md",
    ] {
        assert!(note.message.contains(phrase), "{note}");
    }
    assert_eq!(contract_lower::warning_rank::rank(ID, &note.message), 0);
    for view in [HEADER.to_string(), screen(&screen(HEADER))] {
        assert_eq!(warnings(&app(&view)).len(), 1, "{view}");
    }
    let component = format!(
        "{}component Home\n  view\n{}",
        app("Home()"),
        screen(HEADER)
            .lines()
            .map(|line| format!("    {line}\n"))
            .collect::<String>()
    );
    assert_eq!(warnings(&component).len(), 1);
    // A conditional button cannot change the heading count (design-tool).
    for tail in [
        "if shown\n  button disabled=true\n    text \"Persona\"",
        "match some(1)\n  case some(n)\n    text n.toString()\n  case none\n    text \"None\"",
        "for n in [1, 2] key=n\n  text n.toString()",
    ] {
        let header = format!(
            "{HEADER}\n{}",
            tail.lines()
                .map(|line| format!("  {line}\n"))
                .collect::<String>()
        );
        assert_eq!(warnings(&app(&screen(&header))).len(), 1, "{header}");
    }
}

#[test]
fn only_a_first_header_with_one_heading_is_named() {
    for view in [
        screen(&HEADER.replace(" role=\"heading\" aria-level=1", "")),
        screen(&HEADER.replace(" role=\"heading\"", "")),
        screen(&HEADER.replace("text \"3 items\"", "text \"3 items\" role=\"heading\"")),
        screen(HEADER).replace("column\n", "column\n  text \"First\"\n"),
        screen(HEADER).replace("column", "section"),
        screen(HEADER).replace("column", "scroll height=400"),
        screen(&format!(
            "if shown\n{}",
            HEADER
                .lines()
                .map(|line| format!("  {line}\n"))
                .collect::<String>()
        )),
        screen(&HEADER.replace(
            "role=\"heading\"",
            "role=(shown ? \"heading\" : \"status\")",
        )),
        screen("header\n  if shown\n    text \"Title\" role=\"heading\""),
        screen(&format!("{HEADER}\n  if shown\n    text \"Second\" role=\"heading\"")),
        screen(&format!("{HEADER}\n  match some(1)\n    case some(n)\n      text n.toString() role=\"heading\"\n    case none\n      text \"None\"")),
        screen(&format!("{HEADER}\n  for n in [1] key=n\n    text n.toString() role=\"heading\"")),
    ] {
        let src = app(&view);
        assert!(warnings(&src).is_empty(), "{src}");
    }
}

#[test]
fn hidden_computed_and_authored_headers_or_wrappers_get_the_benefit() {
    for (node, attr) in [
        ("header", "appearance=\"none\""),
        ("header", "appearance=(shown ? \"none\" : \"auto\")"),
        ("header", "display=\"none\""),
        ("header", "display=(shown ? \"flex\" : \"none\")"),
        ("column", "display=\"none\""),
        ("column", "display=(shown ? \"flex\" : \"none\")"),
    ] {
        let view = screen(&HEADER.replace(" display=\"flex\"", ""));
        let src = app(&view.replacen(node, &format!("{node} {attr}"), 1));
        assert!(warnings(&src).is_empty(), "{src}");
    }
    for (node, row) in [
        ("header", "appearance=\"none\""),
        ("header", "display=\"none\""),
        ("column", "display=\"none\""),
    ] {
        let view = screen(&HEADER.replace(" display=\"flex\"", ""));
        let styled = app(&view.replacen(node, &format!("{node} class=Hidden"), 1));
        let src = format!("style Hidden\n  {row}\n{styled}");
        assert!(warnings(&src).is_empty(), "{src}");
        let choice = src.replace("class=Hidden", "class=(shown ? Hidden : Visible)");
        let choice = format!("style Visible\n  display=\"flex\"\n{choice}");
        assert!(warnings(&choice).is_empty(), "{choice}");
        let override_row = row.replace(
            "none",
            if row.starts_with("display") {
                "flex"
            } else {
                "auto"
            },
        );
        let overridden = src.replace("class=Hidden", &format!("class=Hidden {override_row}"));
        assert_eq!(warnings(&overridden).len(), 1, "{overridden}");
    }
}

#[test]
fn navigation_keys_anywhere_in_the_expanded_view_suppress_the_warning() {
    for tail in [
        "column navigationKey=\"home\"",
        "if shown\n  column navigationKey=\"home\"",
        "if shown\n  text \"Other\"\nelse\n  column navigationKey=\"home\"",
        "match some(1)\n  case some(n)\n    column navigationKey=\"home\"\n  case none\n    text \"Other\"",
        "match some(1)\n  case some(n)\n    text \"Other\"\n  case none\n    column navigationKey=\"home\"",
        "for n in [1] key=n\n  column navigationKey=\"home\"",
        "Home()",
    ] {
        let view = format!("{}{}", screen(HEADER), tail.lines().map(|line| format!("  {line}\n")).collect::<String>());
        let src = format!("{}component Home\n  view\n    column navigationKey=\"home\"\n", app(&view));
        assert!(warnings(&src).is_empty(), "{src}");
    }
    let repaired = screen(&screen(HEADER).replacen("column", "column navigationKey=\"home\"", 1))
        .replacen("column", "column navigationKey=\"home\"", 1);
    assert!(warnings(&app(&repaired)).is_empty());
}
