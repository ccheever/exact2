//! The painter, pixel by pixel: backgrounds land in their boxes with their
//! radii, text leaves ink, an image paints, motion presents as a transform
//! and a group opacity, a scroll container clips, a screenshot is the
//! viewport.

use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;
use std::time::Duration;
use tiny_skia::Pixmap;

fn assets() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain"))
}

/// Whether a GPU is there to test on (the fleet has none; say so, never
/// fail on it — the DRM run on a machine with one is the GPU's check).
fn gpu_available() -> bool {
    match exact_linux::gpu::Gpu::new() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("no GPU here ({e}); the GPU painter's pixels are not checked");
            false
        }
    }
}

/// Every painter that can run here.
fn painters() -> Vec<PainterChoice> {
    let mut v = vec![PainterChoice::Cpu];
    if gpu_available() {
        v.push(PainterChoice::Gpu);
    }
    v
}

/// The pinned font (LLP 1015 §3): the fixture directory and the family
/// name the driver sets, so every number in these tests is the same on a
/// Mac and on a builder. Set once, before the first engine is made; the
/// environment is process-wide and every test wants the same values.
fn pin_font() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var(
            "EXACT_FONTS",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/fixtures/fonts/assets"
            ),
        );
        std::env::set_var("EXACT_FONT", "DejaVu Sans");
    });
}

