//! Media-specific validation; the schema still owns every prop declaration.
use crate::{err, expr, values::numeric_literal, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Span};
use contract_types::{Scope, Ty};
use exact_kernel::PropId;
use exact_plan::{asm::Asm, BindingKind, BindingsRow};

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
        // @ref LLP 1098 D2 — a seek moves a number of seconds, some.
        if SESSION_OFFSETS.contains(&name) && !(number.is_finite() && number > 0.0) {
            return err(
                "lower-attr-value",
                format!("`{name}` is a number of seconds greater than 0"),
                span,
            );
        }
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

/// The media session's actions, by `setActionHandler`'s names (LLP 1098 D2).
pub const SESSION_ACTIONS: [&str; 6] = [
    "seekbackward",
    "seekforward",
    "seekto",
    "previoustrack",
    "nexttrack",
    "stop",
];

/// The seconds each seek moves when the platform gives none (LLP 1098 D2).
const SESSION_OFFSETS: [&str; 2] = ["seekbackwardOffset", "seekforwardOffset"];

/// `MediaMetadata`'s fields and the kernel props they lower to (LLP 1098 D1).
const METADATA: [(&str, PropId); 4] = [
    ("title", PropId::MediaTitle),
    ("artist", PropId::MediaArtist),
    ("album", PropId::MediaAlbum),
    ("artwork", PropId::MediaArtwork),
];

/// @ref LLP 1098 D1, D2 — the media session belongs to HTML's media
/// element: `metadata=` only on `audio` and `video`, and an action or an
/// offset only on one that claims the session with it.
pub(crate) fn check_session(tag: &str, attrs: &[Attr]) -> Result<(), LowerError> {
    let media = matches!(tag, "audio" | "video");
    let claims = attrs.iter().any(|a| a.name == "metadata");
    if let Some(a) = attrs
        .iter()
        .find(|a| a.name == "metadata")
        .filter(|_| !media)
    {
        return err(
            "lower-attr-tag",
            format!("`metadata` belongs to `audio` or `video`, not `{tag}`: it claims the media session for a player"),
            a.span,
        );
    }
    let session = |a: &&Attr| {
        SESSION_ACTIONS.contains(&a.name.as_str()) || SESSION_OFFSETS.contains(&a.name.as_str())
    };
    match attrs.iter().find(session) {
        Some(a) if !media => err(
            "lower-attr-tag",
            format!(
                "`{}` belongs to an `audio` or `video`'s media session, not `{tag}`",
                a.name
            ),
            a.span,
        ),
        Some(a) if !claims => err(
            "lower-media-session",
            format!(
                "`{}=` belongs to a media session: give this `{tag}` a `metadata=`",
                a.name
            ),
            a.span,
        ),
        _ => Ok(()),
    }
}

impl Lowerer<'_> {
    /// @ref LLP 1098 D1 — `metadata=MediaMetadata(…)`: a binding a field,
    /// as `head`'s `title` binds `headTitle`. A literal record binds each
    /// field's own expression, so a field reads only what it names; any
    /// other `MediaMetadata` value is read a field at a time.
    pub(crate) fn media_metadata(
        &mut self,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        let ty = expr::compile(self, &mut Asm::new(), &a.value, scope, &mut { locals })?;
        if ty != Ty::Record("MediaMetadata".into()) {
            return err(
                "lower-attr-type",
                format!("`metadata` takes a `MediaMetadata(title=…, artist=…, album=…, artwork=…)`, given `{ty}`"),
                a.span,
            );
        }
        let literal = match &a.value {
            Expr::Call(name, args, _)
                if name == "MediaMetadata" && contract_types::records::base(args).is_none() =>
            {
                Some(args)
            }
            _ => None,
        };
        for (field, prop) in METADATA {
            let named = literal.and_then(|args| {
                args.iter().find_map(|arg| match arg {
                    Expr::NamedArg(n, value, _) if n == field => Some(value.as_ref().clone()),
                    _ => None,
                })
            });
            let value = named
                .unwrap_or_else(|| Expr::Member(Box::new(a.value.clone()), field.into(), a.span));
            let (code, _) = self.typed_code(&value, scope, locals)?;
            bindings.push(BindingsRow {
                kind: BindingKind::Prop,
                id: prop as u16,
                expr: code,
            });
        }
        Ok(())
    }
}
