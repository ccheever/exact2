//! The painter, pixel by pixel: backgrounds land in their boxes with their
//! radii, text leaves ink, an image paints, motion presents as a transform
//! and a group opacity, a scroll container clips, a screenshot is the
//! viewport.

use exact_linux::Presenter;
use exact_runner::{DataError, DataSource, Value};
use std::path::PathBuf;
use std::time::Duration;
use tiny_skia::Pixmap;

fn assets() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/caltrain"))
}

fn boot() -> Presenter<caltrain_data::Caltrain> {
    let plan = caltrain::build().unwrap();
    let (mut p, _) = Presenter::boot(
        &plan.encode(),
        caltrain_data::Caltrain,
        (390.0, 844.0),
        1.0,
        assets(),
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

fn fixture(name: &str, scale: f32) -> Presenter<NoData> {
    let src = std::fs::read_to_string(format!(
        "{}/../../contract/corpus/{name}.contract",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    Presenter::boot(&plan.encode(), NoData, (390.0, 844.0), scale, assets())
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
    let mut p = boot();
    let frame = p.frame();
    assert_eq!(px(&frame, 2.0, 2.0), (255, 255, 255), "the page is white");
    let button = rect(&mut p, "change-station");
    let (cx, cy) = (button.0 + button.2 / 2.0, button.1 + button.3 / 2.0);
    // Inside the button but off its text: the top-left corner's inset.
    assert_eq!(
        px(&frame, button.0 + 10.0, button.1 + 3.0),
        (238, 238, 238),
        "#eeeeee at {cx},{cy}"
    );
    assert_eq!(
        px(&frame, button.0 + 0.5, button.1 + 0.5),
        (255, 255, 255),
        "radius 8: the corner pixel is outside"
    );
    let card = rect(&mut p, "board-north");
    assert_eq!(
        px(&frame, card.0 + 20.0, card.1 + 3.0),
        (247, 247, 247),
        "#f7f7f7 in the card"
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

#[test]
fn text_and_images_leave_ink_in_their_boxes() {
    let mut p = boot();
    let frame = p.frame();
    let name = rect(&mut p, "station-name");
    assert!(darkest(&frame, name) < 120, "24 pt bold text is dark ink");
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

#[test]
fn motion_presents_as_a_transform_and_a_group_opacity() {
    let mut p = fixture("spring", 1.0);
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
        "opacity 0.5 lightens the ink: {ink_before} → {ink_after}"
    );
}

#[test]
fn a_scroll_container_clips_what_it_scrolled_out() {
    let mut p = fixture("scroll", 1.0);
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
        "the page above the container is white"
    );
}

#[test]
fn a_device_scale_paints_more_pixels_for_the_same_points() {
    let mut p = fixture("scroll", 2.0);
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

#[test]
fn a_screenshot_is_the_viewport_as_a_png() {
    let mut p = boot();
    let path = std::env::temp_dir().join(format!("exact-paint-{}.png", std::process::id()));
    let reply = p.screenshot(path.to_str().unwrap()).unwrap();
    assert!(reply.ends_with(",\"w\":390,\"h\":844}"), "{reply}");
    let png = Pixmap::load_png(&path).unwrap();
    assert_eq!((png.width(), png.height()), (390, 844));
    assert_eq!(px(&png, 2.0, 2.0), (255, 255, 255));
    let _ = std::fs::remove_file(path);
}
