//! LLP 1005 intersection visibility: the browser's rootMargin option.
use crate::{err, LowerError};
use contract_syntax::{Expr, Span};

fn css_number(number: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let mantissa = if let Some((mantissa, exponent)) = number.split_once('e') {
        if !digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent)) {
            return false;
        }
        mantissa
    } else {
        number
    };
    let mantissa = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
    if let Some((integer, fraction)) = mantissa.split_once('.') {
        (integer.is_empty() || digits(integer)) && digits(fraction)
    } else {
        digits(mantissa)
    }
}

pub(crate) fn margin(value: &Expr, span: Span) -> Result<Expr, LowerError> {
    let valid = if let Expr::Str(s, _) = value {
        let parts: Vec<_> = s.split_ascii_whitespace().collect();
        (1..=4).contains(&parts.len())
            && parts.iter().all(|part| {
                let part = part.to_ascii_lowercase();
                let number = part.strip_suffix("px").or_else(|| part.strip_suffix('%'));
                number.is_some_and(|n| css_number(n) && n.parse::<f64>().is_ok_and(f64::is_finite))
            })
    } else {
        false
    };
    match value {
        Expr::Str(s, at) if valid => Ok(Expr::Str(s.to_ascii_lowercase(), *at)),
        _ => err("lower-intersect-margin", "`intersect-margin` takes a literal rootMargin: one to four px or % lengths (for example \"200px 0px\")", span),
    }
}
