//! Importers (LLP 1121 stage 3, commits 6–9): each pinned source under
//! `tests/import/` imports with every value mapped or reported, counted;
//! the transaction applies to a design that checks; and re-importing the
//! unchanged source proposes nothing.

use common::vocab;
use exact_tamber::import::{shadcn, Import};
use exact_tamber::{apply_all, check, parse, Op};

mod common;

const EMPTY: &str = "tamber app\n";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/import/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// The design after the import's transaction (and every offer), checked.
fn accepted(design: &str, import: &Import) -> String {
    let mut ops = import.ops.clone();
    ops.extend(import.offers.iter().flat_map(|o| o.ops.clone()));
    let text = apply_all(design, &ops, &vocab()).unwrap().text;
    check(&text, &vocab()).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    text
}

fn token_value<'a>(text: &'a str, name: &str) -> &'a str {
    text.lines()
        .find_map(|l| l.trim_start().strip_prefix(&format!("{name} = ")))
        .unwrap_or_else(|| panic!("no {name} in\n{text}"))
}

#[test]
fn shadcn_default_theme_imports_counted_and_again_proposes_nothing() {
    let css = fixture("shadcn-neutral.css");
    let first = shadcn::import(&css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    // 24 in :root (23 colours, --radius) and 23 in .dark.
    assert_eq!(
        (
            first.counted.source,
            first.counted.mapped,
            first.counted.reported
        ),
        (47, 46, 1)
    );
    let radius = first
        .unmapped
        .iter()
        .find(|u| u.name == "--radius" && u.block.ends_with(":root"))
        .unwrap();
    assert_eq!(radius.why, "kind pending (stage 6): length");
    assert_eq!(radius.token.as_deref(), Some("shadcn.radius"));
    // The derived sizes are reported with it, outside the count.
    assert_eq!(
        first
            .unmapped
            .iter()
            .filter(|u| u.block == "(derived)")
            .count(),
        4
    );
    assert_eq!(
        first
            .ops
            .iter()
            .filter(|o| matches!(o, Op::AddToken { .. }))
            .count(),
        23
    );
    let text = accepted(EMPTY, &first);
    assert_eq!(
        token_value(&text, "shadcn.background"),
        "light-dark(oklch(1 0 0), oklch(0.145 0 0))"
    );
    assert_eq!(token_value(&text, "shadcn.chart.s1"), "oklch(0.87 0 0)");
    assert_eq!(
        token_value(&text, "shadcn.border"),
        "light-dark(oklch(0.922 0 0), oklch(1 0 0 / 10%))"
    );
    assert_eq!(token_value(&text, "-exact-background"), "shadcn.background");
    let offers: Vec<&str> = first.offers.iter().map(|o| o.role.as_str()).collect();
    assert_eq!(
        offers,
        [
            "-exact-background",
            "-exact-label",
            "-exact-secondary-background",
            "-exact-separator",
            "accent-color"
        ]
    );
    assert!(first.offers[4].ops.is_empty() && first.offers[4].note.is_some());
    // Unchanged source, accepted design: nothing to propose, no offer left
    // but the accent's (a Contract edit).
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap(), &vocab()).unwrap();
    assert_eq!(again.ops, []);
    assert_eq!(
        again
            .offers
            .iter()
            .map(|o| o.role.as_str())
            .collect::<Vec<_>>(),
        ["accent-color"]
    );
}

