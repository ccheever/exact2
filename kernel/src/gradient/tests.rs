use super::*;
use crate::{
    wire::codec::{Reader, Writer},
    DecodeError, StyleId, StyleMask, StyleProps, StyleValue,
};

fn parse(css: &str) -> Gradient {
    BackgroundImage::check(css)
        .unwrap_or_else(|e| panic!("{css}: {e}"))
        .gradient()
        .cloned()
        .unwrap()
}

fn close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

#[test]
fn the_css_grammar_and_its_canonical_form() {
    for (css, canonical) in [
        ("none", "none"),
        (" NONE ", "none"),
        (
            "linear-gradient(#000, #fff)",
            "linear-gradient(180deg, #000000ff 0%, #ffffffff 100%)",
        ),
        (
            "linear-gradient(to right, transparent, #fff 40%)",
            "linear-gradient(90deg, #00000000 0%, #ffffffff 40%)",
        ),
        (
            "linear-gradient(0.25turn, rgb(255 0 0), #00f)",
            "linear-gradient(90deg, #ff0000ff 0%, #0000ffff 100%)",
        ),
        (
            "linear-gradient(to left top, #000, #fff)",
            "linear-gradient(to top left, #000000ff 0%, #ffffffff 100%)",
        ),
        (
            "linear-gradient(transparent, light-dark(#fff, #000))",
            "linear-gradient(180deg, #00000000 0%, light-dark(#ffffffff, #000000ff) 100%)",
        ),
        (
            "radial-gradient(#000, #fff)",
            "radial-gradient(ellipse farthest-corner at 50% 50%, #000000ff 0%, #ffffffff 100%)",
        ),
        (
            "radial-gradient(circle at top left, #000, #fff)",
            "radial-gradient(circle farthest-corner at 0% 0%, #000000ff 0%, #ffffffff 100%)",
        ),
        (
            "radial-gradient(closest-side circle at 10px 30%, #000, #fff)",
            "radial-gradient(circle closest-side at 10px 30%, #000000ff 0%, #ffffffff 100%)",
        ),
        (
            "radial-gradient(at bottom, #000 0% 50%, #fff)",
            "radial-gradient(ellipse farthest-corner at 50% 100%, #000000ff 0%, #000000ff 50%, #ffffffff 100%)",
        ),
    ] {
        let parsed = BackgroundImage::check(css).unwrap_or_else(|e| panic!("{css}: {e}"));
        assert_eq!(parsed.css(), canonical, "{css}");
        assert_eq!(BackgroundImage::parse(&parsed.css()), Some(parsed), "{css}");
    }
}

#[test]
fn stop_positions_follow_the_css_fix_up() {
    let at = |css: &str| parse(css).stops.iter().map(|s| s.at).collect::<Vec<_>>();
    assert_eq!(
        at("linear-gradient(#000, #111, #222, #333)"),
        [0.0, 100.0 / 3.0, 200.0 / 3.0, 100.0]
    );
    assert_eq!(
        at("linear-gradient(#000 20%, #111, #222 80%)"),
        [20.0, 50.0, 80.0]
    );
    // A position before an earlier one moves up to it: a hard stop.
    assert_eq!(
        at("linear-gradient(#000 60%, #111 30%, #222)"),
        [60.0, 60.0, 100.0]
    );
    let round = BackgroundImage::parse("linear-gradient(#000, #111, #222)").unwrap();
    assert_eq!(BackgroundImage::parse(&round.css()), Some(round));
}

#[test]
fn what_is_not_drawn_is_refused_by_name() {
    for (css, says) in [
        (
            "repeating-linear-gradient(#000, #fff 10%)",
            "repeating-linear-gradient",
        ),
        (
            "repeating-radial-gradient(#000, #fff 10%)",
            "repeating-radial-gradient",
        ),
        ("url(a.png)", "image as a background"),
        (
            "linear-gradient(#000, #fff), linear-gradient(#fff, #000), linear-gradient(#000, #fff), linear-gradient(#000, #fff), linear-gradient(#000, #fff)",
            "at most four",
        ),
        ("conic-gradient(from up, #000, #fff)", "`from` takes an angle"),
        ("conic-gradient(#000 10px, #fff)", "percentage or an angle"),
        ("linear-gradient(#000, 30%, #fff)", "colour hints"),
        ("linear-gradient(#000 10px, #fff)", "percentage"),
        ("linear-gradient(#000 -10%, #fff 120%)", "0% to 100%"),
        ("linear-gradient(#000)", "at least two"),
        ("linear-gradient(red, blurple)", "stop's colour"),
        ("linear-gradient(to middle, #000, #fff)", "side or corner"),
        (
            "radial-gradient(circle 20px, #000, #fff)",
            "explicit radial size",
        ),
        (
            "radial-gradient(at right 10px top 5px, #000, #fff)",
            "three or four",
        ),
        ("linear-gradient(#000, #fff", "expected none"),
        ("blur(2px)", "expected none"),
    ] {
        let why = BackgroundImage::check(css).unwrap_err();
        assert!(why.contains(says), "{css}: {why}");
        assert_eq!(BackgroundImage::parse(css), None);
    }
    let many = (0..65).map(|_| "#000").collect::<Vec<_>>().join(", ");
    assert!(BackgroundImage::check(&format!("linear-gradient({many})")).is_err());
}

