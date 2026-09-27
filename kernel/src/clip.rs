//! CSS `clip-path`: `none` or `path([<fill-rule>,]? <string>)`, in CSS
//! pixels from the border box. The string is SVG path data in full, parsed
//! by [`crate::vector`] (LLP 1065 D8), so presenters draw the kernel's
//! normalized commands (absolute `M L C Z`) and parse no CSS.

use crate::generated::FillRule;
use crate::vector::{commands_css, Command, PathData};

/// A validated clipping path. The initial value has no clipping.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipPath {
    commands: Vec<Command>,
    rule: FillRule,
}

impl ClipPath {
    /// CSS's `path()`: an optional fill rule, then SVG path data in a quoted
    /// string. Data with any error is refused whole, as CSS refuses the
    /// declaration; so is data that draws nothing.
    pub fn parse(css: &str) -> Option<Self> {
        let css = css.trim();
        if css == "none" {
            return Some(Self::default());
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
        let path = PathData::parse(data);
        if path.error().is_some() || path.commands().is_empty() {
            return None;
        }
        Some(Self {
            commands: path.commands().to_vec(),
            rule,
        })
    }

    /// Empty for `none`; otherwise normalized drawing commands in border-box
    /// coordinates.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Which points are inside: CSS's `nonzero` unless the value said
    /// `evenodd`.
    pub fn rule(&self) -> FillRule {
        self.rule
    }

    /// Canonical, validated CSS (also the wire representation).
    pub fn css(&self) -> String {
        if self.commands.is_empty() {
            return "none".into();
        }
        let rule = match self.rule {
            FillRule::Nonzero => "",
            FillRule::Evenodd => "evenodd, ",
        };
        format!("path({rule}\"{}\")", commands_css(&self.commands))
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
        let path = ClipPath::parse("path('M0,0 C0,10 5,18 20,18 Q14,15 14,0 Z')").unwrap();
        // A quadratic is its exact cubic (LLP 1065 D8).
        assert_eq!(
            path.css(),
            "path(\"M 0 0 C 0 10 5 18 20 18 C 16 16 14 10 14 0 Z\")"
        );
        assert_eq!(ClipPath::parse(&path.css()), Some(path));
        assert_eq!(ClipPath::default().css(), "none");
        for bad in [
            "path('')",
            "path('L 0 0')",
            "path('M 0 NaN')",
            "path('M 0 0 C 1 2')",
            "path('M 0 0 X')",
            "path(inside, 'M 0 0 H 1')",
            "path('M 0 0');color:red",
        ] {
            assert!(ClipPath::parse(bad).is_none(), "{bad}");
        }
    }

    /// SVG's whole grammar, as the `path` node's `d` (LLP 1065 D8): relative
    /// and shorthand commands, arcs, and CSS's optional fill rule.
    #[test]
    fn the_full_path_grammar_and_a_fill_rule() {
        let relative = ClipPath::parse("path('m0 0 h10 v10 h-10 z')").unwrap();
        assert_eq!(relative.css(), "path(\"M 0 0 L 10 0 L 10 10 L 0 10 Z\")");
        assert_eq!(relative.rule(), crate::FillRule::Nonzero);
        let arc = ClipPath::parse("path(evenodd, \"M0 10 A10 10 0 0 1 20 10 Z\")").unwrap();
        assert_eq!(arc.rule(), crate::FillRule::Evenodd);
        assert!(arc
            .commands()
            .iter()
            .any(|c| matches!(c, crate::vector::Command::Cubic(..))));
        assert!(
            arc.css().starts_with("path(evenodd, \"M 0 10 C"),
            "{}",
            arc.css()
        );
        assert_eq!(ClipPath::parse(&arc.css()), Some(arc));
        assert_eq!(
            ClipPath::parse("path(nonzero, 'M0 0 L1 1')").unwrap().css(),
            "path(\"M 0 0 L 1 1\")"
        );
    }
}
