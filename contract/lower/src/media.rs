//! Media-specific validation; the schema still owns every prop declaration.
use crate::{err, values::numeric_literal, LowerError};
use contract_syntax::{Attr, Expr, Span};

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
            "volume" | "playbackVisibilityThreshold" => Some((0.0, 1.0)),
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

/// HTML's `<audio>` (LLP 1042 §8): the media node a `video` is, marked so
/// the web makes an `<audio>`; [`audio_rows`] gives it HTML's box.
pub(crate) const AUDIO: crate::tags::Tag = crate::tags::Tag {
    node_type: exact_kernel::NodeType::Video,
    fixed_styles: &[],
    fixed_props: &[(exact_kernel::PropId::SemanticTag, "audio")],
    positional: Some(exact_kernel::PropId::Src),
};

/// HTML's `<audio>` (LLP 1042 §8): no picture, so no `poster`, inline-play
/// or visibility policy; a box only with `controls`. Chrome's controls are
/// 300×54 (the author's `width`/`height` win), and the UA's
/// `audio:not([controls]) { display: none !important }` is appended last,
/// so no authored `display` shows an audio without controls; a bound
/// `controls` binds `display` with it. `None` for any other tag.
pub(crate) fn audio_rows(
    tag: &str,
    attrs: &[Attr],
    span: Span,
) -> Result<Option<Vec<Attr>>, LowerError> {
    if tag != "audio" {
        return Ok(None);
    }
    if let Some(a) = attrs.iter().find(|a| {
        matches!(
            a.name.as_str(),
            "poster" | "playsinline" | "playbackVisibilityThreshold"
        )
    }) {
        return err(
            "lower-attr-tag",
            format!(
                "`{}` belongs to `video`: an `audio` shows no picture",
                a.name
            ),
            a.span,
        );
    }
    let attr = |name: &str, value: Expr| Attr {
        name: name.into(),
        value,
        span,
    };
    let sized = || {
        [
            attr("width", Expr::Number(300.0, span)),
            attr("height", Expr::Number(54.0, span)),
        ]
    };
    let shown = attrs
        .iter()
        .rev()
        .find(|a| a.name == "display")
        .map(|a| a.value.clone());
    let mut rows: Vec<Attr> = Vec::with_capacity(attrs.len() + 3);
    match attrs
        .iter()
        .rev()
        .find(|a| a.name == "controls")
        .map(|a| &a.value)
    {
        Some(Expr::Bool(true, _)) => {
            rows.extend(sized());
            rows.extend(attrs.iter().cloned());
        }
        None | Some(Expr::Bool(false, _)) => {
            rows.extend(attrs.iter().filter(|a| a.name != "display").cloned());
            rows.push(attr("display", Expr::Str("none".into(), span)));
        }
        Some(bound) => {
            rows.extend(sized());
            rows.extend(attrs.iter().filter(|a| a.name != "display").cloned());
            let on = shown.unwrap_or_else(|| Expr::Str("block".into(), span));
            let display = Expr::Ternary(
                Box::new(bound.clone()),
                Box::new(on),
                Box::new(Expr::Str("none".into(), span)),
                span,
            );
            rows.push(attr("display", display));
        }
    }
    Ok(Some(rows))
}
