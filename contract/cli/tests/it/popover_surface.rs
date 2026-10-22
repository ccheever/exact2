//! A popover's default surface (LLP 1021; theme2 2026-10-21 18:45, 19:55):
//! the compiler marks a `popover` whose author wrote no background, border,
//! shadow or material, on it or its class, in any state (`defaultSurface`), for a host
//! that paints the popover to draw the platform's surface under it.

use exact_kernel::{Kernel, PropId};
use exact_runner::{DataError, DataSource, Runner, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

const APP: &str = r##"style Card
  border-width=1
  border-style="solid"

component App
  view
    column
      button popovertarget="bare" testId="open"
        text "Open"
      column id="bare" popover="auto" padding=12 width="19rem" testId="bare"
        text "Popover text"
      column id="round" popover="auto" border-radius=12 testId="round"
        text "Rounded"
      column id="filled" popover="auto" background-color="white" testId="filled"
        text "Filled"
      column id="classed" popover="auto" class=Card testId="classed"
        text "Classed"
      column id="shadowed" popover="auto" box-shadow="0 2px 8px black" testId="shadowed"
        text "Shadowed"
      column id="glass" popover="auto" backgroundMaterial="glass" testId="glass"
        text "Glass"
      column id="hovered" popover="auto" testId="hovered"
        :hover
          background-color = "#dddddd"
        text "Hovered"
      column id="pressed" popover="auto" -exact-press-background-color="#dddddd" testId="pressed"
        text "Pressed"
      column id="clipped" popover="auto" background-clip="padding-box" testId="clipped"
        text "Clipped"
      column id="menu" popover="auto" role="menu" testId="menu"
        button testId="item"
          text "Item"
      column testId="plain"
        text "No popover"
"##;

#[test]
fn a_popover_whose_author_wrote_no_surface_is_marked_for_the_platforms() {
    let plan = contract::bake(
        contract::compile(APP).unwrap_or_else(|e| panic!("{e}")),
        NoData,
    )
    .unwrap();
    let r = Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let k = r.kernel();
    let marked = |t: &str| {
        let node = k.node_by_key(k.find_by_test_id(t)[0]).unwrap();
        node.props.bool(PropId::DefaultSurface) == Some(true)
    };
    assert!(marked("bare"), "no surface: the platform's");
    assert!(marked("round"), "a corner draws no surface");
    assert!(marked("clipped"), "nor does a background's clip alone");
    for t in [
        "filled", "classed", "shadowed", "glass", "hovered", "pressed", "menu", "plain", "open",
    ] {
        assert!(!marked(t), "{t}");
    }
}

#[test]
fn default_surface_is_the_compilers() {
    let e = contract::compile(
        "component App\n  view\n    column popover=\"auto\" defaultSurface=true\n      text \"x\"\n",
    )
    .unwrap_err();
    assert_eq!(e.id, "lower-default-surface", "{}", e.message);
}
