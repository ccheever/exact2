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
            "AccentColor"
        ]
    );
    // The accent is the theme's tint, a role override like the others,
    // accepted in the same transaction as the tokens it names.
    assert_eq!(token_value(&text, "AccentColor"), "shadcn.primary");
    // Unchanged source, accepted design: nothing to propose, no offer left.
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap(), &vocab()).unwrap();
    assert_eq!(again.ops, []);
    assert_eq!(again.offers, []);
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
    // The blue theme sets all five overrides' tokens, and a role has one
    // value: each is offered with no operations, and why.
    assert_eq!(import.offers.len(), 5);
    assert!(
        import.offers.iter().all(|o| o.ops.is_empty()
            && o.note
                .as_deref()
                .is_some_and(|n| n.contains("varies by an app axis"))),
        "{:?}",
        import.offers
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
    // `--x` is a length on the dark side: no token, rather than white there.
    assert_eq!(
        why("--x"),
        [
            "`--x` is no colour in `.dark`, where it is set too, so it makes no token",
            "kind pending (stage 6): length"
        ]
    );
    assert_eq!(why("--y"), ["`--x` is no colour where this block reads it"]);
    assert_eq!(
        why("--cc"),
        ["a colour Tamber does not fold yet: `currentcolor` is the element's own colour, which no token can hold"]
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
    assert!(!text.contains("shadcn.x ="), "{text}");
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
    let design = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  shadcn.background = #ffffff\n  -exact-background = #000000\n    when density=compact: #111111\n";
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

/// A role has one value (LLP 1121 D5), judged on the design as accepted:
/// a `when` the design keeps on its own axis makes the token vary, and any
/// role the design has that would then vary is named, with no operations
/// (removing it could break what reads its colour).
#[test]
fn a_role_whose_token_varies_once_accepted_is_not_offered() {
    let design = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  shadcn.background = #ffffff\n    when density=compact: #fafafa\n  -exact-background = #000000\n";
    let css = ":root { --background: #ffffff; }";
    let import = shadcn::import(css, "shadcn", &parse(design).unwrap(), &vocab()).unwrap();
    assert_eq!(import.ops, []);
    assert!(import.offers[0].ops.is_empty());
    assert!(import.offers[0]
        .note
        .as_deref()
        .unwrap()
        .starts_with("not offered"));
    // Roles bound to the token, under any spelling, and one with `when`
    // lines on the axis the import widens.
    let design = "tamber app\n\naxes\n  theme = old\n\ntheme\n  shadcn.primary = #ff0000\n  AccentColor = shadcn.primary\n  -exact-fill = shadcn.primary\n  -exact-label = shadcn.primary\n  Canvas = label\n  CanvasText = #000000\n    when theme=old: #111111\n";
    let css = ":root { --primary: #ff0000; } .theme-blue { --primary: #0000ff; }";
    let import = shadcn::import(css, "shadcn", &parse(design).unwrap(), &vocab()).unwrap();
    assert!(
        import.offers.iter().all(|o| o.ops.is_empty()),
        "{:?}",
        import.offers
    );
    let named: Vec<&str> = import.offers[1..].iter().map(|o| o.role.as_str()).collect();
    assert_eq!(
        named,
        [
            "AccentColor",
            "-exact-fill",
            "-exact-label",
            "Canvas",
            "CanvasText"
        ]
    );
    assert!(import.offers[1..].iter().all(|o| o
        .note
        .as_deref()
        .unwrap()
        .contains("tamber-role-axis")));
}

/// A role chained to one shadcn rebinds, with its own `when` line, could
/// split once accepted: it is named, with no operations.
#[test]
fn shadcn_names_a_design_role_chained_to_a_rebound_role() {
    let design = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  AccentColor = #111111\n  Canvas = AccentColor\n    when density=compact: #111111\n";
    let css = ":root { --primary: #333333; }";
    let import = shadcn::import(css, "shadcn", &parse(design).unwrap(), &vocab()).unwrap();
    let offers: Vec<(&str, bool)> = import
        .offers
        .iter()
        .map(|o| (o.role.as_str(), o.ops.is_empty()))
        .collect();
    assert_eq!(offers, [("AccentColor", false), ("Canvas", true)]);
}

#[test]
fn shadcn_references_fold_as_css_reads_them_and_restricted_rules_are_reported() {
    let css = r#"
:root { --a: red; --b: var(--a); --r: #ff0000; --background: white; &:hover { --background: red; } }
.dark { --a: var(--b); --b: blue; --r: #0000ff; }
.theme-blue { --r: #00ff00; }
.dark .theme-blue { --ref: var(--r); }
.card { --muted: #999999; }
:root { --config: [a; --fake: red; ]; --after: #010101; --bad: "unclosed
  ; --next: #020202; }
:is(.theme-blue, .x) { --ignored: #030303; }
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
    assert!(why("--fake").is_empty() && import.unmapped.iter().all(|u| u.name != "--fake"));
    assert!(why("--background")[0].contains("some elements only"));
    assert!(why("--muted")[0].contains("some elements only"));
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "shadcn.background"), "white");
    assert_eq!(token_value(&text, "shadcn.after"), "#010101");
    assert_eq!(token_value(&text, "shadcn.next"), "#020202");
    // A cycle of tokens is written as the colours it resolves to.
    assert_eq!(token_value(&text, "shadcn.a"), "light-dark(red, blue)");
    // Blue's dark side reads the theme's light --r, as CSS does: the
    // token's blue line is that one colour, and --ref names it.
    assert!(
        text.contains("  shadcn.r = light-dark(#ff0000, #0000ff)\n    when theme=blue: #00ff00\n"),
        "{text}"
    );
    assert_eq!(token_value(&text, "shadcn.ref"), "shadcn.r");
}

#[test]
fn a_theme_axis_past_the_tuples_refuses_the_import() {
    let values: Vec<String> = (0..32).map(|i| format!("d{i}")).collect();
    let design = format!(
        "tamber app\n\naxes\n  density = {}\n  theme = blue | green\n\ntheme\n  ink = #000000\n",
        values.join(" | ")
    );
    let css = ".theme-blue { --background: #0000ff; }";
    let refused = shadcn::import(css, "shadcn", &parse(&design).unwrap(), &vocab()).unwrap_err();
    assert_eq!(refused[0].id, "tamber-import-tuples");
}

#[test]
fn tailwinds_default_theme_imports_counted_and_again_proposes_nothing() {
    use exact_tamber::import::tailwind;
    let css = fixture("tailwind-theme.css");
    let first = tailwind::import(&css, "tw", &parse(EMPTY).unwrap()).unwrap();
    // 419 variables: the 288 colours mapped (13 with a `none` component,
    // written as the zero CSS reads it as); every other kind pending or
    // reported.
    assert_eq!((first.counted.source, first.counted.mapped), (419, 288));
    assert_eq!(first.counted.reported, 131);
    let why = |name: &str| {
        first
            .unmapped
            .iter()
            .find(|u| u.name == name)
            .map(|u| (u.why.clone(), u.token.clone()))
            .unwrap()
    };
    assert_eq!(
        why("--spacing"),
        (
            "kind pending (stage 6): length".into(),
            Some("tw.space.base".into())
        )
    );
    assert_eq!(
        why("--text-xs--line-height"),
        (
            "kind pending (stage 6): type".into(),
            Some("tw.type.xs".into())
        )
    );
    assert_eq!(
        why("--font-weight-bold"),
        (
            "kind pending (stage 6): number".into(),
            Some("tw.weight.bold".into())
        )
    );
    assert_eq!(
        why("--ease-in-out"),
        (
            "kind pending (stage 6): easing".into(),
            Some("tw.ease.in-out".into())
        )
    );
    assert_eq!(
        why("--breakpoint-sm").0,
        "no Tamber kind: a breakpoint is layout, CSS's own"
    );
    assert_eq!(
        why("--inset-shadow-xs").0,
        "kind pending (stage 6): box-shadow (inset)"
    );
    let text = accepted(EMPTY, &first);
    assert_eq!(
        token_value(&text, "tw.red.s500"),
        "oklch(63.7% 0.237 25.331)"
    );
    assert_eq!(token_value(&text, "tw.black"), "#000");
    assert_eq!(token_value(&text, "tw.neutral.s50"), "oklch(98.5% 0 0)");
    let again = tailwind::import(&css, "tw", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn a_tailwind_project_maps_its_dark_block_to_light_dark_and_honours_its_reset() {
    use exact_tamber::import::tailwind;
    let theme = fixture("tailwind-theme.css");
    let project = fixture("tailwind-project.css");
    // The default theme first, then the project, as `@import "tailwindcss"` reads them.
    let css = format!("{theme}\n{project}");
    let import = tailwind::import(&css, "tw", &parse(EMPTY).unwrap()).unwrap();
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    // The reset takes every default colour out.
    assert_eq!(
        import
            .unmapped
            .iter()
            .filter(|u| u.why == "reset by `--color-*: initial`")
            .count(),
        288
    );
    let text = accepted(EMPTY, &import);
    assert!(!text.contains("tw.red.s500"), "{text}");
    assert_eq!(
        token_value(&text, "tw.brand.s500"),
        "light-dark(oklch(0.62 0.19 255), oklch(0.7 0.17 255))"
    );
    assert_eq!(
        token_value(&text, "tw.surface"),
        "light-dark(#ffffff, #0b0f14)"
    );
    assert_eq!(token_value(&text, "tw.ink"), "tw.brand.s600");
    let again = tailwind::import(&css, "tw", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn tailwind_reads_defaults_inline_keys_resets_and_whole_blocks_as_tailwind_does() {
    use exact_tamber::import::tailwind;
    let css = r#"
@theme { --color-brand: red; --spacing-4: 1rem; --font-sans: x; --font-weight-bold: 700; }
@theme default { --color-brand: blue; --color-late: #010101; }
@theme { --color-late: #020202; }
:root { --ink: #111111; }
.dark { --ink: #eeeeee; }
@theme inline { --color-ink: var(--ink); }
.dark { --color-ink: black; }
@theme { --color-x: white; }
.dark { --color-x: black; --color-x: initial; }
:root:not(.dark) { --color-x: red; }
.dark .card { --color-x: navy; }
.card { @variant dark { --color-x: teal; } }
[data-theme=darker] { --color-x: olive; }
@media (prefers-color-scheme: dark) { @theme { --color-y: #121212; } }
@theme { --spacing-*: initial; --font-*: initial; }
"#;
    let import = tailwind::import(css, "tw", &parse(EMPTY).unwrap()).unwrap();
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
    let text = accepted(EMPTY, &import);
    // A default yields to a value the key has, before or after it.
    assert_eq!(token_value(&text, "tw.brand"), "red");
    assert_eq!(token_value(&text, "tw.late"), "#020202");
    // An inline key reads its backing variable on each side.
    assert_eq!(token_value(&text, "tw.ink"), "light-dark(#111111, #eeeeee)");
    assert!(why("--color-ink")
        .iter()
        .any(|w| w.starts_with("an `inline` theme key")));
    // `initial` outside `@theme` makes the dark side invalid: no token
    // wears white there.
    assert!(!text.contains("tw.x ="), "{text}");
    assert!(
        why("--color-x").contains(&"`initial` makes the variable invalid on its side".to_string())
    );
    for restricted in ["red", "navy", "teal", "olive"] {
        assert!(
            import
                .unmapped
                .iter()
                .any(|u| u.value == restricted && u.why.contains("some elements only")),
            "{restricted}: {:?}",
            import.unmapped
        );
    }
    // A theme value under a dark media query is still the theme's.
    assert_eq!(token_value(&text, "tw.y"), "#121212");
    // Resets clear pending rows too; --font-* keeps the weights.
    assert_eq!(why("--spacing-4"), ["reset by `--spacing-*: initial`"]);
    assert_eq!(why("--font-sans"), ["reset by `--font-*: initial`"]);
    assert_eq!(
        why("--font-weight-bold"),
        ["kind pending (stage 6): number"]
    );
}

#[test]
fn tailwind_cascades_roots_defaults_inline_dependents_and_custom_variants() {
    use exact_tamber::import::tailwind;
    let css = r#"
@custom-variant dark (@media print);
:root { --color-early: blue; }
@theme { --color-early: red; --color-a: red; --ink: #343434; --color-b: var(--ink); }
:root { --color-a: blue; }
@theme { --color-c: red; }
@theme default { --color-c: initial; --color-c: blue; }
@theme inline { --color-d: red; --color-e: var(--color-d); }
.dark { --color-d: blue; }
:root { @variant dark { --color-f: black; } }
@theme { --color-g: var(--h); }
:root { --h: red; }
.dark { --h: var(--k); --k: var(--h); }
@theme { --color-n: oklch(NONE 0 0); --color-m: hsl(none 0% 50% / none); }
"#;
    let import = tailwind::import(css, "tw", &parse(EMPTY).unwrap()).unwrap();
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
    let text = accepted(EMPTY, &import);
    // The theme is written at its first @theme: a root before it loses.
    assert_eq!(token_value(&text, "tw.early"), "red");
    // A root redefinition after it sets both sides.
    assert_eq!(token_value(&text, "tw.a"), "blue");
    // A backing variable inside @theme.
    assert_eq!(token_value(&text, "tw.b"), "#343434");
    // A default's own `initial` yields, as its value does.
    assert_eq!(token_value(&text, "tw.c"), "red");
    // A key reading a redefined inline key is reported, not mapped wrong.
    assert_eq!(token_value(&text, "tw.d"), "red");
    assert_eq!(why("--color-e"), ["reads `--color-d`, an `inline` key the stylesheet redefines, which its token cannot follow"]);
    // `dark` is print here.
    assert!(why("--color-f")[0].contains("`@variant dark` is `(@media print)`"));
    // A dark side that does not resolve: no token wearing the light colour.
    assert!(!text.contains("tw.g ="), "{text}");
    // `none` anywhere, in any case.
    assert_eq!(token_value(&text, "tw.n"), "oklch(0 0 0)");
    assert_eq!(token_value(&text, "tw.m"), "hsl(0 0% 50% / 0)");
}

#[test]
fn importers_drop_what_names_a_dropped_token_and_tailwind_keeps_dark_sides() {
    use exact_tamber::import::tailwind;
    // shadcn: --x makes no token (a length under blue), so --y, naming it,
    // makes none either.
    let css =
        ":root { --x: white; --y: var(--x); } .dark { --x: black; } .theme-blue { --x: 1rem; }";
    let import = shadcn::import(css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    let text = accepted(EMPTY, &import);
    assert!(
        !text.contains("shadcn.x =") && !text.contains("shadcn.y ="),
        "{text}"
    );
    assert!(import
        .unmapped
        .iter()
        .filter(|u| u.name == "--x")
        .all(|u| u.token.is_none()));
    let css = r#"
@theme {}
:root { --color-early: blue; }
@theme { --color-early: red; --ink: red; --color-a: var(--ink); --color-b: red; }
.dark { --ink: blue; }
.dark { --color-b: navy; }
:root { --color-b: white; }
@custom-variant  DARK (@media print);
:root { @variant dark { --color-c: black; } }
"#;
    let import = tailwind::import(css, "tw", &parse(EMPTY).unwrap()).unwrap();
    let text = accepted(EMPTY, &import);
    // An empty first @theme still places the theme: the root after it wins.
    assert_eq!(token_value(&text, "tw.early"), "blue");
    // A backing variable's dark redefinition is read.
    assert_eq!(token_value(&text, "tw.a"), "light-dark(red, blue)");
    // A dark declaration holds the dark side against a later root.
    assert_eq!(token_value(&text, "tw.b"), "light-dark(white, navy)");
    // The custom variant is read whatever its spacing and case.
    assert!(import
        .unmapped
        .iter()
        .any(|u| u.name == "--color-c" && u.why.contains("@custom-variant")));
}

/// Every file under `tests/import/<dir>`, by its path from there.
fn tree(dir: &str) -> Vec<(String, String)> {
    let root = format!("{}/tests/import/{dir}", env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    let mut stack = vec![std::path::PathBuf::from(&root)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let name = p
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.push((name, std::fs::read_to_string(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_figma_variables_export_imports_counted_with_light_and_dark() {
    use exact_tamber::import::dtcg;
    let files = tree("figma");
    let first = dtcg::import(&files, "figma", &parse(EMPTY).unwrap()).unwrap();
    assert_eq!(
        first.counted.mapped + first.counted.reported,
        first.counted.source
    );
    assert_eq!(first.counted.source, 31 + 31 + 4 + 19 + 19);
    // 11 + 11 primitive colours (two themes) and 9 + 9 token colours (light, dark).
    assert_eq!(first.counted.mapped, 40);
    let text = accepted(EMPTY, &first);
    assert!(
        text.contains("  primitives-completed = brutal-theme | modern-theme\n"),
        "{text}"
    );
    let primary = token_value(&text, "figma.text.text-primary");
    assert!(
        primary.starts_with("light-dark(figma.color.gray.s900, "),
        "{primary}"
    );
    let again = dtcg::import(&files, "figma", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn a_tokens_studio_sample_imports_counted_its_groups_proposed_axes() {
    use exact_tamber::import::dtcg;
    let files = tree("tokens-studio");
    let first = dtcg::import(&files, "ts", &parse(EMPTY).unwrap()).unwrap();
    assert_eq!(
        (
            first.counted.source,
            first.counted.mapped,
            first.counted.reported
        ),
        (49, 44, 5)
    );
    assert!(first
        .unmapped
        .iter()
        .any(|u| u.why == "kind pending (stage 6): length"
            && u.token.as_deref() == Some("ts.borderRadius.lg")));
    let text = accepted(EMPTY, &first);
    assert!(text.contains("  color = green | blue | purple\n"), "{text}");
    assert!(
        text.contains("when color=blue: ts.colors.blue.s600"),
        "{text}"
    );
    let again = dtcg::import(&files, "ts", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn a_resolver_documents_modifiers_are_the_scheme_and_an_axis() {
    use exact_tamber::import::dtcg;
    let files = vec![("resolver.json".to_string(), fixture("resolver.json"))];
    let first = dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).unwrap();
    assert_eq!(
        (
            first.counted.source,
            first.counted.mapped,
            first.counted.reported
        ),
        (6, 6, 0)
    );
    let text = accepted(EMPTY, &first);
    // The colour space is the value; `hex` its fallback.
    assert_eq!(
        token_value(&text, "ds.color.ink"),
        "color(srgb 0.1 0.1 0.1)"
    );
    assert_eq!(
        token_value(&text, "ds.color.surface"),
        "light-dark(ds.color.paper, oklch(0.2 0.01 250))"
    );
    assert!(text.contains("  brand = calm | loud\n"), "{text}");
    assert!(text.contains("when brand=loud: #ff3366"), "{text}");
    let again = dtcg::import(&files, "ds", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

fn dtcg_files(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, t)| (n.to_string(), t.to_string()))
        .collect()
}

/// Each theme of a Tokens Studio group of several is a mode: one with no
/// name, or two with one name, refuse the import rather than fold unseen.
#[test]
fn dtcg_refuses_a_theme_group_with_a_nameless_or_repeated_theme() {
    use exact_tamber::import::dtcg;
    let doc = |themes: &str| {
        format!(
            r##"{{ "s": {{ "c": {{ "a": {{ "value": "#010101", "type": "color" }} }} }}, "$themes": [{themes}] }}"##
        )
    };
    let refused = |themes: &str| {
        dtcg::import(
            &dtcg_files(&[("t.json", &doc(themes))]),
            "ts",
            &parse(EMPTY).unwrap(),
        )
        .err()
        .map(|d| d[0].message.clone())
    };
    let theme = |name: &str, group: &str| {
        format!(
            r#"{{ "name": "{name}", "group": "{group}", "selectedTokenSets": {{ "s": "enabled" }} }}"#
        )
    };
    assert!(refused(&[theme("", "g"), theme("", "g")].join(","))
        .unwrap()
        .contains("has a theme with no name"));
    assert!(refused(&[theme("A", ""), theme("A", "")].join(","))
        .unwrap()
        .contains("the ungrouped themes names two themes `A`"));
    // One theme alone is the base, named or not; distinct names are modes.
    assert_eq!(refused(&theme("", "g")), None);
    assert_eq!(refused(&[theme("A", "g"), theme("B", "g")].join(",")), None);
}

#[test]
fn dtcg_resolves_each_mode_tuple_in_order() {
    use exact_tamber::import::dtcg;
    // Inline entries; a context naming a set; a set after a modifier wins.
    let resolver = r##"{
      "sets": {
        "base": { "sources": [ { "c": { "$type": "color", "a": { "$value": "#111111" }, "late": { "$value": "#121212" } } } ] },
        "dark": { "sources": [ { "c": { "$type": "color", "a": { "$value": "#222222" }, "late": { "$value": "#000000" } } } ] }
      },
      "resolutionOrder": [
        { "$ref": "#/sets/base" },
        { "name": "mode", "contexts": {
            "light": [ { "c": { "$type": "color", "late": { "$value": "#ffffff" } } } ],
            "dark": [ { "$ref": "#/sets/dark" } ] } },
        { "name": "fix", "sources": [ { "c": { "$type": "color", "late": { "$value": "#343434" } } } ] }
      ]
    }"##
    .to_string();
    let import = dtcg::import(
        &dtcg_files(&[("r.json", &resolver)]),
        "ds",
        &parse(EMPTY).unwrap(),
    )
    .unwrap();
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "ds.c.a"), "light-dark(#111111, #222222)");
    // The set after the modifier wins both sides.
    assert_eq!(token_value(&text, "ds.c.late"), "#343434");
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
}

#[test]
fn dtcg_reads_the_format_as_written() {
    use exact_tamber::import::dtcg;
    let tokens = r##"{
      "brand": { "$type": "color",
        "$root": { "$value": "#0000ff" },
        "p3": { "$value": { "colorSpace": "display-p3", "components": [1, 0, 0], "hex": "#ff0000" } },
        "ref": { "$value": { "$ref": "#/brand/p3/$value" } },
        "part": { "$value": { "$ref": "#/brand/p3/$value/components/0" } }
      },
      "base": { "$type": "color", "ink": { "$value": "#101010" } },
      "theme": { "$extends": "{base}", "paper": { "$type": "color", "$value": "#fefefe" } },
      "plain": { "value": { "$type": "color", "$value": "#202020" } },
      "alias": { "$value": "{base.ink}" },
      "dangling": { "$type": "color", "$value": "{missing.path}" }
    }"##;
    let import = dtcg::import(
        &dtcg_files(&[("tokens.json", tokens)]),
        "ds",
        &parse(EMPTY).unwrap(),
    )
    .unwrap();
    let why = |name: &str| {
        import
            .unmapped
            .iter()
            .find(|u| u.name == name)
            .map(|u| u.why.clone())
            .unwrap_or_default()
    };
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "ds.brand.root"), "#0000ff");
    assert_eq!(token_value(&text, "ds.brand.p3"), "color(display-p3 1 0 0)");
    assert_eq!(token_value(&text, "ds.brand.ref"), "ds.brand.p3");
    assert!(why("brand.part").contains("part of a value"));
    assert_eq!(token_value(&text, "ds.theme.ink"), "#101010");
    assert_eq!(token_value(&text, "ds.plain.value"), "#202020");
    // An untyped alias takes its target's type.
    assert_eq!(token_value(&text, "ds.alias"), "ds.base.ink");
    assert!(why("dangling").contains("no token"));
}

#[test]
fn tokens_studio_modifiers_empty_themes_and_set_precedence() {
    use exact_tamber::import::dtcg;
    let single = r##"{
      "core": { "c": { "type": "color", "a": { "value": "#010101" }, "m": { "value": "#020202", "$extensions": { "studio.tokens": { "modify": { "type": "lighten", "value": "0.2" } } } } } },
      "over": { "c": { "a": { "value": "#030303", "type": "color" } } },
      "$themes": [],
      "$metadata": { "tokenSetOrder": ["core", "over"] }
    }"##;
    let import = dtcg::import(
        &dtcg_files(&[("tokens.json", single)]),
        "ts",
        &parse(EMPTY).unwrap(),
    )
    .unwrap();
    let text = accepted(EMPTY, &import);
    // No themes: every set, in order, the later winning.
    assert_eq!(token_value(&text, "ts.c.a"), "#030303");
    assert!(import
        .unmapped
        .iter()
        .any(|u| u.name == "c.m" && u.why.contains("modify")));
    assert!(!text.contains("ts.c.m ="));
}

#[test]
fn dtcg_drops_a_token_a_mode_lacks_and_refuses_host_axes() {
    use exact_tamber::import::dtcg;
    let files = dtcg_files(&[
        (
            "Theme.Light.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#ffffff" }, "only": { "$value": "#eeeeee" } } }"##,
        ),
        (
            "Theme.Dark.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#000000" }, "only": { "$value": 4, "$type": "number" } } }"##,
        ),
        (
            "extra.json",
            r##"{ "c": { "$type": "color", "b": { "$value": "#777777" } } }"##,
        ),
    ]);
    let import = dtcg::import(&files, "f", &parse(EMPTY).unwrap()).unwrap();
    let text = accepted(EMPTY, &import);
    // The extra file is the base; Light and Dark still the scheme.
    assert_eq!(token_value(&text, "f.c.a"), "light-dark(#ffffff, #000000)");
    assert_eq!(token_value(&text, "f.c.b"), "#777777");
    // A colour in one mode, a number in the other: no token.
    assert!(!text.contains("f.c.only ="), "{text}");
    let host = dtcg_files(&[
        (
            "Contrast.High.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#000" } } }"##,
        ),
        (
            "Contrast.Low.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#111" } } }"##,
        ),
    ]);
    let refused = dtcg::import(&host, "f", &parse(EMPTY).unwrap()).unwrap_err();
    assert_eq!(refused[0].id, "tamber-import-axis");
}

#[test]
fn dtcg_scheme_and_axis_together_keep_the_scheme_in_the_default() {
    use exact_tamber::import::dtcg;
    let files = dtcg_files(&[
        (
            "Tokens.Light.json",
            r##"{ "c": { "$type": "color", "surface": { "$value": "#ffffff" } } }"##,
        ),
        (
            "Tokens.Dark.json",
            r##"{ "c": { "$type": "color", "surface": { "$value": "#000000" } } }"##,
        ),
        (
            "Brand.Calm.json",
            r##"{ "c": { "$type": "color", "accent": { "$value": "#0000ff" } } }"##,
        ),
        (
            "Brand.Loud.json",
            r##"{ "c": { "$type": "color", "accent": { "$value": "#ff0000" }, "surface": { "$value": "#ff00ff" } } }"##,
        ),
    ]);
    let import = dtcg::import(&files, "f", &parse(EMPTY).unwrap()).unwrap();
    let text = accepted(EMPTY, &import);
    assert!(text.contains("  brand = calm | loud\n"), "{text}");
    // Calm, the default, leaves the scheme; loud's surface wins both sides.
    assert!(
        text.contains(
            "  f.c.surface = light-dark(#ffffff, #000000)\n    when brand=loud: #ff00ff\n"
        ),
        "{text}"
    );
}

#[test]
fn dtcg_round_two_tokens_studio_order_names_and_references() {
    use exact_tamber::import::dtcg;
    // Sets apply in tokenSetOrder whatever the groups; one set in two themes
    // is read once.
    let single = r##"{
      "late": { "c": { "a": { "value": "#020202", "type": "color" } } },
      "early": { "c": { "a": { "value": "#010101", "type": "color" } } },
      "shared": { "c": { "s": { "value": "#030303", "type": "color" } } },
      "$themes": [
        { "name": "One", "group": "g1", "selectedTokenSets": { "late": "enabled", "shared": "enabled" } },
        { "name": "Two", "group": "g2", "selectedTokenSets": { "early": "enabled", "shared": "enabled" } }
      ],
      "$metadata": { "tokenSetOrder": ["early", "late", "shared"] }
    }"##;
    let import = dtcg::import(
        &dtcg_files(&[("t.json", single)]),
        "ts",
        &parse(EMPTY).unwrap(),
    )
    .unwrap();
    assert_eq!(import.counted.source, 3);
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "ts.c.a"), "#020202");
    // Two axes written apart but one identifier refuse the import.
    let clash = dtcg_files(&[
        (
            "Brand Tone.Calm.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#000" } } }"##,
        ),
        (
            "Brand Tone.Loud.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#111" } } }"##,
        ),
        (
            "brand-tone.Calm.json",
            r##"{ "c": { "$type": "color", "b": { "$value": "#222" } } }"##,
        ),
        (
            "brand-tone.Loud.json",
            r##"{ "c": { "$type": "color", "b": { "$value": "#333" } } }"##,
        ),
    ]);
    assert_eq!(
        dtcg::import(&clash, "f", &parse(EMPTY).unwrap()).unwrap_err()[0].id,
        "tamber-import-axis"
    );
    // $extends deep and transitive; a whole-token $ref alias; ~1 decoded; a
    // long untyped chain; an alias to a colliding name inlined.
    let tokens = r##"{
      "base": { "$type": "color", "g": { "a": { "$value": "#0a0a0a" }, "b": { "$value": "#0b0b0b" } } },
      "mid": { "$extends": "{base}" },
      "top": { "$extends": "{mid}", "g": { "a": { "$value": "#0c0c0c" } } },
      "whole": { "$ref": "#/base/g/a" },
      "slash": { "$type": "color", "a/b": { "$value": "#0d0d0d" } },
      "esc": { "$value": { "$ref": "#/slash/a~1b/$value" } },
      "a b": { "$type": "color", "$value": "#0e0e0e" },
      "a-b": { "$type": "color", "$value": "#0f0f0f" },
      "to-collided": { "$value": "{a-b}" },
      "h0": { "$value": "{h1}" }, "h1": { "$value": "{h2}" }, "h2": { "$value": "{h3}" }, "h3": { "$value": "{h4}" },
      "h4": { "$value": "{h5}" }, "h5": { "$value": "{h6}" }, "h6": { "$value": "{h7}" }, "h7": { "$value": "{h8}" },
      "h8": { "$value": "{h9}" }, "h9": { "$value": "{h10}" }, "h10": { "$value": "{h11}" }, "h11": { "$value": "{h12}" },
      "h12": { "$value": "{h13}" }, "h13": { "$value": "{h14}" }, "h14": { "$value": "{h15}" }, "h15": { "$value": "{h16}" },
      "h16": { "$value": "{h17}" }, "h17": { "$value": "{h18}" }, "h18": { "$type": "color", "$value": "#121212" }
    }"##;
    let import = dtcg::import(
        &dtcg_files(&[("tokens.json", tokens)]),
        "ds",
        &parse(EMPTY).unwrap(),
    )
    .unwrap();
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "ds.top.g.a"), "#0c0c0c");
    assert_eq!(token_value(&text, "ds.top.g.b"), "#0b0b0b");
    assert_eq!(token_value(&text, "ds.whole"), "ds.base.g.a");
    assert_eq!(token_value(&text, "ds.esc"), "ds.slash.a-b");
    assert_eq!(token_value(&text, "ds.to-collided"), "#0f0f0f");
    assert_eq!(token_value(&text, "ds.h0"), "ds.h1");
    let cycle = r##"{ "a": { "$extends": "{b}" }, "b": { "$extends": "{a}" } }"##;
    assert!(dtcg::import(
        &dtcg_files(&[("c.json", cycle)]),
        "ds",
        &parse(EMPTY).unwrap()
    )
    .is_err());
}

