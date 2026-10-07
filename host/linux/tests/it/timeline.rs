//! Drag timelines on Linux (LLP 1057.003 D2, D4): the painter presents what
//! the engine's frame gives, so a backdrop bound to a photo's
//! `-exact-drag-timeline` follows the presenter's contact and the release spring,
//! at f(photo) in every frame, with nothing Linux-specific; and in a list of
//! cards that each drive `--swipe`, each row's label follows its own card.
use exact_linux::{presenter::PainterChoice, Presenter};
use exact_runner::{DataError, DataSource};
use std::path::PathBuf;

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

const APP: &str = r##"keyframes fade
  from opacity=1
  to opacity=0
component App
  state y = 0
  action geometry(w: number, h: number, pw: number, ph: number)
    y = y
  action release(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    y = (py > 120 ? 900 : 0)
  view
    box width="100%" height="100%"
      box testId="clip" timeline-scope="--dismiss" position="absolute" left=0 top=0 width=300 height=400 overflow="hidden" box-sizing="border-box" padding=0 border-width=0
        box testId="backdrop" position="absolute" left=0 top=0 width="100%" height="100%" background-color="#172521" animation="fade 1s linear both" animation-timeline="--dismiss" animation-range="0px 300px"
        box id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" margin=0 padding=0 border-width=0 translate=`0px ${y}px` transition="translate -exact-spring(300, 30, 1)" -exact-drag-timeline="--dismiss y"
          box testId="handle" position="absolute" left=0 top=0 width="100%" height="100%" transformDragFor="photo" transformgeometry=geometry transformrelease=release touch-action="none"
"##;

fn id(p: &Presenter<Empty>, name: &str) -> u32 {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(name)[0]).unwrap().id
}

/// The photo's presented `y` and the backdrop's presented opacity.
fn shown(p: &Presenter<Empty>) -> (f32, f32) {
    let photo = p.host().presented(id(p, "photo"));
    let backdrop = p.host().presented(id(p, "backdrop"));
    (photo.translate.1, backdrop.opacity)
}

fn follows(p: &Presenter<Empty>) -> (f32, f32) {
    let (y, o) = shown(p);
    let f = (1. - y / 300.).clamp(0., 1.);
    assert!(
        (o - f).abs() < 1e-5,
        "the backdrop shows {o} at y {y}, not {f}"
    );
    (y, o)
}

#[test]
fn a_backdrop_follows_the_contact_and_the_release_spring_in_every_frame() {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(follows(&p), (0., 1.));
    // The contact, recognized past the slop with no displacement; then each
    // move presents both in the same pass.
    assert!(p.pointer_down(150., 100., 0.).unwrap());
    assert!(p.pointer_move(150., 108., 8.).unwrap());
    assert_eq!(follows(&p), (0., 1.));
    for step in 1..=10 {
        let at = step as f32 * 18.;
        assert!(p.pointer_move(150., 108. + at, step as f64 * 16.).unwrap());
        let (y, _) = follows(&p);
        assert!((y - at).abs() < 1e-3, "the photo is at {y}, not {at}");
    }
    // Released at 180 > 120: the app dismisses; one spring moves both.
    assert!(p.pointer_up(150., 288., 176.).unwrap());
    let mut last = (0., 0.);
    for frame in 1..=60 {
        p.tick(176. + frame as f64 * 16.);
        last = follows(&p);
        if frame == 1 {
            // The spring's first frame, part of the way: the backdrop too.
            assert!(last.0 > 180. && last.1 > 0. && last.1 < 0.4, "{last:?}");
        }
    }
    assert!((last.0 - 900.).abs() < 5., "the photo left, to {}", last.0);
    assert_eq!(shown(&p).1, 0.);
}

/// The gallery's swipe list, four rows of one shape: each row scopes
/// `--swipe`, its card drives it as a swipe holds it, and its label follows
/// over the swipe's 64-point knee.
const LIST: &str = r##"keyframes reveal
  from opacity=0 scale=0.8
  to opacity=1 scale=1
component App
  view
    column gap=8 width="100%" height="100%"
      Row(n=1)
      Row(n=2)
      Row(n=3)
      Row(n=4)
