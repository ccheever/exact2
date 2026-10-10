//! LLP 1077: `corner-shape` and `mask-image` held to Chrome's pixels, and
//! `text-shadow` painted where CSS puts it. The pages are
//! `scripts/fixtures/visual.contract` and `text-shadow.contract`; Chrome's
//! pictures of the first (the web host at 1×) are `visual.web.png` as booted
//! and `visual.web-dark.png` after `dark`. Text is the pinned font here and
//! the system's in Chrome, so the shadow page is checked by where the
//! shadow's colour lands, not against Chrome's glyphs.

use crate::borders::{boot, held_to_chrome, view, NoData};
use exact_linux::presenter::PainterChoice;
use exact_linux::Presenter;
use tiny_skia::Pixmap;

/// Every case Chrome draws as CSS defines it (`apple` and `apple-border`
/// are a declared approximation on the web).
const CASES: [&str; 10] = [
    "squircle",
    "bevel",
    "scoop",
    "bordered",
    "sides",
    "clips",
    "fade",
    "spot",
    "children",
    "masked-round",
];

fn painters() -> Vec<PainterChoice> {
    let mut choices = vec![PainterChoice::Cpu];
    match exact_linux::gpu::Gpu::new() {
        Ok(_) => choices.push(PainterChoice::Gpu),
        Err(error) => eprintln!("GPU painter unavailable: {error}"),
    }
    choices
}

#[test]
fn every_corner_and_mask_case_matches_chrome_light_then_dark() {
    let mut failures = Vec::new();
    for choice in painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "visual.contract");
        failures.extend(held_to_chrome(&mut p, "visual.web.png", &name, &CASES));
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(&mut p, "visual.web-dark.png", &name, &CASES));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Pixels in a case's box (and `pad` points around it) whose colour is
/// within 40 of `rgb` in every channel.
fn count(p: &mut Presenter<NoData>, case: &str, pad: f32, rgb: [u8; 3]) -> usize {
    count_where(p, case, pad, |c| {
        c.iter().zip(rgb).all(|(a, b)| a.abs_diff(b) <= 40)
    })
}

/// Pixels in a case's box (and `pad` points around it) that `like` takes.
fn count_where(
    p: &mut Presenter<NoData>,
    case: &str,
    pad: f32,
    like: impl Fn([u8; 3]) -> bool,
) -> usize {
    let id = view(p, case);
    let (x, y, w, h) = p.boxes().iter().find(|b| b.id == id).unwrap().rect;
    let frame = p.frame();
    count_rect(
        &frame,
        (x - pad, y - pad, w + 2.0 * pad, h + 2.0 * pad),
        like,
    )
}

fn count_rect(
    frame: &Pixmap,
    (x, y, w, h): (f32, f32, f32, f32),
    like: impl Fn([u8; 3]) -> bool,
) -> usize {
    let mut n = 0;
    for py in (y.max(0.0) as u32)..((y + h) as u32).min(frame.height()) {
        for px in (x.max(0.0) as u32)..((x + w) as u32).min(frame.width()) {
            let c = frame.pixel(px, py).unwrap().demultiply();
            if like([c.red(), c.green(), c.blue()]) {
                n += 1;
            }
        }
    }
    n
}

