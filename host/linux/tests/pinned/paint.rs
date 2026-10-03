//! The painter, pixel by pixel: backgrounds land in their boxes with their
//! radii, text leaves ink, an image paints, motion presents as a transform
//! and a group opacity, a scroll container clips, a screenshot is the
//! viewport.

use crate::pin_font;
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

#[derive(Default)]
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

/// LLP 1061 D6: `transform-origin` is the point the transforms turn about;
/// the painted box (what `layout` reports and hit-testing reads) follows.
#[test]
fn transforms_turn_about_the_transform_origin() {
    let src = "component Origin
  view
    column padding=20
      view width=100 height=40 scale=0.5 transform-origin=\"50%\" testId=\"centre\"
      view width=100 height=40 scale=0.5 transform-origin=\"left top\" testId=\"corner\"
      view width=100 height=40 rotate=90 transform-origin=\"0 100%\" testId=\"turned\"
";
    for choice in painters() {
        let mut p = compiled(src, 1.0, choice);
        let close = |a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)| {
            let near = |x: f32, y: f32| (x - y).abs() < 0.01;
            assert!(
                near(a.0, b.0) && near(a.1, b.1) && near(a.2, b.2) && near(a.3, b.3),
                "{a:?} vs {b:?} ({choice:?})"
            );
        };
        close(rect(&mut p, "centre"), (45.0, 30.0, 50.0, 20.0));
        close(rect(&mut p, "corner"), (20.0, 60.0, 50.0, 20.0));
        // A quarter turn clockwise about the bottom-left corner (20, 140):
        // the box now hangs below that corner.
        close(rect(&mut p, "turned"), (20.0, 140.0, 40.0, 100.0));
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
        let l = p.layout_json(None, false);
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

#[test]
fn border_style_controls_pixels_and_layout_with_current_color() {
    for style in ["none", "hidden", "solid"] {
        let source = format!(
            r##"component Borders
  view
    column color="#ff0000" background-color="#ffffff" width="100%" height="100%" padding=20
      box testId="border" width=100 height=60 padding=10 border-width=8 border-style="{style}" background-color="#cccccc"
        box width=20 height=10 background-color="#000000"
"##
        );
        let mut p = compiled(&source, 1.0, PainterChoice::Cpu);
        let (x, y, w, h) = rect(&mut p, "border");
        let frame = p.frame();
        let solid = style == "solid";
        assert_eq!((w, h), if solid { (136.0, 96.0) } else { (120.0, 80.0) });
        assert_eq!(
            px(&frame, x + 2.0, y + 2.0),
            if solid { (255, 0, 0) } else { (204, 204, 204) }
        );
        let inset = if solid { 18.0 } else { 10.0 };
        assert_eq!(px(&frame, x + inset + 2.0, y + inset + 2.0), (0, 0, 0));
    }
}

/// LLP 1064 D2: a `box-shadow` falls outside the border box only — offset
/// hard, or blurred as a Gaussian of half the blur radius — on every painter,
/// and a clipping node's shadow is not clipped by it.
#[test]
fn a_box_shadow_paints_outside_the_box_as_a_gaussian() {
    let source = r##"component Shadows
  view
    column background-color="#ffffff" padding=40 gap=50 width="100%" height="100%"
      view testId="hard" width=100 height=60 background-color="#eeeeee" box-shadow="10px 10px 0 #0000ff"
      view testId="soft" width=100 height=60 border-radius=16 background-color="#eeeeee" box-shadow="0 0 20px #000000"
      view testId="ghost" width=100 height=60 background-color="rgba(255, 255, 255, 0.5)" overflow="hidden" box-shadow="0 30px 0 #ff0000"
"##;
    for choice in painters() {
        let mut p = compiled(source, 1.0, choice);
        let (hard, soft, ghost) = (
            rect(&mut p, "hard"),
            rect(&mut p, "soft"),
            rect(&mut p, "ghost"),
        );
        let frame = p.frame();
        assert_eq!(
            px(&frame, hard.0 + 105.0, hard.1 + 65.0),
            (0, 0, 255),
            "{choice:?}"
        );
        assert_eq!(
            px(&frame, hard.0 + 50.0, hard.1 + 30.0),
            (238, 238, 238),
            "{choice:?}"
        );
        assert_eq!(
            px(&frame, hard.0 + 5.0, hard.1 + 65.0),
            (255, 255, 255),
            "{choice:?}"
        );
        // σ = 10: Φ(-1) of black ten points out, Φ(-3.5) far out.
        let (x, y) = (soft.0 - 10.0, soft.1 + soft.3 / 2.0);
        let ten = px(&frame, x, y).0 as f32;
        assert!(
            (ten - 255.0 * (1.0 - 0.1587)).abs() < 12.0,
            "{ten} ({choice:?})"
        );
        assert!(px(&frame, soft.0 - 35.0, y).0 >= 250, "{choice:?}");
        assert!(px(&frame, soft.0 - 1.0, y).0 < 150, "{choice:?}");
        // Never inside the box, even through a translucent background; a
        // clipping node's shadow still falls outside it.
        assert_eq!(
            px(&frame, ghost.0 + 50.0, ghost.1 + 45.0),
            (255, 255, 255),
            "{choice:?}"
        );
        assert_eq!(
            px(&frame, ghost.0 + 50.0, ghost.1 + 75.0),
            (255, 0, 0),
            "{choice:?}"
        );
    }
}

#[test]
fn an_unsupported_dialog_stays_unpainted_and_cannot_run_its_action() {
    let mut p = compiled(
        r##"component Dialog
  state count = 0
  action invoked
    count = count + 1
  view
    column width="100%" height="100%" background-color="#ffffff"
      button press=invoked commandfor="dialog" command="show-modal" testId="invoker" width=48 height=48
      dialog id="dialog" testId="dialog" closedby="any" width=100 height=100 background-color="#ff0000"
        button press=invoked commandfor="dialog" command="close" testId="action" width=50 height=50
      box testId="after" width=20 height=20 background-color="#00ff00"
"##,
        1.0,
        PainterChoice::Cpu,
    );
    let dialog = view(&p, "dialog");
    let action = view(&p, "action");
    assert!(p.boxes().iter().all(|b| b.id != dialog && b.id != action));
    assert_eq!(rect(&mut p, "after"), (0.0, 48.0, 20.0, 20.0));
    assert_eq!(px(&p.frame(), 10.0, 70.0), (255, 255, 255));
    let invoker = view(&p, "invoker");
    p.tap(invoker).unwrap();
    let state = p.host().agent(r#"{"op":"state"}"#);
    assert!(state.contains("\"count\":0"), "{state}");
    let logs = p.host().agent(r#"{"op":"logs"}"#);
    assert!(
        logs.contains("Linux dialog presentation is not implemented"),
        "{logs}"
    );
}

#[test]
fn clipped_rectangle_fills_match_full_path_pixels() {
    use exact_linux::paint::{Backend, Shape};
    use exact_linux::raster::{rounded_rect, Raster};
    use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Rect, Transform};

    let mut cases = 0;
    for scale in [0.75_f32, 1.0, 1.5, 2.0] {
        for offset in [0.0, 0.25, 0.5, 0.99] {
            for transform in [
                Transform::identity(),
                Transform::from_translate(3.25, -2.5),
                Transform::from_scale(0.75, 1.25),
                Transform::from_row(-1.0, 0.0, 0.0, 1.0, 95.0, 0.0),
                Transform::from_rotate(11.0),
            ] {
                for radius in [0.0, 7.0] {
                    for damage in [false, true] {
                        let mut raster = Raster::new();
                        raster.begin(96.0, 80.0, scale);
                        let (width, height) =
                            ((96.0 * scale).round() as u32, (80.0 * scale).round() as u32);
                        let mut expected = Pixmap::new(width, height).unwrap();
                        expected.fill(Color::WHITE);
                        let device = Transform::from_scale(scale, scale);
                        let mut mask = Mask::new(width, height).unwrap();
                        if damage {
                            expected.fill(Color::from_rgba8(39, 73, 117, 255));
                            let rects = [(12.25, 10.5, 23.5, 30.25), (45.75, 20.25, 18.0, 29.5)];
                            assert!(raster.damage(&expected, &rects));
                            for (x, y, w, h) in rects {
                                mask.fill_path(
                                    &PathBuilder::from_rect(Rect::from_xywh(x, y, w, h).unwrap()),
                                    FillRule::Winding,
                                    false,
                                    device,
                                );
                            }
                            let mut clear = Paint::default();
                            clear.set_color(Color::WHITE);
                            clear.anti_alias = true;
                            expected.fill_path(
                                &PathBuilder::from_rect(
                                    Rect::from_xywh(0.0, 0.0, 96.0, 80.0).unwrap(),
                                ),
                                &clear,
                                FillRule::Winding,
                                device,
                                Some(&mask),
                            );
                        }
                        for (index, clip) in [
                            Shape {
                                rect: (4.5, 5.25, 77.0, 65.5),
                                radii: [(radius, radius); 4],
                                corners: None,
                            },
                            Shape {
                                rect: (14.0 + offset, 9.0 + offset, 47.5, 46.25),
                                radii: [(radius / 2.0, radius / 2.0); 4],
                                corners: None,
                            },
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let ts = Transform::from_rotate_at(3.0, 48.0, 40.0);
                            raster.push_clip(&clip, ts);
                            let path = rounded_rect(&clip).unwrap();
                            if index == 0 && !damage {
                                mask.fill_path(
                                    &path,
                                    FillRule::Winding,
                                    true,
                                    device.pre_concat(ts),
                                );
                            } else {
                                mask.intersect_path(
                                    &path,
                                    FillRule::Winding,
                                    true,
                                    device.pre_concat(ts),
                                );
                            }
                        }
                        for (rect, color) in [
                            ((-20.0, -20.0, 150.0, 140.0), [170, 31, 54, 255]),
                            (
                                (21.0 + offset, 17.0 - offset, 91.25, 69.5),
                                [11, 143, 217, 117],
                            ),
                            ((-30.0, 70.0, 20.0, 15.0), [10, 20, 30, 255]),
                        ] {
                            let shape = Shape {
                                rect,
                                radii: [(radius, radius); 4],
                                corners: None,
                            };
                            raster.fill(&shape, color, transform);
                            let mut paint = Paint::default();
                            paint.set_color(Color::from_rgba8(
                                color[0], color[1], color[2], color[3],
                            ));
                            paint.anti_alias = true;
                            expected.fill_path(
                                &rounded_rect(&shape).unwrap(),
                                &paint,
                                FillRule::Winding,
                                device.pre_concat(transform),
                                Some(&mask),
                            );
                        }
                        assert!(raster.finish().unwrap().data() == expected.data(), "scale={scale}, offset={offset}, transform={transform:?}, radius={radius}, damage={damage}");
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 320);
    // Preserve the full-path fallback at tiny-skia's tiling boundary.
    for width in [8191_u32, 8192] {
        let mut raster = Raster::new();
        raster.begin(width as f32, 8.0, 1.0);
        let mut expected = Pixmap::new(width, 8).unwrap();
        expected.fill(Color::WHITE);
        let clip = Shape::rect((8180.5, 1.25, 18.0, 6.25));
        raster.push_clip(&clip, Transform::identity());
        let mut mask = Mask::new(width, 8).unwrap();
        mask.fill_path(
            &rounded_rect(&clip).unwrap(),
            FillRule::Winding,
            true,
            Transform::identity(),
        );
        let shape = Shape::rect((0.0, 0.0, width as f32, 8.0));
        raster.fill(&shape, [11, 143, 217, 117], Transform::identity());
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(11, 143, 217, 117));
        paint.anti_alias = true;
        expected.fill_path(
            &rounded_rect(&shape).unwrap(),
            &paint,
            FillRule::Winding,
            Transform::identity(),
            Some(&mask),
        );
        assert!(
            raster.finish().unwrap().data() == expected.data(),
            "width={width}"
        );
    }
}

#[test]
fn damage_clearing_matches_the_full_masked_viewport() {
    use exact_linux::paint::Backend;
    use exact_linux::raster::Raster;
    use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Rect, Transform};

    let mut cases = 0;
    for scale in [0.7_f32, 0.75, 1.0, 1.1, 1.25, 1.5, 2.0, 2.3] {
        for (width, height) in [(97.3, 83.7), (96.0, 80.0), (8191.0, 3.0), (8192.0, 3.0)] {
            for offset in [0.0, 0.25, 0.5, 0.99] {
                for rects in [
                    vec![],
                    vec![(0.0, 0.0, width, height)],
                    vec![(12.0 + offset, 9.0 + offset, 24.5, 31.75)],
                    vec![
                        (-5.5, -7.25, 25.0, 33.0),
                        (8.0 + offset, 9.0, 21.0, 23.0),
                        (width - 2.25, height - 1.5, 20.0, 20.0),
                    ],
                    vec![(5.0, 6.0, 0.0, 3.0), (10.0, 9.0, -2.0, 4.0)],
                    vec![(f32::NAN, 0.0, 3.0, 4.0), (20.0, 20.0, 7.0, 8.0)],
                    vec![(-9000.0, -9000.0, 18000.0, 18000.0)],
                ] {
                    let mut raster = Raster::new();
                    let (w, h) = (
                        ((width * scale).round() as u32).max(1),
                        ((height * scale).round() as u32).max(1),
                    );
                    let mut expected = Pixmap::new(w, h).unwrap();
                    expected.fill(Color::from_rgba8(39, 73, 117, 139));
                    assert!(raster.begin_damage(width, height, scale, &expected, &rects));
                    let device = Transform::from_scale(scale, scale);
                    let mut mask = Mask::new(w, h).unwrap();
                    for &(x, y, w, h) in &rects {
                        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
                            mask.fill_path(
                                &PathBuilder::from_rect(rect),
                                FillRule::Winding,
                                false,
                                device,
                            );
                        }
                    }
                    let mut clear = Paint::default();
                    clear.set_color(Color::WHITE);
                    clear.anti_alias = true;
                    expected.fill_path(
                        &PathBuilder::from_rect(
                            Rect::from_xywh(0.0, 0.0, w as f32 / scale, h as f32 / scale).unwrap(),
                        ),
                        &clear,
                        FillRule::Winding,
                        device,
                        Some(&mask),
                    );
                    assert!(
                        raster.finish().unwrap().data() == expected.data(),
                        "scale={scale}, size={width}x{height}, offset={offset}, rects={rects:?}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 896);
}

#[test]
fn rectangular_damage_fills_keep_mask_pixels_and_clip_lifetime() {
    use exact_linux::paint::{Backend, Shape};
    use exact_linux::raster::{rounded_rect, Raster};
    use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Rect, Transform};

    let mut cases = 0;
    for scale in [0.75_f32, 1.0, 1.25, 2.0] {
        for width in [96_u32, 8191, 8192] {
            for rects in [
                vec![(8.0, 8.0, 64.0, 48.0)],
                vec![(8.0, 8.0, 64.0, 48.0), (16.0, 16.0, 8.25, 9.75)],
                vec![(8.0, 8.0, 12.0, 20.0), (40.0, 16.0, 16.0, 16.0)],
                vec![(8.25, 8.5, 64.25, 47.75)],
                vec![(-8.0, -8.0, width as f32 + 16.0, 80.0)],
                vec![(f32::NAN, 8.0, 4.0, 4.0), (8.0, 8.0, 64.0, 48.0)],
            ] {
                for stop in 0..5 {
                    for (rect, alpha, radius, ts) in [
                        (
                            (0.0, 0.0, width as f32, 64.0),
                            255,
                            0.0,
                            Transform::identity(),
                        ),
                        ((16.0, 16.0, 32.0, 32.0), 255, 0.0, Transform::identity()),
                        ((0.0, 0.0, 96.0, 64.0), 117, 0.0, Transform::identity()),
                        ((0.25, 0.5, 95.5, 63.25), 255, 0.0, Transform::identity()),
                        ((0.0, 0.0, 96.0, 64.0), 255, 4.0, Transform::identity()),
                        (
                            (0.0, 0.0, 96.0, 64.0),
                            255,
                            0.0,
                            Transform::from_rotate(7.0),
                        ),
                        (
                            (0.0, 0.0, 96.0, 64.0),
                            255,
                            0.0,
                            Transform::from_row(-1.0, 0.0, 0.0, 1.0, 96.0, 0.0),
                        ),
                    ]
                    .into_iter()
                    .take(if width == 96 { 7 } else { 2 })
                    {
                        let (w, h) = (
                            (width as f32 * scale).round() as u32,
                            (64.0 * scale).round() as u32,
                        );
                        let mut raster = Raster::new();
                        raster.begin(width as f32, 64.0, scale);
                        let mut expected = Pixmap::new(w, h).unwrap();
                        expected.fill(Color::WHITE);
                        assert!(raster.damage(&expected, &rects));
                        let device = Transform::from_scale(scale, scale);
                        let mut root = Mask::new(w, h).unwrap();
                        for &(x, y, w, h) in &rects {
                            if let Some(rect) = Rect::from_xywh(x, y, w, h) {
                                root.fill_path(
                                    &PathBuilder::from_rect(rect),
                                    FillRule::Winding,
                                    false,
                                    device,
                                );
                            }
                        }
                        let clip = Shape {
                            rect: (16.0, 12.0, 48.0, 40.0),
                            radii: [(7.0, 7.0); 4],
                            corners: None,
                        };
                        let mut nested = root.clone();
                        nested.intersect_path(
                            &rounded_rect(&clip).unwrap(),
                            FillRule::Winding,
                            true,
                            device,
                        );
                        let mut replacement = Mask::new(w, h).unwrap();
                        replacement.fill_path(
                            &rounded_rect(&clip).unwrap(),
                            FillRule::Winding,
                            true,
                            device,
                        );
                        for stage in 0..=stop {
                            let mask = match stage {
                                0 => Some(&root),
                                1 => {
                                    raster.push_clip(&clip, Transform::identity());
                                    Some(&nested)
                                }
                                2 => {
                                    raster.pop_clip();
                                    Some(&root)
                                }
                                3 => {
                                    raster.pop_clip();
                                    raster.push_clip(&clip, Transform::identity());
                                    Some(&replacement)
                                }
                                _ => {
                                    raster.begin(width as f32, 64.0, scale);
                                    expected.fill(Color::WHITE);
                                    None
                                }
                            };

                            let shape = Shape {
                                rect,
                                radii: [(radius, radius); 4],
                                corners: None,
                            };
                            let color = [31 + stage * 23, 117, 193, alpha];
                            raster.fill(&shape, color, ts);
                            let mut paint = Paint::default();
                            paint.set_color(Color::from_rgba8(
                                color[0], color[1], color[2], color[3],
                            ));
                            paint.anti_alias = true;
                            expected.fill_path(
                                &rounded_rect(&shape).unwrap(),
                                &paint,
                                FillRule::Winding,
                                device.pre_concat(ts),
                                mask,
                            );
                        }
                        assert!(
                            raster.finish().unwrap().data() == expected.data(),
                            "scale={scale}, width={width}, stop={stop}, rects={rects:?}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 1320);
}

#[test]
fn rounded_interiors_match_full_masked_paths_at_edges_and_in_layers() {
    use exact_linux::paint::{Backend, Shape};
    use exact_linux::raster::{rounded_rect, Raster};
    use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, PixmapPaint, Rect, Transform};

    let mut cases = 0;
    for scale in [0.75_f32, 1.0, 1.25, 2.0] {
        for transform in [
            Transform::identity(),
            Transform::from_row(-1.0, 0.0, 0.0, 1.0, 96.0, 0.0),
            Transform::from_row(1.0, 0.0, 0.0, -1.0, 0.0, 80.0),
            Transform::from_scale(0.75, 1.25),
            Transform::from_rotate(7.0),
            Transform::from_row(1.0, 0.0, 0.15, 1.0, 0.0, 0.0),
        ] {
            for radii in [
                [4.0; 4],
                [12.0; 4],
                [4.0, 8.0, 12.0, 6.0],
                [40.0; 4],
                [4.0, -1.0, 4.0, 4.0],
                [4.0, 0.0, 0.0, 4.0],
            ] {
                for rects in [
                    vec![(24.0, 8.0, 40.0, 64.0)],
                    vec![(8.0, 24.0, 80.0, 32.0)],
                    vec![(24.0, 6.0, 40.0, 64.0)],
                    vec![(4.0, 4.0, 24.0, 24.0)],
                    vec![(24.0, 8.0, 20.0, 24.0), (48.0, 40.0, 16.0, 24.0)],
                    vec![(24.0, 8.0, 40.0, 64.0), (28.25, 20.0, 8.5, 12.0)],
                ] {
                    for alpha in [117, 255] {
                        for layer in [false, true] {
                            let (w, h) =
                                ((96.0 * scale).round() as u32, (80.0 * scale).round() as u32);
                            let mut raster = Raster::new();
                            raster.begin(96.0, 80.0, scale);
                            let mut expected = Pixmap::new(w, h).unwrap();
                            expected.fill(Color::WHITE);
                            assert!(raster.damage(&expected, &rects));
                            let device = Transform::from_scale(scale, scale);
                            let mut mask = Mask::new(w, h).unwrap();
                            for &(x, y, w, h) in &rects {
                                mask.fill_path(
                                    &PathBuilder::from_rect(Rect::from_xywh(x, y, w, h).unwrap()),
                                    FillRule::Winding,
                                    false,
                                    device,
                                );
                            }
                            let shape = Shape {
                                rect: (3.25, 4.5, 88.25, 70.25),
                                radii: radii.map(|r| (r, r)),
                                corners: None,
                            };
                            if layer {
                                raster.push_opacity(0.37);
                            }
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    raster.fill(&shape, [31, 117, 193, alpha], transform)
                                }));
                            assert!(result.is_ok(), "fill panic: scale={scale}, transform={transform:?}, radii={radii:?}, rects={rects:?}, alpha={alpha}, layer={layer}");
                            let mut reference_layer = Pixmap::new(w, h).unwrap();
                            let target = if layer {
                                &mut reference_layer
                            } else {
                                &mut expected
                            };
                            let mut paint = Paint::default();
                            paint.set_color(Color::from_rgba8(31, 117, 193, alpha));
                            paint.anti_alias = true;
                            if let Some(path) = rounded_rect(&shape) {
                                target.fill_path(
                                    &path,
                                    &paint,
                                    FillRule::Winding,
                                    device.pre_concat(transform),
                                    Some(&mask),
                                );
                            }
                            if layer {
                                raster.pop_opacity();
                                expected.draw_pixmap(
                                    0,
                                    0,
                                    reference_layer.as_ref(),
                                    &PixmapPaint {
                                        opacity: 0.37,
                                        ..PixmapPaint::default()
                                    },
                                    Transform::identity(),
                                    None,
                                );
                            }
                            assert!(raster.finish().unwrap().data() == expected.data(), "scale={scale}, transform={transform:?}, radii={radii:?}, rects={rects:?}, alpha={alpha}, layer={layer}");
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 3456);
}

#[test]
fn damage_begin_resets_frames_and_preserves_full_repaint_fallback() {
    use exact_linux::paint::{Backend, Shape};
    use exact_linux::raster::Raster;
    use tiny_skia::{Color, Transform};

    let mut ordinary = Raster::new();
    let mut direct = Raster::new();
    for scale in [0.75_f32, 1.0, 1.5, 2.0] {
        for (width, height) in [(48.0, 40.0), (63.5, 51.25), (1.0, 1.0)] {
            for mismatch in [false, true] {
                let w = ((width * scale).round() as u32).max(1);
                let h = ((height * scale).round() as u32).max(1);
                let mut previous = Pixmap::new(w + u32::from(mismatch), h).unwrap();
                previous.fill(Color::from_rgba8(39, 73, 117, 139));
                for unfinished in [false, true] {
                    // A previous frame may have a cached clip or unfinished
                    // opacity layers. Both entry points must retire that state.
                    for raster in [&mut ordinary, &mut direct] {
                        raster.begin(48.0, 40.0, 1.0);
                        raster.push_clip(
                            &Shape {
                                rect: (2.0, 3.0, 40.0, 32.0),
                                radii: [(4.0, 4.0); 4],
                                corners: None,
                            },
                            Transform::identity(),
                        );
                        raster.push_opacity(0.3);
                        raster.fill(
                            &Shape::rect((0.0, 0.0, 48.0, 40.0)),
                            [190, 20, 80, 190],
                            Transform::identity(),
                        );
                        if !unfinished {
                            raster.finish().unwrap();
                        }
                    }
                    let damage = [(5.0, 7.0, 20.0, 18.0)];
                    ordinary.begin(width, height, scale);
                    let expected_partial = ordinary.damage(&previous, &damage);
                    let actual_partial =
                        direct.begin_damage(width, height, scale, &previous, &damage);
                    assert_eq!(actual_partial, expected_partial);
                    assert_eq!(actual_partial, !mismatch);
                    assert_eq!(
                        direct.finish().unwrap().data(),
                        ordinary.finish().unwrap().data()
                    );
                    // A following ordinary frame must lose all damage clipping.
                    for raster in [&mut ordinary, &mut direct] {
                        raster.begin(width, height, scale);
                        raster.fill(
                            &Shape::rect((0.0, 0.0, width, height)),
                            [20, 170, 90, 255],
                            Transform::identity(),
                        );
                    }
                    assert_eq!(
                        direct.finish().unwrap().data(),
                        ordinary.finish().unwrap().data()
                    );
                }
            }
        }
    }
}

#[test]
fn accessibility_focus_is_session_scoped_and_buttons_activate_from_keys() {
    let source = include_str!("../../../../contract/corpus/accessibility.contract");
    let mut p = compiled(source, 1.0, PainterChoice::Cpu);
    let id = |p: &Presenter<NoData>, name: &str| view(p, name);
    let first = id(&p, "first");
    assert_eq!(p.focus(), Some(first));
    let reply = exact_linux::agent::handle(
        &mut p,
        &format!(r#"{{"op":"type","id":{first},"key":"Space"}}"#),
    );
    assert!(!reply.contains("error"), "{reply}");
    assert!(p.host().agent(r#"{"op":"state"}"#).contains(r#""count":1"#));
    p.key(Some('\n'), false, 0.0);
    assert!(p.host().agent(r#"{"op":"state"}"#).contains(r#""count":2"#));
    p.tap(id(&p, "other")).unwrap();
    let other = id(&p, "other");
    assert_eq!(p.focus(), Some(other));
    p.clock(1000.0);
    p.clock(2000.0);
    assert_eq!(p.focus(), Some(other));
    let tree: serde_json::Value =
        serde_json::from_str(&exact_linux::agent::handle(&mut p, r#"{"op":"tree"}"#)).unwrap();
    let row = tree["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == other)
        .unwrap();
    assert_eq!(row["focused"], true);
    assert_eq!(row["accessibleName"], "Other");
    // The web's accname: a label names a text; an unlabelled text has none.
    let named = |test_id: &str| {
        let row = tree["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["props"]["testId"] == test_id)
            .unwrap();
        row.get("accessibleName").cloned()
    };
    assert_eq!(named("labelled"), Some("20 sheckles".into()));
    assert_eq!(named("live-count"), None);
    let state: serde_json::Value =
        serde_json::from_str(&exact_linux::agent::handle(&mut p, r#"{"op":"state"}"#)).unwrap();
    assert_eq!(state["focus"]["logical"], other);
    p.reload(&contract::compile(source).unwrap().encode(), NoData)
        .unwrap();
    // A carried reload is the same document: focus stays at Other's place,
    // and First's autofocus does not take it back (LLP 1035.000 D9).
    assert_eq!(p.focus(), Some(id(&p, "other")));
    assert!(p.host().agent(r#"{"op":"state"}"#).contains(r#""count":3"#));
    p.key(Some('\n'), false, 2000.0);
    assert!(p.host().agent(r#"{"op":"state"}"#).contains(r#""count":4"#));
}

#[test]
fn percentage_radius_paints_an_ellipse_in_a_nonsquare_box() {
    let source = r##"component Rounded
  view
    view width=200 height=150 background-color="#ffffff"
      view width=160 height=80 border-radius="50%" background-color="#2468ac"
"##;
    for choice in painters() {
        let mut p = compiled(source, 1.0, choice);
        let frame = p.frame();
        assert_eq!(px(&frame, 80.0, 40.0), (36, 104, 172));
        assert_eq!(
            px(&frame, 20.0, 5.0),
            (255, 255, 255),
            "ellipse, not a pill: {choice:?}"
        );
        assert_eq!(px(&frame, 80.0, 5.0), (36, 104, 172));
    }
}