fn boot(choice: PainterChoice) -> Presenter<caltrain_data::Caltrain> {
    pin_font();
    let plan = caltrain::build().unwrap();
    let (mut p, _) = Presenter::boot_with(
        &plan.encode(),
        caltrain_data::Caltrain,
        (390.0, 844.0),
        1.0,
        assets(),
        choice,
    )
    .unwrap();
    p.wait_images(Duration::from_secs(2));
    p
}

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn fixture(name: &str, scale: f32, choice: PainterChoice) -> Presenter<NoData> {
    let src = std::fs::read_to_string(format!(
        "{}/../../contract/corpus/{name}.contract",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    compiled(&src, scale, choice)
}

/// A card with an opaque colour and a radius on a white page: the fixture
/// for radii (the app's own panels are translucent white over the sky since
/// LLP 1014 §1a, and say nothing about a corner).
const CARD: &str = "component Card
  view
    column background-color=\"#ffffff\" padding=20 width=\"100%\" height=\"100%\"
      view width=300 height=120 border-radius=16 background-color=\"#f7f7f7\" testId=\"card\"
";

fn compiled(src: &str, scale: f32, choice: PainterChoice) -> Presenter<NoData> {
    pin_font();
    let plan = contract::compile(src).unwrap();
    Presenter::boot_with(
        &plan.encode(),
        NoData,
        (390.0, 844.0),
        scale,
        assets(),
        choice,
    )
    .unwrap()
    .0
}

fn view<D: DataSource>(p: &Presenter<D>, test_id: &str) -> u32 {
    let k = p.host().kernel();
    k.node_by_key(k.find_by_test_id(test_id)[0]).unwrap().id
}

fn rect<D: DataSource>(p: &mut Presenter<D>, test_id: &str) -> (f32, f32, f32, f32) {
    let id = view(p, test_id);
    p.boxes().iter().find(|b| b.id == id).unwrap().rect
}

/// A pixel's straight (r, g, b).
fn px(frame: &Pixmap, x: f32, y: f32) -> (u8, u8, u8) {
    let c = frame.pixel(x as u32, y as u32).unwrap().demultiply();
    (c.red(), c.green(), c.blue())
}

/// The page's colour: the root's background as the kernel holds it (the
/// app paints the sky's colour under everything since LLP 1014 §1a; a
/// fixture with no background is white — never assumed here).
fn page<D: DataSource>(p: &Presenter<D>) -> (u8, u8, u8) {
    let id = view(p, "caltrain-main");
    // The row holds a colour as authored, which may be a `light-dark()` pair
    // (LLP 1034 D1). This fixture reads the light half, which is the
    // appearance a headless painter draws in unless the app says otherwise.
    let c = p.host().kernel().node(id).unwrap().style.background_color;
    let [r, g, b, a] = exact_linux::paint::rgba(c.resolve(false));
    assert_eq!(a, 255, "the page is opaque");
    (r, g, b)
}

/// The darkest luminance in a box.
fn darkest(frame: &Pixmap, r: (f32, f32, f32, f32)) -> u32 {
    let mut min = 255 * 3;
    for y in (r.1.max(0.0) as u32)..((r.1 + r.3) as u32).min(frame.height()) {
        for x in (r.0.max(0.0) as u32)..((r.0 + r.2) as u32).min(frame.width()) {
            let (rr, g, b) = px(frame, x as f32, y as f32);
            min = min.min(rr as u32 + g as u32 + b as u32);
        }
    }
    min
}

#[test]
fn backgrounds_land_in_their_boxes_with_their_radii() {
    for choice in painters() {
        let mut p = boot(choice);
        let frame = p.frame();
        assert_eq!(
            px(&frame, 2.0, 2.0),
            page(&p),
            "the page is the root's colour"
        );
        let button = rect(&mut p, "change-station");
        let (cx, cy) = (button.0 + button.2 / 2.0, button.1 + button.3 / 2.0);
        // Inside the button but off its text: the top-left corner's inset.
        assert_eq!(
            px(&frame, button.0 + 10.0, button.1 + 3.0),
            (238, 238, 238),
            "#eeeeee at {cx},{cy} ({choice:?})"
        );
        // Radius 8: the corner pixel is whatever lies outside the button —
        // the panel it sits on — never the button's own colour.
        let corner = px(&frame, button.0 + 0.5, button.1 + 0.5);
        assert_eq!(
            corner,
            px(&frame, button.0 - 1.5, button.1 - 1.5),
            "radius 8: the corner pixel is outside ({choice:?})"
        );
        assert_ne!(corner, (238, 238, 238), "the corner is not the button");
        let mut p = compiled(CARD, 1.0, choice);
        let frame = p.frame();
        let card = rect(&mut p, "card");
        assert_eq!(
            px(&frame, card.0 + 20.0, card.1 + 3.0),
            (247, 247, 247),
            "#f7f7f7 in the card ({choice:?})"
        );
        assert_eq!(
            px(&frame, card.0 + 1.0, card.1 + 1.0),
            (255, 255, 255),
            "radius 16: the corner is the page"
        );
        assert_eq!(
            px(&frame, card.0 + 16.0, card.1 + 16.0),
            (247, 247, 247),
            "past the radius it is the card"
        );
    }
}

#[test]
fn text_and_images_leave_ink_in_their_boxes() {
    for choice in painters() {
        let mut p = boot(choice);
        let frame = p.frame();
        let name = rect(&mut p, "station-name");
        assert!(
            darkest(&frame, name) < 120,
            "24 pt bold text is dark ink ({choice:?})"
        );
        let logo = rect(&mut p, "logo");
        assert!(
            darkest(&frame, logo) < 700,
            "the picture painted: darkest {}",
            darkest(&frame, logo)
        );
        let outside = (name.0, name.1 + name.3 + 2.0, name.2, 1.0);
        assert!(
            darkest(&frame, outside) > 600,
            "a line between boxes is light"
        );
    }
}

#[test]
fn motion_presents_as_a_transform_and_a_group_opacity() {
    for choice in painters() {
        let mut p = fixture("spring", 1.0, choice);
        let before = rect(&mut p, "hello");
        let ink_before = darkest(&p.frame(), before);
        let _ = p.tap(view(&p, "toggle")).unwrap();
        let _ = p.clock(20_000.0);
        let after = rect(&mut p, "hello");
        assert!(
            after.2 > before.2 * 1.4,
            "scaled 1.5×: {before:?} → {after:?}"
        );
        // The lower half: scaled about its center, the box now reaches up into
        // the button above it, whose text is at full opacity.
        let lower = (after.0, after.1 + after.3 / 2.0, after.2, after.3 / 2.0);
        let ink_after = darkest(&p.frame(), lower);
        assert!(
            ink_after > ink_before + 60,
            "opacity 0.5 lightens the ink: {ink_before} → {ink_after} ({choice:?})"
        );
    }
}

#[test]
fn a_scroll_container_clips_what_it_scrolled_out() {
    for choice in painters() {
        let mut p = fixture("scroll", 1.0, choice);
        let rows = rect(&mut p, "rows");
        let above = rect(&mut p, "above");
        let _ = p.wheel(view(&p, "row-1"), 0.0, 100.0);
        let frame = p.frame();
        assert!(
            darkest(&frame, above) < 200,
            "text above the container still shows"
        );
        let row0 = rect(&mut p, "row-0");
        assert!(row0.1 < rows.1, "row 0 is above the container's top");
        // Between the container's top and the first visible row there is only
        // the container's background: nothing of row 0 leaks out above.
        let strip = (rows.0 + 1.0, rows.1 - 1.0, rows.2 - 2.0, 1.0);
        assert_eq!(
            darkest(&frame, strip),
            255 * 3,
            "the page above the container is white ({choice:?})"
        );
    }
}

#[test]
fn a_device_scale_paints_more_pixels_for_the_same_points() {
    for choice in painters() {
        let mut p = fixture("scroll", 2.0, choice);
        let frame = p.frame();
        assert_eq!((frame.width(), frame.height()), (780, 1688));
        let l = p.layout_json();
        assert!(
            l.contains("\"viewport\":{\"w\":390,\"h\":844}"),
            "points, not pixels"
        );
        let above = rect(&mut p, "above");
        let scaled = (above.0 * 2.0, above.1 * 2.0, above.2 * 2.0, above.3 * 2.0);
        assert!(darkest(&frame, scaled) < 200, "ink where the box is, at 2×");
    }
}

#[test]
fn a_screenshot_is_the_viewport_as_a_png() {
    for choice in painters() {
        let mut p = boot(choice);
        let path = std::env::temp_dir().join(format!("exact-paint-{}.png", std::process::id()));
        let reply = p.screenshot(path.to_str().unwrap()).unwrap();
        assert!(reply.ends_with(",\"w\":390,\"h\":844}"), "{reply}");
        let png = Pixmap::load_png(&path).unwrap();
        assert_eq!((png.width(), png.height()), (390, 844));
        assert_eq!(
            px(&png, 2.0, 2.0),
            page(&p),
            "the page is the root's colour"
        );
        let _ = std::fs::remove_file(path);
    }
}

/// The two painters over one frame: the mean absolute difference per
/// channel and the share of pixels where any channel differs by more than
/// 32, in 0–255 terms.
fn band(a: &Pixmap, b: &Pixmap) -> (f64, f64) {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let (mut sum, mut over) = (0u64, 0u64);
    for (pa, pb) in a.pixels().iter().zip(b.pixels()) {
        let (ca, cb) = (pa.demultiply(), pb.demultiply());
        let d = [
            (ca.red() as i32 - cb.red() as i32).unsigned_abs(),
            (ca.green() as i32 - cb.green() as i32).unsigned_abs(),
            (ca.blue() as i32 - cb.blue() as i32).unsigned_abs(),
        ];
        sum += d.iter().map(|x| *x as u64).sum::<u64>();
        if d.iter().any(|x| *x > 32) {
            over += 1;
        }
    }
    let n = (a.width() * a.height()) as f64;
    (sum as f64 / (3.0 * n), 100.0 * over as f64 / n)
}

#[test]
fn the_two_painters_agree_within_a_band() {
    // LLP 1015 §2: tiny-skia is the pixel oracle and vello must land within
    // a band of it over the same frame — the whole app at 390×844, text and
    // all. Glyphs are where they part (outlines hinted by vello against
    // swash's bitmaps at snapped positions), so the band is on the frame,
    // not a pixel. Measured on Metal 2026-08-29: mean 3.24/255, 2.98% of
    // pixels differing by more than 32 — the band is that with room.
    if !gpu_available() {
        return;
    }
    let cpu = boot(PainterChoice::Cpu).frame();
    let gpu = boot(PainterChoice::Gpu).frame();
    let (mean, over) = band(&cpu, &gpu);
    eprintln!("cpu vs gpu at 390x844: mean {mean:.2}/255, {over:.2}% of pixels differ by > 32");
    assert!(mean < 5.0, "mean {mean:.2}/255");
    assert!(over < 6.0, "{over:.2}% of pixels differ by > 32");
}
