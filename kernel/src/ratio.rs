//! CSS `aspect-ratio` (CSS Box Sizing 4 §5.1): `auto || <ratio>`.
//!
//! The row keeps what was authored — whether `auto` was given and the two
//! numbers of the ratio — so a web host emits the declaration exactly
//! (`16 / 9`, not a rounded float) and the kernel can tell `4 / 3` (the
//! ratio, always) from `auto 4 / 3` (an image's natural ratio when it has
//! one, else the ratio).

use std::fmt::Write;

/// A computed `aspect-ratio`. The initial value is `auto`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AspectRatio {
    /// `auto` was given: a replaced element's natural ratio wins.
    pub auto: bool,
    /// `<ratio>` as authored, `[width, height]`; a lone number `n` is `n / 1`.
    pub ratio: Option<[f32; 2]>,
}

impl Default for AspectRatio {
    fn default() -> Self {
        Self {
            auto: true,
            ratio: None,
        }
    }
}

impl AspectRatio {
    /// CSS's grammar: `auto`, `<ratio>`, or both in either order, where
    /// `<ratio>` is `<number [0,∞]> [ / <number [0,∞]> ]?`. Spaces around
    /// the slash are optional.
    pub fn parse(css: &str) -> Option<Self> {
        let spaced = css.replace('/', " / ");
        let mut tokens = spaced.split_ascii_whitespace().peekable();
        let (mut auto, mut ratio) = (false, None);
        while let Some(token) = tokens.next() {
            if token.eq_ignore_ascii_case("auto") {
                if auto {
                    return None;
                }
                auto = true;
                continue;
            }
            if ratio.is_some() {
                return None;
            }
            let width = number(token)?;
            let height = if tokens.peek() == Some(&"/") {
                tokens.next();
                number(tokens.next()?)?
            } else {
                1.0
            };
            ratio = Some([width, height]);
        }
        (auto || ratio.is_some()).then_some(Self { auto, ratio })
    }

    /// The declaration's CSS value: `auto`, `16 / 9`, or `auto 16 / 9`.
    pub fn css(&self) -> String {
        let mut out = String::new();
        if self.auto {
            out.push_str("auto");
        }
        if let Some([w, h]) = self.ratio {
            if self.auto {
                out.push(' ');
            }
            let _ = write!(
                out,
                "{} / {}",
                exact_num::Shortest32(w),
                exact_num::Shortest32(h)
            );
        }
        out
    }

    /// The authored ratio as width ÷ height, or `None` when there is none or
    /// it is degenerate — a zero or infinite component, where CSS says the
    /// property behaves as `auto`.
    pub fn preferred(&self) -> Option<f32> {
        let [w, h] = self.ratio?;
        let r = w / h;
        (w > 0.0 && h > 0.0 && r.is_finite() && r > 0.0).then_some(r)
    }

    /// Whether a replaced element's natural ratio is used: `auto` given, or
    /// the ratio degenerate (which behaves as `auto`).
    pub fn defers_to_natural(&self) -> bool {
        self.auto || self.preferred().is_none()
    }

    /// Whether sizing through the ratio uses the content box whatever
    /// `box-sizing` says: CSS's rule for `auto && <ratio>`.
    pub fn content_box(&self) -> bool {
        self.auto && self.preferred().is_some()
    }
}

fn number(token: &str) -> Option<f32> {
    let n = exact_num::parse_f32(token).ok()?;
    (n.is_finite() && n >= 0.0).then_some(n)
}

#[cfg(test)]
mod tests {
    use super::AspectRatio;

    #[test]
    fn the_css_grammar() {
        let r = |css: &str| AspectRatio::parse(css);
        let ratio = |auto, w, h| {
            Some(AspectRatio {
                auto,
                ratio: Some([w, h]),
            })
        };
        assert_eq!(r("auto"), Some(AspectRatio::default()));
        assert_eq!(r("2"), ratio(false, 2.0, 1.0));
        assert_eq!(r("16/9"), ratio(false, 16.0, 9.0));
        assert_eq!(r(" 16 /  9 "), ratio(false, 16.0, 9.0));
        assert_eq!(r("auto 4/3"), ratio(true, 4.0, 3.0));
        assert_eq!(r("4 / 3 auto"), ratio(true, 4.0, 3.0));
        assert_eq!(r("0"), ratio(false, 0.0, 1.0));
        assert_eq!(r("1.5 / 0.5"), ratio(false, 1.5, 0.5));
        for bad in [
            "",
            "none",
            "-1",
            "1 / -2",
            "auto auto",
            "1 2",
            "1 / ",
            "/ 2",
            "1 / 2 / 3",
            "2 auto 3",
            "16:9",
            "1px",
            "inf",
            "NaN",
        ] {
            assert_eq!(r(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn round_trips_and_resolves() {
        for css in ["auto", "16 / 9", "auto 4 / 3", "0 / 1", "1.5 / 1"] {
            let parsed = AspectRatio::parse(css).unwrap();
            assert_eq!(parsed.css(), css);
            assert_eq!(AspectRatio::parse(&parsed.css()), Some(parsed));
        }
        let p = |css: &str| AspectRatio::parse(css).unwrap();
        assert_eq!(p("16/9").preferred(), Some(16.0 / 9.0));
        assert_eq!(p("0").preferred(), None);
        assert_eq!(p("1/0").preferred(), None);
        assert!(p("auto").defers_to_natural() && p("0").defers_to_natural());
        assert!(!p("4/3").defers_to_natural() && p("auto 4/3").defers_to_natural());
        assert!(p("auto 4/3").content_box() && !p("4/3").content_box());
    }
}