#[test]
fn a_linear_line_spans_the_box_as_css_draws_it() {
    let line = |css: &str, w: f32, h: f32| match parse(css).geometry(w, h) {
        Geometry::Linear { start, end } => (start, end),
        other => panic!("{other:?}"),
    };
    let (s, e) = line("linear-gradient(#000, #fff)", 200.0, 100.0);
    assert!(
        close(s, (100.0, 0.0)) && close(e, (100.0, 100.0)),
        "{s:?} {e:?}"
    );
    let (s, e) = line("linear-gradient(90deg, #000, #fff)", 200.0, 100.0);
    assert!(
        close(s, (0.0, 50.0)) && close(e, (200.0, 50.0)),
        "{s:?} {e:?}"
    );
    // 45° on a square: the line is the diagonal, corner to corner.
    let (s, e) = line("linear-gradient(45deg, #000, #fff)", 100.0, 100.0);
    assert!(
        close(s, (0.0, 100.0)) && close(e, (100.0, 0.0)),
        "{s:?} {e:?}"
    );
    // `to top right` on a 200×100 box: perpendicular to the other diagonal,
    // so the 50% line passes through the top-left and bottom-right corners.
    let (s, e) = line("linear-gradient(to top right, #000, #fff)", 200.0, 100.0);
    let (dx, dy) = (e.0 - s.0, e.1 - s.1);
    assert!((dx * 200.0 + dy * 100.0).abs() < 1e-2, "{s:?} {e:?}");
    assert!(dx > 0.0 && dy < 0.0);
    // The ends' perpendiculars meet the corners: bottom-left projects to 0.
    let t = |(x, y): (f32, f32)| ((x - s.0) * dx + (y - s.1) * dy) / (dx * dx + dy * dy);
    assert!(t((0.0, 100.0)).abs() < 1e-4 && (t((200.0, 0.0)) - 1.0).abs() < 1e-4);
}

#[test]
fn a_radial_extent_is_measured_from_its_centre() {
    let ellipse = |css: &str| match parse(css).geometry(200.0, 100.0) {
        Geometry::Radial { center, radii } => (center, radii),
        other => panic!("{other:?}"),
    };
    let r2 = std::f32::consts::SQRT_2;
    let (c, r) = ellipse("radial-gradient(#000, #fff)");
    assert!(
        close(c, (100.0, 50.0)) && close(r, (100.0 * r2, 50.0 * r2)),
        "{r:?}"
    );
    let (_, r) = ellipse("radial-gradient(closest-side, #000, #fff)");
    assert!(close(r, (100.0, 50.0)), "{r:?}");
    let (_, r) = ellipse("radial-gradient(circle closest-side, #000, #fff)");
    assert!(close(r, (50.0, 50.0)), "{r:?}");
    let (c, r) = ellipse("radial-gradient(circle farthest-corner at 0 0, #000, #fff)");
    let far = 200f32.hypot(100.0);
    assert!(close(c, (0.0, 0.0)) && close(r, (far, far)), "{r:?}");
    let (c, r) = ellipse("radial-gradient(circle farthest-side at 25% 10px, #000, #fff)");
    assert!(
        close(c, (50.0, 10.0)) && close(r, (150.0, 150.0)),
        "{c:?} {r:?}"
    );
}

