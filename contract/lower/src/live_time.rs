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
                format!("`{}` belongs to `time` or an interval `progress`", a.name),
                a.span,
            );
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
    Ok(())
}
