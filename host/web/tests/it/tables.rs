//! HTML's table and description list are their elements on the web (LLP
//! 1153 D7.2): the browser's table layout and UA sheet, a cell's attributes
//! the DOM's, and no `display` of the host's on any of them.

use exact_web::Host;

const SOURCE: &str = r#"component App
  state up = true
  view
    column
      table testId="table"
        caption testId="caption"
          text "Moves"
        thead testId="thead"
          tr testId="head-row"
            th scope="col" aria-sort=(up ? "ascending" : "descending") testId="name"
              text "Name"
            th scope="col" testId="power"
              text "Power"
        tbody testId="tbody"
          tr testId="row"
            th scope="row" testId="move"
              text "Tackle"
            td colspan=2 testId="wide"
              text "40"
          tr
            td rowspan=2 testId="tall"
              text "a"
            td testId="plain"
              text "b"
        tfoot testId="tfoot"
          tr
            td testId="total"
              text "2 moves"
      dl testId="dl"
        dt testId="dt"
          text "Type"
        dd testId="dd"
          text "Electric"
"#;

/// The create op for `test_id`'s view, up to the next op.
fn create<'a>(host: &Host<caltrain_data::Caltrain>, batch: &'a str, test_id: &str) -> &'a str {
    let kernel = host.runner().kernel();
    let id = kernel
        .node_by_key(kernel.find_by_test_id(test_id)[0])
        .unwrap()
        .id;
    let at = batch
        .find(&format!("\"op\":\"create\",\"id\":{id},"))
        .unwrap_or_else(|| panic!("no create for {test_id}: {batch}"));
    let rest = &batch[at..];
    &rest[..rest[1..].find("{\"op\"").map_or(rest.len(), |end| end + 1)]
}

#[test]
fn a_table_and_a_description_list_are_their_elements() {
    let plan = contract::compile(SOURCE).unwrap();
    let (host, batch) = Host::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        Default::default(),
        "/",
    )
    .unwrap();
    for (test_id, tag) in [
        ("table", "table"),
        ("caption", "caption"),
        ("thead", "thead"),
        ("head-row", "tr"),
        ("name", "th"),
        ("tbody", "tbody"),
        ("wide", "td"),
        ("tfoot", "tfoot"),
        ("dl", "dl"),
        ("dt", "dt"),
        ("dd", "dd"),
    ] {
        let op = create(&host, &batch, test_id);
        assert!(
            op.contains(&format!("\"tag\":\"{tag}\"")),
            "{test_id}: {op}"
        );
        // The browser's UA sheet gives a table's elements their display: a
        // cell that holds a folded text keeps `table-cell`, where a box
        // holding one is made a block (as a `dt`, a block already, is).
        if !tag.starts_with('d') {
            assert!(!op.contains("display:"), "{test_id}: {op}");
        }
    }
    for (test_id, attr) in [
        ("name", "\"scope\":\"col\""),
        ("name", "\"aria-sort\":\"ascending\""),
        ("move", "\"scope\":\"row\""),
        ("wide", "\"colspan\":\"2\""),
        ("tall", "\"rowspan\":\"2\""),
    ] {
        let op = create(&host, &batch, test_id);
        assert!(op.contains(attr), "{test_id} has {attr}: {op}");
    }
    let plain = create(&host, &batch, "plain");
    assert!(
        !plain.contains("colspan") && !plain.contains("rowspan"),
        "{plain}"
    );
    // A header cell is bold and centred, as HTML's UA sheet has it.
    let name = create(&host, &batch, "name");
    assert!(
        name.contains("font-weight:700") && name.contains("text-align:center"),
        "{name}"
    );
}
