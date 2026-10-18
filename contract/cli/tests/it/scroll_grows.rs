//! `lower-scroll-grows`: a scroller filling a flex column that is at least
//! as tall as its content. Each case is laid out too, so the warning is held
//! to the layout: it names a wrapper exactly when the scroller comes out as
//! tall as its 30 rows (1,500 points) inside a 400-point column.

use crate::warnings::warned;
use exact_kernel::{Kernel, Offer};
use exact_runner::{DataError, DataSource, Runner, Value};

const ID: &str = "lower-scroll-grows";

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

/// An app whose view is `body` (indented under a 400-point column) and whose
/// scroller, `id="s"`, holds 30 rows of 50 points.
fn app(body: &str) -> String {
    let mut src = "component App\n  view\n    column height=400\n".to_string();
    for line in body.lines() {
        src += &format!("      {line}\n");
        if line.contains("id=\"s\"") {
            let indent = " ".repeat(line.len() - line.trim_start().len() + 8);
            src += &format!("{indent}for i in [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30] key=i\n{indent}  column height=50\n");
        }
    }
    src
}

fn scroller_height(src: &str) -> f32 {
    let plan = contract::compile(src).unwrap_or_else(|e| panic!("{e}\n{src}"));
    let mut r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = r.kernel().roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(400.0, 800.0))
        .unwrap();
    let key = r.kernel().find_by_id("s")[0];
    let view = r.kernel().arena().local_id(key.index);
    r.kernel().node(view).unwrap().frame.height
}

/// The lines (in the view, from 1) the warning names for `body`.
fn flagged(body: &str) -> Vec<u32> {
    warned(&app(body))
        .into_iter()
        .filter(|(id, _)| id == ID)
        .map(|(_, line)| line - 3)
        .collect()
}

#[test]
fn a_wrapper_that_grows_with_its_scroller_is_named_and_one_that_scrolls_is_not() {
    for (body, lines) in [
        // The scroller is the column's flex item: its minimum is zero.
        ("scroll id=\"s\" flex=1", vec![]),
        ("view id=\"s\" overflow-y=\"auto\" flex=1", vec![]),
        // A wrapper between them is at least as tall as its content.
        ("column flex=1\n  scroll id=\"s\" flex=1", vec![1]),
        ("column\n  scroll id=\"s\" flex=1", vec![1]),
        (
            "view display=\"flex\" flex-direction=\"column\" flex=1\n  view id=\"s\" overflow=\"auto\" flex=1",
            vec![1],
        ),
        ("column flex=1\n  if true\n    scroll id=\"s\" flex=1", vec![1]),
        // Every wrapper of a chain: fixing the inner one alone still grows.
        ("column\n  column flex=1\n    scroll id=\"s\" flex=1", vec![1, 2]),
        ("column\n  column flex=1 min-height=0\n    scroll id=\"s\" flex=1", vec![1]),
        (
            "column\n  column flex=\"1 1 0px\" min-height=0\n    scroll id=\"s\" flex=1",
            vec![1],
        ),
        // The repair, and what else bounds the wrapper or the scroller.
        ("column flex=1 min-height=0\n  scroll id=\"s\" flex=1", vec![]),
        ("column min-height=0\n  scroll id=\"s\" flex=1", vec![]),
        (
            "column flex=1 min-height=0\n  column min-height=0\n    scroll id=\"s\" flex=1",
            vec![],
        ),
        ("column flex=1 height=400\n  scroll id=\"s\" flex=1", vec![]),
        ("column flex=1 overflow=\"hidden\"\n  scroll id=\"s\" flex=1", vec![]),
        ("column flex=1\n  scroll id=\"s\" flex=\"1 1 0px\"", vec![]),
        // A basis it cannot grow from, under a zero minimum, caps what it
        // passes up (CSS Flexbox §9.9.3).
        (
            "column flex=1\n  column flex=\"0 1 200px\" min-height=0\n    scroll id=\"s\" flex=1",
            vec![],
        ),
    ] {
        assert_eq!(flagged(body), lines, "{body}");
        let height = scroller_height(&app(body));
        let grows = height > 400.0;
        assert_eq!(grows, !lines.is_empty(), "{body}: the scroller is {height} tall");
    }
}

#[test]
fn what_the_walk_cannot_be_sure_of_is_not_named() {
    for body in [
        // The scroller's own height bounds it, whatever its wrapper does.
        "column flex=1\n  scroll id=\"s\" height=200",
        // Out of flow, or computed.
        "column flex=1 position=\"absolute\" top=0 bottom=0\n  scroll id=\"s\" flex=1",
        "column flex=1 min-height=(true ? 0 : 10)\n  scroll id=\"s\" flex=1",
        "column flex=1 display=(true ? \"flex\" : \"none\")\n  scroll id=\"s\" flex=1",
        // A row's items stretch on the cross axis: not this warning's.
        "row flex=1\n  scroll id=\"s\" flex=1",
    ] {
        assert_eq!(flagged(body), Vec::<u32>::new(), "{body}");
    }
}

/// The view's lines, under `component App` and `view`, and the lines (from 1)
/// the warning names in them.
fn flagged_view(view: &str) -> Vec<u32> {
    let src: String = view.lines().map(|l| format!("    {l}\n")).collect();
    warned(&format!("component App\n  view\n{src}"))
        .into_iter()
        .filter(|(id, _)| id == ID)
        .map(|(_, line)| line - 2)
        .collect()
}

