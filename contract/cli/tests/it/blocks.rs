//! Geometry reads where a host shows a block's subtree (LLP 1115.002 D8.2,
//! LLP 1123 D2): a block's root placed among its siblings or lifted to the
//! top layer, its clip, what it inherits for hit testing, and the modal
//! boundary a presentation draws for structural reads.

use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

fn num(r: &Runner<NoData>, name: &str) -> f64 {
    r.slot(name).and_then(Value::as_number).unwrap()
}

/// The view the first node with `id` is.
fn view(r: &Runner<NoData>, id: &str) -> exact_kernel::ViewId {
    let key = r.kernel().find_by_id(id)[0];
    r.kernel().arena().local_id(key.index)
}

fn boot(plan: exact_plan::Plan) -> Runner<NoData> {
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// Gives `view`'s root a block that is the size on both axes.
fn block(r: &mut Runner<NoData>, view: exact_kernel::ViewId, width: f32, height: f32) {
    let is = exact_kernel::BlockAxis::Is;
    r.kernel_mut()
        .set_host_block(
            view,
            Some(exact_kernel::HostBlock {
                width: is(width),
                height: is(height),
            }),
        )
        .unwrap();
}

fn lay_out(r: &mut Runner<NoData>) {
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
}

/// A subtree a host lays out in a containing block of its own (LLP 1115.002
/// D8.2) and presents above the page, as iOS presents a sheet's route (LLP
/// 1123 D2): its frames are in the block's space, and the host reports where
/// the block's root is shown as it reports a dialog's (`exact_top_layer`).
/// `frame` and `elementFromPoint` answer where a person sees the boxes.
#[test]
fn a_subtree_in_a_host_block_is_read_where_its_host_shows_it() {
    let plan = contract::compile(
        r#"component App
  state x = 0
  state y = 0
  state width = 0
  state hit = "unset"
  action read
    x = frame("card").x
    y = frame("card").y
    width = frame("card").width
    hit = match elementFromPoint(40, 245) { case some(id) => id, case none => "none" }
  view
    column id="page" height=800
      column id="sheet" position="absolute" top=0 right=0 bottom=0 left=0
        column id="card" margin-top=30 height=40
      button press=read testId="read"
        text "read"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let sheet = view(&r, "sheet");
    block(&mut r, sheet, 300.0, 500.0);
    lay_out(&mut r);
    r.top_layer_frames(&[(sheet, [10.0, 200.0, 300.0, 500.0])], None);
    r.act("read", vec![]).unwrap();
    assert_eq!(num(&r, "width"), 300.0, "the block's width, not the page's");
    assert_eq!(
        (num(&r, "x"), num(&r, "y")),
        (10.0, 230.0),
        "where it is shown"
    );
    assert_eq!(r.slot("hit").and_then(Value::as_str), Some("card"));
}

/// A subtree a host lays out in a block of its own and shows in place, as
/// iOS shows a stack's route in its controller's view (LLP 1123 D2): the host
/// reports where the block's root is in the page, and the root keeps its
/// place among its siblings. `frame` reads the subtree there, scaled when
/// the rectangle is another size than the block, and `elementFromPoint`
/// finds a root toast written after the tablist over the route, as UIKit
/// shows it over the tab container (contract/corpus/tabs.contract,
/// `NavigationTabsIOSTests.testARootToastAfterTheTablistIsOverTheNativeTabs`),
/// before the route under it.
#[test]
fn a_placed_block_keeps_its_place_among_its_siblings() {
    let plan = contract::compile(
        r#"component App
  state x = 0
  state y = 0
  state width = 0
  state toast = "unset"
  state route = "unset"
  action read
    x = frame("card").x
    y = frame("card").y
    width = frame("card").width
    toast = match elementFromPoint(100, 330) { case some(id) => id, case none => "none" }
    route = match elementFromPoint(40, 245) { case some(id) => id, case none => "none" }
  view
    main id="page" height=800 display="flex" flex-direction="column"
      column role="tabpanel" id="panel" position="absolute" inset=0
        column id="route" position="absolute" top=0 right=0 bottom=0 left=0
          column id="card" margin-top=30 height=200
      row role="tablist" height=56
        button press=read testId="read"
          text "read"
      button id="toast" position="absolute" left=50 top=300 width=200 height=60
        text "Saved"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 300.0, 500.0);
    lay_out(&mut r);
    r.place_blocks(&[(
        route,
        Some(([10.0, 200.0, 300.0, 500.0], view(&r, "panel"))),
    )]);
    r.act("read", vec![]).unwrap();
    assert_eq!(num(&r, "width"), 300.0, "the block's width, not the page's");
    assert_eq!(
        (num(&r, "x"), num(&r, "y")),
        (10.0, 230.0),
        "where it is shown"
    );
    assert_eq!(r.slot("route").and_then(Value::as_str), Some("card"));
    assert_eq!(
        r.slot("toast").and_then(Value::as_str),
        Some("toast"),
        "a later sibling stays above the route"
    );
    // Shown at half its size, the subtree is read at half its size
    r.place_blocks(&[(
        route,
        Some(([10.0, 200.0, 150.0, 250.0], view(&r, "panel"))),
    )]);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        (num(&r, "x"), num(&r, "y"), num(&r, "width")),
        (10.0, 215.0, 150.0)
    );
    // Forgotten, it is read at its block's origin again
    r.place_blocks(&[(route, None)]);
    r.act("read", vec![]).unwrap();
    assert_eq!((num(&r, "x"), num(&r, "y")), (0.0, 30.0));
}

/// The rectangle a host reports for a block's root is the block's, not the
/// root's border box: the root sits in the block by its own frame (its
/// margins, a width narrower than the block), so a block shown at its own
/// size reads unscaled, in the top layer and placed in its parent alike. A
/// placement is in the parent's local coordinates, before the parent's
/// transform, which the runner applies once above it. An invalid placement
/// is ignored and the one it would replace kept.
#[test]
fn a_blocks_rectangle_holds_its_root_by_the_roots_own_frame() {
    let plan = contract::compile(
        r#"component App
  state x = 0
  state y = 0
  state width = 0
  state at = "unset"
  state beside = "unset"
  action read
    x = frame("card").x
    y = frame("card").y
    width = frame("card").width
    at = match elementFromPoint(25, 45) { case some(id) => id, case none => "none" }
    beside = match elementFromPoint(5, 45) { case some(id) => id, case none => "none" }
  view
    column id="page" height=800
      column id="panel" position="absolute" inset=0 translate="0px 40px"
        column id="route" position="absolute" top=0 bottom=0 left=0 right=0 margin-left=20 margin-right=20
          column id="card" height=100
      button press=read testId="read"
        text "read"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 300.0, 500.0);
    lay_out(&mut r);
    // Placed at the panel's origin: the panel's 40-point translation once
    r.place_blocks(&[(route, Some(([0.0, 0.0, 300.0, 500.0], view(&r, "panel"))))]);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        (num(&r, "x"), num(&r, "y"), num(&r, "width")),
        (20.0, 40.0, 260.0),
        "the root by its margins, unscaled, the parent's transform once"
    );
    assert_eq!(r.slot("at").and_then(Value::as_str), Some("card"));
    assert_ne!(
        r.slot("beside").and_then(Value::as_str),
        Some("card"),
        "the margin is not the card"
    );
    // An invalid placement leaves the last one
    r.place_blocks(&[(
        route,
        Some(([f64::NAN, 0.0, 300.0, 500.0], view(&r, "panel"))),
    )]);
    r.place_blocks(&[(route, Some(([0.0, 0.0, -1.0, 500.0], view(&r, "panel"))))]);
    r.act("read", vec![]).unwrap();
    assert_eq!((num(&r, "x"), num(&r, "y")), (20.0, 40.0));
    // In the top layer, the block's rectangle in viewport points
    r.place_blocks(&[(route, None)]);
    r.top_layer_frames(&[(route, [100.0, 200.0, 150.0, 250.0])], None);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        (num(&r, "x"), num(&r, "y"), num(&r, "width")),
        (110.0, 200.0, 130.0),
        "half the block's size: the margin and the width halved"
    );
}

