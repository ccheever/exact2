//! The folder's forms are the kernel's (LLP 1121 D7): a literal folds to
//! text the kernel parses to the same `ColorValue` as the literal itself,
//! and every value the corpus folds parses in the kernel.

use exact_kernel::style::StyleValue;
use exact_kernel::{Color, ColorValue, StyleId, StyleProps};
use exact_tamber::{check, Kind, Vocab};

use crate::common;

fn kernel(text: &str) -> Option<ColorValue> {
    ColorValue::parse_light_dark(text).or_else(|| Color::parse(text).map(ColorValue::Fixed))
}

const LITERALS: &[&str] = &[
    "#f0a",
    "#11223380",
    "rebeccapurple",
    "transparent",
    "rgb(10, 20, 30)",
    "rgb(10 20 30 / 50%)",
    "rgba(255, 0, 0, 0.25)",
    "hsl(210 50% 40%)",
    "hsl(210, 50%, 40%)",
    "hwb(210 10% 20%)",
    "oklch(0.7 0.1 200)",
    "oklab(0.5 0.1 -0.1 / 0.3)",
    "lab(50 20 -30 / 0.5)",
    "lch(60 40 120)",
    "color(display-p3 1 0 0)",
    "color(srgb 0.5 0.25 1)",
    "color(rec2020 0.2 0.4 0.6)",
    "light-dark(#fff, #000)",
    "light-dark(rgb(1 2 3), hsl(0 100% 50%))",
    "light-dark(#fff, oklch(0.3 0.05 250))",
    "light-dark(color(display-p3 0 1 0), #000)",
    "-exact-secondary-label",
    "Canvas",
    "LinkText",
];

#[test]
fn a_literal_folds_to_the_kernels_own_form() {
    let vocab = common::vocab();
    for literal in LITERALS {
        let src = format!("tamber parity\n\ntheme\n  t = {literal}\n");
        let folding = check(&src, &vocab).unwrap_or_else(|e| panic!("{literal}: {e:?}"));
        let folded = folding.value("t", 0).unwrap().css();
        let want = kernel(literal).unwrap_or_else(|| panic!("the kernel reads {literal}"));
        assert_eq!(kernel(&folded), Some(want), "{literal} folded to {folded}");
    }
}

#[test]
fn every_corpus_value_parses_in_the_kernel() {
    let vocab: Vocab = common::vocab();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "tamber") {
            continue;
        }
        let Ok(folding) = check(&std::fs::read_to_string(&path).unwrap(), &vocab) else {
            continue;
        };
        for token in &folding.tokens {
            for value in &token.values {
                let text = value.css();
                let read = match token.kind {
                    Kind::Color => kernel(&text).is_some(),
                    // A length in some length row the kernel has (a text
                    // style only as a font size, a hairline not as one).
                    Kind::Length => [StyleId::PaddingTop, StyleId::FontSize].iter().any(|row| {
                        StyleProps::default()
                            .set_dynamic(*row, &StyleValue::Text(text.as_str().into()))
                            .is_ok()
                    }),
                    Kind::Number => text.parse::<f64>().is_ok(),
                    Kind::BoxShadow => exact_kernel::style::BoxShadows::check(&text).is_ok(),
                    Kind::TextShadow => exact_kernel::style::TextShadow::check(&text).is_ok(),
                    // A component of a transition, as the build writes it in.
                    // (A spring is its own timing: no duration beside it.)
                    Kind::Duration | Kind::Easing => {
                        let forms = match token.kind {
                            Kind::Duration => vec![format!("opacity {text}")],
                            _ => vec![format!("opacity 1s {text}"), format!("opacity {text}")],
                        };
                        forms.into_iter().any(|transition| {
                            StyleProps::default()
                                .set_dynamic(
                                    StyleId::Transition,
                                    &StyleValue::Text(transition.into()),
                                )
                                .is_ok()
                        })
                    }
                };
                assert!(read, "{}: {}: {text}", path.display(), token.name);
            }
        }
    }
}

#[test]
fn a_tuple_is_named_by_its_modes() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/axes.tamber"
    ))
    .unwrap();
    let folding = check(&src, &common::vocab()).unwrap();
    assert_eq!(folding.tuples, 6);
    assert_eq!(folding.tuple(&[]), Some(0));
    let dim_compact = folding
        .tuple(&[("dark-style", "dim"), ("density", "compact")])
        .unwrap();
    assert_eq!(dim_compact, 4);
    assert_eq!(
        folding.value("surface.page", dim_compact).unwrap().css(),
        "light-dark(#fafafaff, #10161dff)"
    );
    assert_eq!(folding.tuple(&[("dark-style", "grey")]), None);
    assert_eq!(folding.tuple(&[("contrast", "more")]), None);
}
