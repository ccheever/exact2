//! `transform` on a box. LLP 1055.000 D5 deferred the property on boxes
//! (matrices, skews, functions in any order) and built the individual
//! `translate`, `rotate` and `scale` rows instead. CSS applies those rows
//! in that order, before `transform` (CSS Transforms 2 §6.1), so a
//! `transform` that is each of those functions at most once, in that order,
//! is the rows it names, and lowers to them: `transform="translate(10px)
//! rotate(45deg) scale(2)"` is `translate="10px" rotate="45deg" scale=2`,
//! where `scale(2) translate(10px)` is another transform and stays refused.
//! A literal is read wherever the lowering can see it, through a class and
//! each arm of a choice; a bound string is refused as before.

use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Span};

/// The rows, in the order CSS applies them.
const ROWS: [&str; 3] = ["translate", "rotate", "scale"];

/// A box's `transform` attribute rewritten as its rows, `None` when the
/// element has none or is an SVG element (whose `transform` is the row
/// itself). `in_svg` says the element is inside an `svg`.
pub(crate) fn lower(
    tag: &str,
    in_svg: bool,
    attrs: &[Attr],
) -> Result<Option<Vec<Attr>>, LowerError> {
    // The elements `check_svg` counts as SVG.
    if crate::svg::is_element(tag) || tag == "svg" || (tag == "text" && in_svg) {
        return Ok(None);
    }
    let Some(index) = attrs
        .iter()
        .position(|a| a.name == "transform" && !crate::native::leftover(tag, a))
    else {
        return Ok(None);
    };
    let transform = &attrs[index];
    if let Some(row) = attrs.iter().find(|a| ROWS.contains(&a.name.as_str())) {
        return err(
            "lower-attr-tag",
            format!(
                "`transform` and `{row}` on one box: `transform` lowers to the `translate`, `rotate` and `scale` rows, so both set `{row}`. Keep the rows, writing `transform`'s functions as rows (`translate(0px, 25px)` is `translate=\"0px 25px\"`), or keep `transform` alone with `{row}`'s value as its function",
                row = row.name
            ),
            transform.span,
        );
    }
    let rows = split(&transform.value).map_err(|clause| LowerError {
        id: "lower-attr-tag",
        message: format!("{}; here {clause}", crate::vocab::BOX_TRANSFORM_REFUSAL),
        span: transform.span,
    })?;
    let mut out = attrs.to_vec();
    out.remove(index);
    for (name, value) in ROWS.into_iter().zip(rows) {
        if matches!(value, Expr::None(_)) {
            continue;
        }
        out.insert(
            index,
            Attr {
                name: name.into(),
                value,
                span: transform.span,
                written: transform.written,
            },
        );
    }
    Ok(Some(out))
}

/// The value's three rows, `none` for one its literal does not set. A
/// choice between literals is the same choice per row, with `none` on a
/// side that lacks it, as a class choice's rows are (`class::rows`); a
/// string the compiler cannot read is the refusal's clause.
fn split(value: &Expr) -> Result<[Expr; 3], String> {
    match value {
        Expr::Str(text, span) => literal(text, *span),
        Expr::None(span) => Ok([Expr::None(*span), Expr::None(*span), Expr::None(*span)]),
        Expr::Ternary(cond, yes, no, span) => {
            let (yes, no) = (split(yes)?, split(no)?);
            Ok(yes
                .into_iter()
                .zip(no)
                .map(|(a, b)| choice(a, b, *span, |a, b| Expr::Ternary(cond.clone(), a, b, *span)))
                .collect::<Vec<_>>()
                .try_into()
                .expect("three rows"))
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let (some, none) = (split(some)?, split(none)?);
            Ok(some
                .into_iter()
                .zip(none)
                .map(|(a, b)| {
                    choice(a, b, *span, |some, none| Expr::Match {
                        subject: subject.clone(),
                        var: var.clone(),
                        some,
                        none,
                        span: *span,
                    })
                })
                .collect::<Vec<_>>()
                .try_into()
                .expect("three rows"))
        }
        Expr::Let {
            name,
            value,
            body,
            span,
        } => Ok(split(body)?.map(|row| match row {
            Expr::None(_) => row,
            body => Expr::Let {
                name: name.clone(),
                value: value.clone(),
                body: Box::new(body),
                span: *span,
            },
        })),
        _ => Err("the value is a string the compiler cannot read; bind `translate`, `rotate` or `scale` instead".into()),
    }
}

