//! CSS `backdrop-filter`: an ordered blur/saturation pair (LLP 1053.000 D1).
//!
//! One of each function, alone or together in authored order. The same
//! linked grammar checks literals, computed values, and wire decoding.

use super::{parse_css_number, parse_pixel_length};

/// The grammar, once linked ([`link`]).
static LINKED: std::sync::OnceLock<Parse> = std::sync::OnceLock::new();

/// The grammar's signature: the ordered list, or the reason a value is refused.
type Parse = fn(&str) -> Result<BackdropFilter, &'static str>;

/// One admitted CSS backdrop operation, in encoded sRGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackdropOp {
    /// Gaussian standard deviation in pixels.
    Blur(f32),
    /// Saturation multiplier; one is unchanged, zero is grayscale.
    Saturate(f32),
}

/// `none` (empty), or one blur and one saturation in their authored order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackdropFilter(pub Vec<BackdropOp>);

impl BackdropFilter {
    /// The linked grammar, or no value when refused.
    pub fn parse(text: &str) -> Option<Self> {
        Self::check(text).ok()
    }

    /// The linked grammar, retaining the named refusal.
    pub fn check(text: &str) -> Result<Self, &'static str> {
        LINKED.get().map_or(
            Err("`backdrop-filter` is not linked into this artifact (LLP 1053.000 §2)"),
            |parse| parse(text),
        )
    }

    /// Whether no filter is applied. Identity functions still form a backdrop root.
    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }

    /// Canonical CSS, preserving the operation order and identity functions.
    pub fn css(&self) -> String {
        if self.is_none() {
            return "none".into();
        }
        self.0
            .iter()
            .map(|op| match op {
                BackdropOp::Blur(n) => format!("blur({}px)", exact_num::Shortest32(*n)),
                BackdropOp::Saturate(n) => format!("saturate({})", exact_num::Shortest32(*n)),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Structured values must carry finite, nonnegative arguments too.
    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|op| match op {
            BackdropOp::Blur(n) | BackdropOp::Saturate(n) => n.is_finite() && *n >= 0.0,
        })
    }
}

/// Link `backdrop-filter`'s grammar and its named refusals: until a host
/// calls this, a text value for the row is refused as unlinked. The
/// compiler and the native hosts link it at start; a web artifact links it
/// when its plan uses the row (LLP 1047 D2, linked by use; the web core's
/// budget, LLP 1053.000 §2).
pub fn link() {
    let _ = LINKED.set(parse);
}

/// The admitted CSS grammar, in authored order.
fn parse(text: &str) -> Result<BackdropFilter, &'static str> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("none") {
        return Ok(BackdropFilter::default());
    }
    let (mut blur, mut saturation) = (false, false);
    let mut operations = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let open = rest.find('(').ok_or(EXPECTED)?;
        let name = rest[..open].to_ascii_lowercase();
        let close = open + rest[open..].find(')').ok_or(EXPECTED)?;
        let argument = rest[open + 1..close].trim();
        rest = rest[close + 1..].trim_start();
        match name.as_str() {
            "blur" if blur => {
                return Err("`backdrop-filter` takes one `blur()` in exact2 (LLP 1053.000 D1)")
            }
            "blur" => {
                let sigma = if argument.is_empty() {
                    0.0
                } else {
                    parse_pixel_length(argument)
                        .ok_or("`blur()` takes a length in px (unitless zero)")?
                };
                if !(sigma.is_finite() && sigma >= 0.0) {
                    return Err("`blur()` takes a nonnegative length");
                }
                blur = true;
                operations.push(BackdropOp::Blur(sigma));
            }
            "saturate" if saturation => {
                return Err("`backdrop-filter` takes one `saturate()` in exact2 (LLP 1053.000 D1)")
            }
            "saturate" => {
                let amount = if argument.is_empty() {
                    1.0
                } else {
                    let (n, divisor) = argument
                        .strip_suffix('%')
                        .map_or((argument, 1.0), |n| (n, 100.0));
                    parse_css_number(n)
                        .ok_or("`saturate()` takes a nonnegative number or percentage")?
                        / divisor
                };
                if !(amount as f32).is_finite() || amount < 0.0 {
                    return Err("`saturate()` takes a finite nonnegative number or percentage");
                }
                saturation = true;
                operations.push(BackdropOp::Saturate(amount as f32));
            }
            other => return Err(refused(other)),
        }
    }
    if operations.is_empty() {
        Err(EXPECTED)
    } else {
        Ok(BackdropFilter(operations))
    }
}

const EXPECTED: &str = "`backdrop-filter` is `none` or one `blur(<length>)` and/or `saturate(<number-or-percentage>)` (LLP 1053.000 D1)";

/// The CSS filter functions exact2 has not built for a backdrop, each named.
fn refused(name: &str) -> &'static str {
    match name {
        "brightness" => "`brightness()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "contrast" => "`contrast()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "grayscale" => "`grayscale()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "hue-rotate" => "`hue-rotate()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "invert" => "`invert()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "opacity" => "`opacity()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "sepia" => "`sepia()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "drop-shadow" => "`drop-shadow()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()` (LLP 1053.000 D1)",
        "url" => "`url()` is CSS, but exact2's `backdrop-filter` builds only `blur()` and `saturate()`; an SVG filter on a backdrop is not built (LLP 1053.000 D1)",
        _ => EXPECTED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_and_saturation_preserve_order_and_identity() {
        for (input, canonical) in [
            (" none ", "none"),
            (" blur(20px) ", "blur(20px)"),
            ("blur(0)", "blur(0px)"),
            ("blur()", "blur(0px)"),
            ("BLUR(2.5px)", "blur(2.5px)"),
            ("saturate()", "saturate(1)"),
            ("saturate(0%)", "saturate(0)"),
            ("saturate(180%)", "saturate(1.8)"),
            ("blur(12px) saturate(1.14)", "blur(12px) saturate(1.14)"),
            ("saturate(1.8) blur(4px)", "saturate(1.8) blur(4px)"),
        ] {
            let value = parse(input).unwrap();
            assert_eq!(value.css(), canonical, "{input}");
            assert_eq!(parse(&value.css()).unwrap(), value);
        }
        assert!(!parse("saturate(1)").unwrap().is_none());
        assert!(!parse("blur(0)").unwrap().is_none());
    }

    #[test]
    fn invalid_arguments_and_unbuilt_functions_are_refused() {
        for text in [
            "blur(-1px)",
            "blur(2em)",
            "blur(1px) blur(2px)",
            "saturate(1) saturate(2)",
            "saturate(-1)",
            "saturate(-2%)",
            "saturate(-1e-50)",
            "saturate(2px)",
            "saturate(NaN)",
            "saturate(inf)",
            "saturate(1e100)",
            "saturate(1 2)",
            "saturate(1.)",
            "saturate(1 %)",
            "saturate (1)",
            "none saturate(1)",
            "saturate(1) none",
            "20px",
            "",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
        assert!(parse("blur(20px) brightness(1.2)")
            .unwrap_err()
            .contains("`brightness()` is CSS"));
        assert!(parse("url(#f)").unwrap_err().contains("`url()` is CSS"));
    }
}
