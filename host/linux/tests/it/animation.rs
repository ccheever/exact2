//! Keyframe animations on Linux (LLP 1055 D5/D7): the host's engine samples the
//! `animation` row under its clock, as it does a transition, and the painter
//! draws the presented values.
use exact_kernel::MonospaceMeasurer;
use exact_linux::Host;
use exact_runner::Event;

fn view(h: &Host<()>, name: &str) -> u32 {
    let k = h.kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

fn close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1e-5, "{actual} vs {expected}");
}

#[test]
fn an_animation_plays_under_the_clock_and_stops_with_its_row() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/keyframes.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap().encode();
    let mut h = Host::boot(
        &plan,
        (),
        Box::new(MonospaceMeasurer::default()),
        400.,
        600.,
    )
    .unwrap()
    .0;
    let (mark, added) = (view(&h, "mark"), view(&h, "added"));
    assert!(h.motion(), "an endless animation keeps frames coming");
    close(h.presented(mark).opacity, 0.4);
    close(h.presented(mark).scale, 0.9);
    close(h.presented(added).translate.1, 8.0);
    h.tick(400.);
    close(h.presented(mark).opacity, 0.7);
    close(h.presented(added).opacity, 1.0);
    assert_eq!(h.agent("{\"op\":\"settle\"}"), "{\"settle\":null}");
    h.dispatch_at(view(&h, "toggle"), Event::Press, 500.);
    close(h.presented(mark).opacity, 1.0);
    assert!(!h.motion());
}

fn presenter(animation: &str) -> exact_linux::Presenter<()> {
    let source = format!(
        r##"keyframes blink
  from opacity=1
  to opacity=0
component App
  view
    box testId="caret" width=1 height=20 background-color="#007aff" animation="{animation}"
"##
    );
    let (mut p, error) = exact_linux::Presenter::boot_with(
        &contract::compile(&source).unwrap().encode(),
        (),
        (80., 40.),
        1.,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        exact_linux::presenter::PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p.frame();
    p
}

#[test]
fn a_step_animation_paints_each_step_and_no_frame_between() {
    let mut p = presenter("blink 1s steps(2, jump-none) infinite");
    assert!(!p.needs_animation_frame());
    assert_eq!(p.wake_due_ms(), Some(500.));
    let mut frames = Vec::new();
    for period in 1..=120 {
        let now = period as f64 * 1000. / 120.;
        assert!(p.advance(now).is_none());
        // An unrelated frame task may sample the same held value too.
        assert!(p.animation_frame(now).is_none());
        p.tick(now);
        if p.dirty() {
            frames.push(now);
            p.frame();
        }
        assert!(!p.needs_animation_frame(), "display period {period}");
    }
    assert_eq!(frames, [500., 1000.]);
    assert_eq!(p.wake_due_ms(), Some(1500.));
}

#[test]
fn paused_delayed_and_finished_animations_do_not_hold_the_display_pump() {
    let mut paused = presenter("blink 1s steps(2) infinite paused");
    assert_eq!(paused.host().motion_due_ms(), None);
    paused.tick(1000.);
    assert!(!paused.dirty());

    let mut delayed = presenter("blink 1s steps(2, jump-none) 2s forwards");
    assert_eq!(delayed.host().motion_due_ms(), Some(2000.));
    delayed.tick(1999.);
    assert!(!delayed.dirty());
    delayed.tick(2000.);
    assert!(!delayed.dirty());
    assert_eq!(delayed.host().motion_due_ms(), Some(2500.));
    delayed.tick(2500.);
    assert!(delayed.dirty());
    delayed.frame();
    delayed.tick(3000.);
    assert!(!delayed.dirty());
    assert_eq!(delayed.host().motion_due_ms(), None);
}

#[test]
fn continuous_animation_still_paints_each_display_period() {
    let mut p = presenter("blink 1s linear infinite");
    for period in 1..=120 {
        assert!(p.needs_animation_frame());
        p.tick(period as f64 * 1000. / 120.);
        assert!(p.dirty());
        p.frame();
    }
}
