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

#[test]
fn an_empty_route_measures_its_padding_and_a_scrolling_one_its_scroll_extent() {
    let src = r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      column testId="empty" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 padding-top=12 padding-bottom=20
      column testId="clips" navigationDetent="fit-content" overflow="hidden" position="absolute" top=0 right=0 bottom=0 left=0 padding=16
        row height=44
          text "Row"
      column testId="scrolls" navigationDetent="fit-content large" overflow-y="auto" position="absolute" top=0 right=0 bottom=0 left=0 padding=16
        row height=500 flex-shrink=0
          text "Long"
"##;
    let plan = contract::compile(src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        300.0,
    )
    .unwrap();
    // Laid out empty is measured, not unmeasured: the sheet does not open
    // at its maximum for a menu with no rows.
    assert_eq!(heights(&first, view(&host, "empty")), [32.0]);
    // A route that scrolls itself keeps the extent its scroller needs.
    assert_eq!(
        heights(&first, view(&host, "scrolls")),
        [16.0 + 500.0 + 16.0]
    );
    let scrolls = view(&host, "scrolls");
    let covered = host.set_covers(&[(scrolls, Some(HostCover::Edges([0.0, 0.0, 34.0, 0.0])))]);
    // Its scroller needs the bottom cover in that extent; the sheet takes it
    // off again (`ModalIOS.swift`), UIKit adding the band itself.
    assert_eq!(heights(&covered, scrolls), [16.0 + 500.0 + 16.0 + 34.0]);
    // One that only clips is measured as one that does not: no cover.
    let clips = view(&host, "clips");
    assert_eq!(heights(&first, clips), [16.0 + 44.0 + 16.0]);
    let covered = host.set_covers(&[(clips, Some(HostCover::Edges([0.0, 0.0, 34.0, 0.0])))]);
    assert!(heights(&covered, clips).is_empty(), "{covered}");
}

/// Astra's regression: rows that may shrink (CSS's default `flex-shrink:
/// 1`) in a sheet already at the content's height. The fourth row is
/// measured as asked for, not squeezed into the sheet's box; so are a
/// child that grows and one at a percentage of the route's height.
#[test]
fn rows_that_may_shrink_still_grow_the_extent() {
    let src = r##"component Menu
  state rows = [0, 1]
  action more
    rows = concat(rows, [length(rows)])
  view
    column position="relative" width="100%" height="100%"
      column testId="sheet" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 display="flex" flex-direction="column" padding=16
        button press=more testId="more" height=44
          text "More"
        each i in rows key=i
          row height=44
            text `Row ${i}`
      column testId="grows" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 display="flex" flex-direction="column"
        row height=44
          text "Row"
        box flex-grow=1
        box height="50%"
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
    assert_eq!(heights(&first, sheet), [16.0 + 3.0 * 44.0 + 16.0]);
    // Neither the spare height nor half of it is the content's.
    let grows = view(&host, "grows");
    assert_eq!(heights(&first, grows), [44.0]);
    // The sheet at the content's height, as UIKit sets it.
    let fitted = host.resize(390.0, 164.0);
    assert!(heights(&fitted, sheet).is_empty(), "{fitted}");
    assert!(heights(&fitted, grows).is_empty(), "{fitted}");
    let more = view(&host, "more");
    let grown = host.dispatch_at(more, Event::Press, 0.0);
    assert_eq!(
        heights(&grown, sheet),
        [16.0 + 4.0 * 44.0 + 16.0],
        "{grown}"
    );
}

/// A percentage padding is of the containing block's width (CSS Box Model
/// §3), not the route's: a 200-point route in a 400-point block pads 10% as
/// 40 points, whether it clips, scrolls or neither.
#[test]
fn percentage_padding_is_of_the_containing_block() {
    let src = r##"component Menu
  view
    column position="relative" width=400 height="100%"
      column testId="visible" navigationDetent="fit-content" position="absolute" top=0 left=0 bottom=0 width=200 display="flex" flex-direction="column" padding-bottom="10%"
        row height=44 flex-shrink=0
          text "Row"
      column testId="hidden" navigationDetent="fit-content" overflow="hidden" position="absolute" top=0 left=0 bottom=0 width=200 display="flex" flex-direction="column" padding-bottom="10%"
        row height=44 flex-shrink=0
          text "Row"
      column testId="flex-scrolls" navigationDetent="fit-content large" overflow-y="auto" position="absolute" top=0 left=0 bottom=0 width=200 display="flex" flex-direction="column" padding-bottom="10%"
        row height=44 flex-shrink=0
          text "Row"
      column testId="block-scrolls" navigationDetent="fit-content large" overflow-y="auto" position="absolute" top=0 left=0 bottom=0 width=200 display="block" padding-bottom="10%"
        row height=44
          text "Row"
"##;
    let plan = contract::compile(src).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    for id in ["visible", "hidden", "flex-scrolls", "block-scrolls"] {
        assert_eq!(heights(&first, view(&host, id)), [44.0 + 40.0], "{id}");
    }
}

