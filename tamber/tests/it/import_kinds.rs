//! The importers' kinds besides colour (LLP 1121 stage 6 H): each source's
//! lengths, numbers, type, families, shadows and motion are proposed as
//! their kinds' tokens when they fold, and reported with why when they do
//! not; the count holds, the transaction checks, and re-importing proposes
//! nothing.

use crate::common::vocab;
use exact_tamber::import::{astryx, dtcg, shadcn, tailwind, Import};
use exact_tamber::{apply_all, check, parse};

const EMPTY: &str = "tamber app\n";

/// The design after the import's transaction, checked; the count held.
fn accepted(import: &Import) -> String {
    let c = import.counted;
    assert_eq!(c.mapped + c.reported, c.source, "{c:?}");
    let text = apply_all(EMPTY, &import.ops, &vocab()).unwrap().text;
    check(&text, &vocab()).unwrap_or_else(|e| panic!("{e:?}\n{text}"));
    text
}

fn why(import: &Import, name: &str) -> Vec<String> {
    import
        .unmapped
        .iter()
        .filter(|u| u.name == name)
        .map(|u| u.why.clone())
        .collect()
}

/// Tailwind's default theme: each kind's keys as tokens.
#[test]
fn tailwinds_default_theme_maps_each_kind() {
    let css = std::fs::read_to_string(format!(
        "{}/tests/import/tailwind-theme.css",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let first = tailwind::import(&css, "tw", &parse(EMPTY).unwrap()).unwrap();
    let kinded = |name: &str| {
        first.ops.iter().find_map(|o| match o {
            exact_tamber::Op::AddToken {
                name: n,
                kind,
                value,
                ..
            } if n == name => Some((kind.clone().unwrap_or_default(), value.clone())),
            _ => None,
        })
    };
    let has = |name: &str, kind: &str, value: &str| {
        assert_eq!(
            kinded(name),
            Some((kind.to_string(), value.to_string())),
            "{name}"
        )
    };
    has("tw.space.base", "length", "0.25rem");
    has("tw.radius.lg", "length", "0.5rem");
    // `--text-xs` and `--text-xs--line-height`, one type token.
    has("tw.type.xs", "type", "0.75rem/1.3333333333333333");
    has("tw.weight.bold", "number", "700");
    has("tw.leading.tight", "number", "1.25");
    has("tw.tracking.tight", "length", "-0.025em");
    has("tw.ease.in-out", "easing", "cubic-bezier(0.4, 0, 0.2, 1)");
    has(
        "tw.inset-shadow.xs",
        "box-shadow",
        "inset 0 1px 1px rgb(0 0 0 / 0.05)",
    );
    has(
        "tw.text-shadow.xs",
        "text-shadow",
        "0px 1px 1px rgb(0 0 0 / 0.2)",
    );
    has(
        "tw.family.mono",
        "family",
        "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace",
    );
}

#[test]
fn tailwind_folds_each_key_with_what_it_names_and_reports_the_rest() {
    let css = r#"
@theme {
  --color-ring: #3b82f6;
  --shadow-focus: 0 0 0 2px var(--color-ring);
  --text-tiny: 0.625rem;
  --text-tiny--line-height: 1rem;
  --text-tiny--font-weight: var(--font-weight-heavy);
  --text-tiny--letter-spacing: 0.01em;
  --font-weight-heavy: 800;
  --text-ghost--line-height: 1.2;
  --spacing-gap: 12%;
  --radius-card: var(--radius-base);
  --radius-base: 6px;
  --radius-moved: 4px;
  --font-sans--font-feature-settings: "cv11";
  --font-sans: Inter, sans-serif;
  --ease-snap: cubic-bezier(0.2, 0, 0, 1);
}
:root { --radius-moved: 8px; }
"#;
    let import = tailwind::import(css, "tw", &parse(EMPTY).unwrap()).unwrap();
    let text = accepted(&import);
    // A shadow names the colour token its `var()` names.
    assert!(
        text.contains("  tw.shadow.focus: box-shadow = 0 0 0 2px tw.ring\n"),
        "{text}"
    );
    // `--text-tiny` with its leading and weight, one type token; a weight is
    // written as its number.
    assert!(
        text.contains("  tw.type.tiny: type = 800 0.625rem/1rem\n"),
        "{text}"
    );
    assert_eq!(
        why(&import, "--text-tiny--letter-spacing"),
        ["a type token holds no `letter-spacing`"]
    );
    assert_eq!(
        why(&import, "--text-ghost--line-height"),
        ["beside `--text-ghost`, which the theme does not declare"]
    );
    assert!(
        why(&import, "--spacing-gap")[0].starts_with("not a length Tamber folds: "),
        "{:?}",
        why(&import, "--spacing-gap")
    );
    // A reference to a key declared later is still the token it names.
    assert!(
        text.contains("  tw.radius.card: length = tw.radius.base\n"),
        "{text}"
    );
    // A key the stylesheet redefines: its utilities follow the redefinition.
    assert!(why(&import, "--radius-moved")
        .iter()
        .all(|w| w.contains("redefin")));
    assert!(!text.contains("tw.radius.moved"), "{text}");
    assert_eq!(
        why(&import, "--font-sans--font-feature-settings"),
        ["a family token holds no `font-feature-settings`"]
    );
    assert!(
        text.contains("  tw.family.sans: family = Inter, sans-serif\n"),
        "{text}"
    );
    assert!(
        text.contains("  tw.ease.snap: easing = cubic-bezier(0.2, 0, 0, 1)\n"),
        "{text}"
    );
    let again = tailwind::import(css, "tw", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn shadcn_keeps_a_themes_length_and_refuses_one_with_a_dark_side() {
    let css = r#"
:root { --radius: 8px; --shadow-sm: 0 1px 2px var(--ring); --ring: #111111; --font-sans: Inter, sans-serif; --gap: 4px; }
.dark { --gap: 6px; --ring: #eeeeee; }
.theme-roomy { --radius: 12px; }
"#;
    let import = shadcn::import(css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    let text = accepted(&import);
    // A theme's length is its `when` line; px radii derive in `calc()`.
    assert!(
        text.contains("  shadcn.radius: length = 8px\n    when theme=roomy: 12px\n"),
        "{text}"
    );
    assert!(
        text.contains("  shadcn.radius.sm: length = calc(shadcn.radius - 4px)\n"),
        "{text}"
    );
    assert!(
        text.contains("  shadcn.radius.xl: length = calc(shadcn.radius + 4px)\n"),
        "{text}"
    );
    // A shadow's colour keeps its sides, through the token it names.
    assert!(
        text.contains("  shadcn.shadow-sm: box-shadow = 0 1px 2px shadcn.ring\n"),
        "{text}"
    );
    assert!(
        text.contains("  shadcn.font-sans: family = Inter, sans-serif\n"),
        "{text}"
    );
    assert_eq!(
        why(&import, "--gap"),
        [
            "differs between light and dark, and only a colour has a dark side (`light-dark()`)",
            "differs between light and dark, and only a colour has a dark side (`light-dark()`)"
        ]
    );
    let again = shadcn::import(css, "shadcn", &parse(&text).unwrap(), &vocab()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn dtcg_writes_each_type_out_and_reports_what_a_kind_cannot_hold() {
    let tokens = r##"{
      "size": { "$type": "dimension",
        "sm": { "$value": { "value": 4, "unit": "px" } },
        "md": { "$value": "{size.sm}" },
        "pct": { "$value": { "value": 50, "unit": "%" } } },
      "font": {
        "body": { "$type": "fontFamily", "$value": ["Inter", "Helvetica Neue", "sans-serif"] },
        "weight": { "$type": "fontWeight", "$value": "bold" } },
      "ink": { "$type": "color", "$value": "#101010" },
      "lift": { "$type": "shadow", "$value": [
        { "color": "{ink}", "offsetX": "{size.sm}", "offsetY": { "value": 2, "unit": "px" }, "blur": { "value": 6, "unit": "px" }, "spread": { "value": 0, "unit": "px" } },
        { "color": "#00000033", "offsetX": { "value": 0, "unit": "px" }, "offsetY": { "value": 0, "unit": "px" }, "blur": { "value": 1, "unit": "px" }, "spread": { "value": 0, "unit": "px" }, "inset": true } ] },
      "text": { "$type": "typography",
        "body": { "$value": { "fontFamily": "{font.body}", "fontSize": { "value": 1, "unit": "rem" }, "fontWeight": "{font.weight}", "lineHeight": 1.5, "letterSpacing": { "value": 0, "unit": "px" } } },
        "caps": { "$value": { "fontSize": { "value": 12, "unit": "px" }, "letterSpacing": { "value": 1, "unit": "px" } } } },
      "fast": { "$type": "duration", "$value": { "value": 0.2, "unit": "s" } },
      "ease": { "$type": "cubicBezier", "$value": [0.4, 0, 0.2, 1] }
    }"##;
    let files = vec![("tokens.json".to_string(), tokens.to_string())];
    let import = dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).unwrap();
    let text = accepted(&import);
    for line in [
        "ds.size.sm: length = 4px",
        // An alias is written out.
        "ds.size.md: length = 4px",
        "ds.font.body: family = Inter, Helvetica Neue, sans-serif",
        "ds.font.weight: number = 700",
        "ds.lift: box-shadow = 4px 2px 6px 0px #101010, inset 0px 0px 1px 0px #00000033",
        "ds.text.body: type = 700 1rem/1.5 Inter, Helvetica Neue, sans-serif",
        "ds.fast: duration = 200ms",
        "ds.ease: easing = cubic-bezier(0.4, 0, 0.2, 1)",
    ] {
        assert!(text.contains(&format!("  {line}\n")), "{line}\n{text}");
    }
    assert!(why(&import, "size.pct")[0].starts_with("not a length Tamber folds: "));
    assert!(
        why(&import, "text.caps")[0].contains("a type token holds none"),
        "{:?}",
        why(&import, "text.caps")
    );
    let again = dtcg::import(&files, "ds", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
    // A dimension with a value per side of the scheme is no token.
    let light = r#"{ "gap": { "$type": "dimension", "$value": { "value": 4, "unit": "px" } } }"#;
    let dark = r#"{ "gap": { "$type": "dimension", "$value": { "value": 6, "unit": "px" } } }"#;
    let files = vec![
        ("Tokens.Light.json".to_string(), light.to_string()),
        ("Tokens.Dark.json".to_string(), dark.to_string()),
    ];
    let import = dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).unwrap();
    accepted(&import);
    assert!(why(&import, "gap")
        .iter()
        .all(|w| w.starts_with("differs between light and dark")));
}

#[test]
fn astryx_refuses_a_length_with_a_dark_side_and_a_style_with_no_size() {
    let json = r##"{
      "source": "t", "core": "t",
      "light": { "--spacing-1": "4px", "--radius-x": "4px", "--text-lone-weight": "600", "--duration-fast": "100ms" },
      "dark": { "--spacing-1": "4px", "--radius-x": "6px", "--text-lone-weight": "600", "--duration-fast": "100ms" }
    }"##;
    let import = astryx::import(json, "astryx", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    let text = accepted(&import);
    assert!(text.contains("  astryx.space.s1: length = 4px\n"), "{text}");
    assert!(
        text.contains("  astryx.duration.fast: duration = 100ms\n"),
        "{text}"
    );
    assert!(why(&import, "--radius-x")[0].starts_with("differs between light and dark"));
    assert_eq!(
        why(&import, "--text-lone-weight"),
        ["a text style with no size"]
    );
}

/// Review round 1: what a source means after folding, not just a token
/// that checks.
#[test]
fn each_importer_maps_what_the_source_means_or_reports_it() {
    let tw = |css: &str| tailwind::import(css, "tw", &parse(EMPTY).unwrap()).unwrap();
    // A sub-property redefined outside `@theme`: the type token is none.
    let i = tw("@theme { --text-sm: 12px; --text-sm--line-height: 1.5; } .dark { --text-sm--line-height: 2; }");
    assert!(!accepted(&i).contains("tw.type.sm"));
    assert!(
        why(&i, "--text-sm")[0].contains("redefined"),
        "{:?}",
        why(&i, "--text-sm")
    );
    // A weight named through a key redefined outside `@theme`.
    let i = tw("@theme { --font-weight-heavy: 700; --text-body: 16px; --text-body--font-weight: var(--font-weight-heavy); } :root { --font-weight-heavy: 800; }");
    assert!(!accepted(&i).contains("tw.type.body"));
    // A type size naming another type key is its size alone.
    let i = tw("@theme { --text-base: 16px; --text-base--font-weight: 700; --text-copy: var(--text-base); }");
    let text = accepted(&i);
    assert!(text.contains("  tw.type.copy: type = 16px\n"), "{text}");
    // A reference to a key another owns the name of makes no token.
    let i = tw("@theme { --radius-1: 4px; --radius-s1: 8px; --spacing-gap: var(--radius-s1); }");
    assert!(!accepted(&i).contains("tw.space.gap"));
    // A quoted family keeps its spaces and its text.
    let i = tw("@theme { --font-brand: \"A  var(--x)\", serif; }");
    assert!(accepted(&i).contains("  tw.family.brand: family = \"A  var(--x)\", serif\n"));
    // A design token of another kind is not set to this one.
    let design = parse("tamber app\n\ntheme\n  tw.radius.md: number = 1\n").unwrap();
    let i = tailwind::import("@theme { --radius-md: 4px; }", "tw", &design).unwrap();
    assert!(i.ops.is_empty(), "{:?}", i.ops);
    assert_eq!(
        why(&i, "--radius-md"),
        ["not a length Tamber folds: the design's `tw.radius.md` is a number token, not a length"]
    );

    let sc = |css: &str| shadcn::import(css, "shadcn", &parse(EMPTY).unwrap(), &vocab()).unwrap();
    // A dark value of no kind: no token for either side.
    let i = sc(":root { --gap: 8px; } .dark { --gap: 2vh; }");
    assert!(!accepted(&i).contains("shadcn.gap"));
    assert!(why(&i, "--gap")
        .iter()
        .any(|w| w.contains("a token is one kind")));
    // A length named `--primary` is offered no colour role.
    let i = sc(":root { --primary: 8px; }");
    assert!(i.offers.iter().all(|o| o.ops.is_empty()), "{:?}", i.offers);
    // A reference through a reference takes the kind at its end.
    let text = accepted(&sc(":root { --a: 4px; --b: var(--a); --c: var(--b); }"));
    assert!(text.contains("  shadcn.c: length = shadcn.b\n"), "{text}");

    let ax = |light: &str, dark: &str| {
        let json =
            format!("{{\"source\": \"t\", \"core\": \"t\", \"light\": {light}, \"dark\": {dark}}}");
        astryx::import(&json, "astryx", &parse(EMPTY).unwrap(), &vocab()).unwrap()
    };
    // Two variables to one name: the second is reported.
    let i = ax(
        r#"{"--spacing-1": "4px", "--spacing-s1": "8px"}"#,
        r#"{"--spacing-1": "4px", "--spacing-s1": "8px"}"#,
    );
    assert!(accepted(&i).contains("  astryx.space.s1: length = 4px\n"));
    assert_eq!(
        why(&i, "--spacing-s1"),
        ["its token is `--spacing-1`'s (astryx.space.s1)"]
    );
    // A side that lacks it: nothing invented.
    let i = ax(r#"{"--spacing-1": "4px"}"#, "{}");
    assert!(!accepted(&i).contains("astryx.space"));

    // DTCG: a `fontFamily` string is one family, commas and all.
    let files = vec![(
        "t.json".to_string(),
        r#"{"f": {"$type": "fontFamily", "$value": "Acme, Display"}}"#.to_string(),
    )];
    let i = dtcg::import(&files, "ds", &parse(EMPTY).unwrap()).unwrap();
    assert!(accepted(&i).contains("  ds.f: family = \"Acme, Display\"\n"));
}

/// Review round 1 (Grok): chains of references, a leading over a token, and
/// a design token's written kind.
#[test]
fn chains_resolve_to_what_they_end_at() {
    let i = shadcn::import(
        ":root { --font-sans: 16px; --font-sans: Inter, sans-serif; --font-body: var(--font-sans); --shadow-lg: 0 1px 2px #111111; --shadow-md: var(--shadow-lg); --shadow-sm: var(--shadow-md); }",
        "shadcn",
        &parse(EMPTY).unwrap(),
        &vocab(),
    )
    .unwrap();
    let text = accepted(&i);
    assert!(
        text.contains("  shadcn.font-body: family = shadcn.font-sans\n"),
        "{text}"
    );
    assert!(
        text.contains("  shadcn.shadow-sm: box-shadow = shadcn.shadow-md\n"),
        "{text}"
    );
    let css = "@theme { --leading-tight: 1.25; --text-body: 1rem; --text-body--line-height: calc(var(--leading-tight) * 2); --font-weight-bold: 700; --font-weight-heading: var(--font-weight-bold); --text-body--font-weight: var(--font-weight-heading); }";
    let text = accepted(&tailwind::import(css, "tw", &parse(EMPTY).unwrap()).unwrap());
    assert!(
        text.contains("  tw.type.body: type = 700 1rem/2.5\n"),
        "{text}"
    );
    let design = parse("tamber app\n\ntheme\n  shadcn.radius: color = #ffffff\n").unwrap();
    let i = shadcn::import(":root { --radius: 8px; }", "shadcn", &design, &vocab()).unwrap();
    assert!(i.ops.is_empty(), "{:?}", i.ops);
    assert!(why(&i, "--radius")[0].contains("is a color token, not a length"));
}