/// A block its host holds beside its parent, as UIKit's tab container holds
/// a tab's route beside its panel, is placed in an ancestor that holds it in
/// both trees and is read there: the boxes between move nothing the host
/// shows, so a panel translated and turned a quarter turn, or scaled to
/// nothing, moves neither `frame`, the point reads nor the intersection
/// observer. A placement in a box that is not above the root reads as none.
#[test]
fn a_block_held_beside_its_parent_is_read_where_its_holder_shows_it() {
    let plan = contract::compile(
        r#"component App
  state turned = false
  state flat = false
  state x = 0
  state y = 0
  state width = 0
  state height = 0
  state at = "unset"
  state all = "unset"
  action turn
    turned = true
  action flatten
    flat = true
  action seen(e: IntersectionObserverEntry)
    turned = turned
  action read
    x = frame("card").x
    y = frame("card").y
    width = frame("card").width
    height = frame("card").height
    at = match elementFromPoint(20, 245) { case some(id) => id, case none => "none" }
    all = elementsFromPoint(20, 245).join(",")
  view
    main id="page" height=800
      column id="panels" position="absolute" inset=0 translate=(turned ? "0px 30px" : "0px 0px")
        column role="tabpanel" id="panel" position="absolute" inset=0 rotate=(turned ? "90deg" : "none") scale=(flat ? 0 : 1)
          column id="route" position="absolute" inset=0
            column id="card" margin-top=30 width=100 height=40 intersect=seen intersect-margin="0px 0px 0px -200px"
      button id="read" press=read testId="read"
        text "read"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 300.0, 500.0);
    lay_out(&mut r);
    let read = |r: &mut Runner<NoData>| {
        r.act("read", vec![]).unwrap();
        let text = |name: &str| r.slot(name).and_then(Value::as_str).map(str::to_owned);
        let rect = ["x", "y", "width", "height"].map(|name| num(r, name));
        (rect, text("at"), text("all"))
    };
    let near = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-9);
    r.place_blocks(&[(route, Some(([0.0, 200.0, 300.0, 500.0], view(&r, "page"))))]);
    let (rect, at, all) = read(&mut r);
    assert!(near(rect, [0.0, 230.0, 100.0, 40.0]), "{rect:?}");
    assert_eq!(at.as_deref(), Some("card"));
    assert!(
        all.as_deref()
            .is_some_and(|all| all.starts_with("card,route")),
        "{all:?}"
    );
    let due = r.intersect_due();
    assert_eq!(
        (due[0].is_intersecting, due[0].intersection_ratio),
        (false, 0.0),
        "left of the observed half"
    );
    assert!(r.intersect_delivered(&due[0]));
    r.act("turn", vec![]).unwrap();
    lay_out(&mut r);
    let (turned, at, all) = read(&mut r);
    assert!(near(turned, rect), "{turned:?}");
    assert_eq!(at.as_deref(), Some("card"));
    assert!(
        all.as_deref()
            .is_some_and(|all| all.starts_with("card,route")),
        "{all:?}"
    );
    let due: Vec<_> = r
        .intersect_due()
        .iter()
        .map(|d| (d.is_intersecting, d.intersection_ratio))
        .collect();
    assert!(due.is_empty(), "the observer sees no change: {due:?}");
    // Scaled to nothing, the panel has no inverse, and needs none
    r.act("flatten", vec![]).unwrap();
    lay_out(&mut r);
    let (flat, at, all) = read(&mut r);
    assert!(near(flat, rect), "{flat:?}");
    assert_eq!(at.as_deref(), Some("card"));
    assert!(
        all.as_deref()
            .is_some_and(|all| all.starts_with("card,route")),
        "{all:?}"
    );
    assert!(r.intersect_due().is_empty(), "the observer sees no change");
    // In a box that is not above the route, at the block's origin, turned with the panel
    r.place_blocks(&[(route, Some(([50.0, 50.0, 300.0, 500.0], view(&r, "read"))))]);
    let (unplaced, ..) = read(&mut r);
    r.place_blocks(&[(route, None)]);
    assert!(near(read(&mut r).0, unplaced), "{unplaced:?}");
}