/// Inline nodes have no frames. These sub-rectangles of each paragraph
/// sample the named runs in both the pinned font and Chrome's system font.
/// Check the reference pictures too: glyph shapes differ, colour placement
/// does not. The black threshold excludes the dark page's #121212.
#[test]
fn inline_shadows_and_strokes_follow_each_runs_computed_style() {
    let red = |[r, g, b]: [u8; 3]| r > g.saturating_add(50) && r > b.saturating_add(50);
    let blue = |[r, g, b]: [u8; 3]| b > r.saturating_add(50) && b > g.saturating_add(30);
    let green = |[r, g, b]: [u8; 3]| g > r.saturating_add(35) && g > b.saturating_add(35);
    let black = |[r, g, b]: [u8; 3]| r < 8 && g < 8 && b < 8;
    type Sample = (
        &'static str,
        &'static str,
        (f32, f32, f32, f32),
        fn([u8; 3]) -> bool,
        usize,
    );
    let samples: &[Sample] = &[
        (
            "one-run",
            "Glow red glow",
            (60.0, 0.0, 160.0, 60.0),
            red,
            40,
        ),
        ("one-run", "Pl no glow", (0.0, 0.0, 32.0, 60.0), red, 0),
        (
            "run-none",
            "Sh blue shadow",
            (0.0, 0.0, 55.0, 60.0),
            blue,
            40,
        ),
        (
            "run-none",
            "None no shadow",
            (100.0, 0.0, 160.0, 60.0),
            blue,
            0,
        ),
        (
            "two-shadows",
            "Up green above",
            (0.0, 0.0, 50.0, 20.0),
            green,
            40,
        ),
        (
            "two-shadows",
            "Up no green below",
            (0.0, 40.0, 50.0, 20.0),
            green,
            0,
        ),
        (
            "two-shadows",
            "Down red below",
            (90.0, 30.0, 150.0, 30.0),
            red,
            40,
        ),
        (
            "two-shadows",
            "Down no red above",
            (90.0, 0.0, 150.0, 10.0),
            red,
            0,
        ),
        (
            "one-stroke",
            "Cd black stroke",
            (78.0, 0.0, 70.0, 60.0),
            black,
            20,
        ),
        (
            "one-stroke",
            "Ab no stroke",
            (0.0, 0.0, 48.0, 60.0),
            black,
            0,
        ),
        (
            "stroke-runs",
            "Ab red stroke",
            (0.0, 0.0, 50.0, 60.0),
            red,
            40,
        ),
        (
            "stroke-runs",
            "Off no red stroke",
            (92.0, 0.0, 28.0, 60.0),
            red,
            0,
        ),
        (
            "stroke-runs",
            "Off no green stroke",
            (92.0, 0.0, 28.0, 60.0),
            green,
            0,
        ),
        (
            "stroke-runs",
            "Gr green inherited width",
            (155.0, 0.0, 75.0, 60.0),
            green,
            20,
        ),
        (
            "stroke-runs",
            "Gr no inherited red",
            (155.0, 0.0, 75.0, 60.0),
            red,
            0,
        ),
    ];
    let mut failures = Vec::new();
    for choice in painters() {
        let mut p = boot(choice, "span-paint.contract");
        assert!(p.resize(390.0, 520.0).is_none());
        for reference in ["span-paint.web.png", "span-paint.web-dark.png"] {
            if reference.contains("dark") {
                let id = view(&p, "dark");
                p.tap(id).unwrap();
                p.run_commands(NoData::default);
            }
            // LLP 1104: Linux's own button ring differs from Chrome's.
            // Compare this text-paint oracle with the scheme button blurred;
            // native_buttons tests the focused chrome separately.
            p.blur();
            failures.extend(held_to_chrome(
                &mut p,
                reference,
                &format!("{choice:?}"),
                &["dark"],
            ));
            let chrome = Pixmap::load_png(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../scripts/fixtures")
                    .join(reference),
            )
            .unwrap();
            let frame = p.frame();
            for &(case, label, (dx, dy, w, h), like, minimum) in samples {
                let id = view(&p, case);
                let (x, y, _, _) = p.boxes().iter().find(|b| b.id == id).unwrap().rect;
                for (name, image) in [
                    (format!("{choice:?}"), frame.as_ref()),
                    ("Chrome".into(), &chrome),
                ] {
                    let n = count_rect(image, (x + dx, y + dy, w, h), like);
                    if (minimum == 0 && n != 0) || (minimum > 0 && n < minimum) {
                        failures.push(format!(
                            "{name} {reference} {case}: {label}: {n} pixels, expected {}",
                            if minimum == 0 {
                                "none".into()
                            } else {
                                format!("at least {minimum}")
                            }
                        ));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_text_shadow_paints_its_colour_under_the_glyphs_and_none_paints_none() {
    for choice in painters() {
        let mut p = boot(choice, "text-shadow.contract");
        let red = [0xe5, 0x39, 0x35];
        assert!(
            count(&mut p, "offset", 4.0, red) > 40,
            "{choice:?}: offset shadow"
        );
        // A blurred red shadow under dark text: red over the white page.
        let reddish =
            |[r, g, b]: [u8; 3]| r > 200 && r > g.saturating_add(40) && r > b.saturating_add(40);
        assert!(
            count_where(&mut p, "lines", 8.0, reddish) > 100,
            "{choice:?}: both lines' shadow"
        );
        // currentcolor: the text's own green, so more green than the glyphs.
        let green = [0x43, 0xa0, 0x47];
        let shadowed = count(&mut p, "current", 6.0, green);
        let plain = count(&mut p, "none", 6.0, green);
        assert!(
            shadowed > plain,
            "{choice:?}: currentcolor shadow {shadowed} vs {plain}"
        );
        // A blurred black shadow: grey around black glyphs.
        assert!(
            count(&mut p, "blurred", 8.0, [0x99, 0x99, 0x99]) > 30,
            "{choice:?}: blur"
        );
    }
}

/// LLP 1077 D4: `box-shadow` lists, spread and `inset` (`shadows.contract`).
const SHADOW_CASES: [&str; 9] = [
    "two", "spread", "shrink", "inset", "ring", "bordered", "both", "squircle", "scheme",
];

#[test]
fn every_shadow_case_matches_chrome_light_then_dark() {
    let mut failures = Vec::new();
    for choice in painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "shadows.contract");
        failures.extend(held_to_chrome(
            &mut p,
            "shadows.web.png",
            &name,
            &SHADOW_CASES,
        ));
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(
            &mut p,
            "shadows.web-dark.png",
            &name,
            &SHADOW_CASES,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// LLP 1077 D5: `conic-gradient()` and stacked layers (`layers.contract`).
const LAYER_CASES: [&str; 9] = [
    "conic", "from", "angles", "two", "three", "bordered", "masked", "scheme", "squircle",
];

#[test]
fn every_layer_case_matches_chrome_light_then_dark() {
    let mut failures = Vec::new();
    for choice in painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "layers.contract");
        failures.extend(held_to_chrome(
            &mut p,
            "layers.web.png",
            &name,
            &LAYER_CASES,
        ));
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(
            &mut p,
            "layers.web-dark.png",
            &name,
            &LAYER_CASES,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// LLP 1077 D6–D7 (`text-paint.contract`): `background-clip` boxes held to
/// Chrome; clipped and stroked text checked by where their colours land.
#[test]
fn background_clip_and_text_stroke_paint_where_css_paints_them() {
    let boxes = ["padding", "content", "gradient-padding"];
    let mut failures = Vec::new();
    for choice in painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "text-paint.contract");
        failures.extend(held_to_chrome(&mut p, "text-paint.web.png", &name, &boxes));
        // Gradient text: red at its left, blue at its right, nothing white
        // between glyphs that a box background would have filled.
        let red = |[r, g, b]: [u8; 3]| r > 180 && g < 120 && b < 120;
        let blue = |[r, _g, b]: [u8; 3]| b > 180 && r < 120;
        assert!(
            count_where(&mut p, "gradient-text", 0.0, red) > 30,
            "{name}: gradient text's red"
        );
        assert!(
            count_where(&mut p, "gradient-text", 0.0, blue) > 30,
            "{name}: gradient text's blue"
        );
        let green = |[r, g, b]: [u8; 3]| g > 120 && r < 120 && b < 120;
        let filled = count_where(&mut p, "color-text", 0.0, green);
        assert!(
            filled > 100 && filled < 3000,
            "{name}: colour text {filled} (the box is 7000)"
        );
        // A black stroke around yellow glyphs; a hollow stroke's inside is
        // the page.
        let black = |[r, g, b]: [u8; 3]| r < 60 && g < 60 && b < 60;
        let yellow = |[r, g, b]: [u8; 3]| r > 200 && g > 180 && b < 120;
        assert!(
            count_where(&mut p, "stroke", 0.0, black) > 100,
            "{name}: stroke"
        );
        assert!(
            count_where(&mut p, "stroke", 0.0, yellow) > 100,
            "{name}: fill"
        );
        assert!(
            count_where(&mut p, "current", 0.0, red) > 60,
            "{name}: a stroke colour of its own"
        );
        assert!(
            count_where(&mut p, "hollow", 0.0, green) > 60,
            "{name}: hollow stroke"
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// LLP 1077 D8: 3D transforms (`space.contract`), the warped planes held to
/// Chrome's.
const SPACE_CASES: [&str; 9] = [
    "y", "x", "axis", "z", "origin", "flat", "hidden", "back", "nested",
];

#[test]
fn every_space_case_matches_chrome_light_then_dark() {
    let mut failures = Vec::new();
    for choice in painters() {
        let name = format!("{choice:?}");
        let mut p = boot(choice, "space.contract");
        failures.extend(held_to_chrome(&mut p, "space.web.png", &name, &SPACE_CASES));
        let id = view(&p, "dark");
        p.tap(id).unwrap();
        p.run_commands(NoData::default);
        failures.extend(held_to_chrome(
            &mut p,
            "space.web-dark.png",
            &name,
            &SPACE_CASES,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// CSS paints each inline box's shadow and then its text, box after box:
/// a later run's shadow cast back over an earlier run lands over that run's
/// glyphs, not under them (LLP 1077 D3). Two paragraphs, the same runs: in
/// `cast` the second run's shadow falls 60 points left, onto the first's
/// glyphs, which are black in `plain`; there the shadow's red must show.
#[test]
fn a_later_runs_shadow_paints_over_an_earlier_runs_glyphs() {
    const PAGE: &str = r##"
component Order
  view
    main position="relative" width=390 height=200 background-color="#ffffff"
      text testId="plain" position="absolute" left=20 top=20 width=350 height=70 font-size=40 font-weight=800 color="#000000"
        text "AAAA"
        text "B"
      text testId="cast" position="absolute" left=20 top=110 width=350 height=70 font-size=40 font-weight=800 color="#000000"
        text "AAAA"
        text "B" text-shadow="-60px 0 0 #ff0000"
"##;
    for choice in painters() {
        crate::pin_font();
        let plan = contract::compile(PAGE).unwrap();
        let assets =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/fixtures");
        let mut p =
            Presenter::boot_with(&plan.encode(), NoData, (390.0, 200.0), 1.0, assets, choice)
                .unwrap()
                .0;
        let (plain, cast) = (view(&p, "plain"), view(&p, "cast"));
        let boxes = p.boxes();
        let rect = |id| boxes.iter().find(|b| b.id == id).unwrap().rect;
        let ((px, py, w, h), (cx, cy, _, _)) = (rect(plain), rect(cast));
        let frame = p.frame();
        let rgb = |x: f32, y: f32| {
            let c = frame.pixel(x as u32, y as u32).unwrap().demultiply();
            [c.red(), c.green(), c.blue()]
        };
        let (mut glyph, mut red) = (0, 0);
        for dy in 0..h as u32 {
            for dx in 0..w as u32 {
                let [r, g, b] = rgb(px + dx as f32, py + dy as f32);
                if r > 40 || g > 40 || b > 40 {
                    continue;
                }
                glyph += 1;
                let [r, g, b] = rgb(cx + dx as f32, cy + dy as f32);
                if r > 180 && g < 80 && b < 80 {
                    red += 1;
                }
            }
        }
        assert!(
            glyph > 500,
            "{choice:?}: the runs paint ({glyph} glyph pixels)"
        );
        assert!(
            red > 100,
            "{choice:?}: {red} of {glyph} glyph pixels show the later run's shadow"
        );
    }
}
