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
    let first = shadcn::import(&css, "shadcn", &parse(EMPTY).unwrap()).unwrap();
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
        .find(|u| u.name == "--radius" && u.block == ":root")
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
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap()).unwrap();
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
    let import = shadcn::import(&css, "shadcn", &parse(EMPTY).unwrap()).unwrap();
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
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}

#[test]
fn a_further_shadcn_theme_class_is_a_proposed_axis() {
    let css = fixture("shadcn-neutral-themes.css");
    let import = shadcn::import(&css, "shadcn", &parse(EMPTY).unwrap()).unwrap();
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
    let again = shadcn::import(&css, "shadcn", &parse(&text).unwrap()).unwrap();
    assert_eq!(again.ops, []);
}
