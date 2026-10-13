//! LLP 1115.000 D16: rows of a vertical `list` are cells. Contract decides
//! which lists, once (`contract/lower/src/cells.rs`), and marks each with
//! `listRows`, the prop every host reads; these hold the rule.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Whether the node `testId="it"` of a view holding `body` is marked.
fn marked(body: &str) -> bool {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    let source = format!(
        "style Sideways\n  display=\"flex\"\nstyle Down\n  display=\"flex\"\n  flex-direction=\"column\"\ncomponent App\n  state wide = false\n  view\n    column width=402 height=874\n{body}component Row\n  props\n    name: string\n  view\n    text name\ncomponent Part\n  view\n    section\n      text \"in a section\"\n"
    );
    let plan = contract::bake(
        contract::compile(&source).unwrap_or_else(|e| panic!("{e}")),
        NoData,
    )
    .unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("it")[0]).unwrap();
    node.props.bool(PropId::ListRows) == Some(true)
}

#[test]
fn a_list_whose_rows_go_down_the_page_is_marked() {
    assert!(marked("list testId=\"it\"\n  text \"a\""));
    assert!(marked(
        "list testId=\"it\" virtualized=true estimated-item-height=40 height=200\n  each n in [\"a\", \"b\"] key=n\n    text n"
    ));
    assert!(marked(
        "list testId=\"it\" display=\"flex\" flex-direction=\"column\"\n  text \"a\""
    ));
    assert!(marked("list testId=\"it\" class=Down\n  text \"a\""));
    // A component's root is the row; an empty state in the list is a row too.
    assert!(marked(
        "list testId=\"it\"\n  each n in [\"a\", \"b\"] key=n\n    Row(name=n)\n  when wide\n    text \"none\""
    ));
}

#[test]
fn the_author_opts_out_and_the_grouped_form_has_its_own_rows() {
    assert!(!marked("list testId=\"it\" appearance=\"none\"\n  text \"a\""));
    assert!(!marked(
        "list testId=\"it\" appearance=\"auto\"\n  section\n    text \"a\""
    ));
    assert!(!marked("scroll testId=\"it\" height=100\n  text \"a\""));
}

#[test]
fn a_list_laid_out_sideways_is_a_carousel() {
    assert!(!marked("list testId=\"it\" display=\"flex\"\n  text \"a\""));
    assert!(!marked(
        "list testId=\"it\" display=\"flex\" flex-direction=\"row\"\n  text \"a\""
    ));
    assert!(!marked("list testId=\"it\" class=Sideways\n  text \"a\""));
    assert!(!marked(
        "list testId=\"it\" display=\"grid\" grid-template-columns=\"1fr 1fr\"\n  text \"a\""
    ));
    assert!(!marked(
        "list testId=\"it\" virtualized=true display=\"flex\" height=120 estimated-item-width=120\n  each n in [\"a\", \"b\"] key=n\n    text n"
    ));
    assert!(!marked(
        "list testId=\"it\" height=120 overflow-x=\"scroll\" overflow-y=\"hidden\"\n  text \"a\""
    ));
    // A direction that is not a literal is not known to go down the page.
    assert!(!marked(
        "list testId=\"it\" display=\"flex\" flex-direction=(wide ? \"row\" : \"column\")\n  text \"a\""
    ));
}

#[test]
fn a_list_of_sections_keeps_its_rows() {
    assert!(!marked("list testId=\"it\"\n  section\n    text \"a\""));
    assert!(!marked(
        "list testId=\"it\"\n  each n in [\"a\", \"b\"] key=n\n    section\n      text n"
    ));
    // Through a component too: the inliner has put its root in the list.
    assert!(!marked("list testId=\"it\"\n  Part()"));
}

#[test]
fn an_author_cannot_write_the_mark() {
    let source = "component App\n  view\n    list listRows=true\n      text \"a\"\n";
    assert!(contract::compile(source).is_err());
}
