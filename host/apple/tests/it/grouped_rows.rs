//! A grouped list's rows as the kernel hands them to UIKit's list.

use crate::host::{view, NoData};
use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;

/// UIKit's content configuration draws none of a row's box, so a row of a
/// standard cell's parts whose author wrote an edge of it, or its
/// background image, is carried as its own views, as every other host
/// draws it (LLP 1084 D7); a row with none is the standard cell.
#[test]
fn a_row_with_an_authored_edge_is_carried() {
    let src = r##"component Form
  view
    list testId="list" appearance="auto" listStyle="inset-grouped" width="100%" height="100%"
      section
        row testId="plain"
          text "Plain"
        row testId="ruled" border-bottom="1px solid -exact-separator"
          text "Ruled"
        row testId="unruled" border-bottom="none"
          text "Unruled"
        row testId="framed" border="1px solid -exact-separator" border-bottom="none"
          text "Framed"
        row testId="pictured" background-image="linear-gradient(red, blue)"
          text "Pictured"
"##;
    let plan = contract::compile(src).unwrap();
    let (host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        402.0,
        874.0,
    )
    .unwrap();
    let list = host
        .runner()
        .kernel()
        .grouped_list(view(&host, "list"))
        .expect("a grouped list");
    let custom = |id: &str| {
        let id = view(&host, id);
        list.sections
            .iter()
            .flat_map(|s| s.rows.iter())
            .find(|r| r.view == id)
            .unwrap_or_else(|| panic!("no row {id}"))
            .custom
    };
    assert!(!custom("plain"), "a title alone is the standard cell");
    for id in ["ruled", "unruled", "framed", "pictured"] {
        assert!(custom(id), "{id}: carried, its box drawn as written");
    }
}