/// A sticky box between a placed block's parent and the view that holds it
/// moves nothing the host shows either: scrolled 100 points, the sticky
/// column would carry the route's card down by 100 points into the
/// observed band, and the observer, which steps from the route straight to
/// the view that holds it, does not see it there.
#[test]
fn a_sticky_box_between_moves_no_held_block() {
    let plan = contract::compile(
        r#"component App
  state seen = "unseen"
  action observe(e: IntersectionObserverEntry)
    seen = `${e.isIntersecting}`
  view
    scroll id="port" height=800
      column id="stuck" position="sticky" top=0 height=600
        column role="tabpanel" id="panel" position="absolute" inset=0
          column id="route" position="absolute" inset=0
            column id="card" margin-top=30 width=100 height=40 intersect=observe intersect-margin="-150px 0px 0px 0px"
      column height=2000
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 300.0, 500.0);
    lay_out(&mut r);
    r.place_blocks(&[(route, Some(([0.0, 0.0, 300.0, 500.0], view(&r, "port"))))]);
    let due = r.intersect_due();
    assert_eq!(due.len(), 1);
    assert!(!due[0].is_intersecting, "above the observed band");
    assert!(r.intersect_delivered(&due[0]));
    r.scrolled(Some(view(&r, "port")), 0.0, 100.0);
    assert!(
        r.intersect_due().is_empty(),
        "the sticky column moves nothing the host shows"
    );
}

