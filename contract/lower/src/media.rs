//! Media-specific validation; the schema still owns every prop declaration.
use crate::{err, numeric_literal, LowerError};
use contract_syntax::{Expr, Span};

pub(crate) fn check(name: &str, value: &Expr, span: Span) -> Result<(), LowerError> {
    if let Expr::Str(text, _) = value {
        let allowed: Option<&[&str]> = match name {
            "preload" => Some(&["", "none", "metadata", "auto"]),
            "crossorigin" => Some(&["", "anonymous", "use-credentials"]),
            _ => None,
        };
        if allowed.is_some_and(|choices| !choices.contains(&text.as_str())) {
            return err(
                "lower-attr-value",
                format!("invalid `{name}` value `{text}`"),
                span,
            );
        }
        if name == "controlslist"
            && text
                .split_whitespace()
                .any(|token| !["nodownload", "nofullscreen", "noremoteplayback"].contains(&token))
        {
            return err("lower-attr-value", "unknown controlslist token", span);
        }
    }
    if let Some(number) = numeric_literal(value) {
        let range = match name {
            "volume" => Some((0.0, 1.0)),
            "playbackRate" => Some((0.25, 4.0)),
            "currentTime" | "preferredPeakBitRate" | "preferredForwardBufferDuration" => {
                Some((0.0, f64::MAX))
            }
            _ => None,
        };
        if range.is_some_and(|(min, max)| !number.is_finite() || number < min || number > max) {
            return err(
                "lower-attr-value",
                format!("`{name}` is outside its supported range"),
                span,
            );
        }
    }
    Ok(())
}
