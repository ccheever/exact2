//! A number that rolls on the CPU painter: frames while it plays, none after,
//! none under Reduce Motion, and the clock's settle waits for it.
use crate::presenter::{PainterChoice, Presenter};
use std::path::PathBuf;

const APP: &str = r#"component App
  state n = 7
  task bump mount
    after(1000, step)
  action step
    n = 42
  view
    column width=300 height=200
      text `${n}` font-size=48 -exact-content-transition="numeric" testId="n"
"#;

fn boot(reduced_motion: bool) -> Presenter<()> {
    let (mut p, error) = Presenter::boot_with(
        &crate::test_plan(APP),
        (),
        (300., 200.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let preferences = exact_runner::Preferences {
        reduced_motion,
        ..Default::default()
    };
    assert!(p.set_preferences(preferences).is_none());
    let _ = p.frame();
    p
}

fn pixels(p: &mut Presenter<()>) -> Vec<u8> {
    p.frame().data().to_vec()
}

#[test]
fn a_changed_number_rolls_then_asks_for_no_more_frames() {
    let mut p = boot(false);
    assert!(!p.needs_animation_frame());
    p.clock(1000.);
    let start = pixels(&mut p);
    assert!(p.needs_animation_frame(), "the roll began with its commit");
    p.clock(1100.);
    let rolling = pixels(&mut p);
    assert!(p.needs_animation_frame());
    // Two changed glyphs: the second starts a beat after the first.
    p.clock(1000. + 550.);
    let settled = pixels(&mut p);
    assert!(!p.needs_animation_frame(), "the roll ended");
    assert_ne!(start, rolling);
    assert_ne!(rolling, settled);
    p.clock(2000.);
    assert_eq!(
        pixels(&mut p),
        settled,
        "the last frame of the roll is the number at rest"
    );
}

#[test]
fn under_reduce_motion_the_number_changes_in_place() {
    let mut p = boot(true);
    p.clock(1000.);
    let changed = pixels(&mut p);
    assert!(!p.needs_animation_frame());
    p.clock(2000.);
    assert_eq!(pixels(&mut p), changed);
}

#[test]
fn the_clock_settles_after_the_roll() {
    let mut p = boot(false);
    let reply = crate::agent::handle(&mut p, r#"{"op":"clock","to":1000}"#);
    assert!(reply.contains("\"clock\":1000"), "{reply}");
    let reply = crate::agent::handle(&mut p, r#"{"op":"clock","settle":true}"#);
    assert!(reply.contains("\"clock\":1550"), "{reply}");
    assert!(!p.needs_animation_frame());
}