#[test]
fn transparent_takes_its_neighbours_hue_on_each_side() {
    let white = Color::rgba(255, 255, 255, 255);
    let red = Color::rgba(255, 0, 0, 255);
    let ramp = premultiplied_ramp(&[(0.0, Color::TRANSPARENT), (1.0, white)]);
    assert_eq!(ramp, [(0.0, Color::rgba(255, 255, 255, 0)), (1.0, white)]);
    // In the middle, one position, two hues: red's side, then white's.
    let ramp = premultiplied_ramp(&[(0.0, red), (0.5, Color::TRANSPARENT), (1.0, white)]);
    assert_eq!(
        ramp,
        [
            (0.0, red),
            (0.5, Color::rgba(255, 0, 0, 0)),
            (0.5, Color::rgba(255, 255, 255, 0)),
            (1.0, white)
        ]
    );
    // Two partial alphas of different hues are sampled premultiplied: the
    // mix leans to the more opaque end, never to grey.
    let half_red = Color::rgba(255, 0, 0, 64);
    let blue = Color::rgba(0, 0, 255, 255);
    let ramp = premultiplied_ramp(&[(0.0, half_red), (1.0, blue)]);
    assert_eq!(ramp.len(), 9);
    let (at, mid) = ramp[4];
    assert_eq!(at, 0.5);
    // premultiplied: r = 255*64*.5 / 159.5 ≈ 51, b = 255*255*.5 / 159.5 ≈ 204
    assert_eq!((mid.r(), mid.g(), mid.b(), mid.a()), (51, 0, 204, 160));
}

#[test]
fn scheme_aware_stops_resolve_per_appearance() {
    let g = parse("linear-gradient(light-dark(#fff, #000), transparent)");
    assert!(g.is_scheme_aware());
    assert_eq!(g.resolved(false)[0], (0.0, Color::WHITE));
    assert_eq!(g.resolved(true)[0], (0.0, Color::BLACK));
    assert!(!parse("linear-gradient(#fff, #000)").is_scheme_aware());
}

#[test]
fn resolved_stops_run_from_zero_to_one() {
    let red = Color::rgba(255, 0, 0, 255);
    let blue = Color::rgba(0, 0, 255, 255);
    assert_eq!(
        parse("linear-gradient(90deg, #f00 50%, #00f 50%)").resolved(false),
        [(0.0, red), (0.5, red), (0.5, blue), (1.0, blue)]
    );
}

#[test]
fn generated_patch_codec_round_trips_and_refuses_bad_wire_data() {
    let mut style = StyleProps::default();
    style
        .set_dynamic(
            StyleId::BackgroundImage,
            &StyleValue::Text(
                "linear-gradient(to top, light-dark(#fff, #000), transparent 30%)".into(),
            ),
        )
        .unwrap();
    let mut bytes = Writer::new();
    style.encode_patch(&mut bytes);
    assert_eq!(
        StyleProps::decode_patch(&mut Reader::new(bytes.as_slice())).unwrap(),
        style
    );
    assert!(!StyleId::BackgroundImage.affects_layout());
    let mut bad = Writer::new();
    bad.style_mask(StyleMask::of(StyleId::BackgroundImage));
    bad.string("repeating-conic-gradient(#000, #fff)");
    assert_eq!(
        StyleProps::decode_patch(&mut Reader::new(bad.as_slice())),
        Err(DecodeError::BadBackgroundImage)
    );
}

/// LLP 1077 D5: `conic-gradient()` from an angle at a position, stops in
/// percentages or angles; and up to four layers, the first on top.
#[test]
fn conic_gradients_and_layers_parse_and_round_trip() {
    let image = BackgroundImage::check(
        "conic-gradient(from 90deg at 25% 10px, #f00, #00f 90deg, #0f0 0.5turn), linear-gradient(#000, #fff)",
    )
    .unwrap();
    assert_eq!(image.layers().len(), 2);
    let conic = &image.layers()[0];
    assert_eq!(
        conic.kind,
        GradientKind::Conic {
            from: 90.0,
            at: [Length::Percent(25.0), Length::Px(10.0)]
        }
    );
    assert_eq!(
        conic.stops.iter().map(|s| s.at).collect::<Vec<_>>(),
        [0.0, 25.0, 50.0]
    );
    assert_eq!(
        image.css(),
        "conic-gradient(from 90deg at 25% 10px, #ff0000ff 0%, #0000ffff 25%, #00ff00ff 50%), linear-gradient(180deg, #000000ff 0%, #ffffffff 100%)"
    );
    assert_eq!(BackgroundImage::check(&image.css()).unwrap(), image);
    assert_eq!(
        conic.geometry(200.0, 100.0),
        Geometry::Conic {
            center: (50.0, 10.0),
            from: 90.0
        }
    );
    let plain = BackgroundImage::check("conic-gradient(#000, #fff)").unwrap();
    assert_eq!(
        plain.gradient().unwrap().kind,
        GradientKind::Conic {
            from: 0.0,
            at: [Length::Percent(50.0); 2]
        }
    );
    assert!(BackgroundImage::check_mask(
        "linear-gradient(#000, #fff), linear-gradient(#fff, #000)"
    )
    .is_err());
    assert!(BackgroundImage::check_mask("conic-gradient(#000, #fff)").is_ok());
}
