//! Motion tokens (LLP 1121 D3, D13; stage 6 E): a `duration` is a CSS
//! time, an `easing` a keyword or an easing function. Each is a component
//! of a `transition` or `animation` value, substituted into it at build, so
//! it has one value (no `when`) and an edit to it rebuilds.

use crate::css::Item;
use crate::fold::{fail, FoldError, Named, Names};
use crate::measure::num;

/// The easing keywords CSS and the motion crate read.
pub(crate) const KEYWORDS: &[&str] = &[
    "linear",
    "ease",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "step-start",
    "step-end",
];

/// The easing functions (`motion/src/parse.rs`'s `EASING_FUNCTIONS`).
const FUNCTIONS: &[&str] = &["cubic-bezier", "steps", "linear", "-exact-spring"];

/// Whether a value reads as a duration or an easing, so its kind is
/// inferred: a time, an easing keyword, an easing function.
pub(crate) fn inferred(items: &[Item]) -> Option<crate::Kind> {
    match items {
        [Item::Dim(_, unit)] if unit == "ms" || unit == "s" => Some(crate::Kind::Duration),
        [Item::Ident(w)] if KEYWORDS.contains(&w.as_str()) => Some(crate::Kind::Easing),
        [Item::Func(f, ..)] if FUNCTIONS.contains(&f.as_str()) => Some(crate::Kind::Easing),
        _ => None,
    }
}

/// Fold a whole value as a duration: milliseconds.
pub(crate) fn duration(items: &[Item], names: &mut dyn Names) -> Result<f64, FoldError> {
    let [one] = items else {
        return fail("tamber-kind", "a duration is one time");
    };
    match one {
        Item::Dim(n, unit) if unit == "ms" => Ok(*n),
        Item::Dim(n, unit) if unit == "s" => Ok(*n * 1000.0),
        Item::Number(n) if *n == 0.0 => Ok(0.0),
        Item::Ident(name) => match names.named(name)? {
            Named::Duration(ms) => Ok(ms),
            Named::Unknown => fail(
                "tamber-unknown-token",
                format!("`{name}` is no duration token"),
            ),
            _ => fail("tamber-kind", format!("`{name}` is not a duration")),
        },
        Item::Dim(n, unit) => fail(
            "tamber-kind",
            format!("`{}{unit}`: a duration is in `ms` or `s`", num(*n)),
        ),
        _ => fail("tamber-kind", "a duration is a time, `150ms` or `0.15s`"),
    }
}

/// Fold a whole value as an easing: its text as written.
pub(crate) fn easing(items: &[Item], names: &mut dyn Names) -> Result<String, FoldError> {
    let [one] = items else {
        return fail("tamber-kind", "an easing is one keyword or function");
    };
    match one {
        Item::Ident(w) if KEYWORDS.contains(&w.as_str()) => Ok(w.clone()),
        Item::Ident(name) => match names.named(name)? {
            Named::Easing(text) => Ok(text),
            Named::Unknown => fail(
                "tamber-unknown-token",
                format!("`{name}` is no easing token or keyword"),
            ),
            _ => fail("tamber-kind", format!("`{name}` is not an easing")),
        },
        Item::Func(f, args, text) if FUNCTIONS.contains(&f.as_str()) => {
            let numbers = || -> Option<Vec<f64>> {
                args.iter()
                    .map(|a| match a.as_slice() {
                        [Item::Number(n)] => Some(*n),
                        _ => None,
                    })
                    .collect()
            };
            match f.as_str() {
                "cubic-bezier" => match numbers() {
                    Some(n)
                        if n.len() == 4
                            && (0.0..=1.0).contains(&n[0])
                            && (0.0..=1.0).contains(&n[2]) => {}
                    _ => {
                        return fail(
                            "tamber-kind",
                            format!("`{text}`: four numbers, the first and third between 0 and 1"),
                        )
                    }
                },
                "steps" => match args.first().map(Vec::as_slice) {
                    Some([Item::Number(n)]) if *n >= 1.0 && n.fract() == 0.0 => {}
                    _ => {
                        return fail(
                            "tamber-kind",
                            format!("`{text}`: a whole number of steps first"),
                        )
                    }
                },
                _ => {}
            }
            Ok(text.clone())
        }
        _ => fail(
            "tamber-kind",
            "an easing is a keyword or an easing function",
        ),
    }
}