component Row
  props
    n: number
  state kept = false
  action keep
    kept = not kept
  view
    box testId=`row-${n}` timeline-scope="--swipe" width=300 height=56 flex-shrink=0 position="relative" overflow="hidden" box-sizing="border-box" padding=0 border-width=0
      box testId=`label-${n}` position="absolute" left=0 top=0 width=64 height="100%" animation="reveal 1s linear both" animation-timeline="--swipe" animation-range="0px 64px"
      box testId=`card-${n}` swiperight=keep touch-action="pan-y" width="100%" height="100%" position="relative" box-sizing="border-box" margin=0 padding=0 border-width=0 transition="translate -exact-spring(300, 30, 1)" -exact-drag-timeline="--swipe x"
        text (kept ? "kept" : "") testId=`kept-${n}`
"##;

/// Each row's card `x` and its label's opacity and scale.
fn rows(p: &Presenter<Empty>) -> Vec<(f32, f32, f32)> {
    (1..=4)
        .map(|n| {
            let card = p.host().presented(id(p, &format!("card-{n}")));
            let label = p.host().presented(id(p, &format!("label-{n}")));
            (card.translate.0, label.opacity, label.scale)
        })
        .collect()
}

/// Row 2's label at f(its card), and every other row at rest.
fn only_row_two(p: &Presenter<Empty>) -> f32 {
    let shown = rows(p);
    let (x, opacity, scale) = shown[1];
    let f = (x / 64.).clamp(0., 1.);
    assert!(
        (opacity - f).abs() < 1e-5 && (scale - (0.8 + 0.2 * f)).abs() < 1e-5,
        "row 2's label shows {opacity}, {scale} at x {x}, not f = {f}"
    );
    for n in [0, 2, 3] {
        assert_eq!(shown[n], (0., 0., 0.8), "row {} moved: {shown:?}", n + 1);
    }
    x
}

#[test]
fn in_a_list_of_cards_each_label_follows_its_own_card() {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(LIST).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    // Every row resolves `--swipe` to its own card.
    let k = p.host().kernel();
    for n in 1..=4 {
        let label = k.find_by_test_id(&format!("label-{n}"))[0];
        let card = k.find_by_test_id(&format!("card-{n}"))[0];
        assert_eq!(
            k.timeline_of(label),
            Some(exact_motion::NamedTimeline::Source(
                exact_kernel::motion_node(card)
            ))
        );
    }
    assert_eq!(only_row_two(&p), 0.);
    // Row 2 is 56 + 8 points down: a swipe right across it, in steps, past
    // the knee, where the card resists.
    let y = 64. + 28.;
    assert!(p.pointer_down(100., y, 0.).unwrap());
    assert!(p.pointer_move(108., y, 8.).unwrap());
    let mut x = 0.;
    for step in 1..=8 {
        assert!(p
            .pointer_move(108. + step as f32 * 12., y, 8. + step as f64 * 16.)
            .unwrap());
        let now = only_row_two(&p);
        assert!(now > x, "the card moves right: {now} after {x}");
        x = now;
    }
    assert!(x > 64. && x < 96., "resisted past the knee: {x}");
    // Released past the knee: the row keeps its study, and the card springs
    // back with its label on it; nothing else moves in any frame.
    assert!(p.pointer_up(204., y, 144.).unwrap());
    let mut moved = 0;
    for frame in 1..=60 {
        p.tick(144. + frame as f64 * 16.);
        let now = only_row_two(&p);
        moved += usize::from(now != x);
        x = now;
    }
    assert!(
        moved > 10 && x.abs() < 0.5,
        "sprang back to {x} over {moved} frames"
    );
    let k = p.host().kernel();
    let kept = |n: u32| {
        let node = k.node_by_key(k.find_by_test_id(&format!("kept-{n}"))[0]);
        node.unwrap()
            .props
            .str(exact_kernel::PropId::Text)
            .map(str::to_string)
    };
    assert_eq!(kept(2).as_deref(), Some("kept"));
    assert_eq!(kept(1).as_deref(), Some(""));
}

