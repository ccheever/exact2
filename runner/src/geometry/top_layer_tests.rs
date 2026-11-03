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

mod sticky {
    use crate::geometry::{GeometryEnv, Scrolled, KERNEL};
    use crate::Runner;
    use exact_kernel::{Kernel, Offer};
    use exact_plan::{Stdlib, Value};
    #[test]
    fn sticky_frames_and_point_reads_follow_the_pinned_box() {
        let source = r#"component App
  view
    scroll id="scroller" width=200 height=100
      column height=500
        box id="sticky" position="sticky" top=0 width=50 height=40
          box id="child" width=20 height=20
"#;
        let p = contract::compile(source).unwrap();
        let p = exact_plan::Plan::decode(&p.encode()).unwrap();
        let mut r = Runner::boot(p, (), Kernel::with_monospace(), Default::default(), "/").unwrap();
        let root = r.roots()[0];
        r.kernel_mut()
            .compute_layout(root, Offer::definite(200., 100.))
            .unwrap();
        let sticky = r.kernel().find_by_id("sticky")[0];
        let scroller = r.kernel().find_by_id("scroller")[0];
        let constraint = r
            .kernel()
            .sticky_constraint(sticky)
            .expect("sticky placement");
        let mut s = Scrolled::default();
        s.note(
            r.kernel(),
            Some(r.kernel().node_by_key(scroller).unwrap().id),
            0.,
            80.,
        );
        let env = GeometryEnv::new(r.kernel(), &s, &KERNEL, &[]);
        let frame = env
            .read(r.plan(), Stdlib::Frame, &[Value::str("sticky")])
            .unwrap();
        let hit = env
            .read(
                r.plan(),
                Stdlib::ElementFromPoint,
                &[Value::Number(10.), Value::Number(10.)],
            )
            .unwrap();
        let Value::Record(fields) = frame else {
            panic!()
        };
        assert_eq!(constraint.offset((0., 80.)).1, 80.);
        assert_eq!(fields[1], Value::Number(0.));
        assert_eq!(hit, Value::some(Value::str("child")));
        let Value::Record(child) = env
            .read(r.plan(), Stdlib::Frame, &[Value::str("child")])
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(child[1], Value::Number(0.));
        let hit = env
            .read(
                r.plan(),
                Stdlib::ElementFromPoint,
                &[Value::Number(40.), Value::Number(10.)],
            )
            .unwrap();
        assert_eq!(hit, Value::some(Value::str("sticky")));
        let old = env
            .read(
                r.plan(),
                Stdlib::ElementFromPoint,
                &[Value::Number(10.), Value::Number(-70.)],
            )
            .unwrap();
        assert_ne!(old, Value::some(Value::str("sticky")));
        assert_ne!(old, Value::some(Value::str("child")));
    }
}

#[test]
fn an_unpainted_border_does_not_clip_descendant_hits() {
    let source = r#"component App
  view
    box id="clip" width=4 height=4 overflow="hidden" border-width=30 border-style="none"
      box id="child" width=4 height=4
"#;
    let mut r = Runner::boot(
        contract::compile(source).unwrap(),
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap();
    let root = r.roots()[0];
    r.kernel_mut()
        .compute_layout(root, Offer::definite(4., 4.))
        .unwrap();
    let scrolled = Scrolled::default();
    let env = GeometryEnv::new(r.kernel(), &scrolled, &KERNEL, &[]);
    for x in [0.5, 3.5] {
        let hit = env
            .read(
                r.plan(),
                exact_plan::Stdlib::ElementFromPoint,
                &[Value::Number(x), Value::Number(x)],
            )
            .unwrap();
        assert_eq!(hit, Value::some(Value::str("child")));
    }
}