/// One row of a choice: unset on both sides is unset, the same literal on
/// both is that literal, anything else the choice.
fn choice(a: Expr, b: Expr, span: Span, build: impl FnOnce(Box<Expr>, Box<Expr>) -> Expr) -> Expr {
    match (&a, &b) {
        (Expr::None(_), Expr::None(_)) => Expr::None(span),
        (Expr::Str(x, _), Expr::Str(y, _)) if x == y => a,
        (Expr::Number(x, _), Expr::Number(y, _)) if x == y => a,
        _ => build(Box::new(a), Box::new(b)),
    }
}

/// A literal's rows: `none` sets none; otherwise each function is its row,
/// in the rows' order, once. Anything else is the clause the refusal ends
/// with.
fn literal(text: &str, span: Span) -> Result<[Expr; 3], String> {
    let mut rows = [Expr::None(span), Expr::None(span), Expr::None(span)];
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("none") {
        return Ok(rows);
    }
    let mut rest = text;
    let mut last: Option<usize> = None;
    while !rest.is_empty() {
        let Some(open) = rest.find('(') else {
            return Err(format!("`{rest}` is not a transform function"));
        };
        let name = rest[..open].trim();
        let Some(close) = rest[open..].find(')').map(|c| c + open) else {
            return Err(format!("`{}(` has no closing parenthesis", name));
        };
        let args: Vec<&str> = rest[open + 1..close]
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .collect();
        let (index, value) = function(name, &args, span)?;
        match last {
            Some(l) if l == index => {
                return Err(format!("`{}(…)` is written twice", ROWS[index]));
            }
            Some(l) if l > index => {
                return Err(format!(
                    "`{}(…)` comes before `{}(…)`, which is another transform: the rows apply translate, then rotate, then scale (CSS Transforms 2 §6.1)",
                    ROWS[index], ROWS[l]
                ));
            }
            _ => {}
        }
        last = Some(index);
        rows[index] = value;
        rest = rest[close + 1..].trim_start();
    }
    Ok(rows)
}

/// A function as its row's index and value. The row's own parser reads a
/// length or an angle when the row lowers; a number is read here, since
/// `scale(x, y)` must be one factor and the `scale` row is a number.
fn function(name: &str, args: &[&str], span: Span) -> Result<(usize, Expr), String> {
    let lower = name.to_ascii_lowercase();
    let one_factor = |a: &str| -> Result<f64, String> {
        let (n, k) = match a.strip_suffix('%') {
            Some(n) => (n, 0.01),
            None => (a, 1.0),
        };
        exact_num::parse_f64(n)
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| n * k)
            .ok_or_else(|| format!("`scale({a})` is not a number"))
    };
    Ok(match (lower.as_str(), args) {
        ("translate", [x]) => (0, Expr::Str((*x).into(), span)),
        ("translate", [x, y]) => (0, Expr::Str(format!("{x} {y}"), span)),
        ("translatex", [x]) => (0, Expr::Str((*x).into(), span)),
        ("translatey", [y]) => (0, Expr::Str(format!("0 {y}"), span)),
        ("translate" | "translatex" | "translatey", _) => {
            return Err(format!("`{name}(…)` takes one or two lengths"));
        }
        ("rotate" | "rotatez", [a]) => (1, Expr::Str((*a).into(), span)),
        ("rotate" | "rotatez", _) => return Err(format!("`{name}(…)` takes one angle")),
        ("scale", [n]) => (2, Expr::Number(one_factor(n)?, span)),
        ("scale", [x, y]) => {
            let (x, y) = (one_factor(x)?, one_factor(y)?);
            if x != y {
                return Err(format!(
                    "`scale({}, {})` scales the axes differently; the `scale` row is one factor for both",
                    args[0], args[1]
                ));
            }
            (2, Expr::Number(x, span))
        }
        ("scale", _) => return Err("`scale(…)` takes one or two numbers".into()),
        ("scalex" | "scaley" | "scalez" | "scale3d", _) => {
            return Err(format!(
                "`{name}(…)` scales one axis; the `scale` row is one factor for both"
            ));
        }
        ("matrix" | "matrix3d" | "skew" | "skewx" | "skewy", _) => {
            return Err(format!("`{name}(…)` has no individual property"));
        }
        ("translatez" | "translate3d" | "rotatex" | "rotatey" | "rotate3d" | "perspective", _) => {
            return Err(format!("`{name}(…)` is a 3D function"));
        }
        _ => return Err(format!("`{name}(…)` is not a transform function")),
    })
}
