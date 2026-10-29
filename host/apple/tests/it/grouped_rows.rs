//! A grouped list's rows as the kernel hands them to UIKit's list.

use crate::host::{view, NoData};
use exact_apple::Host;
use exact_kernel::{GroupedMargins, MonospaceMeasurer};

#[test]
fn horizontal_padding_and_native_card_margins_each_reduce_the_row_once() {
    for direction in ["ltr", "rtl"] {
        for style in ["plain", "inset-grouped"] {
            let src = format!(
                r#"component Form
  view
    column width=402 height=600 direction="{direction}"
      list testId="list" appearance="auto" listStyle="{style}" flex=1 padding-left="4.5rem"
        section
          row testId="row"
            view width=24 height=24
            text "Custom" flex=1
"#
            );
            let plan = contract::compile(&src).unwrap();
            let (mut host, _) = Host::boot(
                &plan.encode(),
                NoData,
                Box::new(MonospaceMeasurer::default()),
                402.0,
                600.0,
            )
            .unwrap();
            let list = view(&host, "list");
            let row = view(&host, "row");
            for root_font in [230.0 / 13.5, 20.0] {
                host.set_root_font_size(root_font);
                let padding = (root_font * 4.5) as f32;
                if style == "plain" {
                    let frame = host.runner().kernel().node(row).unwrap().frame;
                    assert!(
                        (frame.width - (402.0 - padding)).abs() < 0.01,
                        "{style}, {direction}, padding {padding}: {frame:?}"
                    );
                }
                // Unequal card margins are LTR's alone: under RTL the kernel
                // swaps them, padding or none (QUEUE.md).
                let uneven = if direction == "ltr" {
                    [20.0, 16.0]
                } else {
                    [16.0, 16.0]
                };
                for card in [[0.0, 0.0], uneven, [0.0, 0.0]] {
                    let batch = host.set_claims(
                        &[],
                        &[],
                        &[],
                        &[(
                            list,
                            Some(GroupedMargins {
                                card,
                                cell: [16.0, 16.0],
                            }),
                        )],
                        &[],
                        &[],
                    );
                    let batch: serde_json::Value = serde_json::from_str(&batch).unwrap();
                    assert!(batch["error"].is_null(), "{batch}");
                    let frame = host.runner().kernel().node(row).unwrap().frame;
                    let note = format!(
                        "{style}, {direction}, padding {padding}, card {card:?}: {frame:?}"
                    );
                    assert!((frame.x - padding - card[0]).abs() < 0.01, "{note}");
                    assert!(
                        (frame.width - (402.0 - padding - card[0] - card[1])).abs() < 0.01,
                        "{note}"
                    );
                }
            }
        }
    }
}

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