/// The intersection observer's root is the nearest scroller on the steps
/// the target is shown through: a clipping panel between a placed block's
/// parent and the scroller that holds it is stepped past, and the
/// scroller's 100-point scrollport clips the card placed 230 points down.
#[test]
fn a_held_blocks_observer_is_clipped_by_the_scroller_that_holds_it() {
    let plan = contract::compile(
        r#"component App
  state seen = "unseen"
  action observe(e: IntersectionObserverEntry)
    seen = `${e.isIntersecting}`
  view
    column
      scroll id="port" width=400 height=100
        column role="tabpanel" id="panel" height=600 overflow="hidden"
          column id="route" position="absolute" inset=0
            column id="card" margin-top=30 width=100 height=40 intersect=observe
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 300.0, 500.0);
    lay_out(&mut r);
    r.place_blocks(&[(route, Some(([0.0, 200.0, 300.0, 500.0], view(&r, "port"))))]);
    let due = r.intersect_due();
    assert_eq!(
        (due[0].is_intersecting, due[0].intersection_ratio),
        (false, 0.0),
        "below the scroller's 100-point scrollport"
    );
}

/// A block clips what its host shows in it: in a 200-point sheet, a route
/// as wide as the viewport is reached only inside the sheet, and a button
/// laid out past the sheet's edge nowhere, as UIKit reaches nothing of a
/// sheet outside it; so is a part of the route translated out of its block.
#[test]
fn a_block_clips_what_is_reached_in_it() {
    let plan = contract::compile(
        r#"component App
  state inside = "unset"
  state outside = "unset"
  state left = "unset"
  state shifted = false
  action read
    inside = match elementFromPoint(160, 20) { case some(id) => id, case none => "none" }
    outside = match elementFromPoint(360, 20) { case some(id) => id, case none => "none" }
    left = match elementFromPoint(60, 20) { case some(id) => id, case none => "none" }
  action shift
    shifted = true
  view
    column id="page" height=800
      column id="route" position="absolute" top=0 left=0 width="100vw" height=500 translate=(shifted ? "-120px 0px" : "0px 0px")
        row height=40
          button id="near" width=100 height=40
            text "near"
          button id="far" margin-left=150 width=50 height=40
            text "far"
      button press=read testId="read"
        text "read"
      button press=shift testId="shift"
        text "shift"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 200.0, 500.0);
    lay_out(&mut r);
    let hit =
        |r: &Runner<NoData>, name: &str| r.slot(name).and_then(Value::as_str).map(str::to_owned);
    // A sheet 200 points wide at x 100: the route's 400 points overflow it
    r.top_layer_frames(&[(route, [100.0, 0.0, 200.0, 500.0])], None);
    r.act("read", vec![]).unwrap();
    assert_eq!(hit(&r, "inside").as_deref(), Some("near"));
    assert_ne!(
        hit(&r, "outside").as_deref(),
        Some("far"),
        "the far button, at x 350, is past the sheet's 300"
    );
    // Translated 120 points left, the near button is at -20..80: out of the sheet
    r.act("shift", vec![]).unwrap();
    lay_out(&mut r);
    r.top_layer_frames(&[(route, [100.0, 0.0, 200.0, 500.0])], None);
    r.act("read", vec![]).unwrap();
    assert_ne!(
        hit(&r, "left").as_deref(),
        Some("near"),
        "the near button's part out of the sheet is not reached"
    );
    assert_eq!(
        hit(&r, "inside").as_deref(),
        Some("route"),
        "the route still is, inside the sheet"
    );
}

