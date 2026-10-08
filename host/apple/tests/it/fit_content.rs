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

/// The routes of `src`, booted at 390 × 844 with that screen set as a
/// fit-content sheet's host sets it (LLP 1075.003 §9.11): each route's
/// first extent is `routes`' height; none sends another as the sheet
/// resizes (844, 44, 200, 844, 44, 200); and the sheet laid out at a
/// route's extent lays that route's content out at exactly that height —
/// the measure and the layout agree.
fn settles_and_agrees(src: &str, routes: &[(&str, f32)]) {
    let plan = contract::compile(src).unwrap();
    let (mut host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let mut extent = Vec::new();
    for &(id, h) in routes {
        let sent = heights(&first, view(&host, id));
        assert!(
            sent.len() == 1 && (sent[0] - h).abs() < 0.01,
            "{id}: {sent:?}, not {h}"
        );
        extent.push(sent[0]);
    }
    // The screen, as `ExactViewIOS.fit` sends it before the sheet's
    // resize: the boot's viewport, so nothing moves.
    let screened = host.set_screen(Some((390.0, 844.0)));
    for &(id, _) in routes {
        assert!(
            heights(&screened, view(&host, id)).is_empty(),
            "{id}: {screened}"
        );
    }
    for h in [44.0, 200.0, 844.0, 44.0, 200.0] {
        let resized = host.resize(390.0, h);
        for &(id, _) in routes {
            assert!(
                heights(&resized, view(&host, id)).is_empty(),
                "{id} at {h}: {resized}"
            );
        }
    }
    // Each child's height in a sheet as tall as the screen, where none
    // is squeezed.
    let sizes = |host: &Host<NoData>, id: &str| {
        let k = host.runner().kernel();
        let route = k.node(view(host, id)).unwrap();
        let (_, _, _, pad) = k.resolved_padding(route.key).unwrap();
        let children: Vec<_> = route
            .children()
            .into_iter()
            .filter_map(|c| k.node(c))
            .map(|c| (c.frame.height, c.frame.y + c.frame.height - route.frame.y))
            .collect();
        (route.frame.height, pad, children)
    };
    host.resize(390.0, 844.0);
    let tall: Vec<_> = routes.iter().map(|&(id, _)| sizes(&host, id).2).collect();
    for ((&(id, _), &h), tall) in routes.iter().zip(&extent).zip(tall) {
        host.resize(390.0, h);
        let (laid, pad, children) = sizes(&host, id);
        assert!(
            (laid - h).abs() < 0.01,
            "{id}: laid out {laid}, measured {h}"
        );
        let heights: Vec<_> = children.iter().map(|c| c.0).collect();
        assert_eq!(
            heights,
            tall.iter().map(|c| c.0).collect::<Vec<_>>(),
            "{id}: squeezed"
        );
        let bottom = children.iter().map(|c| c.1).fold(0.0_f32, f32::max);
        assert!(
            bottom + pad <= h + 0.01,
            "{id}: content {} past {h}",
            bottom + pad
        );
    }
}

/// `r` as a route that fills the sheet, with `rest` on it.
fn route(id: &str, rest: &str) -> String {
    format!(
        r#"column testId="{id}" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0 {rest}"#
    )
}

/// LLP 1075.003 §9.11, the lead's option (b) after the r4–r6 reviews: in a
/// fit-content sheet `vh` and its kin are the screen's, so a comparison,
/// a cap, a floor, a `vmax` and a floor of a route all settle, each at
/// what it is against the 844-point screen, and the layout agrees.
#[test]
fn viewport_lengths_are_the_screens_and_settle() {
    let src = format!(
        r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      {}
        row height=44
          text "Row"
      {}
        row height=44
          text "Row"
      {}
        box height="min(80vh, 300px)"
      {}
        box height="clamp(10vh, 200px, 40vh)"
      {}
        column height="100vmax"
          row height=44
            text "Row"
      {}
        row height=44
          text "Row"
        box height="50vh"
      {}
        row height=44
          text "One"
        row height=44 margin-top="max(-40px, -10vh)"
          text "Two"
"##,
        route("max", r#"min-height="max(200px, 80vh)""#),
        route("cap", r#"max-height="min(80vh, 300px)""#),
        route("min", ""),
        route("clamp", ""),
        route("vmax", ""),
        route("floor", r#"min-height="100vh""#),
        route("margins", ""),
    );
    settles_and_agrees(
        &src,
        &[
            ("max", 0.8 * 844.0),
            ("cap", 44.0),
            ("min", 300.0),
            ("clamp", 200.0),
            ("vmax", 844.0),
            ("floor", 844.0),
            ("margins", 48.0),
        ],
    );
}

/// The same for padding, a flex basis, a native field's floor and widths
/// in a height unit (through an aspect ratio, and the route's own width),
/// the cases of Astra's and Grok's r6.
#[test]
fn padding_bases_fields_and_widths_in_viewport_lengths_settle() {
    let src = format!(
        r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      {}
        row height=44
          text "Row"
        box flex-basis="10vh" flex-shrink=0
      {}
        input min-height="100vh" flex-shrink=0
      {}
        box width=300 max-width="max(100px, calc(-100vh + 400px))" aspect-ratio=1
      {}
        box width="100%" max-width="50vh" aspect-ratio=1
      {}
        text "Twenty-six letters wrap at each width the route is given, so its height follows that width."
"##,
        route("padding", r#"padding-bottom="50vh""#),
        route("field", ""),
        route("floor", r#"display="block""#),
        route("ratio", ""),
        route("own", r#"max-width="50vh" padding-left="10vh""#),
    );
    settles_and_agrees(
        &src,
        &[
            ("padding", 44.0 + 84.4 + 422.0),
            ("field", 844.0),
            ("floor", 100.0),
            ("ratio", 390.0),
            ("own", 3.0 * 19.2),
        ],
    );
}

/// Astra's review of option (b): the screen is a fit-content route's, not
/// the session's. With sheets stacked in either order, a medium sheet's
/// `50vh` is half its own viewport and a fit-content one's half the
/// screen; a route's detent turning to `fit-content` turns its lengths to
/// the screen; and a fit-content route that appears over a 400-point sheet
/// in an 800-point window measures against the screen from its first
/// extent.
#[test]
fn the_screen_is_a_fit_content_routes_alone() {
    for fit_first in [true, false] {
        let fit = r#"      column testId="fit" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0
        box testId="fit-half" height="50vh" flex-shrink=0
"#;
        let medium = r#"      column testId="medium" navigationDetent=(fitted ? "fit-content" : "medium") position="absolute" top=0 right=0 bottom=0 left=0
        box testId="medium-half" height="50vh" flex-shrink=0
"#;
        let src = format!(
            r##"component Menu
  state fitted = false
  state shown = false
  action fit
    fitted = true
  action show
    shown = true
  view
    column position="relative" width="100%" height="100%"
      button press=fit testId="to-fit" height=44
        text "Fit"
      button press=show testId="show" height=44
        text "Show"
{}{}      when shown
        column testId="late" navigationDetent="fit-content" position="absolute" top=0 right=0 bottom=0 left=0
          box height="50vh" flex-shrink=0
"##,
            if fit_first { fit } else { medium },
            if fit_first { medium } else { fit },
        );
        let plan = contract::compile(&src).unwrap();
        let (mut host, _) = Host::boot(
            &plan.encode(),
            NoData,
            Box::new(MonospaceMeasurer::default()),
            390.0,
            800.0,
        )
        .unwrap();
        host.set_screen(Some((390.0, 800.0)));
        // A medium sheet owns the viewport now.
        host.resize(390.0, 400.0);
        let height = |host: &Host<NoData>, id: &str| {
            let k = host.runner().kernel();
            k.node(view(host, id)).unwrap().frame.height
        };
        assert_eq!(height(&host, "fit-half"), 400.0, "fit first: {fit_first}");
        assert_eq!(
            height(&host, "medium-half"),
            200.0,
            "fit first: {fit_first}"
        );
        let to_fit = view(&host, "to-fit");
        host.dispatch_at(to_fit, Event::Press, 0.0);
        assert_eq!(
            height(&host, "medium-half"),
            400.0,
            "fit first: {fit_first}"
        );
        let show = view(&host, "show");
        let shown = host.dispatch_at(show, Event::Press, 0.0);
        assert_eq!(heights(&shown, view(&host, "late")), [400.0], "{shown}");
    }
}

/// Astra's review of option (b): a route whose width follows the sheet
/// through a percentage of its height and a ratio (100, 300, 100, … when
/// the trial took its published width) is measured with its width derived
/// as the layout derives it, its block size indefinite: it settles.
#[test]
fn a_width_through_a_percentage_height_does_not_cycle() {
    let src = r##"component Menu
  view
    column position="relative" width="100%" height="100%"
      column testId="ratio" navigationDetent="fit-content" position="absolute" top=0 left=0 height="calc(400px - 100%)" aspect-ratio=1 min-width=100 max-width=300
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
    host.set_screen(Some((390.0, 844.0)));
    let ratio = view(&host, "ratio");
    let start = heights(&first, ratio);
    let mut last = *start.last().unwrap();
    for h in [100.0, 300.0, 100.0, 300.0, 100.0] {
        let resized = host.resize(390.0, h);
        if let Some(&next) = heights(&resized, ratio).last() {
            last = next;
        }
        assert!(
            heights(&host.resize(390.0, last), ratio).is_empty(),
            "at {last}: the next measure moved"
        );
    }
    assert_eq!(last, 100.0);
}
