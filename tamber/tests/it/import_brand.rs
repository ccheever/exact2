//! Brand offers (theme2's brand-fill ruling, 2026-10-29): an import's accent
//! offered as the packages' brand fills, each with an ink that reaches 4.5:1
//! on its fill in each scheme, measured as the checker measures a pair.

use crate::common;
use common::vocab;
use exact_tamber::import::brand::{brands, ink_on, Brand};
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

/// A shadcn import of `css` into the design `text`, and its brand offers.
fn offered(text: &str, css: &str) -> Vec<Brand> {
    let design = parse(text).unwrap();
    let import = shadcn::import(css, "shadcn", &design, &vocab()).unwrap();
    brands(&import, &design, text, &vocab(), &pairs())
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
    let translucent = offered(
        "tamber app\n",
        &theme("rgb(0 106 255 / 50%)", "rgb(0 106 255 / 50%)"),
    );
    assert_eq!(translucent.len(), 2, "{translucent:?}");
    for b in &translucent {
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
    let both = offered("tamber app\n", &theme("#006aff", "#ffd400"));
    let sets: Vec<&str> = both.iter().map(|b| b.sets.as_str()).collect();
    assert_eq!(sets, ["actions.follow-fill", "cards.empty-button"]);
    for (b, (fill, ink)) in both.iter().zip(PAIRS) {
        assert_eq!(b.token, "shadcn.primary");
        assert_eq!(b.note, None);
        assert_eq!(set(&b.ops[0]), (fill, "shadcn.primary"));
        assert_eq!(set(&b.ops[1]), (ink, "light-dark(#ffffff, #000000)"));
    }
}

#[test]
fn no_accent_no_brand_offer_and_a_pair_already_the_accent_and_its_ink_is_left_alone() {
    // Without a --primary the import carries no accent: nothing here, even
    // with an AccentColor of the design's own naming another token.
    let css = ":root {\n  --background: #ffffff;\n}\n";
    assert!(offered("tamber app\n", css).is_empty());
    let own = "tamber app\n\ntheme\n  brand = #006aff\n  AccentColor = brand\n";
    assert!(offered(own, css).is_empty(), "{:?}", offered(own, css));
    // FollowButton's fill and ink are already the accent and white: only EmptyState's.
    let set_both = "tamber app\n\ntheme\n  actions.follow-fill = shadcn.primary\n  actions.follow-ink = #ffffff\n";
    let sets: Vec<String> = offered(set_both, &theme("#006aff", "#006aff"))
        .into_iter()
        .map(|b| b.sets)
        .collect();
    assert_eq!(sets, ["cards.empty-button"]);
    // The fill already the accent but a yellow one now: its black ink is offered.
    let again = offered(set_both, &theme("#ffd400", "#ffd400"));
    let follow = again
        .iter()
        .find(|b| b.sets == "actions.follow-fill")
        .unwrap();
    assert_eq!(set(&follow.ops[1]), ("actions.follow-ink", "#000000"));
}

#[test]
fn with_the_accent_override_already_in_the_design_the_brand_fills_are_still_offered() {
    // A re-import after AccentColor = shadcn.primary was accepted: no AccentColor offer is left to make.
    let text = "tamber app\n\ntheme\n  shadcn.primary = #006aff\n  AccentColor = shadcn.primary\n";
    let design = parse(text).unwrap();
    let import = shadcn::import(&theme("#006aff", "#006aff"), "shadcn", &design, &vocab()).unwrap();
    assert!(!import
        .offers
        .iter()
        .any(|o| o.role == "AccentColor" && !o.ops.is_empty()));
    let brands = brands(&import, &design, text, &vocab(), &pairs());
    assert_eq!(brands.len(), 2, "{brands:?}");
    assert!(brands.iter().all(|b| b.token == "shadcn.primary"));
}

#[test]
fn an_accent_naming_another_token_is_read_through_it() {
    // shadcn keeps `--primary: var(--brand)` as a reference to its own token.
    let css = ":root {\n  --brand: #ffd400;\n  --primary: var(--brand);\n}\n.dark {\n  --brand: #ffd400;\n  --primary: var(--brand);\n}\n";
    let both = offered("tamber app\n", css);
    assert_eq!(both.len(), 2, "{both:?}");
    assert_eq!(set(&both[0].ops[1]), ("actions.follow-ink", "#000000"));
}

#[test]
fn an_ink_the_app_varies_by_mode_is_one_value_in_every_mode() {
    // The app's own ink keeps a compact-mode line: the offer takes it out, so
    // the ratio it measured is the one every mode shows.
    let text = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  actions.follow-ink = #ffffff\n    when density=compact: #000000\n";
    let both = offered(text, &theme("#006aff", "#006aff"));
    let follow = both
        .iter()
        .find(|b| b.sets == "actions.follow-fill")
        .unwrap();
    assert!(
        follow.ops.iter().any(
            |op| matches!(op, Op::UnsetTokenWhen { name, .. } if name == "actions.follow-ink")
        ),
        "{:?}",
        follow.ops
    );
}

#[test]
fn a_dial_the_accent_override_or_a_brand_line_alone_reads_is_left_as_it_is() {
    // AccentColor alone reads the dial `brand`, so its override isn't offered,
    // but the imported accent is a fill of its own: the brand fills still are.
    let dialed = "tamber app\n\ndials\n  brand: color = #111111\n\ntheme\n  AccentColor = brand\n";
    let both = offered(dialed, &theme("#006aff", "#006aff"));
    assert_eq!(both.len(), 2, "{both:?}");
    // FollowButton's own fill line alone reads a dial: setting it over would
    // leave the dial unused, so only EmptyState's is offered.
    let own = "tamber app\n\ndials\n  pop: color = #ff0000\n\ntheme\n  actions.follow-fill = pop\n";
    let sets: Vec<String> = offered(own, &theme("#006aff", "#006aff"))
        .into_iter()
        .map(|b| b.sets)
        .collect();
    assert_eq!(sets, ["cards.empty-button"]);
}

#[test]
fn a_design_that_checks_only_with_its_packages_still_gets_its_brand_fills() {
    // `page.fill` names a package token, so the design alone doesn't check: the
    // accent's own value is measured instead.
    let packaged = "tamber app\n\ntheme\n  page.fill = actions.follow-fill\n";
    let both = offered(packaged, &theme("#ffd400", "#ffd400"));
    assert_eq!(both.len(), 2, "{both:?}");
    assert_eq!(set(&both[0].ops[1]), ("actions.follow-ink", "#000000"));
}