/// A block lifted to the top layer, as a sheet's route is (LLP 1123 D2),
/// escapes its logical ancestors' clips and transforms but not their
/// `inert`: with `inert` on the logical root above it, `elementFromPoint`
/// at its button finds nothing, as UIKit reaches nothing there (HTML's
/// inert subtrees), and finds the button again once `inert` goes.
#[test]
fn an_inert_ancestor_keeps_a_lifted_block_from_being_reached() {
    let plan = contract::compile(
        r#"component App
  state hit = "unset"
  state still = false
  action read
    hit = match elementFromPoint(150, 20) { case some(id) => id, case none => "none" }
  action calm
    still = true
  view
    column
      column id="page" height=800 inert=(not still)
        column id="route" position="absolute" top=0 left=0 width=200 height=500
          button id="sheet-button" width=100 height=40
            text "button"
      button press=read testId="read"
        text "read"
      button press=calm testId="calm"
        text "calm"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    block(&mut r, route, 200.0, 500.0);
    lay_out(&mut r);
    let hit = |r: &Runner<NoData>| r.slot("hit").and_then(Value::as_str).map(str::to_owned);
    r.top_layer_frames(&[(route, [100.0, 0.0, 200.0, 500.0])], None);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        hit(&r).as_deref(),
        Some("none"),
        "inert above the lifted block"
    );
    r.act("calm", vec![]).unwrap();
    lay_out(&mut r);
    r.top_layer_frames(&[(route, [100.0, 0.0, 200.0, 500.0])], None);
    r.act("read", vec![]).unwrap();
    assert_eq!(hit(&r).as_deref(), Some("sheet-button"));
}