#[test]
fn dtcg_round_two_resolver_documents_figma_case_and_existing_axes() {
    use exact_tamber::import::dtcg;
    // A set in another file whose own references are its own; sets within
    // sets; a set never reached reported.
    let resolver = r##"{
      "sets": { "unused": { "sources": [ { "u": { "$type": "color", "x": { "$value": "#999999" } } } ] } },
      "resolutionOrder": [ { "$ref": "sub/sets.json#/sets/outer" } ]
    }"##;
    let sets = r##"{
      "sets": {
        "outer": { "sources": [ { "$ref": "#/sets/inner" } ] },
        "inner": { "sources": [ { "$ref": "./colors.json" } ] }
      }
    }"##;
    let colors = r##"{ "c": { "$type": "color", "a": { "$value": "#123123" } } }"##;
    let files = dtcg_files(&[
        ("r.json", resolver),
        ("sub/sets.json", sets),
        ("sub/colors.json", colors),
    ]);
    let import = dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).unwrap();
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "ds.c.a"), "#123123");
    assert!(import
        .unmapped
        .iter()
        .any(|u| u.name == "u.x" && u.why.contains("never reaches")));
    // Figma files named in upper case.
    let upper = dtcg_files(&[
        (
            "T.Light.JSON",
            r##"{ "c": { "$type": "color", "a": { "$value": "#fff" } } }"##,
        ),
        (
            "T.Dark.JSON",
            r##"{ "c": { "$type": "color", "a": { "$value": "#000" } } }"##,
        ),
    ]);
    let text = accepted(
        EMPTY,
        &dtcg::import(&upper, "f", &parse(EMPTY).unwrap()).unwrap(),
    );
    assert_eq!(token_value(&text, "f.c.a"), "light-dark(#fff, #000)");
    // An existing axis under the dark scheme, or one the values would take
    // past the tuples, refuses.
    let modes = dtcg_files(&[
        (
            "Brand.Calm.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#000" } } }"##,
        ),
        (
            "Brand.Loud.json",
            r##"{ "c": { "$type": "color", "a": { "$value": "#111" } } }"##,
        ),
    ]);
    let dark = "tamber app\n\naxes\n  brand = calm | quiet  under scheme=dark\n";
    assert_eq!(
        dtcg::import(&modes, "f", &parse(dark).unwrap()).unwrap_err()[0].id,
        "tamber-import-axis"
    );
    let values: Vec<String> = std::iter::once("calm".to_string())
        .chain((0..63).map(|i| format!("v{i}")))
        .collect();
    let full = format!("tamber app\n\naxes\n  brand = {}\n", values.join(" | "));
    assert_eq!(
        dtcg::import(&modes, "f", &parse(&full).unwrap()).unwrap_err()[0].id,
        "tamber-import-tuples"
    );
}

