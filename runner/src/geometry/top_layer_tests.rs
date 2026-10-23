//! A popover's geometry in its block, the viewport (stage 14 B): sized
//! against the viewport, not its narrow positioned ancestor, and read where
//! its host shows it.
use super::*;
use crate::{DataError, DataSource, Runner};
use exact_kernel::Offer;

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, source: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(source.into()))
    }
}

#[test]
fn a_popover_is_read_where_its_host_shows_its_block() {
    let source = r#"component App
  state point = "unset"
  action read(x: number, y: number)
    point = match elementFromPoint(x, y) { case some(id) => id, case none => "none" }
  view
    column id="page" width=400 height=800
      column id="header" position="relative" width=100 height=40
        column id="pins" popover="manual" width="50%"
          column id="pin" height=30
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
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
    let pins = view_of(r.kernel(), "pins").unwrap();
    let pin = view_of(r.kernel(), "pin").unwrap();
    assert_eq!(r.kernel().host_block_size(pins), Some((200.0, 30.0)));
    // The host places the block, here under the header.
    let rect = [50.0, 60.0, 200.0, 30.0];
    r.top_layer_frames(&[(pins, rect, None)], None);
    let mut scrolled = Scrolled::default();
    scrolled.top_layer_frames(r.kernel(), &[(pins, rect, None)], None);
    for view in [pins, pin] {
        let f = kernel_frame(r.kernel(), &scrolled, view);
        assert_eq!((f.x, f.y, f.width, f.height), (50.0, 60.0, 200.0, 30.0));
    }
    for (x, y, hit) in [
        (240.0, 70.0, "pin"),
        (260.0, 70.0, "page"),
        (60.0, 20.0, "header"),
    ] {
        r.act("read", vec![Value::Number(x), Value::Number(y)])
            .unwrap();
        assert_eq!(r.slot("point"), Some(&Value::str(hit)), "at {x}, {y}");
    }
}

/// The viewport is no box: a popover's implied block clips nothing, so a
/// child it lets overflow is read where it shows, past the popover's box.
#[test]
fn a_popovers_block_clips_nothing() {
    let source = r#"component App
  state point = "unset"
  action read(x: number, y: number)
    point = match elementFromPoint(x, y) { case some(id) => id, case none => "none" }
  view
    column id="page" width=400 height=800
      column id="pins" popover="manual" width=100 height=40
        column id="below" position="absolute" left=0 top=50 width=100 height=20
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
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
    let pins = view_of(r.kernel(), "pins").unwrap();
    r.top_layer_frames(&[(pins, [10.0, 10.0, 100.0, 40.0], None)], None);
    r.act("read", vec![Value::Number(20.0), Value::Number(65.0)])
        .unwrap();
    assert_eq!(r.slot("point"), Some(&Value::str("below")));
}
