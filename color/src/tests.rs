use super::*;

fn wide(s: &str) -> Wide {
    match parse(s) {
        Some(Parsed::Wide(w)) => w,
        other => panic!("{s}: {other:?}"),
    }
}
fn close(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tolerance)
}

/// RGB → XYZ from CIE xy primaries and white.
fn to_xyz(p: [(f64, f64); 3], w: (f64, f64)) -> M3 {
    let col = |(x, y): (f64, f64)| [x / y, 1.0, (1.0 - x - y) / y];
    let m: M3 = [0, 1, 2].map(|r| [col(p[0])[r], col(p[1])[r], col(p[2])[r]]);
    let det = |m: &M3| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let wv = col(w);
    // Cramer's rule for m · s = white.
    let s = [0, 1, 2].map(|k| {
        let mut mk = m;
        for r in 0..3 {
            mk[r][k] = wv[r];
        }
        det(&mk) / det(&m)
    });
    m.map(|r| [r[0] * s[0], r[1] * s[1], r[2] * s[2]])
}

#[test]
fn the_matrices_are_the_standards_primaries() {
    let d65 = (0.3127, 0.3290);
    let d50 = (0.3457, 0.3585);
    for (m, p, w) in [
        (
            P3_TO_XYZ,
            [(0.680, 0.320), (0.265, 0.690), (0.150, 0.060)],
            d65,
        ),
        (A98_TO_XYZ, [(0.64, 0.33), (0.21, 0.71), (0.15, 0.06)], d65),
        (
            REC2020_TO_XYZ,
            [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046)],
            d65,
        ),
        (
            PROPHOTO_TO_XYZ_D50,
            [
                (0.734699, 0.265301),
                (0.159597, 0.840403),
                (0.036598, 0.000105),
            ],
            d50,
        ),
    ] {
        let want = to_xyz(p, w);
        for r in 0..3 {
            assert!(close(m[r], want[r], 2e-4), "{:?} vs {:?}", m[r], want[r]);
        }
    }
}

#[test]
fn every_form_parses_into_its_own_space_and_serialises() {
    assert_eq!(
        parse("red"),
        Some(Parsed::Legacy(Rgba8 {
            r: 255,
            g: 0,
            b: 0,
            a: 1.0
        }))
    );
    assert_eq!(
        parse("hsl(120 100% 50%)"),
        Some(Parsed::Legacy(Rgba8 {
            r: 0,
            g: 255,
            b: 0,
            a: 1.0
        }))
    );
    assert_eq!(parse("currentColor"), Some(Parsed::Current));
    for (text, space, c) in [
        ("color(display-p3 1 0 0)", Space::DisplayP3, [1.0, 0.0, 0.0]),
        (
            "color(display-p3-linear 0.5 0.5 0.5)",
            Space::DisplayP3Linear,
            [0.5; 3],
        ),
        ("color(a98-rgb 0 1 0)", Space::A98Rgb, [0.0, 1.0, 0.0]),
        (
            "color(prophoto-rgb 50% 0 0)",
            Space::ProphotoRgb,
            [0.5, 0.0, 0.0],
        ),
        ("color(rec2020 0 0 1)", Space::Rec2020, [0.0, 0.0, 1.0]),
        ("color(xyz 0.95 1 1.09)", Space::XyzD65, [0.95, 1.0, 1.09]),
        (
            "color(xyz-d50 0.96 1 0.82)",
            Space::XyzD50,
            [0.96, 1.0, 0.82],
        ),
        (
            "color(rec2100-pq 0.75 0.75 0.75)",
            Space::Rec2100Pq,
            [0.75; 3],
        ),
        ("lab(50% 40 59.5)", Space::Lab, [50.0, 40.0, 59.5]),
        ("lch(50 30 400deg)", Space::Lch, [50.0, 30.0, 40.0]),
        ("oklab(0.5 -0.1 0.1)", Space::Oklab, [0.5, -0.1, 0.1]),
        ("oklch(70% 0.1 0.5turn)", Space::Oklch, [0.7, 0.1, 180.0]),
    ] {
        let w = wide(text);
        assert_eq!(w.space, space, "{text}");
        assert!(close(w.c, c, 1e-9), "{text}: {:?}", w.c);
    }
    for bad in [
        "color(display-p3 1, 0, 0)",
        "color(adobe 1 0 0)",
        "lab(50, 0, 0)",
        "oklch(0.5 0.1 10%)",
        "color(srgb 1 0)",
    ] {
        assert_eq!(parse(bad), None, "{bad}");
    }
    assert_eq!(
        wide("color(display-p3 100% 0 0)").css(),
        "color(display-p3 1 0 0)"
    );
    assert_eq!(
        wide("oklch(0.7 0.1 200 / 50%)").css(),
        "oklch(0.7 0.1 200 / 0.5)"
    );
    assert_eq!(wide("lab(50% 40 59.5)").css(), "lab(50 40 59.5)");
    assert_eq!(wide("color(xyz 0 0 0)").css(), "color(xyz-d65 0 0 0)");
}

