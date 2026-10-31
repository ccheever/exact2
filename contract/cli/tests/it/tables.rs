//! HTML's table and description list (LLP 1153 D7): each element is a
//! `View` its `semanticTag` names, a cell's `colspan`, `rowspan` and `scope`
//! are props, and the compiler holds them to HTML's content model.

use exact_kernel::{Kernel, PropId, PropValue};
use exact_runner::{DataError, DataSource, Runner, Value};
use std::process::Command;

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _args: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.to_string()))
    }
}

fn boot(src: &str) -> Runner<NoData> {
    Runner::boot(
        contract::compile(src).unwrap_or_else(|e| panic!("{e}")),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

fn prop(r: &Runner<NoData>, test_id: &str, id: PropId) -> Option<PropValue> {
    let key = r.kernel().find_by_test_id(test_id)[0];
    r.kernel().node_by_key(key).unwrap().props.get(id).cloned()
}

fn tag(r: &Runner<NoData>, test_id: &str) -> Option<String> {
    match prop(r, test_id, PropId::SemanticTag) {
        Some(PropValue::Str(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn refused(view: &str) -> (String, String) {
    let e = contract::compile(&format!(
        "component App\n  state xs = [1, 2]\n  view\n{view}"
    ))
    .unwrap_err();
    (e.id.clone(), e.to_string())
}

const TABLE: &str = "component App
  state up = true
  state moves = [\"Tackle\", \"Growl\"]
  view
    column
      table testId=\"table\"
        caption testId=\"caption\"
          text \"Moves\"
        thead testId=\"head\"
          tr testId=\"head-row\"
            th scope=\"col\" aria-sort=(up ? \"ascending\" : \"descending\") testId=\"name\"
              text \"Name\"
            th scope=\"col\" testId=\"power\"
              text \"Power\"
        tbody testId=\"body\"
          for m in moves key=m
            tr
              th scope=\"row\"
                text m
              td colspan=2 rowspan=1 testId=(m + \"-cell\")
                text \"40\"
        tfoot testId=\"foot\"
          tr
            td testId=\"total\"
              text \"2 moves\"
      dl testId=\"dl\"
        dt testId=\"dt\"
          text \"Type\"
        dd testId=\"dd\"
          text \"Electric\"
";

/// Each element is a `View` its `semanticTag` names, with a cell's
/// attributes as props and `aria-sort` the prop it already is.
#[test]
fn the_elements_are_views_by_their_html_names() {
    let r = boot(TABLE);
    for (id, name) in [
        ("table", "table"),
        ("caption", "caption"),
        ("head", "thead"),
        ("head-row", "tr"),
        ("name", "th"),
        ("body", "tbody"),
        ("Tackle-cell", "td"),
        ("foot", "tfoot"),
        ("dl", "dl"),
        ("dt", "dt"),
        ("dd", "dd"),
    ] {
        assert_eq!(tag(&r, id).as_deref(), Some(name), "{id}");
        let key = r.kernel().find_by_test_id(id)[0];
        let node = r.kernel().node_by_key(key).unwrap();
        assert_eq!(node.node_type, exact_kernel::NodeType::View, "{id}");
    }
    assert_eq!(
        prop(&r, "name", PropId::Scope),
        Some(PropValue::Str("col".into()))
    );
    assert_eq!(
        prop(&r, "name", PropId::AccessibilitySort),
        Some(PropValue::Str("ascending".into()))
    );
    assert_eq!(
        prop(&r, "Tackle-cell", PropId::ColSpan),
        Some(PropValue::Int(2))
    );
    assert_eq!(
        prop(&r, "Growl-cell", PropId::RowSpan),
        Some(PropValue::Int(1))
    );
    assert_eq!(prop(&r, "power", PropId::ColSpan), None, "unset is 1");
}

/// Where HTML's content model puts each element, and what a cell's
/// attributes take: every refusal says what goes where.
#[test]
fn the_content_model_and_the_cell_attributes_are_held() {
    for (view, id, says) in [
        (
            "    tr\n      td\n        text \"a\"\n",
            "lower-table-content",
            "`tr` belongs in a `table`, `thead`, `tbody` or `tfoot`",
        ),
        (
            "    table\n      td\n        text \"a\"\n",
            "lower-table-content",
            "`td` belongs in a `tr`",
        ),
        (
            "    column\n      th\n        text \"a\"\n",
            "lower-table-content",
            "`th` belongs in a `tr`",
        ),
        (
            "    table\n      text \"a\"\n",
            "lower-table-content",
            "a `table` holds `caption`, `thead`, `tbody`, `tfoot` and `tr`, not `text`",
        ),
        (
            "    table\n      tbody\n        column\n",
            "lower-table-content",
            "a `tbody` holds `tr`s, not `column`",
        ),
        (
            "    table\n      tr\n        text \"a\"\n",
            "lower-table-content",
            "a `tr` holds `th` and `td`, not `text`",
        ),
        (
            "    column\n      dt\n        text \"a\"\n",
            "lower-dl-content",
            "`dt` belongs in a `dl`",
        ),
        (
            "    column\n      dd\n        text \"a\"\n",
            "lower-dl-content",
            "`dd` belongs in a `dl`",
        ),
        (
            "    dl\n      text \"a\"\n",
            "lower-dl-content",
            "a `dl` holds `dt` and `dd`, not `text`",
        ),
        (
            "    dl\n      dd\n        text \"a\"\n",
            "lower-dl-content",
            "a `dl`'s first group starts with its `dt`",
        ),
        (
            "    dl\n      dt\n        text \"a\"\n      dd\n        text \"b\"\n      dt\n        text \"c\"\n",
            "lower-dl-content",
            "a `dt` is followed by its `dd`",
        ),
        (
            "    dl\n      dd\n        text \"a\"\n      for x in xs key=x\n        dt\n          text \"t\"\n        dd\n          text \"d\"\n",
            "lower-dl-content",
            "a `dl`'s first group starts with its `dt`",
        ),
        (
            "    dl\n      for x in xs key=x\n        dt\n          text \"t\"\n        dd\n          text \"d\"\n      dt\n        text \"last\"\n",
            "lower-dl-content",
            "a `dt` is followed by its `dd`",
        ),
        (
            "    table\n      tr\n        td colspan=0\n          text \"a\"\n",
            "lower-attr-value",
            "`colspan` is a whole number of columns from 1 to 1000 (HTML's limit), written as a number: `colspan=2`",
        ),
        (
            "    table\n      tr\n        td rowspan=\"2\"\n          text \"a\"\n",
            "lower-attr-value",
            "`rowspan` is a whole number of rows from 0 (the rest of its row group) to 65534",
        ),
        (
            "    table\n      tr\n        td colspan=1.5\n          text \"a\"\n",
            "lower-attr-value",
            "`colspan` is a whole number",
        ),
        (
            "    column colspan=2\n",
            "lower-attr-tag",
            "`colspan` belongs to `th` or `td`",
        ),
        (
            "    table\n      tr\n        td scope=\"col\"\n          text \"a\"\n",
            "lower-attr-tag",
            "`scope` belongs to `th`",
        ),
        (
            "    table\n      tr\n        th scope=\"both\"\n          text \"a\"\n",
            "lower-attr-value",
            "`scope` takes \"col\" (the header of its column) or \"row\" (of its row)",
        ),
    ] {
        let (got, message) = refused(view);
        assert_eq!(got, id, "{view}: {message}");
        assert!(message.contains(says), "{view}: {message}");
    }
}

/// Rows from a `for` and a cell under an `if` are a table's; a `dl`'s
/// groups from a `for` are the data's.
#[test]
fn rows_may_repeat_and_a_cell_may_come_and_go() {
    contract::compile(
        "component App\n  state xs = [1, 2]\n  state on = true\n  view\n    column\n      table\n        for x in xs key=x\n          tr\n            td\n              text \"a\"\n            if on\n              td\n                text \"b\"\n      dl\n        for x in xs key=x\n          dt\n            text \"t\"\n          dd\n            text \"d\"\n      dl\n        dt\n          text \"one\"\n        dt\n          text \"two\"\n        dd\n          text \"both\"\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
}

/// A row's cells may come from a `for`, as HTML's do from any template:
/// the kernel places them, and the written cells below share their columns.
#[test]
fn a_rows_cells_may_come_from_a_for() {
    let mut r = boot(
        "component App\n  state names = [\"Name\", \"Power\", \"Accuracy\"]\n  view\n    table\n      thead\n        tr\n          for h in names key=h\n            th testId=h\n              view width=40 height=10\n      tbody\n        tr\n          td testId=\"a\"\n            view width=20 height=10\n          td testId=\"b\"\n            view width=20 height=10\n          td testId=\"c\"\n            view width=20 height=10\n",
    );
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, exact_kernel::Offer::definite(400.0, 400.0))
        .unwrap();
    let x = |id: &str| {
        let key = r.kernel().find_by_test_id(id)[0];
        r.kernel().node_by_key(key).unwrap().frame.x
    };
    let heads = ["Name", "Power", "Accuracy"].map(x);
    assert_eq!(heads, [2.0, 46.0, 90.0], "three 42 px columns");
    assert_eq!(["a", "b", "c"].map(x), heads, "each cell under its heading");
}

/// The rows of an `if` that makes rows in one branch and a row group in the
/// other are in a `tbody` inside the branch, where the browser's parser
/// would put them: no branch leaves a `tr` the table's own child.
#[test]
fn rows_in_a_mixed_branch_are_in_a_tbody_of_their_own() {
    for open in [true, false] {
        let r = boot(&format!(
            "component App\n  state open = {open}\n  view\n    table testId=\"t\"\n      if open\n        tr testId=\"row\"\n          td\n            text \"a\"\n      else\n        tbody\n          tr testId=\"row\"\n            td\n              text \"b\"\n"
        ));
        let kernel = r.kernel();
        let node = |id: &str| kernel.node_by_key(kernel.find_by_test_id(id)[0]).unwrap();
        let body = kernel.node(node("row").parent.unwrap()).unwrap();
        assert_eq!(
            body.props.str(PropId::SemanticTag),
            Some("tbody"),
            "open={open}"
        );
        assert_eq!(body.parent, Some(node("t").id), "open={open}");
    }
}

/// The rows a table holds directly are in a `tbody` of their own, as
/// HTML's parser puts them, one for each run (the rows from a `for` among
/// them); a `rowspan` of 0 is HTML's, the rest of the cell's row group.
#[test]
fn a_tables_own_rows_are_a_tbody_and_rowspan_0_is_admitted() {
    let r = boot(
        "component App\n  state xs = [1, 2]\n  view\n    table testId=\"t\"\n      caption\n        text \"Moves\"\n      tr testId=\"first\"\n        td rowspan=0\n          text \"a\"\n      for x in xs key=x\n        tr\n          td\n            text \"b\"\n      tfoot\n        tr testId=\"foot\"\n          td\n            text \"c\"\n      tr testId=\"last\"\n        td\n          text \"d\"\n",
    );
    let kernel = r.kernel();
    let node = |id: &str| {
        let key = kernel.find_by_test_id(id)[0];
        kernel.node_by_key(key).unwrap()
    };
    let tag = |id: u32| {
        kernel
            .node(id)
            .and_then(|n| n.props.str(PropId::SemanticTag).map(str::to_string))
    };
    let children: Vec<_> = node("t").children().iter().map(|c| tag(*c)).collect();
    let said = |t: &str| Some(t.to_string());
    assert_eq!(
        children,
        [said("caption"), said("tbody"), said("tfoot"), said("tbody")]
    );
    let first = node("first");
    let body = kernel.node(first.parent.unwrap()).unwrap();
    assert_eq!(
        body.children().len(),
        3,
        "the written row and the two from the `for`"
    );
    assert_eq!(tag(node("foot").parent.unwrap()), said("tfoot"));
    assert_eq!(tag(node("last").parent.unwrap()), said("tbody"));
    let cell = kernel.node(first.children()[0]).unwrap();
    assert_eq!(cell.props.get(PropId::RowSpan), Some(&PropValue::Int(0)));
}

/// `contract vocab` answers for the elements and the cell attributes.
#[test]
fn vocab_answers_with_the_tags() {
    let out = Command::new(env!("CARGO_BIN_EXE_contract"))
        .args(["vocab", "table", "tr", "dl", "colspan"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let out = String::from_utf8(out.stdout).unwrap();
    for line in [
        "table: tag, View (html); sets semanticTag=table",
        "tr: tag, View (html); sets semanticTag=tr",
        "dl: tag, View (html); sets semanticTag=dl",
        "colspan: prop attribute",
        "only on `th` or `td`",
    ] {
        assert!(out.contains(line), "{line} in {out}");
    }
    assert!(out.contains("HTML table: a `table` holds"), "{out}");
    assert!(out.contains("HTML description list"), "{out}");
}
