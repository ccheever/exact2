//! Roster parameters written as literals at the call: a style (`"medium" |
//! "month-year"`) or a digit count (`0..=100`).

use crate::{err, TypeError};
use contract_syntax::{Expr, Span};

/// A roster parameter spelled as string literals (`"medium" | "month-year"`)
/// takes one of them, written as a literal: a style is chosen where the call
/// is written, never computed or forwarded (@ref LLP 1054.000.003 D9; the
/// precedent is `path()`'s route name).
pub(crate) fn literal_argument(
    name: &str,
    i: usize,
    spec: &str,
    arg: &Expr,
) -> Result<(), TypeError> {
    let written = match arg {
        Expr::Str(value, _) => {
            let quoted = format!("\"{value}\"");
            if spec.split(" | ").any(|choice| choice == quoted) {
                return Ok(());
            }
            format!("`{quoted}`")
        }
        _ => "an expression".into(),
    };
    err(
        "type-format-style",
        format!(
            "argument {} of `{name}` is one of {}, written as a string literal; given {written}",
            i + 1,
            spec.split(" | ")
                .map(|c| format!("`{c}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        arg.span(),
    )
}

/// A roster parameter spelled as a range (`0..=100`) takes a whole number in
/// it, written as a number literal, as a style is (@ref LLP 1102 §3.2:
/// `toFixed`'s and `formatDecimal`'s digits; JavaScript's `toFixed` throws a
/// RangeError past 100).
pub(crate) fn digits_argument(
    name: &str,
    i: usize,
    (lo, hi): (&str, &str),
    arg: &Expr,
) -> Result<(), TypeError> {
    let written = match arg {
        Expr::Number(v, _) => {
            if v.fract() == 0.0
                && (lo.parse().unwrap_or(0.0)..=hi.parse().unwrap_or(0.0)).contains(v)
            {
                return Ok(());
            }
            format!("`{v}`")
        }
        _ => "an expression".into(),
    };
    err(
        "type-literal-digits",
        format!(
            "argument {} of `{name}` is a whole number from {lo} to {hi}, written as a number literal; given {written}",
            i + 1
        ),
        arg.span(),
    )
}

/// `formatNumber`'s third argument, a currency's ISO 4217 code, is written
/// with `"currency"` and only with it (@ref LLP 1116 D8): the code picks the
/// symbol and the fraction digits, and no other style has one.
pub(crate) fn currency_code(args: &[Expr], span: Span) -> Result<(), TypeError> {
    let currency = matches!(args.get(1), Some(Expr::Str(style, _)) if style == "currency");
    match (currency, args.get(2)) {
        (true, None) => err(
            "type-format-style",
            "`formatNumber(n, \"currency\", code)` names its currency: an ISO 4217 code written as a string literal, as in `formatNumber(total, \"currency\", \"USD\")`",
            span,
        ),
        (false, Some(code)) => err(
            "type-format-style",
            "argument 3 of `formatNumber` is a currency's ISO 4217 code, written only with `\"currency\"` (`formatNumber(total, \"currency\", \"USD\")`); `\"compact\"`, `\"decimal\"` and `\"percent\"` take two arguments",
            code.span(),
        ),
        _ => Ok(()),
    }
}