#[test]
fn dtcg_round_three_theme_order_chains_and_replaced_groups() {
    use exact_tamber::import::dtcg;
    // Calm selects no set and comes first: it is still the default.
    let single = r##"{
      "base": { "c": { "a": { "value": "#010101", "type": "color" } } },
      "loud": { "c": { "a": { "value": "#020202", "type": "color" } } },
      "$themes": [
        { "name": "Base", "group": "core", "selectedTokenSets": { "base": "enabled" } },
        { "name": "Calm", "group": "brand", "selectedTokenSets": {} },
        { "name": "Loud", "group": "brand", "selectedTokenSets": { "loud": "enabled" } }
      ],
      "$metadata": { "tokenSetOrder": ["loud", "base"] }
    }"##;
    let import = dtcg::import(
        &dtcg_files(&[("t.json", single)]),
        "ts",
        &parse(EMPTY).unwrap(),
    )
    .unwrap();
    let text = accepted(EMPTY, &import);
    assert!(text.contains("  brand = calm | loud\n"), "{text}");
    // A whole-token $ref chain; a later token replacing an earlier group.
    let tokens = r##"{
      "c": { "$type": "color", "x": { "$value": "#0a0a0a" } },
      "b": { "$ref": "#/c/x" },
      "a": { "$ref": "#/b" }
    }"##;
    let over = r##"{ "g": { "$type": "color", "$value": "#0b0b0b" } }"##;
    let under = r##"{ "g": { "$type": "color", "inner": { "$value": "#0c0c0c" } } }"##;
    let files = dtcg_files(&[("1.json", under), ("2.json", over), ("tokens.json", tokens)]);
    let import = dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).unwrap();
    assert_eq!(
        import.counted.mapped + import.counted.reported,
        import.counted.source
    );
    let text = accepted(EMPTY, &import);
    assert_eq!(token_value(&text, "ds.a"), "ds.b");
    assert_eq!(token_value(&text, "ds.g"), "#0b0b0b");
    assert!(!text.contains("ds.g.inner ="), "{text}");
    // A reference leaving the sources is an error.
    let out = r##"{ "resolutionOrder": [ { "$ref": "../colors.json" } ] }"##;
    let files = dtcg_files(&[("r.json", out), ("colors.json", "{}")]);
    assert!(dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).is_err());
}

