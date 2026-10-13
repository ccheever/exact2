//! LLP 1115.000 D16: rows of a vertical `list` are cells. Contract decides
//! which lists, once (`contract/lower/src/cells.rs`), and marks each with
//! `listRows`, the prop every host reads; these hold the rule.

use exact_kernel::{Kernel, Offer, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// Whether the node `testId="it"` of a view holding `body` is marked.
fn marked(body: &str) -> bool {
    let r = boot(body);
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("it")[0]).unwrap();
    node.props.bool(PropId::ListRows) == Some(true)
}

/// A view holding `body`, booted and laid out at 402 by 874.
fn boot(body: &str) -> Runner<NoData> {
    let body = body
        .lines()
        .map(|l| format!("      {l}\n"))
        .collect::<String>();
    let source = format!(
        "style Sideways\n  display=\"flex\"\nstyle Down\n  display=\"flex\"\n  flex-direction=\"column\"\ncomponent App\n  state wide = false\n  action go\n    wide = not wide\n  view\n    column width=402 height=874\n{body}component Row\n  props\n    name: string\n  view\n    text name\ncomponent Part\n  view\n    section\n      text \"in a section\"\n"
    );
    let plan = contract::bake(
        contract::compile(&source).unwrap_or_else(|e| panic!("{e}")),
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
    let root = k.roots()[0];
    k.compute_layout(root, Offer::definite(402.0, 874.0))
        .unwrap();
    r
}

/// The wire id of the node `test_id` names.
fn id(r: &Runner<NoData>, test_id: &str) -> u32 {
    let k = r.kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

/// What a host that paints a list's rows asks the kernel
/// (`kernel/src/rows.rs`): which node is a row, and whether the platform's
/// separator belongs under it. The same six rows, with the same answers,
/// are the web's sheet's in `host/web/tests/neutral-look.test.mjs`.
#[test]
fn a_row_has_the_platforms_separator_under_it_unless_its_author_wrote_that_edge() {
    let r = boot(
        "list testId=\"it\"\n  text \"One\" testId=\"one\"\n  row press=go testId=\"two\"\n    text \"Two\" testId=\"inside\"\n  text \"Its own line\" border-bottom=\"2px solid red\" testId=\"own\"\n  text \"Before a row with a top edge\" testId=\"before\"\n  text \"No line above\" border-top=\"none\" testId=\"top\"\n  text \"Last\" testId=\"last\"\nlist appearance=\"none\" testId=\"opted-out\"\n  text \"The author's\" testId=\"none-1\"\n  text \"rows\"",
    );
    let k = r.kernel();
    let list = id(&r, "it");
    for row in ["one", "two", "own", "before", "top", "last"] {
        assert_eq!(k.row_list(id(&r, row)), Some(list), "{row} is a row");
    }
    let separated: Vec<bool> = ["one", "two", "own", "before", "top", "last"]
        .iter()
        .map(|row| k.row_separator(id(&r, row)))
        .collect();
    assert_eq!(separated, [true, true, false, false, true, false]);
    // What is in a row is in that row; the list itself is in none.
    assert_eq!(k.row_holding(id(&r, "inside")), Some(id(&r, "two")));
    assert_eq!(k.row_holding(id(&r, "two")), Some(id(&r, "two")));
    assert_eq!(k.row_holding(list), None);
    // A list whose author opted out has no rows of the platform's.
    assert_eq!(k.row_list(id(&r, "none-1")), None);
    assert!(!k.row_separator(id(&r, "none-1")));
    assert_eq!(k.row_holding(id(&r, "none-1")), None);
}

/// A windowed list's rows are in the runner's wrappers, and its spacers are
/// no rows: the wrapper is the row, and what its author wrote is read from
/// the row inside it.
#[test]
fn a_windowed_lists_rows_are_its_wrappers() {
    let r = boot(
        "list testId=\"it\" virtualized=true estimated-item-height=40 height=400\n  each n in [\"a\", \"b\", \"c\"] key=n\n    text n border-bottom=(n == \"b\" ? \"1px solid red\" : \"none\") testId=`row-${n}`",
    );
    let k = r.kernel();
    let list = k.node(id(&r, "it")).unwrap();
    let wrapper = |name: &str| k.node(id(&r, name)).unwrap().parent.unwrap();
    assert_eq!(k.row_list(wrapper("row-a")), Some(list.id));
    assert_eq!(
        k.row_list(id(&r, "row-a")),
        None,
        "the row inside is no row"
    );
    assert_eq!(k.row_holding(id(&r, "row-a")), Some(wrapper("row-a")));
    // Every row here wrote its bottom edge (`none` is written too).
    for name in ["row-a", "row-b", "row-c"] {
        assert!(!k.row_separator(wrapper(name)), "{name}");
    }
    let plain = boot(
        "list testId=\"it\" virtualized=true estimated-item-height=40 height=400\n  each n in [\"a\", \"b\", \"c\"] key=n\n    text n testId=`row-${n}`",
    );
    let k = plain.kernel();
    let wrapper = |name: &str| k.node(id(&plain, name)).unwrap().parent.unwrap();
    let separated: Vec<bool> = ["row-a", "row-b", "row-c"]
        .iter()
        .map(|name| k.row_separator(wrapper(name)))
        .collect();
    assert_eq!(separated, [true, true, false], "none after the last row");
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
    assert!(!marked(
        "list testId=\"it\" appearance=\"none\"\n  text \"a\""
    ));
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
