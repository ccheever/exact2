//! Brand offers (theme2's brand-fill ruling, 2026-10-29): an import's accent
//! offered as the packages' brand fills, each with an ink that reaches 4.5:1
//! on its fill in each scheme, measured as the checker measures a pair.

use crate::common;
use common::vocab;
use exact_tamber::import::brand::{brands, ink_on};
use exact_tamber::import::shadcn;
use exact_tamber::{parse, ratio, Op};

const PAIRS: [(&str, &str); 2] = [
    ("actions.follow-fill", "actions.follow-ink"),
    ("cards.empty-button", "cards.empty-button-ink"),
];

fn pairs() -> Vec<(String, String)> {
    PAIRS
        .iter()
        .map(|(f, i)| (f.to_string(), i.to_string()))
        .collect()
}

/// A shadcn theme whose `--primary` is `light` in :root and `dark` in .dark.
fn theme(light: &str, dark: &str) -> String {
    format!(":root {{\n  --background: #ffffff;\n  --primary: {light};\n}}\n.dark {{\n  --background: #000000;\n  --primary: {dark};\n}}\n")
}

fn set(op: &Op) -> (&str, &str) {
    match op {
        Op::SetToken { name, value, .. } => (name.as_str(), value.as_str()),
        other => panic!("not set-token: {other:?}"),
    }
}

#[test]
fn a_blue_accent_gets_white_ink_and_a_yellow_one_black_each_reaching_4_5() {
    for (fill, ink) in [("#006aff", "#ffffff"), ("#ffd400", "#000000")] {
        let got = ink_on(fill, &vocab()).unwrap();
        assert_eq!(got.value, ink, "{fill}");
        assert_eq!(got.below, [false, false], "{fill}");
        let r = ratio(ink, fill).unwrap();
        assert!(r >= 4.5, "{fill}: {r}");
        assert_eq!(got.contrast[0], Some((r * 100.0).floor() / 100.0));
    }
}

#[test]
fn a_light_dark_accent_gets_its_ink_per_scheme() {
    // Near black in light, near white in dark (shadcn's neutral primary).
    let got = ink_on("light-dark(#171717, #e5e5e5)", &vocab()).unwrap();
    assert_eq!(got.value, "light-dark(#ffffff, #000000)");
    assert!(got.contrast.iter().all(|r| r.unwrap() >= 4.5), "{got:?}");
}

#[test]
fn a_mid_tone_accent_still_reaches_4_5_and_a_translucent_one_is_offered_with_a_warning() {
    // White and black ratios on an opaque fill multiply to 21: the higher is
    // at least 4.58:1, even at the middle grey where they cross.
    let mid = ink_on("#757575", &vocab()).unwrap();
    assert!(mid.contrast[0].unwrap() >= 4.58, "{mid:?}");
    assert_eq!(mid.below, [false, false]);
    // A fill that isn't opaque has no one ratio: offered, marked, never ticked.
    let css = theme("rgb(0 106 255 / 50%)", "rgb(0 106 255 / 50%)");
    let design = parse("tamber app\n").unwrap();
    let import = shadcn::import(&css, "shadcn", &design, &vocab()).unwrap();
    let offered = brands(&import, &design, &vocab(), &pairs());
    assert_eq!(offered.len(), 2, "{offered:?}");
    for b in &offered {
        assert_eq!(b.contrast, [None, None]);
        let note = b.note.as_deref().unwrap();
        assert!(
            note.contains("not opaque") && note.contains("4.5:1"),
            "{note}"
        );
    }
}

#[test]
fn a_shadcn_import_offers_both_brand_fills_set_to_its_accent_with_their_inks() {
    let css = theme("#006aff", "#ffd400");
    let design = parse("tamber app\n").unwrap();
    let import = shadcn::import(&css, "shadcn", &design, &vocab()).unwrap();
    let offered = brands(&import, &design, &vocab(), &pairs());
    let sets: Vec<&str> = offered.iter().map(|b| b.sets.as_str()).collect();
    assert_eq!(sets, ["actions.follow-fill", "cards.empty-button"]);
    for (b, (fill, ink)) in offered.iter().zip(PAIRS) {
        assert_eq!(b.token, "shadcn.primary");
        assert_eq!(b.note, None);
        assert_eq!(set(&b.ops[0]), (fill, "shadcn.primary"));
        assert_eq!(set(&b.ops[1]), (ink, "light-dark(#ffffff, #000000)"));
    }
}

#[test]
fn no_accent_no_brand_offer_and_a_fill_already_the_accent_is_left_alone() {
    // Without a --primary there is no AccentColor offer, so nothing here.
    let css = ":root {\n  --background: #ffffff;\n}\n";
    let design = parse("tamber app\n").unwrap();
    let import = shadcn::import(css, "shadcn", &design, &vocab()).unwrap();
    assert!(brands(&import, &design, &vocab(), &pairs()).is_empty());
    // The app already sets FollowButton's fill to the accent: only EmptyState's.
    let design = parse("tamber app\n\ntheme\n  actions.follow-fill = shadcn.primary\n").unwrap();
    let import = shadcn::import(&theme("#006aff", "#006aff"), "shadcn", &design, &vocab()).unwrap();
    let sets: Vec<String> = brands(&import, &design, &vocab(), &pairs())
        .into_iter()
        .map(|b| b.sets)
        .collect();
    assert_eq!(sets, ["cards.empty-button"]);
}

#[test]
fn with_the_accent_override_already_in_the_design_the_brand_fills_are_still_offered() {
    // A re-import after AccentColor = shadcn.primary was accepted: no AccentColor offer is left to make.
    let design =
        parse("tamber app\n\ntheme\n  shadcn.primary = #006aff\n  AccentColor = shadcn.primary\n")
            .unwrap();
    let import = shadcn::import(&theme("#006aff", "#006aff"), "shadcn", &design, &vocab()).unwrap();
    assert!(!import
        .offers
        .iter()
        .any(|o| o.role == "AccentColor" && !o.ops.is_empty()));
    let offered = brands(&import, &design, &vocab(), &pairs());
    assert_eq!(offered.len(), 2, "{offered:?}");
    assert!(offered.iter().all(|b| b.token == "shadcn.primary"));
}