#[test]
fn a_shadcn_colour_theme_in_hsl_channels_imports_counted() {
    let css = fixture("shadcn-blue.css");
    let import = shadcn::import(&css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    // 20 in :root (19 colours, an empty --radius) and 19 in .dark.
    assert_eq!(
        (
            import.counted.source,
            import.counted.mapped,
            import.counted.reported
        ),
        (39, 38, 1)
    );
    assert_eq!(import.unmapped[0].why, "an empty value");
    let text = accepted(EMPTY, &import);
    assert_eq!(
        token_value(&text, "shadcn.primary"),
        "light-dark(hsl(221.2 83.2% 53.3%), hsl(217.2 91.2% 59.8%))"
    );
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap(), &vocab()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn a_further_shadcn_theme_class_is_a_proposed_axis() {
    let css = fixture("shadcn-neutral-themes.css");
    let import = shadcn::import(&css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    assert_eq!(import.counted.source, 47 + 39);
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    assert_eq!(
        import.ops[0],
        Op::AddAxis {
            name: "theme".into(),
            values: vec!["default".into(), "blue".into()],
            under_dark: false,
        }
    );
    let text = accepted(EMPTY, &import);
    assert!(text.contains("axes\n  theme = default | blue\n"), "{text}");
    assert!(
        text.contains(
            "    when theme=blue: light-dark(hsl(221.2 83.2% 53.3%), hsl(217.2 91.2% 59.8%))"
        ),
        "{text}"
    );
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap(), &vocab()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn shadcn_reads_css_as_css_and_maps_or_reports_every_declaration() {
    let css = r#"
/* a comment with { and ; */
:root {
  --font: "a { b }; c";
  --background: oklch(1 0 0);
  --foreground: 222.2deg, 84%, 4.9%;
  --gap: 0.5rem;
  --space: var(--gap);
  --ring: var(--foreground);
  --loop-a: var(--loop-b);
  --loop-b: var(--loop-a);
  --chart-1: #111111;
  --chart-s1: #222222;
  --muted: #333333;
  --muted: #444444;
  @media (min-width: 40rem) { --wide: #555555; }
}
@media (prefers-color-scheme: dark) { :root { --background: oklch(0.2 0 0); } }
@media print { :root { --background: #ffffff; } }
.light, :root { --accent: #666666; }
.theme-blue, .theme-red { --primary: #0000ff; }
.theme-default { --card: #777777; }
.theme-2 { --card: #888888; }
html.dark .theme-blue { --primary: #000088; }
"#;
    let import = shadcn::import(css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    let why = |name: &str| -> Vec<String> {
        import
            .unmapped
            .iter()
            .filter(|u| u.name == name)
            .map(|u| u.why.clone())
            .collect()
    };
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    assert_eq!(import.counted.source, 20);
    assert_eq!(
        why("--font"),
        ["not a colour, and no other kind is built yet (stage 6)"]
    );
    assert_eq!(why("--gap"), ["kind pending (stage 6): length"]);
    assert_eq!(
        why("--space"),
        ["`--gap` is no colour where this block reads it"]
    );
    assert_eq!(why("--loop-a").len() + why("--loop-b").len(), 2);
    assert_eq!(
        why("--chart-s1"),
        ["its name is `--chart-1`'s (shadcn.chart.s1)"]
    );
    assert_eq!(
        why("--muted"),
        ["declared again later for the same theme block, which wins"]
    );
    assert_eq!(
        why("--wide"),
        ["inside `@media (min-width: 40rem)`, which no theme block is"]
    );
    assert_eq!(
        why("--background"),
        ["inside `@media print`, which no theme block is"]
    );
    assert_eq!(
        why("--card"),
        ["the theme `2` is no axis value (an identifier)"]
    );
    let text = accepted(EMPTY, &import);
    assert_eq!(
        token_value(&text, "shadcn.background"),
        "light-dark(oklch(1 0 0), oklch(0.2 0 0))"
    );
    assert_eq!(
        token_value(&text, "shadcn.foreground"),
        "hsl(222.2deg, 84%, 4.9%)"
    );
    assert_eq!(token_value(&text, "shadcn.ring"), "shadcn.foreground");
    assert_eq!(token_value(&text, "shadcn.muted"), "#444444");
    assert_eq!(token_value(&text, "shadcn.accent"), "#666666");
    assert_eq!(token_value(&text, "shadcn.card"), "#777777");
    assert!(text.contains("  theme = default | blue | red\n"), "{text}");
    // `--primary` only under themes: the first's (blue's, dark side too) is
    // the default, and red, light only, keeps its own line.
    assert_eq!(
        token_value(&text, "shadcn.primary"),
        "light-dark(#0000ff, #000088)"
    );
    assert!(text.contains("    when theme=red: #0000ff\n"), "{text}");
}

#[test]
fn a_when_the_source_no_longer_gives_is_taken_back_and_a_role_spelled_bare_is_set() {
    let design = "tamber app\n\naxes\n  theme = default | blue\n\ntheme\n  shadcn.background = #ffffff\n    when theme=blue: #eeeeff\n  background = #000000\n";
    let css = ":root { --background: #ffffff; }";
    let import = shadcn::import(css, "shadcn", &parse(design).unwrap(), &vocab()).unwrap();
    assert_eq!(
        import.ops,
        [Op::UnsetTokenWhen {
            name: "shadcn.background".into(),
            when: vec![("theme".into(), "blue".into())],
        }]
    );
    assert_eq!(
        import.offers[0].ops,
        [Op::SetToken {
            name: "background".into(),
            when: Vec::new(),
            value: "shadcn.background".into(),
            at: None,
            comment: None,
        }]
    );
    accepted(design, &import);
}

#[test]
fn shadcn_winners_nesting_and_recovery() {
    let css = r#"
:root { --x: #ffffff; --z: #000000; --z: 1rem; --ml: oklch(0.5
  0.1 200); --cc: currentColor; --config: {a: b}; --after: #121212; }
.dark { --x: 1rem; --y: var(--x); }
.theme-blue { &.dark { --background: #111111; } --radius: 1rem; }
.dark { --d: #111111; @media (prefers-color-scheme: dark) { --d: #222222; } }
:root, .theme-2 { --a: red; }
:root { --open: #343434;
"#;
    let import = shadcn::import(css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    let why = |name: &str| -> Vec<String> {
        import
            .unmapped
            .iter()
            .filter(|u| u.name == name)
            .map(|u| u.why.clone())
            .collect()
    };
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    assert_eq!(import.counted.source, 15);
    assert_eq!(
        why("--z"),
        [
            "declared again later for the same theme block, which wins",
            "kind pending (stage 6): length"
        ]
    );
    assert_eq!(why("--x"), ["kind pending (stage 6): length"]);
    assert_eq!(why("--y"), ["`--x` is no colour where this block reads it"]);
    assert_eq!(
        why("--cc"),
        ["not a colour, and no other kind is built yet (stage 6)"]
    );
    assert_eq!(
        why("--config"),
        ["not a colour, and no other kind is built yet (stage 6)"]
    );
    assert_eq!(
        why("--d"),
        ["declared again later for the same theme block, which wins"]
    );
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "shadcn.x"), "#ffffff");
    assert_eq!(token_value(&text, "shadcn.ml"), "oklch(0.5 0.1 200)");
    assert_eq!(token_value(&text, "shadcn.after"), "#121212");
    assert_eq!(token_value(&text, "shadcn.d"), "#222222");
    assert_eq!(token_value(&text, "shadcn.a"), "red");
    assert_eq!(token_value(&text, "shadcn.open"), "#343434");
    // Blue named only by its nested dark rule and a length: still an axis value.
    assert!(text.contains("  theme = default | blue\n"), "{text}");
    assert_eq!(token_value(&text, "shadcn.background"), "#111111");
}

#[test]
fn an_authored_when_on_another_axis_stays_and_a_role_offer_clears_its_own() {
    let design = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  shadcn.background = #ffffff\n    when density=compact: #fafafa\n  -exact-background = #000000\n    when density=compact: #111111\n";
    let css = ":root { --background: #ffffff; }";
    let import = shadcn::import(css, "shadcn", &parse(design).unwrap(), &vocab()).unwrap();
    assert_eq!(import.ops, []);
    assert_eq!(import.offers[0].ops.len(), 2);
    assert!(matches!(import.offers[0].ops[1], Op::UnsetTokenWhen { .. }));
    let text = accepted(design, &import);
    let again = shadcn::import(css, "shadcn", &parse(&text).unwrap(), &vocab()).unwrap();
    assert!(
        again.offers.iter().all(|o| o.ops.is_empty()),
        "{:?}",
        again.offers
    );
}
