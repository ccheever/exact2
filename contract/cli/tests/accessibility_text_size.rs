//! `exactViewport().accessibilityTextSize`: true on iOS while the host's
//! text size is an accessibility size (a Dynamic Type body of 28 or more,
//! UIKit's `isAccessibilityCategory`), re-answered as the size crosses it,
//! and false in any other column. One test, its own process: the column is
//! the process's.
use exact_kernel::style::metrics::{set_column, Column};
use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;

impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const SOURCE: &str = r#"use EmptyState from "exact:content"

shape Text
  accessibilityTextSize: bool

component App
  resource media = exactViewport() as shape Text
  view
    column
      text (media.accessibilityTextSize ? "large" : "standard") testId="size"
      column testId="empty" height=400
        EmptyState(symbol="symbol:calendar", title="Nothing yet", message="Log a glass.")
      Probe()
      text "28" font-size="min(-exact-subheadline, 31px)" testId="day"

component Probe
  resource fact = exactViewport() as shape Text
  state seen = "unread"
  action read
    seen = fact.accessibilityTextSize ? "large" : "standard"
  view
    button press=read testId="read"
      text seen testId="seen"
"#;

fn boot() -> Runner<NoData> {
    boot_at(None)
}

/// Booted on a kernel the host primed with `size` before the runner, as the
/// Apple host does.
fn boot_at(size: Option<f32>) -> Runner<NoData> {
    let dir = std::env::temp_dir().join(format!("exact2-ax-text-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("app.contract");
    std::fs::write(&path, SOURCE).unwrap();
    let plan = contract::bake(contract::compile_path(&path).unwrap(), NoData).unwrap();
    let mut kernel = Kernel::with_monospace();
    if let Some(size) = size {
        kernel.set_root_font_size(size).unwrap();
    }
    Runner::boot(plan, NoData, kernel, Default::default(), "/").unwrap()
}

/// Whether the empty state shows its symbol, a library child reading the
/// fact itself.
fn symbol(r: &Runner<NoData>) -> bool {
    let k = r.kernel();
    let mut stack = vec![k.node_by_key(k.find_by_test_id("empty")[0]).unwrap().id];
    while let Some(id) = stack.pop() {
        let node = k.node(id).unwrap();
        if node.node_type == exact_kernel::NodeType::Image {
            return true;
        }
        stack.extend(node.children());
    }
    false
}

fn shown(r: &Runner<NoData>) -> String {
    let k = r.kernel();
    let node = k.node_by_key(k.find_by_test_id("size")[0]).unwrap();
    node.props.str(PropId::Text).unwrap_or("").to_owned()
}

#[test]
fn the_accessibility_sizes_are_ios_bodies_of_28_and_more() {
    set_column(Column::Ios);
    let mut r = boot();
    assert_eq!(shown(&r), "standard", "the default size");
    assert!(symbol(&r), "EmptyState's symbol at the default size");
    r.set_root_font_size(23.0).unwrap();
    assert_eq!(shown(&r), "standard", "the largest standard size");
    let day = |r: &Runner<NoData>| {
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id("day")[0]).unwrap();
        node.computed_row(exact_kernel::StyleId::FontSize, |s| s.font_size)
    };
    assert!(
        day(&r) < 31.0,
        "a capped style at the default size is its own: {}",
        day(&r)
    );
    for body in [28.0, 53.0] {
        r.set_root_font_size(body).unwrap();
        assert_eq!(shown(&r), "large", "{body}: an accessibility size");
        assert!(!symbol(&r), "{body}: no symbol, as UIKit's empty view");
    }
    assert_eq!(day(&r), 31.0, "held to its cap at AX5");
    // A child's action reads its own fact too.
    let read = |r: &mut Runner<NoData>| {
        let k = r.kernel();
        let id = k.node_by_key(k.find_by_test_id("read")[0]).unwrap().id;
        r.dispatch(id, exact_runner::Event::Press).unwrap();
        let k = r.kernel();
        let node = k.node_by_key(k.find_by_test_id("seen")[0]).unwrap();
        node.props.str(PropId::Text).unwrap_or("").to_owned()
    };
    assert_eq!(read(&mut r), "large", "a child's action");
    r.set_root_font_size(17.0).unwrap();
    assert_eq!(read(&mut r), "standard", "a child's action, again");
    assert_eq!(shown(&r), "standard", "back to the default");
    assert!(symbol(&r), "the symbol again");
    // A size the host primed before boot.
    let mut r = boot_at(Some(33.0));
    assert_eq!(shown(&r), "large", "primed before boot");
    r.set_root_font_size(33.0).unwrap();
    assert_eq!(shown(&r), "large", "told again");
    set_column(Column::Web);
    let mut r = boot();
    r.set_root_font_size(40.0).unwrap();
    assert_eq!(shown(&r), "standard", "no such fact on the web");
}