/// Grok's r4: a height that reads the viewport's height reads the sheet,
/// so the trial takes it as `auto`. A route at least `100vh` tall would
/// pin the sheet where it is, and a child `50vh` tall would halve it at
/// each pass.
#[test]
fn viewport_heights_do_not_read_the_sheet() {
    let src = r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      column testId="sheet" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 min-height="100vh" max-height="200vh"
        row height=44
          text "Row"
        box testId="half" height="50vh"
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
    assert_eq!(heights(&first, sheet), [44.0]);
    for h in [44.0, 300.0, 600.0] {
        let resized = host.resize(390.0, h);
        assert!(heights(&resized, sheet).is_empty(), "{h}: {resized}");
    }
}

/// Grok's r4: a block route's percentage padding changes with its
/// containing block while its own box, its children and its extent stay
/// where they are, so nothing under it is laid out anew: it refits on its
/// resolved padding.
#[test]
fn a_block_routes_padding_change_alone_refits() {
    let src = r##"component Menu
  state wide = false
  action widen
    wide = true
  view
    column width="100%" height="100%"
      button press=widen testId="widen" height=44
        text "Widen"
      column position="relative" width=(wide ? 800 : 400) flex=1
        column testId="sheet" navigationDetent="fit-content" display="block" position="absolute" top=0 left=0 bottom=0 width=200 padding-bottom="10%"
          row height=44
            text "Row"
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
    assert_eq!(heights(&first, sheet), [44.0 + 40.0]);
    let widen = view(&host, "widen");
    let wide = host.dispatch_at(widen, Event::Press, 0.0);
    assert_eq!(heights(&wide, sheet), [44.0 + 80.0], "{wide}");
}

/// A scroller inside the route: one sized by its rows in the trial (`flex:
/// 1` has no space to grow into there) grows the extent as rows arrive;
/// one of a fixed height holds them, and the route is not measured again.
#[test]
fn a_nested_scroller_grows_the_extent_unless_its_height_is_fixed() {
    let src = r##"component Menu
  state rows = [0]
  action more
    rows = concat(rows, [length(rows)])
  view
    column position="relative" width="100%" height="100%"
      button press=more testId="more" height=44
        text "More"
      column testId="flexed" navigationDetent="fit-content" position="absolute" top=44 right=0 bottom=0 left=0
        scroll flex=1 min-height=0
          each i in rows key=i
            row height=44 flex-shrink=0
              text `Row ${i}`
      column testId="fixed" navigationDetent="fit-content" position="absolute" top=44 right=0 bottom=0 left=0
        scroll height=100
          each i in rows key=i
            row height=44 flex-shrink=0
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
    let (flexed, fixed) = (view(&host, "flexed"), view(&host, "fixed"));
    assert_eq!(heights(&first, flexed), [44.0]);
    assert_eq!(heights(&first, fixed), [100.0]);
    let more = view(&host, "more");
    let grown = host.dispatch_at(more, Event::Press, 0.0);
    assert_eq!(heights(&grown, flexed), [88.0], "{grown}");
    assert!(heights(&grown, fixed).is_empty(), "{grown}");
}

/// Grok's r5: a comparison is not one taller sample. A bare height-axis
/// unit is `auto`; a comparison that also holds a length is that length,
/// its viewport terms at zero. A sampled stand-in flipped between them as
/// the sheet resized (44, 200, 44, …); each of these settles.
#[test]
fn viewport_comparisons_settle() {
    let src = r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      column testId="max" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 min-height="max(200px, 80vh)"
        row height=44
          text "Row"
      column testId="vmax" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 height="100vmax"
        row height=44
          text "Row"
      column testId="clamp" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 min-height="clamp(100px, 50vh, 400px)"
        row height=44
          text "Row"
      column testId="only" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 min-height="max(50vh, 30dvh)"
        row height=44
          text "Row"
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
    // A comparison of nothing but viewport heights is `auto`, as a bare one.
    let routes = [
        ("max", 200.0),
        ("vmax", 44.0),
        ("clamp", 100.0),
        ("only", 44.0),
    ];
    for (id, h) in routes {
        assert_eq!(heights(&first, view(&host, id)), [h], "{id}");
    }
    for h in [44.0, 200.0, 844.0, 44.0, 200.0] {
        let resized = host.resize(390.0, h);
        for (id, _) in routes {
            assert!(
                heights(&resized, view(&host, id)).is_empty(),
                "{id} at {h}: {resized}"
            );
        }
    }
}

/// Grok's r5: a flex basis and block-axis margins and padding are taken
/// without the viewport's height too, on the route and under it.
#[test]
fn bases_margins_and_padding_do_not_read_the_sheet() {
    let src = r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      column testId="sheet" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 padding-bottom="50vh"
        row height=44 margin-top="20vh"
          text "Row"
        box flex-basis="100vh"
        box padding-top="100vh"
        box padding-top="max(10px, 5vh)"
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
    assert_eq!(heights(&first, sheet), [44.0 + 10.0]);
    for h in [54.0, 300.0, 844.0] {
        let resized = host.resize(390.0, h);
        assert!(heights(&resized, sheet).is_empty(), "{h}: {resized}");
    }
}
