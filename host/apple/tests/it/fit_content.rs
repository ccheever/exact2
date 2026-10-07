//! LLP 1075.003 §9.11: a route whose `navigationDetent` names `fit-content`
//! carries its content's extent, which the sheet's detent reads. The extent
//! follows the rows, not the box the sheet gives the route.

use crate::host::{count, view, NoData};
use exact_apple::Host;
use exact_kernel::{HostCover, MonospaceMeasurer};
use exact_runner::Event;

/// The `content` ops for `id`, in order.
fn heights(batch: &str, id: u32) -> Vec<f32> {
    let marker = format!("\"op\":\"content\",\"id\":{id},");
    batch
        .match_indices(marker.as_str())
        .map(|(at, _)| {
            let rest = &batch[at..];
            let h = &rest[rest.find("\"h\":").unwrap() + 4..];
            h[..h.find('}').unwrap()].parse().unwrap()
        })
        .collect()
}

#[test]
fn a_fit_content_route_reports_its_rows_and_not_its_box() {
    let src = r##"component Menu
  state rows = [0, 1]
  action more
    rows = concat(rows, [length(rows)])
  view
    column position="relative" width="100%" height="100%"
      column testId="plain" position="absolute" top=0 right=0 bottom=0 left=0
        text "Behind"
      column testId="sheet" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 display="flex" flex-direction="column" padding=16
        button press=more testId="more" height=44
          text "More"
        each i in rows key=i
          row height=44
            text `Row ${i}`
"##;
    let plan = contract::compile(src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let sheet = view(&host, "sheet");
    // The handle's row and two more, inside 16 points of padding; the box
    // is the whole viewport.
    assert_eq!(heights(&first, sheet), [16.0 + 3.0 * 44.0 + 16.0]);
    assert!(heights(&first, view(&host, "plain")).is_empty());
    assert_eq!(count(&first, "content"), 1, "{first}");
    let more = view(&host, "more");
    let grown = host.dispatch_at(more, Event::Press, 0.0);
    assert_eq!(heights(&grown, sheet), [16.0 + 4.0 * 44.0 + 16.0]);
    // The sheet's own height is the viewport the route lays out in: a
    // shorter one leaves the extent alone, so the detent cannot feed back.
    let shorter = host.resize(390.0, 300.0);
    assert!(heights(&shorter, sheet).is_empty(), "{shorter}");
    // Under a tab bar the route's container covers the sheet's safe area at
    // the bottom, which UIKit adds below the detent itself: not counted. A
    // bar over the top is, as the rows move down under it.
    let bottom = host.set_covers(&[(sheet, Some(HostCover::Edges([0.0, 0.0, 34.0, 0.0])))]);
    assert!(heights(&bottom, sheet).is_empty(), "{bottom}");
    let top = host.set_covers(&[(sheet, Some(HostCover::Edges([56.0, 0.0, 34.0, 0.0])))]);
    assert_eq!(heights(&top, sheet), [56.0 + 16.0 + 4.0 * 44.0 + 16.0]);
}
