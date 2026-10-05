//! CSS `visibility` on the Linux host: a hidden box keeps its geometry and
//! paints, hits and focuses nothing of its own. A descendant that computes
//! `visible` still paints and is hit. One that inherits `hidden` does not.
use super::*;
use exact_runner::{DataError, Value};

#[derive(Default)]
struct Empty;
impl DataSource for Empty {
    fn query(&mut self, name: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(name.into()))
    }
}

const APP: &str = r##"component App
  view
    column width=200 height=200 background-color="#ffffff"
      box testId="parent" width=120 height=100 background-color="#ff0000" visibility="hidden" position="relative"
        box testId="shown" width=40 height=30 background-color="#00ff00" visibility="visible" tabindex=0 position="absolute" left=8 top=8
        box testId="kept" width=40 height=30 background-color="#0000ff" tabindex=0 position="absolute" left=72 top=8
"##;

fn boot() -> Presenter<Empty> {
    let (p, error) = Presenter::boot_with(
        &contract::compile(APP).unwrap().encode(),
        Empty,
        (200., 200.),
        1.,
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain")),
        PainterChoice::Cpu,
    )
    .unwrap();
    assert!(error.is_none(), "{error:?}");
    p
}

fn id(p: &Presenter<Empty>, test_id: &str) -> ViewId {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn rgba(p: &mut Presenter<Empty>, x: f32, y: f32) -> [u8; 4] {
    let c = p.frame().pixel(x as u32, y as u32).unwrap().demultiply();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

fn holds(rect: Rect4, point: (f32, f32)) -> bool {
    point.0 >= rect.0 && point.1 >= rect.1 && point.0 < rect.0 + rect.2 && point.1 < rect.1 + rect.3
}

#[test]
fn a_visible_descendant_of_a_hidden_box_paints_and_is_hit() {
    let mut p = boot();
    let (parent, shown, kept) = (id(&p, "parent"), id(&p, "shown"), id(&p, "kept"));
    let parent_rect = p.rect_of(parent).unwrap();
    let shown_rect = p.rect_of(shown).unwrap();
    let kept_rect = p.rect_of(kept).unwrap();
    assert!(
        parent_rect.2 > 0. && parent_rect.3 > 0.,
        "the hidden box keeps its geometry"
    );
    assert!(shown_rect.2 > 0. && kept_rect.2 > 0.);
    let mid = |r: Rect4| (r.0 + r.2 / 2., r.1 + r.3 / 2.);
    let (sx, sy) = mid(shown_rect);
    let (kx, ky) = mid(kept_rect);
    // Below both absolutely placed children, still inside the parent.
    let probe = (parent_rect.0 + 4., parent_rect.1 + parent_rect.3 - 4.);
    assert!(
        holds(parent_rect, probe) && !holds(shown_rect, probe) && !holds(kept_rect, probe),
        "the probe must be the parent's own area: {probe:?} shown {shown_rect:?} kept {kept_rect:?}"
    );
    assert_eq!(
        rgba(&mut p, sx, sy),
        [0, 255, 0, 255],
        "the visible child paints"
    );
    let kept_px = rgba(&mut p, kx, ky);
    assert_ne!(
        kept_px,
        [0, 0, 255, 255],
        "an inheriting child paints nothing: {kept_px:?}"
    );
    let own = rgba(&mut p, probe.0, probe.1);
    assert_ne!(
        own,
        [255, 0, 0, 255],
        "the hidden box paints nothing of its own: {own:?}"
    );
    assert_eq!(p.hit(sx, sy), Some(shown));
    let at_kept = p.hit(kx, ky);
    assert_ne!(at_kept, Some(kept));
    assert_ne!(at_kept, Some(parent));
    let at_own = p.hit(probe.0, probe.1);
    assert_ne!(at_own, Some(parent));
    assert!(p.focusable(shown), "a visible tabindex box takes the focus");
    assert!(!p.focusable(kept), "a hidden box is not focusable");
}
