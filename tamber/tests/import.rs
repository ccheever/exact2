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