#[test]
fn the_arithmetic_is_css_colour_4s() {
    // CSS Color 4's sample code: P3 red in linear sRGB.
    assert!(close(
        wide("color(display-p3 1 0 0)").linear_srgb(),
        [1.2249, -0.0421, -0.0196],
        2e-4
    ));
    // sRGB white in every space comes back as white.
    for text in [
        "color(srgb 1 1 1)",
        "color(display-p3 1 1 1)",
        "color(a98-rgb 1 1 1)",
        "color(rec2020 1 1 1)",
        "color(prophoto-rgb 1 1 1)",
        "lab(100 0 0)",
        "oklab(1 0 0)",
        "oklch(1 0 0)",
    ] {
        assert!(
            close(wide(text).linear_srgb(), [1.0; 3], 2e-3),
            "{text}: {:?}",
            wide(text).linear_srgb()
        );
    }
    // PQ's 203 cd/m² is SDR white.
    let pq203 = 0.58069;
    assert!(close(
        wide(&format!("color(rec2100-pq {pq203} {pq203} {pq203})")).linear_srgb(),
        [1.0; 3],
        2e-3
    ));
    assert!((pq_nits(0.75) - 983.0).abs() < 2.0);
    assert!(close(
        wide("color(rec2100-hlg 0.75 0.75 0.75)").linear_srgb(),
        [1.0; 3],
        1e-6
    ));
    assert!(close(
        wide("color(rec2100-linear 4 4 4)").linear_srgb(),
        [4.0; 3],
        1e-9
    ));
}

#[test]
fn srgb_containment_and_the_clipped_fallback() {
    assert!(wide("color(display-p3 0.5 0.5 0.5)").in_srgb());
    assert!(!wide("color(display-p3 1 0 0)").in_srgb());
    assert!(!wide("color(rec2100-linear 2 2 2)").in_srgb());
    assert!(wide("oklch(0.7 0.05 200)").in_srgb());
    assert_eq!(
        wide("color(display-p3 1 0 0)").srgb8(),
        Rgba8 {
            r: 255,
            g: 0,
            b: 0,
            a: 1.0
        }
    );
    assert_eq!(
        wide("color(srgb 1 0 0.5 / 0.5)").srgb8(),
        Rgba8 {
            r: 255,
            g: 0,
            b: 128,
            a: 128.0 / 255.0
        }
    );
}

fn srgb(r: f64, g: f64, b: f64, alpha: f64) -> Wide {
    Wide {
        space: Space::Srgb,
        c: [r, g, b],
        alpha,
    }
}

#[test]
fn every_interpolation_space_round_trips() {
    use crate::mix::MixSpace::*;
    let samples = [
        [0.2, 0.5, 0.8],
        [1.0, 0.0, 0.0],
        [0.05, 0.9, 0.3],
        [1.2, -0.04, -0.02],
    ];
    for space in [
        Srgb,
        SrgbLinear,
        DisplayP3,
        DisplayP3Linear,
        A98Rgb,
        ProphotoRgb,
        Rec2020,
        Lab,
        Oklab,
        XyzD50,
        XyzD65,
        Hsl,
        Hwb,
        Lch,
        Oklch,
    ] {
        for v in samples {
            if matches!(space, Hsl | Hwb) && v.iter().any(|c| *c < 0.0 || *c > 1.0) {
                continue; // HSL and HWB are defined over sRGB's gamut
            }
            let back = space.to_linear_srgb(space.from_linear_srgb(v));
            assert!(close(back, v, 1e-6), "{space:?}: {v:?} -> {back:?}");
        }
    }
}

#[test]
fn interpolation_is_premultiplied_and_in_its_space() {
    let red = srgb(1.0, 0.0, 0.0, 1.0);
    let blue = srgb(0.0, 0.0, 1.0, 1.0);
    let legacy = Interpolation {
        space: crate::mix::MixSpace::Srgb,
        hue: HueMethod::Shorter,
    };
    // In gamma-encoded sRGB the midpoint is (0.5, 0, 0.5) encoded.
    let (mid, a) = mix(&red, &blue, 0.5, legacy);
    assert!(close(mid.map(linear_to_srgb), [0.5, 0.0, 0.5], 1e-9) && a == 1.0);
    // Premultiplied: red towards transparent black stays red, half as opaque.
    let (half, a) = mix(&red, &srgb(0.0, 0.0, 0.0, 0.0), 0.5, legacy);
    assert!(
        close(half.map(linear_to_srgb), [1.0, 0.0, 0.0], 1e-9),
        "{half:?}"
    );
    assert!((a - 0.5).abs() < 1e-12);
    // In oklab the midpoint is lighter than sRGB's dark purple.
    let (ok, _) = mix(&red, &blue, 0.5, Interpolation::OKLAB);
    assert!(
        crate::mix::MixSpace::Oklab.from_linear_srgb(ok)[0]
            > crate::mix::MixSpace::Oklab.from_linear_srgb(mid)[0]
    );
}

