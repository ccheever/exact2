//! HTML instants and runner-owned presentations. @ref LLP 1126 D6
use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Span};

pub(crate) fn check(tag: &str, attrs: &[Attr], span: Span) -> Result<(), LowerError> {
    let has = |name: &str| attrs.iter().any(|a| a.name == name);
    for a in attrs {
        if (matches!(a.name.as_str(), "datetime" | "format") && tag != "time")
            || (matches!(a.name.as_str(), "start" | "end") && tag != "progress")
        {
            return err(
                "lower-attr-tag",
                format!(
                    "`{}` on `{tag}` belongs to {}",
                    a.name,
                    if matches!(a.name.as_str(), "datetime" | "format") {
                        "`time`"
                    } else {
                        "an interval `progress`"
                    }
                ),
                a.span,
            );
        }
        if a.name == "datetime" && matches!(&a.value, Expr::Str(_, _)) {
            return err("lower-time-instant", "`datetime` is an instant in epoch milliseconds; write `datetime=date(\"2026-10-17T12:00:00Z\")` to parse an ISO date", a.span);
        }
        if a.name == "format"
            && !matches!(&a.value, Expr::Str(v, _) if exact_kernel::time::FORMATS.contains(&v.as_str()))
        {
            return err(
                "lower-time-format",
                format!(
                    "`format` is a named presentation: {}",
                    exact_kernel::time::FORMATS.join(", ")
                ),
                a.span,
            );
        }
    }
    if tag == "time" && !has("datetime") {
        return err(
            "lower-time-instant",
            "`time` needs `datetime`, an instant in epoch milliseconds",
            span,
        );
    }
    if tag == "progress"
        && (has("start") || has("end"))
        && (!has("start") || !has("end") || has("value") || has("max"))
    {
        return err(
            "lower-progress-interval",
            "write `progress start=… end=…` or `progress value=… max=…`",
            span,
        );
    }
    if tag == "progress" {
        let literal = |name| {
            attrs
                .iter()
                .find(|a| a.name == name)
                .and_then(|a| match a.value {
                    Expr::Number(n, _) => Some(n),
                    _ => None,
                })
        };
        if literal("start")
            .zip(literal("end"))
            .is_some_and(|(a, b)| b <= a)
        {
            return err(
                "lower-progress-interval",
                "write `progress start=… end=…` with `end` after `start`",
                span,
            );
        }
    }
    Ok(())
}
