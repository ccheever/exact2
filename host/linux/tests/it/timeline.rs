//! Drag timelines on Linux (LLP 1057.003 D2): the painter presents what the
//! engine's frame gives, so a backdrop bound to a photo's `drag-timeline`
//! follows the presenter's contact and the release spring, at f(photo) in
//! every frame, with nothing Linux-specific.
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
  action geometry(w: number, h: number, pw: number, ph: number) writes y
    y = y
  action release(px: number, py: number, s: number, vx: number, vy: number, vs: number) writes y
    y = (py > 120 ? 900 : 0)
  view
    box width="100%" height="100%"
      box testId="clip" position="absolute" left=0 top=0 width=300 height=400 overflow="hidden" box-sizing="border-box" padding=0 border-width=0
        box testId="backdrop" position="absolute" left=0 top=0 width="100%" height="100%" background-color="#172521" animation="fade 1s linear both" animation-timeline="--dismiss" animation-range="0px 300px"
        box id="photo" testId="photo" width="100%" height="100%" box-sizing="border-box" margin=0 padding=0 border-width=0 translate=`0px ${y}px` transition="translate spring(300, 30, 1)" drag-timeline="--dismiss y"
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
