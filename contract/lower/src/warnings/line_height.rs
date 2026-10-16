//! `lower-line-height-multiplier`: a unitless `line-height` no text is set
//! with. CSS reads a bare number there as a multiple of the font size (CSS
//! Inline 3 §5.1, `<number>`), while every length row reads one as pixels,
//! so a React Native author's `line-height=28` is 28 lines' worth: the euxy
//! port's 22-point header became 616 points tall. No app in this repository
//! writes a multiplier above 2; one of [`IMPLAUSIBLE`] or more is named.

use crate::LowerError;
use contract_syntax::{Attr, Expr};

/// The smallest multiplier taken to mean points.
const IMPLAUSIBLE: f64 = 4.0;

/// The warning for each literal `line-height` among `attrs` (an arm of a
/// choice included) that is a bare number of [`IMPLAUSIBLE`] or more.
pub(super) fn multiplier(attrs: &[Attr], out: &mut Vec<LowerError>) {
    for a in attrs.iter().filter(|a| a.name == "line-height") {
        let mut stack = vec![&a.value];
        while let Some(value) = stack.pop() {
            let n = match value {
                Expr::Number(n, _) => *n,
                Expr::Ternary(_, yes, no, _) => {
                    stack.extend([&**no, &**yes]);
                    continue;
                }
                Expr::Match { some, none, .. } => {
                    stack.extend([&**none, &**some]);
                    continue;
                }
                _ => continue,
            };
            if n >= IMPLAUSIBLE && n.is_finite() {
                out.push(LowerError {
                    id: "lower-line-height-multiplier",
                    message: format!("`line-height={n}` is {n} times the font size: a unitless line-height is a multiplier, as in CSS; for {n} points write `line-height=\"{n}px\"`"),
                    span: value.span(),
                });
            }
        }
    }
}