#[test]
fn astryx_neutral_imports_counted_and_reports_its_kinds_probe() {
    use exact_tamber::import::astryx;
    let json = fixture("astryx-neutral.json");
    let first = astryx::import(&json, "astryx", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    // 258 Astryx tokens and the theme's 10 own: 160 colours, 108 reported.
    assert_eq!(
        (
            first.counted.source,
            first.counted.mapped,
            first.counted.reported
        ),
        (268, 160, 108)
    );
    // The probe: what Tamber's kinds cannot hold yet.
    let tiers: Vec<&str> = first
        .unmapped
        .iter()
        .filter(|u| u.why.starts_with("a duration's min or max tier"))
        .map(|u| u.name.as_str())
        .collect();
    assert_eq!(
        tiers,
        [
            "--duration-fast-max",
            "--duration-fast-min",
            "--duration-medium-max",
            "--duration-medium-min",
            "--duration-slow-max",
            "--duration-slow-min"
        ]
    );
    assert!(
        first
            .unmapped
            .iter()
            .any(|u| u.name == "--duration-fast"
                && u.token.as_deref() == Some("astryx.duration.fast"))
    );
    assert!(first
        .unmapped
        .iter()
        .any(|u| u.name == "--shadow-inset-hover"
            && u.why == "kind pending (stage 6): box-shadow (inset)"));
    let text = accepted(EMPTY, &first);
    assert_eq!(
        token_value(&text, "astryx.accent"),
        "light-dark(#1b1b1b, #f1f1f1)"
    );
    assert_eq!(
        token_value(&text, "astryx.surface.body"),
        "light-dark(#f1f1f1, #1b1b1b)"
    );
    assert_eq!(token_value(&text, "astryx.data.blue.s5"), "#02165E");
    assert_eq!(
        token_value(&text, "-exact-background"),
        "astryx.surface.body"
    );
    // The accent is the theme's tint, offered as `AccentColor` and accepted
    // in the same transaction as the tokens.
    let roles: Vec<&str> = first.offers.iter().map(|o| o.role.as_str()).collect();
    assert_eq!(roles, ["-exact-background", "AccentColor"]);
    assert_eq!(token_value(&text, "AccentColor"), "astryx.accent");
    assert_eq!(
        token_value(&text, "astryx.local.status-fill-accent"),
        "light-dark(#0074e2, #6d9cfe)"
    );
    let again = astryx::import(&json, "astryx", &parse(&text).unwrap(), &vocab()).unwrap();
    assert_eq!(again.ops, []);
    assert!(again.offers.is_empty());
}

/// The design's own `when` line on the accent stays (Astryx owns no axis),
/// so the role would vary: it is offered with no operations (LLP 1121 D5).
#[test]
fn astryx_offers_no_accent_role_over_a_token_the_design_varies() {
    use exact_tamber::import::astryx;
    let design = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  astryx.accent = #111111\n    when density=compact: #222222\n";
    let json = r##"{"source": "test", "light": {"--color-accent": "#333333"}, "dark": {"--color-accent": "#333333"}}"##;
    let import = astryx::import(json, "astryx", &parse(design).unwrap(), &vocab()).unwrap();
    assert_eq!(import.offers.len(), 1);
    assert!(import.offers[0].ops.is_empty());
    assert!(import.offers[0]
        .note
        .as_deref()
        .unwrap()
        .starts_with("not offered"));
}

/// A role that reaches a changed value and holds a `when` line could
/// split once accepted, though Astryx adds no `when`: it is named, with no
/// operations (the reviewers' two cases: a `when` that folds to today's
/// default, and a role chained to the rebound accent).
#[test]
fn astryx_names_a_design_role_a_changed_value_could_split() {
    use exact_tamber::import::astryx;
    let json = r##"{"source": "test", "light": {"--color-accent": "#333333"}, "dark": {"--color-accent": "#333333"}}"##;
    let names = |design: &str| -> Vec<(String, bool)> {
        astryx::import(json, "astryx", &parse(design).unwrap(), &vocab())
            .unwrap()
            .offers
            .iter()
            .map(|o| (o.role.clone(), o.ops.is_empty()))
            .collect()
    };
    let kept = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  astryx.accent = #111111\n    when density=compact: #111\n  Canvas = astryx.accent\n";
    assert_eq!(
        names(kept),
        [
            ("AccentColor".to_string(), true),
            ("Canvas".to_string(), true)
        ]
    );
    let chained = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  AccentColor = #111111\n  Canvas = AccentColor\n    when density=compact: #111111\n";
    assert_eq!(
        names(chained),
        [
            ("AccentColor".to_string(), false),
            ("Canvas".to_string(), true)
        ]
    );
}

/// Rebinding a role that is a dial's only use would leave the dial unused,
/// which the design refuses: not offered. And a role spelled otherwise is
/// named once.
#[test]
fn astryx_offers_no_rebinding_that_orphans_a_dial_and_names_a_role_once() {
    use exact_tamber::import::astryx;
    let json = r##"{"source": "test", "light": {"--color-accent": "#333333"}, "dark": {"--color-accent": "#333333"}}"##;
    let design = "tamber app\n\ndials\n  brand: color = #111111\n\ntheme\n  AccentColor = brand\n";
    let import = astryx::import(json, "astryx", &parse(design).unwrap(), &vocab()).unwrap();
    assert_eq!(import.offers.len(), 1);
    assert!(import.offers[0].ops.is_empty());
    assert!(import.offers[0]
        .note
        .as_deref()
        .unwrap()
        .contains("tamber-dial-inert"));
    accepted(design, &import);
    let design = "tamber app\n\naxes\n  density = default | compact\n\ntheme\n  astryx.accent = #111111\n    when density=compact: #222222\n  accentcolor = astryx.accent\n";
    let import = astryx::import(json, "astryx", &parse(design).unwrap(), &vocab()).unwrap();
    assert_eq!(import.offers.len(), 1, "{:?}", import.offers);
}

#[test]
fn astryx_reports_a_second_variable_to_one_token() {
    use exact_tamber::import::astryx;
    let json = r##"{ "light": { "--color-data-blue-5": "#000000", "--color-data-blue-s5": "#111111" },
                    "dark": { "--color-data-blue-5": "#000000", "--color-data-blue-s5": "#111111" } }"##;
    let import = astryx::import(json, "astryx", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    assert_eq!(
        (
            import.counted.source,
            import.counted.mapped,
            import.counted.reported
        ),
        (2, 1, 1)
    );
    assert!(import.unmapped[0]
        .why
        .contains("its token is `--color-data-blue-5`'s"));
    accepted(EMPTY, &import);
}