/// Three rows of the phase 3 Chrome probe's cases: a label no scope reaches
/// (its card is a sibling), a scope with no card, and a scope whose second
/// card's name comes and goes.
const UNRESOLVED: &str = r##"keyframes fade
  from opacity=0
  to opacity=1
component App
  state two = true
  action flip
    two = not two
  action noop
    two = two
  view
    column width=300 height="100%"
      box testId="flip" press=flip width=300 height=40 flex-shrink=0
      box testId="m-row" width=300 height=40 position="relative" flex-shrink=0
        box testId="m-label" position="absolute" left=0 top=0 width=64 height=40 opacity=0.55 animation="fade 1s linear both" animation-timeline="--t" animation-range="0px 64px"
        box testId="m-card" swiperight=noop touch-action="pan-y" width="100%" height="100%" position="relative" transition="translate -exact-spring(300, 30, 1)" -exact-drag-timeline="--t x"
      box testId="z-row" timeline-scope="--t" width=300 height=40 position="relative" flex-shrink=0
        box testId="z-label" position="absolute" left=0 top=0 width=64 height=40 opacity=0.55 animation="fade 1s linear both" animation-timeline="--t" animation-range="0px 64px"
      box testId="t-row" timeline-scope="--t" width=300 height=80 position="relative" flex-shrink=0
        box testId="t-label" position="absolute" left=0 top=0 width=64 height=40 opacity=0.55 animation="fade 1s linear both" animation-timeline="--t" animation-range="0px 64px"
        box testId="t-card-a" swiperight=noop touch-action="pan-y" width="100%" height=40 position="relative" transition="translate -exact-spring(300, 30, 1)" -exact-drag-timeline="--t x"
        box testId="t-card-b" width="100%" height=40 position="relative" -exact-drag-timeline=(two ? "--t x" : "none")
"##;

/// What Chrome 154 shows (the phase 3 probe): a name no element in scope
/// declares leaves its animation at time 0, the fade's start; a scope with
/// no declaring descendant, or two, is an inactive timeline, whose
/// animation has no effect, so the label's own 0.55 shows. A swipe moves
/// neither; when the scope finds one card, its label follows that card.
#[test]
fn an_unresolved_name_holds_time_zero_and_an_inactive_one_has_no_effect() {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(UNRESOLVED).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    let shown = |p: &Presenter<Empty>| {
        ["m-label", "z-label", "t-label", "t-card-a"].map(|t| {
            let v = p.host().presented(id(p, t));
            if t.ends_with("card-a") {
                v.translate.0
            } else {
                v.opacity
            }
        })
    };
    assert_eq!(shown(&p), [0., 0.55, 0.55, 0.]);
    let swipe = |p: &mut Presenter<Empty>, y: f32, at: f64| {
        assert!(p.pointer_down(100., y, at).unwrap());
        assert!(
            p.pointer_move(108., y, at + 8.).unwrap(),
            "recognized at {y}, {at}"
        );
        assert!(p.pointer_move(148., y, at + 24.).unwrap());
    };
    let settle = |p: &mut Presenter<Empty>, y: f32, at: f64| {
        assert!(p.pointer_up(148., y, at).unwrap());
        for frame in 1..=90 {
            p.tick(at + frame as f64 * 16.);
        }
    };
    // Row m's card is a sibling with no scope: its label stays at time 0.
    swipe(&mut p, 60., 0.);
    assert_eq!(shown(&p)[0], 0.);
    settle(&mut p, 60., 40.);
    // Two cards under row t's scope: inactive, whatever the card does.
    swipe(&mut p, 140., 2000.);
    assert_eq!(shown(&p), [0., 0.55, 0.55, 40.]);
    settle(&mut p, 140., 2040.);
    // One card: the label follows it, from rest.
    let flip = id(&p, "flip");
    p.tap(flip).unwrap();
    p.tick(4000.);
    assert_eq!(shown(&p), [0., 0.55, 0., 0.]);
    swipe(&mut p, 140., 4000.);
    let [_, _, label, card] = shown(&p);
    assert!((card - 40.).abs() < 1e-3 && (label - card / 64.).abs() < 1e-5);
    // Two again: no effect, at once.
    p.tap(flip).unwrap();
    p.tick(4050.);
    assert_eq!(shown(&p)[2], 0.55);
}