/// A lifted block inherits its logical ancestors' `visibility` and
/// `pointer-events` (CSS inheritance follows the tree, not the top layer):
/// under a hidden or `pointer-events: none` root its button is not reached,
/// and a descendant that sets `visible` or `auto` again is.
#[test]
fn a_lifted_block_inherits_visibility_and_pointer_events() {
    let source = |page: &str, button: &str| {
        format!(
            r#"component App
  state hit = "unset"
  action read
    hit = match elementFromPoint(150, 20) {{ case some(id) => id, case none => "none" }}
  view
    column
      column id="page" height=800 {page}
        column id="route" position="absolute" top=0 left=0 width=200 height=500
          button id="sheet-button" width=100 height=40 {button}
            text "button"
      button press=read testId="read"
        text "read"
"#
        )
    };
    let read = |page: &str, button: &str| {
        let mut r = boot(contract::compile(&source(page, button)).unwrap());
        let route = view(&r, "route");
        block(&mut r, route, 200.0, 500.0);
        lay_out(&mut r);
        r.top_layer_frames(&[(route, [100.0, 0.0, 200.0, 500.0])], None);
        r.act("read", vec![]).unwrap();
        r.slot("hit").and_then(Value::as_str).map(str::to_owned)
    };
    assert_eq!(read("", "").as_deref(), Some("sheet-button"));
    assert_eq!(
        read(r#"visibility="hidden""#, "").as_deref(),
        Some("none"),
        "hidden by its logical root"
    );
    assert_eq!(
        read(r#"visibility="hidden""#, r#"visibility="visible""#).as_deref(),
        Some("sheet-button"),
        "visible again on the button"
    );
    assert_eq!(
        read(r#"pointer-events="none""#, "").as_deref(),
        Some("none"),
        "no pointer events from its logical root"
    );
    assert_eq!(
        read(r#"pointer-events="none""#, r#"pointer-events="auto""#).as_deref(),
        Some("sheet-button"),
        "auto again on the button"
    );
}

/// A modal presentation lifted to the top layer, as a sheet's route is
/// (LLP 1123 D2: every presentation is modal), makes the document behind it
/// inert to point reads, as UIKit's dimming view and the host's
/// `inertBehind` do: a root-level toast is not found outside the sheet nor
/// under it, and `elementsFromPoint` stops at the sheet. The host says
/// where the presentation begins in the top layer. A non-popover `dialog`
/// there is also a modal boundary. For a lifted box that is not such a
/// dialog, a host modal index of none leaves the document reachable.
#[test]
fn a_modal_presentation_makes_the_document_behind_it_inert_to_point_reads() {
    let source = r#"component App
  state outside = "unset"
  state under = "unset"
  state all = ""
  action read
    outside = match elementFromPoint(40, 80) { case some(id) => id, case none => "none" }
    under = match elementFromPoint(150, 300) { case some(id) => id, case none => "none" }
    all = elementsFromPoint(150, 300).join(",")
  view
    column id="root" height=800
      column id="route" navigationKey="1" position="absolute" top=0 left=0 width=200 height=500
        button id="sheet-button" width=100 height=40
          text "button"
      button id="toast" position="absolute" left=16 top=60 width=370 height=400
        text "toast"
      button press=read testId="read" position="absolute" top=700
        text "read"
"#;
    let read = |modal: Option<usize>| {
        let mut r = boot(contract::compile(source).unwrap());
        let route = view(&r, "route");
        block(&mut r, route, 200.0, 500.0);
        lay_out(&mut r);
        // A medium sheet: the lower part of the screen, from y 250
        r.top_layer_frames(&[(route, [100.0, 250.0, 200.0, 250.0])], modal);
        r.act("read", vec![]).unwrap();
        let slot = |name: &str| r.slot(name).and_then(Value::as_str).map(str::to_owned);
        (slot("outside"), slot("under"), slot("all"))
    };
    let (outside, under, all) = read(Some(0));
    assert_eq!(
        outside.as_deref(),
        Some("none"),
        "the toast behind the sheet, outside it"
    );
    assert_ne!(under.as_deref(), Some("toast"), "the toast under the sheet");
    assert!(
        !all.as_deref().unwrap_or("").contains("toast"),
        "nothing behind the sheet: {all:?}"
    );
    let (outside, _, all) = read(None);
    assert_eq!(
        outside.as_deref(),
        Some("toast"),
        "a popover leaves the page reachable"
    );
    assert!(all.as_deref().unwrap_or("").contains("toast"), "{all:?}");
}

/// The modal boundary holds whatever the sheet's own content admits: with
/// `inert` on the tab panel the sheet's route is declared under, its button
/// is not reached, and still nothing behind the sheet is (a root-level toast
/// outside it), nor matched by `querySelectorAll`, which leaves out what is
/// inert behind a presentation as it leaves out authored `inert`.
#[test]
fn the_modal_boundary_holds_for_an_inert_sheet_and_for_selectors() {
    let source = |panel: &str| {
        format!(
            r#"component App
  state outside = "unset"
  state all = ""
  state matched = ""
  action read
    outside = match elementFromPoint(40, 80) {{ case some(id) => id, case none => "none" }}
    all = elementsFromPoint(40, 80).join(",")
    matched = querySelectorAll("[data-paint~=\"accent\"]").join(",")
  view
    column id="root" height=800
      column id="panel" height=800 {panel}
        column id="route" navigationKey="1" position="absolute" top=0 left=0 width=200 height=500
          button id="sheet-button" width=100 height=40 data-paint="accent"
            text "button"
      button id="toast" position="absolute" left=16 top=60 width=370 height=60 data-paint="accent"
        text "toast"
      button press=read testId="read" position="absolute" top=700
        text "read"
"#
        )
    };
    let read = |panel: &str| {
        let mut r = boot(contract::compile(&source(panel)).unwrap());
        let route = view(&r, "route");
        block(&mut r, route, 200.0, 500.0);
        lay_out(&mut r);
        r.top_layer_frames(&[(route, [100.0, 250.0, 200.0, 250.0])], Some(0));
        r.act("read", vec![]).unwrap();
        let slot = |name: &str| r.slot(name).and_then(Value::as_str).map(str::to_owned);
        (slot("outside"), slot("all"), slot("matched"))
    };
    let (outside, all, matched) = read("inert=true");
    assert_eq!(
        outside.as_deref(),
        Some("none"),
        "the toast behind an inert sheet"
    );
    assert_eq!(all.as_deref(), Some(""), "nothing behind it either");
    assert_eq!(
        matched.as_deref(),
        Some(""),
        "the sheet's button is inert, the toast behind it: {matched:?}"
    );
    let (outside, _, matched) = read("");
    assert_eq!(outside.as_deref(), Some("none"));
    assert_eq!(
        matched.as_deref(),
        Some("sheet-button"),
        "the background toast is not matched"
    );
}

/// A block of no height (a sheet's content empty at a fitted detent) still
/// holds its root by the root's own frame: its 20-point margin is kept.
#[test]
fn a_block_of_no_height_holds_its_root_by_its_frame() {
    let plan = contract::compile(
        r#"component App
  state x = 0
  action read
    x = frame("route").x
  view
    column id="page" height=800
      column id="route" position="absolute" top=0 left=0 right=0 margin-left=20
      button press=read testId="read" position="absolute" top=700
        text "read"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let route = view(&r, "route");
    r.kernel_mut()
        .set_host_block(
            route,
            Some(exact_kernel::HostBlock {
                width: exact_kernel::BlockAxis::Is(300.0),
                height: exact_kernel::BlockAxis::Is(0.0),
            }),
        )
        .unwrap();
    lay_out(&mut r);
    r.top_layer_frames(&[(route, [100.0, 200.0, 300.0, 0.0])], None);
    r.act("read", vec![]).unwrap();
    assert_eq!(
        num(&r, "x"),
        120.0,
        "the block's origin and the root's margin"
    );
}

/// The frontmost presentation begins where the host says, and every box
/// from there is live: two routes of one sheet, shown together
/// mid-transition, are reached, and a sheet under them is not. A box the
/// runner ignores (a non-finite rectangle) before the index leaves the
/// index on the box it named. An index past every box, a presentation
/// none of whose routes is shown yet, leaves every box behind it.
#[test]
fn every_box_from_where_the_host_begins_the_presentation_is_live() {
    let plan = contract::compile(
        r#"component App
  state hits = ""
  action read
    let a = match elementFromPoint(40, 20) { case some(id) => id, case none => "none" }
    let b = match elementFromPoint(240, 20) { case some(id) => id, case none => "none" }
    let c = match elementFromPoint(340, 20) { case some(id) => id, case none => "none" }
    hits = [a, b, c].join(",")
  view
    column id="root" height=800
      column id="lower" navigationKey="1" position="absolute" top=0 left=0 width=200 height=500
        button id="lower-button" width=100 height=40
          text "lower"
      column id="first" navigationKey="2" position="absolute" top=0 left=0 width=200 height=500
        button id="first-button" width=100 height=40
          text "first"
      column id="gone" navigationKey="3" position="absolute" top=0 left=0 width=200 height=500
      column id="top" navigationKey="4" position="absolute" top=0 left=0 width=200 height=500
        button id="top-button" width=100 height=40
          text "top"
      button press=read testId="read" position="absolute" top=700
        text "read"
"#,
    )
    .unwrap();
    let mut r = boot(plan);
    let routes = ["lower", "first", "gone", "top"].map(|id| view(&r, id));
    for route in routes {
        block(&mut r, route, 200.0, 500.0);
    }
    lay_out(&mut r);
    let [lower, first, gone, top] = routes;
    // The lower sheet at the left, the front one's two routes beside it
    let frames = [
        (lower, [0.0, 0.0, 200.0, 500.0]),
        (gone, [f64::NAN, 0.0, 200.0, 500.0]),
        (first, [200.0, 0.0, 100.0, 500.0]),
        (top, [300.0, 0.0, 100.0, 500.0]),
    ];
    let mut read = |modal: Option<usize>| {
        r.top_layer_frames(&frames, modal);
        r.act("read", vec![]).unwrap();
        r.slot("hits").and_then(Value::as_str).map(str::to_owned)
    };
    assert_eq!(
        read(Some(2)).as_deref(),
        Some("none,first-button,top-button")
    );
    assert_eq!(
        read(None).as_deref(),
        Some("lower-button,first-button,top-button"),
        "no presentation: every box is reached"
    );
    assert_eq!(read(Some(4)).as_deref(), Some("none,none,none"));
}

/// A modal `dialog` escapes the inertness of its ancestors (HTML's modal
/// dialogs), and so does a popover open inside it; `inert` on the dialog
/// itself still holds.
#[test]
fn a_modal_dialog_escapes_an_inert_ancestor_but_not_its_own_inert() {
    let source = |dialog: &str| {
        format!(
            r#"component App
  state hit = "unset"
  state inner = "unset"
  action read
    hit = match elementFromPoint(20, 20) {{ case some(id) => id, case none => "none" }}
    inner = match elementFromPoint(20, 120) {{ case some(id) => id, case none => "none" }}
  view
    column
      column id="page" height=400 inert=true
        dialog id="dialog" position="absolute" left=0 top=0 width=200 height=200 {dialog}
          button id="dialog-button" width=100 height=40
            text "dialog"
          column id="menu" popover="auto" position="absolute" left=0 top=100 width=100 height=40
            button id="menu-button" width=100 height=40
              text "menu"
      button press=read testId="read" position="absolute" top=500
        text "read"
"#
        )
    };
    let read = |dialog: &str| {
        let mut r = boot(contract::compile(&source(dialog)).unwrap());
        lay_out(&mut r);
        let open = [view(&r, "dialog"), view(&r, "menu")];
        r.top_layer(&open);
        r.act("read", vec![]).unwrap();
        let slot = |name: &str| r.slot(name).and_then(Value::as_str).map(str::to_owned);
        (slot("hit"), slot("inner"))
    };
    let (hit, inner) = read("");
    assert_eq!(hit.as_deref(), Some("dialog-button"), "above an inert page");
    assert_eq!(
        inner.as_deref(),
        Some("menu-button"),
        "a popover open in the dialog"
    );
    let (hit, inner) = read("inert=true");
    assert_eq!(hit.as_deref(), Some("none"), "inert itself");
    assert_eq!(inner.as_deref(), Some("none"));
}
