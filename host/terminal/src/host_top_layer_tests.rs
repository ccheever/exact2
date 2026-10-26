//! A popover in the terminal's top layer (stage 14 B): laid out in the
//! viewport's block, painted by its insets in the grid, centred with none,
//! anchored to the bottom inline, and not revealed through the page.
use super::*;

fn app(insets: &str) -> String {
    format!(
        r#"component App
  state at = []
  state hit = ""
  action read
    at = [frame("pop").x, frame("pop").y, frame("pop").width]
    hit = match elementFromPoint(24, 40) {{ case some(id) => id, case none => "none" }}
  view
    scroll id="page" height="100%"
      box height=1000
      box id="narrow" position="relative" width=40 height=32
        column id="pop" popover="manual" width=80 height=32 {insets}
          button id="inside" width=80
            text "In"
      box height=1000
"#
    )
}

fn boot(insets: &str, mode: Mode) -> Host<()> {
    let plan = contract::compile(&app(insets)).unwrap();
    Host::boot(plan, (), mode, 40, 20).unwrap()
}

fn node(host: &Host<()>, name: &str) -> ViewId {
    host.find(|n| n.props.str(PropId::Id) == Some(name))
        .unwrap()
}

/// The open popover's cells, full screen.
fn layer(host: &mut Host<()>) -> (i32, i32, i32, i32) {
    let pop = node(host, "pop");
    host.open_layer(pop, false);
    let r = host.frame().layer.expect("a layer");
    (r.x, r.y, r.w, r.h)
}

#[test]
fn a_popover_is_placed_by_its_insets_in_the_grid() {
    let mut host = boot("top=32 left=16", Mode::Fullscreen);
    assert!(host
        .runner
        .kernel()
        .host_block(node(&host, "pop"))
        .is_some());
    assert_eq!(layer(&mut host), (2, 2, 10, 2), "2 columns in, 2 rows down");
    let mut host = boot("right=0 bottom=0", Mode::Fullscreen);
    assert_eq!(
        layer(&mut host),
        (30, 18, 10, 2),
        "at the grid's far corner"
    );
    let mut host = boot("", Mode::Fullscreen);
    assert_eq!(layer(&mut host), (15, 9, 10, 2), "centred with no inset");
    let mut host = boot(
        "left=0 right=0 margin-left=\"auto\" margin-right=\"auto\" top=0",
        Mode::Fullscreen,
    );
    assert_eq!(
        layer(&mut host),
        (15, 0, 10, 2),
        "centred between its insets by auto margins"
    );
    let mut host = boot(
        "left=0 right=0 margin-left=\"auto\" margin-right=0 top=0",
        Mode::Fullscreen,
    );
    assert_eq!(
        layer(&mut host),
        (30, 0, 10, 2),
        "one auto margin takes the room left"
    );
}

/// The runner reads the popover where it paints: `frame` and
/// `elementFromPoint` through the block's rectangle the host tells it.
#[test]
fn the_runner_reads_a_popover_where_it_paints() {
    let mut host = boot("top=32 left=16", Mode::Fullscreen);
    layer(&mut host);
    host.act("read");
    let at = host.runner.slot("at").cloned();
    let n = |v: f64| exact_runner::Value::Number(v);
    assert_eq!(
        at,
        Some(exact_runner::Value::list(vec![n(16.0), n(32.0), n(80.0)]))
    );
    let hit = host.runner.slot("hit").cloned();
    assert_eq!(
        hit,
        Some(exact_runner::Value::str("inside")),
        "a point in its painted cells"
    );
}

/// Inline, the offer's height is the content's: no viewport, so no block,
/// and the layer is anchored to the bottom of the rows painted.
#[test]
fn inline_a_popover_is_anchored_to_the_bottom() {
    let mut host = boot("top=32 left=16", Mode::Inline);
    let pop = node(&host, "pop");
    assert!(
        host.runner.kernel().host_block(pop).is_none(),
        "no viewport inline"
    );
    host.open_layer(pop, false);
    let r = host.render(0, 6).layer.expect("a layer");
    assert_eq!((r.y, r.h), (4, 2));
}

/// Focus inside a popover scrolls nothing behind it: the walk up to the
/// scrollers stops at the popover, the root of its block.
#[test]
fn reveal_stops_at_the_popover() {
    let mut host = boot("top=32 left=16", Mode::Fullscreen);
    let page = node(&host, "page");
    host.scroll.insert(page, 500.0);
    let pop = node(&host, "pop");
    host.open_layer(pop, false);
    let inside = node(&host, "inside");
    host.reveal_to(inside, "start");
    assert_eq!(host.scroll.get(&page).copied(), Some(500.0));
}
