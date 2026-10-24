//! An SVG file as an image on Linux (LLP 1011.001 §4): the header read, the
//! plan at the box's pixels, the re-ask band, and a picture drawn through
//! the loader on the worker and, with text, on the paint thread.
use super::png_decode::inspect;
use super::svg_picture::{plan, resize_changes, Ask};
use super::*;
use exact_kernel::{AxisOffer, Dimension, ObjectFit, Offer, Op, StyleId, StyleProps};
use std::io::Cursor;

const WAIT: Duration = Duration::from_secs(60);

fn header(text: &str) -> Result<png_decode::Header, Refusal> {
    inspect(&mut Cursor::new(text.as_bytes()), text.len() as u64)
}

fn natural(text: &str) -> (u32, u32) {
    let h = header(text).unwrap();
    assert!(h.is_svg(), "{text}");
    (h.metadata.natural.width, h.metadata.natural.height)
}

#[test]
fn the_header_gives_the_natural_size_from_the_root_tag_and_sniffs_by_bytes() {
    assert_eq!(
        natural(r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="24"/>"#),
        (48, 24)
    );
    assert_eq!(
        natural("<svg width=\"10pt\" viewBox=\"0 0 30 10\"></svg>"),
        (14, 5)
    );
    assert_eq!(
        natural("\u{feff}<?xml version=\"1.0\"?>\n<!-- a mark -->\n<svg viewBox=\"0 0 120 40\"/>"),
        (120, 40)
    );
    assert_eq!(natural("<svg/>"), (300, 150));
    assert_eq!(natural("<svg width=\"50%\" height=\"20\"/>"), (300, 20));
    // Not an SVG: the raster readers' own refusal, whatever the name.
    assert!(matches!(header("hello <svg>"), Err(Refusal::DecodeFailed)));
    let png = decode_tests::png(4, 4);
    let raster = inspect(&mut Cursor::new(&png), png.len() as u64).unwrap();
    assert!(!raster.is_svg());
    // A size of nothing counts as absent, as a percentage does: the default.
    assert_eq!(
        natural("<svg width=\"0\" height=\"0\" viewBox=\"0 0 0 0\"/>"),
        (300, 150)
    );
}

#[test]
fn the_plan_is_the_boxs_pixels_above_the_natural_size_and_under_the_caps() {
    let h = header("<svg width=\"24\" height=\"24\"/>").unwrap();
    let at = |desired, fit, scale| {
        let p = plan(h, desired, Ask { fit, scale }, SESSION_BYTES).unwrap();
        (p.pixels.width, p.pixels.height)
    };
    // A 24-unit logo in a 96-point box at 3×: 288 pixels, not 24.
    assert_eq!(at((288.0, 288.0), ObjectFit::Contain, 3.0), (288, 288));
    // `fill` takes both axes; `contain` and `cover` keep the ratio.
    assert_eq!(at((200.0, 50.0), ObjectFit::Fill, 1.0), (200, 50));
    assert_eq!(at((200.0, 50.0), ObjectFit::Contain, 1.0), (50, 50));
    assert_eq!(at((200.0, 50.0), ObjectFit::Cover, 1.0), (200, 200));
    assert_eq!(at((200.0, 50.0), ObjectFit::None, 2.0), (48, 48));
    assert_eq!(at((200.0, 50.0), ObjectFit::ScaleDown, 1.0), (24, 24));
    // Not laid out yet: the natural size in device pixels.
    assert_eq!(at((0.0, 0.0), ObjectFit::Fill, 2.0), (48, 48));
    // The island's side caps a side; a budget halves the picture.
    assert_eq!(at((9000.0, 100.0), ObjectFit::Fill, 1.0), (4096, 100));
    let tight = plan(h, (1000.0, 1000.0), Ask::default(), 1024 * 1024 + 1_200_000).unwrap();
    assert!(
        tight.pixels.width < 1000 && tight.pixels.width >= 250,
        "{:?}",
        tight.pixels
    );
    assert!(png_decode::DecodePlan::new(h, (5000, 10)).is_err());
}

#[test]
fn a_box_moving_past_the_band_on_either_axis_asks_again() {
    let h = header("<svg viewBox=\"0 0 10 10\"/>").unwrap();
    let fill = Ask::default();
    assert!(!resize_changes(h, fill, (100.0, 100.0), (110.0, 100.0)));
    assert!(resize_changes(h, fill, (100.0, 100.0), (100.0, 130.0)));
    assert!(resize_changes(h, fill, (100.0, 100.0), (40.0, 100.0)));
    assert!(!resize_changes(h, fill, (100.0, 100.0), (100.0, 100.0)));
}

const MARK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10">
  <style>.l{fill:#ff0000}</style>
  <rect class="l" width="10" height="10"/>
  <rect x="10" width="10" height="10" fill="#0000ff"/>
  <script>alert(1)</script>
</svg>"##;

const LABEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20">
  <text x="2" y="15" font-size="14" fill="#000">Hi</text>
</svg>"##;

fn assets() -> Assets {
    Assets::selected(
        PathBuf::new(),
        Arc::new(|name| {
            Ok(Some(match name {
                "mark.svg" => Arc::from(MARK.as_bytes()),
                "label.svg" => Arc::from(LABEL.as_bytes()),
                _ => return Ok(None),
            }))
        }),
    )
}

/// One image node `1` of `source` in a `width`×`height` box at `fit`, laid out.
fn kernel(source: &str, width: f32, height: f32, fit: ObjectFit) -> Kernel {
    let mut k = Kernel::with_monospace();
    let mut style = StyleProps {
        width: Dimension::Points(width),
        height: Dimension::Points(height),
        ..Default::default()
    };
    style.object_fit = fit;
    for row in [StyleId::Width, StyleId::Height, StyleId::ObjectFit] {
        style.mask.set(row);
    }
    k.apply(
        0,
        1,
        &[
            Op::CreateView {
                id: 1,
                node_type: NodeType::Image,
            },
            Op::SetProp {
                id: 1,
                prop: PropId::ImageSource,
                value: source.into(),
            },
            Op::SetStyle {
                id: 1,
                patch: Box::new(style),
            },
            Op::AttachRoot { id: 1 },
        ],
    )
    .unwrap();
    let offer = Offer {
        width: AxisOffer::Definite(1000.0),
        height: AxisOffer::Definite(1000.0),
    };
    k.compute_layout(1, offer).unwrap();
    k
}

fn rgba(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 4] {
    let p = bitmap.pixels();
    let i = ((y * p.width() + x) * 4) as usize;
    p.data()[i..i + 4].try_into().unwrap()
}

#[test]
fn a_picture_without_text_is_drawn_on_the_worker_at_the_boxs_pixels() {
    // A 20×10 mark in an 80×40 box at 2×: 160×80 pixels, left red, right blue.
    let k = kernel("mark.svg", 80.0, 40.0, ObjectFit::Fill);
    let mut images = Images::with_assets(assets());
    let reports = images.sync_visible(&k, &k.roots(), 2.0, |_| true);
    assert!(reports.is_empty());
    let reports = images.wait(WAIT);
    assert_eq!(reports, vec![(1, Some((20.0, 10.0)))]);
    let bitmap = images.bitmaps[&1].clone();
    assert_eq!((bitmap.width(), bitmap.height()), (160, 80));
    assert_eq!(bitmap.natural(), (20, 10));
    assert_eq!(rgba(&bitmap, 40, 40), [255, 0, 0, 255]);
    assert_eq!(rgba(&bitmap, 120, 40), [0, 0, 255, 255]);
    assert_eq!(rgba(&bitmap, 79, 1), [255, 0, 0, 255]);
    assert_eq!(rgba(&bitmap, 80, 78), [0, 0, 255, 255]);
    // `contain` in a square box keeps the ratio: the picture is 80×40 pixels.
    let k = kernel("mark.svg", 40.0, 40.0, ObjectFit::Contain);
    let mut images = Images::with_assets(assets());
    images.sync_visible(&k, &k.roots(), 2.0, |_| true);
    images.wait(WAIT);
    let bitmap = &images.bitmaps[&1];
    assert_eq!((bitmap.width(), bitmap.height()), (80, 40));
}

#[test]
fn a_picture_with_text_waits_for_the_paint_thread_and_draws_with_its_fonts() {
    let k = kernel("label.svg", 40.0, 20.0, ObjectFit::Fill);
    let mut images = Images::with_assets(assets());
    images.sync_visible(&k, &k.roots(), 1.0, |_| true);
    // No painter on this thread: the picture is resolved and waits.
    let started = Instant::now();
    loop {
        images.poll();
        let decoding = images.views[&1].request.is_some_and(|(id, _)| {
            images.backend.session.status(id) == Some(RequestStatus::Decoding)
        });
        if decoding && images.backend.pictures_waiting() == 1 {
            break;
        }
        assert!(started.elapsed() < WAIT, "the picture never waited");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!images.bitmaps.contains_key(&1));
    images.paint_with(crate::text::TextEngine::shared());
    let reports = images.wait(WAIT);
    assert_eq!(reports, vec![(1, Some((40.0, 20.0)))]);
    let bitmap = &images.bitmaps[&1];
    assert_eq!((bitmap.width(), bitmap.height()), (40, 20));
    let p = bitmap.pixels();
    let inked = p.data().chunks(4).filter(|px| px[3] > 0).count();
    assert!(inked > 20 && inked < 40 * 20, "{inked} pixels inked");
}

/// A file past a cost limit (LLP 1011.001 D2): the decode is refused, so
/// no picture is made and the view keeps its refusal; nothing is asked again.
#[test]
fn a_picture_past_a_cost_limit_is_refused_by_the_loader() {
    let mut defs = String::from("<path id=\"l0\" d=\"M0 0");
    defs += &"l1 1l1 -1".repeat(15);
    defs += "\"/>";
    for k in 1..=16 {
        defs += &format!(
            "<g id=\"l{k}\"><use href=\"#l{0}\"/><use href=\"#l{0}\"/></g>",
            k - 1
        );
    }
    let bomb: Arc<[u8]> = Arc::from(
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"9\" height=\"9\"><defs>{defs}</defs><use href=\"#l16\"/></svg>")
            .into_bytes(),
    );
    let assets = Assets::selected(
        PathBuf::new(),
        Arc::new(move |name| Ok((name == "bomb.svg").then(|| bomb.clone()))),
    );
    let k = kernel("bomb.svg", 64.0, 64.0, ObjectFit::Fill);
    let mut images = Images::with_assets(assets);
    images.sync_visible(&k, &k.roots(), 1.0, |_| true);
    let started = Instant::now();
    while images.views[&1].refusal.is_none() {
        images.poll();
        images.sync_visible(&k, &k.roots(), 1.0, |_| true);
        assert!(started.elapsed() < WAIT, "never refused");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(images.views[&1].refusal, Some(Refusal::DecodeFailed));
    assert!(!images.bitmaps.contains_key(&1));
}

/// tiny-skia panics on a path reaching past about ±2^31 device pixels; a
/// file's coordinates, `viewBox` and `stroke-width` can reach that, and
/// such a path is not drawn while the rest of the picture is.
#[test]
fn a_path_out_of_tiny_skias_reach_is_not_drawn() {
    let file = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
      <clipPath id="c"><rect x="-1e10" y="-1e10" width="2e10" height="2e10"/></clipPath>
      <rect x="-1e10" y="-1e10" width="2e10" height="2e10" fill="#f00"/>
      <path d="M10 10 L50 10" stroke="#f00" stroke-width="1e10"/>
      <rect width="30" height="30" clip-path="url(#c)" fill="#f00"/>
      <svg x="-1e10" y="-1e10" width="2e10" height="2e10" overflow="hidden"><rect x="1e10" y="1e10" width="30" height="30" fill="#f00"/></svg>
      <rect x="50" y="50" width="50" height="50" fill="#00f"/>
    </svg>"##;
    let loaded = exact_svg_file::load(file).unwrap();
    let scene = loaded.scene(64.0, 64.0);
    let pixels = exact_raster::PixelSize {
        width: 64,
        height: 64,
    };
    let drawn = super::svg_picture::paint(
        crate::text::TextEngine::shared(),
        &scene,
        (64.0, 64.0),
        1.0,
        pixels,
    )
    .unwrap();
    let at = |x: u32, y: u32| drawn.pixel(x, y).unwrap();
    assert_eq!(
        at(5, 5).alpha(),
        0,
        "the far rect, stroke, clip and viewport draw nothing"
    );
    let blue = at(50, 50);
    assert_eq!((blue.red(), blue.blue(), blue.alpha()), (0, 255, 255));
}
