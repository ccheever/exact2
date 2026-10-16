//! Invoker-popover flip fallbacks (LLP 1021 D2). Each comma starts a new
//! attempt against the base style; whitespace combines flips in one attempt.

/// Ordered flip combinations; an empty list is CSS `none`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PositionTryFallbacks(Vec<u8>);

impl PositionTryFallbacks {
    /// Parse the admitted flip-block/flip-inline grammar.
    pub fn parse(css: &str) -> Option<Self> {
        if css.trim().eq_ignore_ascii_case("none") {
            return Some(Self::default());
        }
        let mut attempts = Vec::new();
        for attempt in css.split(',') {
            let mut flips = 0;
            for word in attempt.split_ascii_whitespace() {
                let bit = match word.to_ascii_lowercase().as_str() {
                    "flip-block" => 1,
                    "flip-inline" => 2,
                    _ => return None,
                };
                if flips & bit != 0 {
                    return None;
                }
                flips |= bit;
            }
            if flips == 0 {
                return None;
            }
            attempts.push(flips);
        }
        Some(Self(attempts))
    }

    /// Canonical CSS, retaining the order of attempts.
    pub fn css(&self) -> String {
        if self.0.is_empty() {
            return "none".into();
        }
        self.0
            .iter()
            .map(|flips| match flips {
                1 => "flip-block",
                2 => "flip-inline",
                _ => "flip-block flip-inline",
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_attempts_and_combined_flips_round_trip() {
        for (source, expected) in [
            ("none", "none"),
            (
                " FLIP-INLINE , flip-block, flip-inline flip-block ",
                "flip-inline, flip-block, flip-block flip-inline",
            ),
            ("flip-block, flip-block", "flip-block, flip-block"),
        ] {
            let value = PositionTryFallbacks::parse(source).unwrap();
            assert_eq!(value.css(), expected);
            assert_eq!(PositionTryFallbacks::parse(&value.css()), Some(value));
        }
        for source in [
            "",
            "flip-block,",
            "none, flip-block",
            "flip-block flip-block",
            "flip-start",
            "--custom",
            "top",
        ] {
            assert_eq!(PositionTryFallbacks::parse(source), None, "{source}");
        }
    }
}
