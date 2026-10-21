//! `font-weight` and `font-stretch` transitions (LLP 1019 §13 D10): the
//! runner plays them, writing each frame's value as a commit, with CSS's
//! before-change value and reversal.
use exact_kernel::Kernel;
use exact_plan::Value;
use exact_runner::{agent, DataError, DataSource, Runner};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        panic!("unexpected {name}")
    }
}

fn boot(transition: &str) -> Runner<NoData> {
    let source = format!(
        r#"
component Main
  state bold = false
  action toggle
    bold = !bold
  view
    column
      text "Library" testId="title" font-weight=(bold ? 700 : 300) font-stretch=(bold ? "125%" : "75%") transition="{transition}"
"#
    );
    let plan = contract::compile(&source).unwrap();
    Runner::boot(
        plan,
        NoData,
        Kernel::with_monospace(),
        Default::default(),
        "/",
    )
    .unwrap()
}

/// The title's weight and stretch, as the kernel holds them.
fn axes(r: &Runner<NoData>) -> (u16, f32) {
    let found = agent::handle(r, r#"{"op":"tree","target":"title","shallow":true}"#);
    let id: u32 = found
        .split("{\"id\":")
        .nth(1)
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let node = r.kernel().node(id).unwrap();
    let style = r.kernel().arena().style(node.key.index);
    (style.font_weight, style.font_stretch)
}

#[test]
fn a_weight_change_moves_through_the_frames_and_lands_on_its_target() {
    let mut r = boot("font-weight 400ms linear, font-stretch 400ms linear");
    assert_eq!(axes(&r), (300, 75.0));
    r.act("toggle", vec![]).unwrap();
    // The commit shows the value presented now, the before-change one.
    assert_eq!(axes(&r), (300, 75.0));
    assert!(r.wants_frames());
    assert_eq!(r.text_axes_settle_ms(), Some(400.0));
    r.advance(100.0).unwrap();
    assert_eq!(axes(&r), (400, 87.5));
    r.advance(200.0).unwrap();
    assert_eq!(axes(&r), (500, 100.0));
    r.advance(400.0).unwrap();
    assert_eq!(axes(&r), (700, 125.0));
    assert!(!r.wants_frames());
    assert_eq!(r.text_axes_settle_ms(), None);
}

#[test]
fn a_reversal_halfway_returns_in_half_the_time() {
    let mut r = boot("font-weight 400ms linear");
    r.act("toggle", vec![]).unwrap();
    r.advance(200.0).unwrap();
    assert_eq!(axes(&r).0, 500);
    // Stretch has no transition: it lands at once both ways.
    assert_eq!(axes(&r).1, 125.0);
    r.act("toggle", vec![]).unwrap();
    assert_eq!(axes(&r), (500, 75.0));
    // CSS Transitions §3.2: the reversing shortening factor is one half.
    assert_eq!(r.text_axes_settle_ms(), Some(400.0));
    r.advance(300.0).unwrap();
    assert_eq!(axes(&r).0, 400);
    r.advance(400.0).unwrap();
    assert_eq!(axes(&r).0, 300);
}

#[test]
fn a_spring_plays_as_its_curve_and_settles() {
    let mut r = boot("font-weight -exact-spring(240, 26, 1)");
    r.act("toggle", vec![]).unwrap();
    let end = r.text_axes_settle_ms().unwrap();
    assert!(end > 100.0 && end < 2000.0, "{end}");
    r.advance(end / 4.0).unwrap();
    let mid = axes(&r).0;
    assert!(mid > 300 && mid < 700, "{mid}");
    r.advance(end).unwrap();
    assert_eq!(axes(&r).0, 700);
}

#[test]
fn without_a_transition_a_weight_lands_at_once() {
    let mut r = boot("opacity 400ms");
    r.act("toggle", vec![]).unwrap();
    assert_eq!(axes(&r), (700, 125.0));
    assert!(!r.wants_frames());
}
