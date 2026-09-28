//! CSS `clip-path`: `none`, `path([<fill-rule>,]? <string>)` in CSS pixels
//! from the border box, or (on an SVG element) a `clipPath` by reference,
//! `url(#id)` (LLP 1055.000 D10). The string is SVG path data in full, read
//! by the SVG path parser (LLP 1055.000 addendum A); the parsed commands
//! (absolute `M L C Z`) cross the Apple batch, so presenters parse no CSS.

use crate::generated::FillRule;
use crate::svg::{parse_d_whole, Paint, PaintFallback, Seg};

/// A validated clipping path. The initial value has no clipping.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipPath {
    commands: Vec<(char, Vec<f32>)>,
    rule: FillRule,
    url: Option<Box<str>>,
}

impl ClipPath {
    /// `none`, `url(#id)`, or CSS's `path()`: an optional fill rule, then
    /// SVG path data in a quoted string. Data with any error is refused
    /// whole, as CSS refuses the declaration; so is data that draws nothing.
    /// On the web, once linked ([`crate::style::link_effects`]).
    pub fn parse(css: &str) -> Option<Self> {
        crate::style::effects::parse(css, |e| e.clip_path)
    }

    /// [`Self::parse`]'s grammar.
    pub(crate) fn grammar(css: &str) -> Option<Self> {
        let css = css.trim();
        if css == "none" {
            return Some(Self::default());
        }
        if let Some(Paint::Url(id, PaintFallback::Default)) = Paint::parse(css) {
            return Some(Self {
                url: Some(id),
                ..Self::default()
            });
        }
        let inner = css.strip_prefix("path(")?.strip_suffix(')')?.trim();
        let (rule, inner) = match inner.split_once(',') {
            Some((rule, rest)) if !rule.trim_start().starts_with(['"', '\'']) => {
                (FillRule::from_name(rule.trim())?, rest.trim())
            }
            _ => (FillRule::Nonzero, inner),
        };
        let quote = inner.chars().next()?;
        if quote != '\'' && quote != '"' {
            return None;
        }
        let data = inner.strip_prefix(quote)?.strip_suffix(quote)?;
        if data.contains(quote) {
            return None;
        }
        let commands = parse_d_whole(data)?
            .0
            .into_iter()
            .map(|seg| match seg {
                Seg::Move(x, y) => ('M', vec![x, y]),
                Seg::Line(x, y) => ('L', vec![x, y]),
                Seg::Cubic(a, b, c, d, x, y) => ('C', vec![a, b, c, d, x, y]),
                Seg::Close => ('Z', Vec::new()),
            })
            .collect();
        Some(Self {
            commands,
            rule,
            url: None,
        })
    }

    /// Which points are inside: CSS's `nonzero` unless the value said
    /// `evenodd`.
    pub fn rule(&self) -> FillRule {
        self.rule
    }

    /// The `clipPath` a `url(#id)` names, if this is one.
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Empty for `none`; otherwise drawing commands in border-box coordinates.
    pub fn commands(&self) -> &[(char, Vec<f32>)] {
        &self.commands
    }

    /// Canonical, validated CSS (also the wire representation).
    pub fn css(&self) -> String {
        if let Some(id) = &self.url {
            return format!("url(#{id})");
        }
        if self.commands.is_empty() {
            return "none".into();
        }
        use std::fmt::Write as _;
        let mut text = String::from("path(");
        if self.rule == FillRule::Evenodd {
            text.push_str("evenodd, ");
        }
        text.push('"');
        for (i, (command, values)) in self.commands.iter().enumerate() {
            if i > 0 {
                text.push(' ');
            }
            text.push(*command);
            for value in values {
                let _ = write!(text, " {}", exact_num::Shortest32(*value));
            }
        }
        text.push_str("\")");
        text
    }
}

#[cfg(test)]
mod tests {
    use super::ClipPath;
    use crate::{
        wire::codec::{Reader, Writer},
        DecodeError, StyleId, StyleMask, StyleProps, StyleValue,
    };

    #[test]
    fn generated_patch_codec_preserves_curves_and_refuses_bad_wire_data() {
        let mut style = StyleProps::default();
        style
            .set_dynamic(
                StyleId::ClipPath,
                &StyleValue::Text("path('M 0 0 C 1 2 3 4 5 6 Z')".into()),
            )
            .unwrap();
        let mut bytes = Writer::new();
        style.encode_patch(&mut bytes);
        assert_eq!(
            StyleProps::decode_patch(&mut Reader::new(bytes.as_slice())).unwrap(),
            style
        );
        assert!(!StyleId::ClipPath.affects_layout());
        let mut bad = Writer::new();
        bad.style_mask(StyleMask::of(StyleId::ClipPath));
        bad.string("path('M 0 NaN')");
        assert_eq!(
            StyleProps::decode_patch(&mut Reader::new(bad.as_slice())),
            Err(DecodeError::BadClipPath)
        );
    }

    #[test]
    fn curves_round_trip_and_invalid_paths_are_refused() {
        // Quadratics (and arcs) arrive as cubics: presenters draw four verbs.
        let path = ClipPath::parse("path('M0,0 C0,10 5,18 20,18 Q14,15 14,0 Z')").unwrap();
        assert_eq!(
            path.css(),
            "path(\"M 0 0 C 0 10 5 18 20 18 C 16 16 14 10 14 0 Z\")"
        );
        assert_eq!(ClipPath::parse(&path.css()), Some(path));
        assert_eq!(ClipPath::default().css(), "none");
        // SVG path data in full, relative and implicit commands included, and
        // a fill rule before it.
        let full = ClipPath::parse("path(evenodd, 'm10 10 h20 v20 h-20 z M0 0 l5 5')").unwrap();
        assert_eq!(full.rule(), crate::generated::FillRule::Evenodd);
        assert_eq!(
            full.css(),
            "path(evenodd, \"M 10 10 L 30 10 L 30 30 L 10 30 Z M 0 0 L 5 5\")"
        );
        assert_eq!(ClipPath::parse(&full.css()), Some(full));
        for bad in [
            "path('')",
            "path('M0 0 Z 1')",
            "path('M0 0 z 1')",
            "path('L 0 0')",
            "path('M 0 NaN')",
            "path('M 0 0 C 1 2')",
            "path('M 0 0 L 1 1,')",
            "path(winding, 'M 0 0 L 1 1')",
            "path('M 0 0');color:red",
        ] {
            assert!(ClipPath::parse(bad).is_none(), "{bad}");
        }
    }
}
