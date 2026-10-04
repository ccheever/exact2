//! A complete rapid box-selection gesture must keep its actual start point,
//! even if every event lands in one tick or waits across a checkpoint.
use exact_game::*;

#[derive(Default, Resource)]
struct Selection {
    start: Option<Vec2>,
    end: Option<Vec2>,
    pointer: Option<PointerState>,
    presses: u32,
    releases: u32,
}
struct Select;
impl Game for Select {
    const ID: &'static str = "pointer-origin";
    type Args = ();
    fn setup(w: &mut World, _: &()) {
        w.insert_resource(Selection::default());
    }
    fn actions() -> Actions {
        Actions::new().button("select", &["MouseLeft"])
    }
    fn tick(w: &mut World, input: &Input, _: &()) {
        let mut selection = w.resource_mut::<Selection>();
        selection.pointer = input.pointer();
        if input.pressed("select") {
            selection.presses += 1;
            selection.start = input.pointer().and_then(|p| p.press_origin);
        }
        if input.released("select") {
            selection.releases += 1;
            selection.end = input.pointer().map(|p| p.position);
        }
    }
}
fn scene(epoch: f64) -> Sim<Select> {
    let mut s = Sim::new(()).unwrap();
    s.advance(epoch, Clock::Seekable);
    s
}
fn pointer(s: &mut Sim<Select>, phase: PointerPhase, x: f32, y: f32, buttons: u32, at_ms: f64) {
    s.input(InputEvent::Pointer {
        id: 1,
        phase,
        x,
        y,
        dx: 7.,
        dy: 9.,
        buttons,
        at_ms,
    });
}
fn queue_gesture(s: &mut Sim<Select>, at: f64) {
    pointer(s, PointerPhase::Move, 900., 600., 0, at);
    pointer(s, PointerPhase::Down, 10., 20., 1, at + 1.);
    pointer(s, PointerPhase::Move, 140., 90., 1, at + 2.);
    pointer(s, PointerPhase::Up, 150., 100., 0, at + 3.);
}
fn assert_selection(s: &Sim<Select>) {
    let selection = s.world().resource::<Selection>();
    assert_eq!(
        (selection.start, selection.end),
        (Some(Vec2::new(10., 20.)), Some(Vec2::new(150., 100.)))
    );
    assert_eq!((selection.presses, selection.releases), (1, 1));
    let p = selection.pointer.unwrap();
    assert_eq!(p.press_origin, selection.start);
    assert!(!p.down);
}
#[test]
fn a_whole_gesture_in_one_tick_uses_down_instead_of_final_or_hover_position() {
    let mut s = scene(0.);
    queue_gesture(&mut s, 1.);
    s.advance(17., Clock::Seekable);
    assert_selection(&s);
    let saved = s.save().unwrap();
    let mut restored = scene(2000.);
    restored.restore(&saved).unwrap();
    assert_eq!(restored.save().unwrap(), saved);
    restored.advance(3000., Clock::Seekable);
    restored.run(100.);
    assert_selection(&restored); // origin persists, action edges are not replayed
}
#[test]
fn queued_complete_press_restores_at_a_new_epoch_without_losing_its_origin() {
    let mut original = scene(500.);
    queue_gesture(&mut original, 501.);
    let saved = original.save().unwrap();
    let mut restored = scene(9000.);
    restored.restore(&saved).unwrap();
    assert_eq!(restored.save().unwrap(), saved);
    original.run(17.);
    restored.run(17.);
    assert_selection(&original);
    assert_selection(&restored);
    assert_eq!(original.save().unwrap(), restored.save().unwrap());
}
#[test]
fn an_active_drag_origin_survives_restore_and_a_later_release() {
    let mut original = scene(0.);
    pointer(&mut original, PointerPhase::Down, 10., 20., 1, 1.);
    original.advance(17., Clock::Seekable);
    let saved = original.save().unwrap();
    let mut restored = scene(10000.);
    restored.restore(&saved).unwrap();
    assert_eq!(restored.save().unwrap(), saved);
    original.advance(20., Clock::Seekable);
    restored.advance(12000., Clock::Seekable);
    pointer(&mut original, PointerPhase::Up, 150., 100., 0, 21.);
    pointer(&mut restored, PointerPhase::Up, 150., 100., 0, 12001.);
    original.run(17.);
    restored.run(17.);
    assert_selection(&original);
    assert_selection(&restored);
}
