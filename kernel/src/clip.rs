//! CSS `clip-path`: `none` or an absolute SVG path, in CSS pixels.
//! The parsed commands also cross the Apple batch, so presenters do not parse CSS.

/// A validated clipping path. The initial value has no clipping.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipPath {
    commands: Vec<(char, Vec<f32>)>,
}

impl ClipPath {
    /// Supported path commands: explicit absolute M, L, Q, C and Z.
    /// Coordinates are finite; unsupported CSS shapes and SVG commands fail.
    pub fn parse(css: &str) -> Option<Self> {
        let css = css.trim();
        if css == "none" {
            return Some(Self::default());
        }
        let inner = css.strip_prefix("path(")?.strip_suffix(')')?.trim();
        let quote = inner.chars().next()?;
        if quote != '\'' && quote != '"' {
            return None;
        }
        let data = inner.strip_prefix(quote)?.strip_suffix(quote)?;
        let mut tokens = String::new();
        for c in data.chars() {
            if "MLQCZ".contains(c) {
                tokens.push(' ');
                tokens.push(c);
                tokens.push(' ');
            } else if c == ',' {
                tokens.push(' ');
            } else {
                tokens.push(c);
            }
        }
        let mut tokens = tokens.split_whitespace();
        let mut commands = Vec::new();
        while let Some(token) = tokens.next() {
            let (command, count) = match token {
                "M" => ('M', 2),
                "L" => ('L', 2),
                "Q" => ('Q', 4),
                "C" => ('C', 6),
                "Z" => ('Z', 0),
                _ => return None,
            };
            if commands.is_empty() && command != 'M' {
                return None;
            }
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                let number = tokens.next()?.parse::<f32>().ok()?;
                if !number.is_finite() {
                    return None;
                }
                values.push(number);
            }
            commands.push((command, values));
        }
        if commands.is_empty() {
            return None;
        }
        Some(Self { commands })
    }

    /// Empty for `none`; otherwise drawing commands in border-box coordinates.
    pub fn commands(&self) -> &[(char, Vec<f32>)] {
        &self.commands
    }

    /// Canonical, validated CSS (also the wire representation).
    pub fn css(&self) -> String {
        if self.commands.is_empty() {
            return "none".into();
        }
        let commands: Vec<_> = self
            .commands
            .iter()
            .map(|(command, values)| {
                let mut text = command.to_string();
                for value in values {
                    text.push(' ');
                    text.push_str(&value.to_string());
                }
                text
            })
            .collect();
        format!("path(\"{}\")", commands.join(" "))
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
        assert_eq!(ClipPath::parse(&path.css()), Some(path));
        assert_eq!(ClipPath::default().css(), "none");
        for bad in [
            "path('')",
            "path('L 0 0')",
            "path('M 0 NaN')",
            "path('M 0 0 C 1 2')",
            "path('M 0 0 z')",
            "path('M 0 0');color:red",
        ] {
            assert!(ClipPath::parse(bad).is_none(), "{bad}");
        }
    }
}
