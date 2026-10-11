//! A press's screen is laid out ahead of its release: its text is shaped by
//! then, and nothing shown, committed or kept by the runner has changed.
use exact_linux::{presenter::PainterChoice, Presenter};
use exact_runner::{agent, DataError, DataSource};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Default)]
struct Empty;
impl DataSource for Empty {
    fn query(
        &mut self,
        name: &str,
        _: &[exact_runner::Value],
    ) -> Result<exact_runner::Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const APP: &str = r##"component App
  state shown = none
  state n = 0
  action open(subject: string)
    shown = some(subject)
  action bump
    n = n + 1
  view
    box width="100%" height="100%" position="relative" font-family="system-ui"
      column position="absolute" top=0 left=0 right=0 bottom=0
        button press=open("Quarterly planning") testId="row" width="100%" height=76
          text "Open the mail" font-size=16
        button press=bump testId="bump" width="100%" height=76
          text `${n}` testId="count"
      column testId="detail" position="absolute" top=0 left=0 right=0 bottom=0 padding=16 inert=(shown == none) translate=(shown == none ? "400px 0px" : "0px 0px")
        match shown
          case some(subject)
            text subject testId="subject" font-size=18 font-weight=600
            text "The first paragraph of the body is long enough to wrap onto a second line at this width, and then some." testId="p1" font-size=16 line-height="24px"
            text "A second paragraph, shorter than the first, which still wraps at four hundred points." testId="p2" font-size=16 line-height="24px"
            row gap=16
              text "Reply" testId="reply" flex=1 font-size=15 font-weight=600
              text "Forward" flex=1 font-size=15 font-weight=600
          case none
"##;

fn boot() -> Presenter<Empty> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn frame(p: &Presenter<Empty>, name: &str) -> exact_kernel::Frame {
    let kernel = p.host().kernel();
    let key = kernel.find_by_test_id(name)[0];
    kernel.node_by_key(key).unwrap().frame
}

fn centre(p: &Presenter<Empty>, name: &str) -> (f32, f32) {
    let f = frame(p, name);
    (f.x + f.width / 2., f.y + f.height / 2.)
}

/// (sources shaped, layout misses) so far.
fn work(p: &Presenter<Empty>) -> (usize, usize) {
    let text = p.text().borrow();
    (text.shape_calls, text.measures - text.hits)
}

fn seen(p: &Presenter<Empty>) -> (String, String, usize, u64) {
    let host = p.host();
    (
        agent::tree(host.runner()),
        agent::state(host.runner()),
        host.kernel().live_count(),
        host.kernel().epoch(),
    )
}

#[test]
fn the_release_finds_its_screen_shaped_and_nothing_changed_before_it() {
    let mut p = boot();
    let (x, y) = centre(&p, "row");
    p.pointer_down(x, y, 0.).unwrap();
    let (before, cold) = (seen(&p), work(&p));
    p.foresee_press(x, y, 0.);
    let mut slices = 0;
    while p.foresee_slice(Duration::from_secs(1)) {
        slices += 1;
        assert!(
            slices < 4,
            "an unhurried warm is the runner's slice and one pass"
        );
    }
    let warm = work(&p);
    // The subject, two paragraphs and two labels.
    assert_eq!(warm.0 - cold.0, 5, "five texts shaped ahead");
    assert_eq!(
        seen(&p),
        before,
        "the warm commits nothing and mounts nothing"
    );
    assert!(p.host().kernel().find_by_test_id("p1").is_empty());

    p.pointer_up(x, y, 60.).unwrap();
    let released = work(&p);
    assert_eq!(released.0, warm.0, "the release shapes nothing");

    // What the release shows is what it shows with no warm, which shapes the
    // same five then and breaks more lines (the cache keeps a text's last
    // width: the widths a pass tries before it are broken again).
    let mut twin = boot();
    twin.pointer_down(x, y, 0.).unwrap();
    twin.pointer_up(x, y, 60.).unwrap();
    let unwarmed = work(&twin);
    assert_eq!(unwarmed.0 - cold.0, 5, "the same five, at the release");
    assert!(
        2 * (released.1 - warm.1) < unwarmed.1 - cold.1,
        "{released:?} after {warm:?}, against {unwarmed:?}"
    );
    for name in ["subject", "p1", "p2", "reply"] {
        assert_eq!(frame(&p, name), frame(&twin, name), "{name}");
    }
    assert!(frame(&p, "p1").height > 24., "the paragraph wrapped");
    assert_eq!(
        agent::tree(p.host().runner()),
        agent::tree(twin.host().runner())
    );
}

#[test]
fn a_slice_stops_at_its_budget_and_the_slices_end() {
    let mut p = boot();
    let (x, y) = centre(&p, "row");
    p.pointer_down(x, y, 0.).unwrap();
    let cold = work(&p);
    p.foresee_press(x, y, 0.);
    // The runner's slice, then passes with no time to shape in.
    assert!(p.foresee_slice(Duration::ZERO));
    let mut passes = 0;
    while p.foresee_slice(Duration::ZERO) {
        passes += 1;
        assert!(passes <= 12, "a pass that never finishes is given up");
    }
    assert_eq!(passes, 12);
    assert_eq!(work(&p), cold, "a spent slice shapes nothing");
    assert!(
        !p.foresee_slice(Duration::from_secs(1)),
        "and it is forgotten"
    );
}

#[test]
fn input_or_a_commit_abandons_it_and_a_press_that_mounts_nothing_leaves_nothing() {
    let mut p = boot();
    let (x, y) = centre(&p, "row");
    // The touch ends before a slice ran.
    p.pointer_down(x, y, 0.).unwrap();
    let cold = work(&p);
    p.foresee_press(x, y, 0.);
    assert_eq!(p.foreseen_at(), Some(0.));
    p.pointer_cancel(10.).unwrap();
    p.forget_press();
    assert!(!p.foresee_slice(Duration::from_secs(1)));
    assert_eq!(work(&p).0, cold.0);

    // Something commits between the touch and the slice.
    p.foresee_press(x, y, 20.);
    assert!(
        p.foresee_slice(Duration::from_secs(1)),
        "the runner was asked"
    );
    let count = p.host().kernel().find_by_test_id("bump")[0];
    let id = p.host().kernel().node_by_key(count).unwrap().id;
    p.tap(id).unwrap();
    let after = work(&p);
    assert!(!p.foresee_slice(Duration::from_secs(1)));
    assert_eq!(work(&p), after);

    // A press that opens nothing: the runner's slice is the last.
    let (bx, by) = centre(&p, "bump");
    p.foresee_press(bx, by, 40.);
    assert!(!p.foresee_slice(Duration::from_secs(1)));
    assert_eq!(work(&p), after);
}
