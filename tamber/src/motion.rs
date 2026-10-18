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

/// Whether `transition` reads as the motion crate reads a transition, and
/// its timing is one the sampler takes.
fn reads(transition: &str) -> bool {
    exact_motion::Transitions::parse(transition).is_ok_and(|t| t.validate().is_ok())
}

/// Fold a whole value as a duration: milliseconds, one a transition takes.
pub(crate) fn duration(items: &[Item], names: &mut dyn Names) -> Result<f64, FoldError> {
    let [one] = items else {
        return fail("tamber-kind", "a duration is one time");
    };
    match one {
        Item::Dim(n, unit) if unit == "ms" || unit == "s" => {
            let ms = if unit == "s" { *n * 1000.0 } else { *n };
            if reads(&format!("opacity {}ms", num(ms))) {
                Ok(ms)
            } else {
                fail(
                    "tamber-kind",
                    format!("`{}{unit}` is no duration a transition takes", num(*n)),
                )
            }
        }
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

/// Fold a whole value as an easing: its text as written, one a transition
/// takes (with a duration beside it, or as a spring is written, alone). A
/// keyword's spelling that is a declared token's or dial's name is that.
pub(crate) fn easing(items: &[Item], names: &mut dyn Names) -> Result<String, FoldError> {
    let [one] = items else {
        return fail("tamber-kind", "an easing is one keyword or function");
    };
    let text = match one {
        Item::Ident(name) => match names.named(name)? {
            Named::Easing(text) => return Ok(text),
            Named::Unknown if KEYWORDS.contains(&name.as_str()) => name.clone(),
            Named::Unknown => {
                return fail(
                    "tamber-unknown-token",
                    format!("`{name}` is no easing token or keyword"),
                )
            }
            _ => return fail("tamber-kind", format!("`{name}` is not an easing")),
        },
        Item::Func(f, _, text) if FUNCTIONS.contains(&f.as_str()) => text.clone(),
        _ => {
            return fail(
                "tamber-kind",
                "an easing is a keyword or an easing function",
            )
        }
    };
    if reads(&format!("opacity 1s {text}")) || reads(&format!("opacity {text}")) {
        Ok(text)
    } else {
        fail(
            "tamber-kind",
            format!("`{text}` is no easing a transition takes"),
        )
    }
}