#[test]
fn hue_methods_go_round_as_css_says() {
    let how = |hue| Interpolation {
        space: crate::mix::MixSpace::Oklch,
        hue,
    };
    let at = |h: f64| Wide {
        space: Space::Oklch,
        c: [0.7, 0.1, h],
        alpha: 1.0,
    };
    let hue = |v: [f64; 3]| crate::mix::MixSpace::Oklch.from_linear_srgb(v)[2];
    let (shorter, _) = mix(&at(10.0), &at(350.0), 0.5, how(HueMethod::Shorter));
    assert!(
        hue(shorter) < 1.0 || hue(shorter) > 359.0,
        "through 0: {}",
        hue(shorter)
    );
    let (longer, _) = mix(&at(10.0), &at(350.0), 0.5, how(HueMethod::Longer));
    assert!(
        (hue(longer) - 180.0).abs() < 0.5,
        "through 180: {}",
        hue(longer)
    );
    let (increasing, _) = mix(&at(350.0), &at(10.0), 0.5, how(HueMethod::Increasing));
    assert!(hue(increasing) < 1.0 || hue(increasing) > 359.0);
    let (decreasing, _) = mix(&at(350.0), &at(10.0), 0.5, how(HueMethod::Decreasing));
    assert!((hue(decreasing) - 180.0).abs() < 0.5);
    // A gray has no hue: white to blue keeps blue's hue all the way.
    let blue = Wide {
        space: Space::Oklch,
        c: [0.45, 0.3, 264.0],
        alpha: 1.0,
    };
    let white = Wide {
        space: Space::Oklch,
        c: [1.0, 0.0, 0.0],
        alpha: 1.0,
    };
    let (m, _) = mix(&white, &blue, 0.5, how(HueMethod::Shorter));
    assert!((hue(m) - 264.0).abs() < 0.5, "{}", hue(m));
}

#[test]
fn interpolation_methods_parse_and_serialise() {
    let (how, n) = Interpolation::parse(&["in", "oklch", "longer", "hue", "45deg"]).unwrap();
    assert_eq!((how.css().as_str(), n), ("in oklch longer hue", 4));
    let (how, n) = Interpolation::parse(&["in", "display-p3"]).unwrap();
    assert_eq!((how.css().as_str(), n), ("in display-p3", 2));
    assert_eq!(
        Interpolation::parse(&["in", "srgb", "longer", "hue"]).map(|(_, n)| n),
        Some(2),
        "no hue method in a rectangular space"
    );
    assert!(Interpolation::parse(&["in", "cmyk"]).is_none());
    assert!(Interpolation::parse(&["to", "right"]).is_none());
}

#[test]
fn display_p3_bytes_are_css_conversion_clipped() {
    let p3 = |s: &str| match parse(s) {
        Some(Parsed::Wide(w)) => {
            let c = w.display_p3_8();
            [c.r, c.g, c.b]
        }
        Some(Parsed::Legacy(c)) => {
            let c = srgb8_to_p3(c);
            [c.r, c.g, c.b]
        }
        _ => panic!("{s}"),
    };
    // sRGB red is inside P3: color(display-p3 0.9175 0.2003 0.1386).
    assert_eq!(p3("red"), [234, 51, 35]);
    assert_eq!(p3("white"), [255, 255, 255]);
    assert_eq!(p3("color(display-p3 1 0 0)"), [255, 0, 0]);
    assert_eq!(p3("color(display-p3 0.2 0.4 0.6)"), [51, 102, 153]);
    // Outside P3 clips.
    assert_eq!(p3("color(rec2020 0 1 0)")[1], 255);
    // And back: P3 red is outside sRGB, so it clips to sRGB red.
    let back = p3_8_to_srgb(Rgba8 {
        r: 255,
        g: 0,
        b: 0,
        a: 1.0,
    });
    assert_eq!([back.r, back.g, back.b], [255, 0, 0]);
    // Greys are the same bytes in both (one white, one transfer).
    let grey = p3_8_to_srgb(srgb8_to_p3(Rgba8 {
        r: 99,
        g: 99,
        b: 99,
        a: 0.5,
    }));
    assert_eq!(
        [grey.r, grey.g, grey.b, (grey.a * 255.0).round() as u8],
        [99, 99, 99, 128]
    );
}
