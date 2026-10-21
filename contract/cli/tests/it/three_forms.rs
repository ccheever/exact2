//! LLP 1116.017: the three forms the compiler accepts, each compiled to the
//! plan of the form it means, byte for byte.

fn same(written: &str, means: &str) {
    let a = contract::compile(written).unwrap_or_else(|e| panic!("{e}\n{written}"));
    let b = contract::compile(means).unwrap_or_else(|e| panic!("{e}\n{means}"));
    assert_eq!(a.encode(), b.encode(), "\n{written}\nis not\n{means}");
}

fn view(lines: &str) -> String {
    let body: String = lines.lines().map(|l| format!("      {l}\n")).collect();
    format!("component App\n  state on = true\n  state busy = false\n  view\n    column\n{body}")
}

#[test]
fn h1_to_h6_are_headings_of_their_level() {
    for level in 1..=6 {
        same(
            &view(&format!("h{level} \"Recipes\" testId=\"title\"")),
            &view(&format!(
                "text \"Recipes\" testId=\"title\" role=\"heading\" aria-level={level}"
            )),
        );
    }
    let e = contract::compile(&view("h2 \"Recipes\" aria-level=3")).unwrap_err();
    assert_eq!(e.id, "syntax-heading-level", "{e}");
}

#[test]
fn a_fn_body_may_start_on_the_next_line() {
    let app = "component App\n  view\n    text (mood(5))\n";
    same(
        &format!("fn mood(n: number): string =\n  n >= 4 ? \"Good\" : \"Low\"\n{app}"),
        &format!("fn mood(n: number): string = n >= 4 ? \"Good\" : \"Low\"\n{app}"),
    );
    // One expression: a second line at its level is refused.
    let e =
        contract::compile(&format!("fn mood(n: number): string =\n  n\n  n\n{app}")).unwrap_err();
    assert_eq!(e.id, "syntax-expected-expression", "{e}");
}

#[test]
fn a_footer_that_chooses_its_text_is_one_text() {
    let list = |footer: &str| {
        let footer: String = footer.lines().map(|l| format!("    {l}\n")).collect();
        view(&format!(
            "list appearance=\"auto\"\n  section\n    row\n      text \"x\"\n{footer}"
        ))
    };
    same(
        &list("footer\n  if on\n    text \"On\" testId=\"f\"\n  else if busy\n    text \"Busy\" testId=\"f\"\n  else\n    text \"Off\" testId=\"f\""),
        &list("footer\n  text (on ? \"On\" : busy ? \"Busy\" : \"Off\") testId=\"f\""),
    );
    let headed = |header: &str| {
        let header: String = header.lines().map(|l| format!("    {l}\n")).collect();
        view(&format!(
            "list appearance=\"auto\"\n  section\n{header}    row\n      text \"x\"\n"
        ))
    };
    same(
        &headed("header\n  if on\n    text \"On\"\n  else\n    text \"Off\""),
        &headed("header\n  text (on ? \"On\" : \"Off\")"),
    );
    // Arms that differ in more than their words, and a footer that comes and
    // goes, stay refused.
    for footer in [
        "footer\n  if on\n    text \"On\" testId=\"a\"\n  else\n    text \"Off\" testId=\"b\"",
        "footer\n  if on\n    text \"On\"",
    ] {
        let e = contract::compile(&list(footer)).unwrap_err();
        assert_eq!(e.id, "lower-grouped-list", "{e}");
    }
}