/// The gallery's card deck (LLP 1057.003 C6): the top card drives `--fling`
/// on x; its face, a child, tilts x / 20 degrees about its bottom edge, and
/// the card beneath, the top card's sibling through the clip's scope, rises
/// from 0.95 to full size as it leaves either way. A fast release throws it
/// on the release spring, which starts at the fling's velocity.
const DECK: &str = r##"keyframes tilt
  from rotate=-15
  to rotate=15
keyframes rise
  from scale=1
  50% scale=0.95
  to scale=1
component App
  state x = 0
  action geometry(w: number, h: number, pw: number, ph: number)
    x = x
  action release(px: number, py: number, s: number, vx: number, vy: number, vs: number)
    x = ((px > 120 or vx > 800) ? 600 : 0)
  view
    box width="100%" height="100%"
      box testId="clip" timeline-scope="--fling" position="absolute" left=0 top=0 width=300 height=400 overflow="hidden" box-sizing="border-box" padding=0 border-width=0
        box testId="next" position="absolute" left=40 top=20 width=220 height=280 animation="rise 1s linear both" animation-timeline="--fling" animation-range="-300px 300px"
        box id="top" testId="top" width="100%" height="100%" box-sizing="border-box" margin=0 padding=0 border-width=0 translate=`${x}px 0px` transition="translate -exact-spring(300, 30, 1)" -exact-drag-timeline="--fling x"
          box testId="face" position="absolute" left=40 top=20 width=220 height=280 transform-origin="50% 100%" animation="tilt 1s linear both" animation-timeline="--fling" animation-range="-300px 300px"
          box testId="handle" position="absolute" left=0 top=0 width="100%" height="100%" transformDragFor="top" transformgeometry=geometry transformrelease=release touch-action="none"
"##;

/// The top card's presented `x`, after checking its face's tilt and the next
/// card's rise against it.
fn dealt(p: &Presenter<Empty>) -> f32 {
    let x = p.host().presented(id(p, "top")).translate.0;
    let tilt = p.host().presented(id(p, "face")).rotate;
    let rise = p.host().presented(id(p, "next")).scale;
    let (want_tilt, want_rise) = (
        (x / 20.).clamp(-15., 15.),
        1. - 0.05 * (1. - x.abs().min(300.) / 300.),
    );
    assert!(
        (tilt - want_tilt).abs() < 1e-4 && (rise - want_rise).abs() < 1e-5,
        "at x {x}: the face tilts {tilt}, not {want_tilt}; the next card is at {rise}, not {want_rise}"
    );
    x
}

#[test]
fn a_thrown_card_tilts_its_face_and_raises_the_next_in_every_frame() {
    let (mut p, error) = Presenter::boot_with(
        &contract::compile(DECK).unwrap().encode(),
        Empty,
        (400., 500.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(dealt(&p), 0.);
    // Recognized past the slop, then 20 points every 16 ms: 1,250 pt/s.
    assert!(p.pointer_down(150., 200., 0.).unwrap());
    assert!(p.pointer_move(158., 200., 8.).unwrap());
    for step in 1..=8 {
        assert!(p
            .pointer_move(158. + step as f32 * 20., 200., 8. + step as f64 * 16.)
            .unwrap());
        let x = dealt(&p);
        assert!((x - step as f32 * 20.).abs() < 1e-3, "the card is at {x}");
    }
    // Lifted at 160 > 120, still moving: thrown, from 160 at the finger's speed.
    let up = 8. + 8. * 16.;
    assert!(p.pointer_up(318., 200., up).unwrap());
    p.tick(up + 1.);
    let v = (dealt(&p) - 160.) / 0.001;
    assert!(
        (v - 1250.).abs() < 125.,
        "the spring leaves at {v} pt/s, the finger at 1,250"
    );
    let mut x = 0.;
    for frame in 1..=60 {
        p.tick(up + frame as f64 * 16.);
        x = dealt(&p);
    }
    assert!((x - 600.).abs() < 5., "thrown to {x}");
    let face = p.host().presented(id(&p, "face"));
    assert_eq!(
        (face.rotate, p.host().presented(id(&p, "next")).scale),
        (15., 1.)
    );
}