#[test]
fn a_wrapper_is_named_only_under_a_box_that_bounds_it() {
    const RUN: &str = "  text \"Title\"\n  column flex=1\n    scroll flex=1\n      text \"body\"";
    for (top, lines) in [
        // No height anywhere: the page grows as a whole, and `min-height=0`
        // would change nothing.
        ("column", vec![]),
        ("column flex=1", vec![]),
        ("column height=\"50%\"", vec![3]),
        ("column max-height=400", vec![3]),
        ("column height=\"100%\" padding=16", vec![3]),
        ("column position=\"absolute\" inset=0", vec![3]),
        // The viewport's units are lengths.
        ("column height=\"100vh\"", vec![3]),
        ("column max-height=\"50dvh\"", vec![3]),
        // Both vertical insets lengths, after the shorthand and longhands in
        // order; `auto` or a computed one is no bound.
        ("column position=\"absolute\" inset=\"0 16px\"", vec![3]),
        ("column position=\"absolute\" top=0 bottom=0", vec![3]),
        ("column position=\"absolute\" inset=\"auto\"", vec![]),
        ("column position=\"absolute\" top=0 bottom=\"auto\"", vec![]),
        (
            "column position=\"absolute\" inset=0 bottom=\"auto\"",
            vec![],
        ),
        (
            "column position=\"absolute\" top=(true ? 0 : 8) bottom=0",
            vec![],
        ),
    ] {
        assert_eq!(flagged_view(&format!("{top}\n{RUN}")), lines, "{top}");
    }
    // A percentage of a height that is not definite is not one.
    let nested = format!(
        "column\n  column height=\"50%\"\n  {}",
        RUN.replace('\n', "\n  ")
    );
    assert_eq!(flagged_view(&nested), Vec::<u32>::new(), "{nested}");
    // A route is sized by its host (the screen, or the sheet): the euxy
    // sheet, whose component root had `flex=1` and no minimum.
    let route = "routes nav\n  home \"/\"\ncomponent App\n  view\n    main navigationKey=`${top(nav).id}`\n      for e in stack(nav) key=e.id\n        column navigationKey=`${e.id}` navigationPresentation=\"modal\" position=\"absolute\" inset=0\n          Editor()\ncomponent Editor\n  view\n    column flex=1\n      text \"Edit\"\n      scroll flex=1\n        text \"body\"\n";
    let lines: Vec<u32> = warned(route)
        .into_iter()
        .filter(|(id, _)| id == ID)
        .map(|(_, line)| line)
        .collect();
    assert_eq!(lines, [11]);
}

/// The lines of an app whose navigation root is `root`, holding one route
/// `route` whose child is a wrapper over a scroller, that the warning names.
fn flagged_route(root: &str, route: &str) -> Vec<u32> {
    let src = format!("routes nav\n  home \"/\"\ncomponent App\n  view\n{root}\n        for e in stack(nav) key=e.id\n          {route}\n            text \"Title\"\n            column flex=1\n              scroll flex=1\n                text \"body\"\n");
    warned(&src)
        .into_iter()
        .filter(|(id, _)| id == ID)
        .map(|(_, line)| line)
        .collect()
}

#[test]
fn a_navigation_key_alone_bounds_nothing() {
    const ROOT: &str = "      main navigationKey=`${top(nav).id}` height=\"100%\"";
    const ROUTE: &str = "column navigationKey=`${e.id}` position=\"absolute\" inset=0";
    // A pushed route in a root whose height is definite: its block is.
    assert_eq!(flagged_route(ROOT, ROUTE), [9]);
    // A navigation root inside a scrolling page grows with the page.
    let scrolled = "    scroll height=\"100%\"\n      column navigationKey=`${top(nav).id}`";
    assert_eq!(flagged_route(scrolled, ROUTE), Vec::<u32>::new());
    // A route that is itself the scroller is measured by what it scrolls.
    let scroller = "scroll navigationKey=`${e.id}` position=\"absolute\" inset=0 height=\"100%\" display=\"flex\" flex-direction=\"column\"";
    assert_eq!(flagged_route(ROOT, scroller), Vec::<u32>::new());
    // A route with no insets is as tall as its content.
    let bare = "column navigationKey=`${e.id}`";
    assert_eq!(flagged_route(ROOT, bare), Vec::<u32>::new());
}

#[test]
fn a_wrapper_that_cannot_shrink_ends_the_run() {
    // The inner column keeps its content's height whatever its minimum, so
    // `min-height=0` on the outer one would free nothing: silence.
    let view = "column height=400\n  column\n    column flex-shrink=0\n      scroll flex=1\n        text \"body\"";
    assert_eq!(flagged_view(view), Vec::<u32>::new());
}

#[test]
fn the_warning_names_its_repair_and_the_scroller() {
    let src = app("column flex=1\n  scroll id=\"s\" flex=1");
    let (_, warnings) = contract::compile_warned(&src);
    let w = warnings.iter().find(|w| w.id == ID).unwrap();
    assert!(w.message.contains("`scroll` at line 5"), "{w}");
    assert!(
        w.message.contains("Write `min-height=0` on this `column`"),
        "{w}"
    );
}

#[test]
fn a_component_root_is_named_where_it_is_declared() {
    let src = "component App\n  view\n    column height=400\n      Sheet()\n      Sheet()\n\ncomponent Sheet\n  view\n    column flex=1\n      text \"Title\"\n      scroll flex=1\n        text \"body\"\n";
    let lines: Vec<u32> = warned(src)
        .into_iter()
        .filter(|(id, _)| id == ID)
        .map(|(_, line)| line)
        .collect();
    assert_eq!(lines, [9]);
}
