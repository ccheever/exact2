//! Hand-written expected values (theme1's review of ed747eeaa): each fold
//! checked against a value worked out by hand from CSS Color 4/5, not a
//! blessed golden.

use exact_tamber::check;

mod common;

/// The folded text and fallback of `t = <value>`.
fn fold(value: &str) -> (String, String) {
    let src = format!("tamber t\n\ntheme\n  t = {value}\n");
    let f = check(&src, &common::vocab()).unwrap_or_else(|e| panic!("{value}: {e:?}"));
    let ir = f.ir();
    let fallback = ir
        .split("\"fallback\": [\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_string();
    (f.value("t", 0).unwrap().css(), fallback)
}

fn refused(value: &str) -> String {
    let src = format!("tamber t\n\ntheme\n  t = {value}\n");
    match check(&src, &common::vocab()) {
        Ok(_) => panic!("{value} folded"),
        Err(e) => e[0].id.to_string(),
    }
}

#[test]
fn mixes() {
    // Even red and blue in sRGB: each channel the mean.
    assert_eq!(
        fold("color-mix(in srgb, #ff0000, #0000ff)"),
        ("color(srgb 0.5 0 0.5)".into(), "#800080ff".into())
    );
    // No method is oklab (CSS Color 5): the same as writing it.
    assert_eq!(
        fold("color-mix(#ff0000, #0000ff)"),
        fold("color-mix(in oklab, #ff0000, #0000ff)")
    );
    // Three colours, equal shares: a third each.
    assert_eq!(
        fold("color-mix(in srgb, #ff0000, #00ff00, #0000ff)").1,
        "#555555ff"
    );
    // 20% + 30%: weights 0.4/0.6, alpha 0.5.
    assert_eq!(
        fold("color-mix(in srgb, #000 20%, #fff 30%)").0,
        "color(srgb 0.6 0.6 0.6 / 0.5)"
    );
    // 0% and 0%: an even mix at alpha 0.
    assert_eq!(
        fold("color-mix(in srgb, red 0%, blue 0%)").0,
        "color(srgb 0.5 0 0.5 / 0)"
    );
    // One colour: itself, in the mix's space.
    assert_eq!(fold("color-mix(in srgb, #ff0000)").0, "color(srgb 1 0 0)");
    // All 0% in three: even steps, so the last colour is half (CSS Color 5).
    assert_eq!(
        fold("color-mix(in srgb, red 0%, lime 0%, blue 0%)").0,
        "color(srgb 0.25 0.25 0.5 / 0)"
    );
    // A running mix keeps its hue though the first two are transparent:
    // 30 and 90 make 60, then a third of the way to 120 is 80.
    let (hue, _) = fold(
        "color-mix(in oklch, oklch(0.7 0.1 30 / 0), oklch(0.7 0.1 90 / 0), oklch(0.7 0.1 120))",
    );
    let h: f64 = hue
        .split(" / ")
        .next()
        .unwrap()
        .rsplit(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((h - 80.0).abs() < 1e-9, "{hue}");
    // Over 100% scales down: 60/60 is even.
    assert_eq!(
        fold("color-mix(in srgb, #ff0000 60%, #0000ff 60%)").0,
        "color(srgb 0.5 0 0.5)"
    );
    assert_eq!(
        refused("color-mix(in srgb, red 120%, blue)"),
        "tamber-syntax"
    );
}

#[test]
fn absolute_and_relative() {
    assert_eq!(
        fold("rgb(calc(50%) 0 0)"),
        ("#800000ff".into(), "#800000ff".into())
    );
    assert_eq!(
        fold("color(from #ff0000 srgb r g b)").0,
        "color(srgb 1 0 0)"
    );
    assert_eq!(
        fold("rgb(from #ff0000 calc(r / 2) g b)").0,
        "color(srgb 0.5 0 0)"
    );
    assert_eq!(refused("oklch(from #ff0000 l c 50%)"), "tamber-syntax");
    // A relative colour into an HDR space, and back: sRGB red's own
    // components through Rec. 2100 linear are the BT.2020 matrix's.
    let (hdr, _) = fold("color(from #ff0000 rec2100-linear r g b)");
    assert!(hdr.starts_with("color(rec2100-linear 0.627"), "{hdr}");
    assert_eq!(
        fold("color(from color(rec2100-pq 0.5 0.5 0.5) rec2100-pq r g b)").0,
        "color(rec2100-pq 0.5 0.5 0.5)"
    );
}

#[test]
fn platform_colours() {
    assert_eq!(
        fold("-exact-platform-color(ios systemTealColor, light-dark(#30b0c7, #40c8e0))").1,
        "#30b0c7ff"
    );
    for bad in [
        "-exact-platform-color()",
        "-exact-platform-color(#000)",
        "-exact-platform-color(ios teal, #000)",
        "-exact-platform-color(android x, #000)",
        "-exact-platform-color(ios systemTealColor, oklch(0.7 0.1 200))",
    ] {
        refused(bad);
    }
}

#[test]
fn sixty_four_tuples_are_accepted_and_sixty_five_are_not() {
    let axes = |n: usize| {
        (0..n)
            .map(|i| format!("  a{i} = x | y\n"))
            .collect::<String>()
    };
    let ok = format!("tamber t\n\naxes\n{}\ntheme\n  t = #000\n", axes(6));
    assert_eq!(check(&ok, &common::vocab()).unwrap().tuples, 64);
    let over = format!(
        "tamber t\n\naxes\n{}  b = x | y | z\n\ntheme\n  t = #000\n",
        axes(5)
    );
    assert_eq!(
        check(&over, &common::vocab()).unwrap_err()[0].id,
        "tamber-tuples"
    );
}
