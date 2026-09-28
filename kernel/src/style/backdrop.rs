//! CSS `backdrop-filter`, as the `backdrop_blur` row holds it.
//!
//! @ref LLP 1053.000 D1 — Contract's `backdrop-filter` takes CSS's grammar
//! and the row keeps its one built function: `none`, or one `blur(<length>)`
//! (the Gaussian's standard deviation, `px` or unitless zero, `blur()` being
//! zero). Every other filter function is CSS but not built, and is refused by
//! name, the same text at compile time and at run time.

use super::parse_pixel_length;

/// The grammar, once linked ([`link`]).
static LINKED: std::sync::OnceLock<Parse> = std::sync::OnceLock::new();

/// The grammar's signature: σ, or the reason a value is refused.
type Parse = fn(&str) -> Result<f32, &'static str>;

/// Link `backdrop-filter`'s grammar and its named refusals: until a host
/// calls this, a text value for the row is refused as unlinked. The
/// compiler and the native hosts link it at start; a web artifact links it
/// when its plan uses the row (LLP 1047 D2, linked by use; the web core's
/// budget, LLP 1053.000 §2).
pub fn link() {
    let _ = LINKED.set(parse);
}

/// [`parse`], if linked.
pub(crate) fn parse_linked(text: &str) -> Result<f32, &'static str> {
    LINKED.get().map_or(
        Err("`backdrop-filter` is not linked into this artifact (LLP 1053.000 §2)"),
        |parse| parse(text),
    )
}

/// The blur's standard deviation in points; 0 for `none`.
pub(crate) fn parse(text: &str) -> Result<f32, &'static str> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("none") {
        return Ok(0.0);
    }
    let mut blur = None;
    let mut rest = text;
    while !rest.is_empty() {
        let open = rest.find('(').ok_or(EXPECTED)?;
        let name = rest[..open].trim().to_ascii_lowercase();
        let close = open + rest[open..].find(')').ok_or(EXPECTED)?;
        let argument = rest[open + 1..close].trim();
        rest = rest[close + 1..].trim_start();
        match name.as_str() {
            "blur" if blur.is_some() => {
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
                blur = Some(sigma);
            }
            other => return Err(refused(other)),
        }
    }
    blur.ok_or(EXPECTED)
}

const EXPECTED: &str = "`backdrop-filter` is `none` or `blur(<length>)` (LLP 1053.000 D1)";

/// The CSS filter functions exact2 has not built for a backdrop, each named.
fn refused(name: &str) -> &'static str {
    match name {
        "saturate" => "`saturate()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "brightness" => "`brightness()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "contrast" => "`contrast()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "grayscale" => "`grayscale()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "hue-rotate" => "`hue-rotate()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "invert" => "`invert()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "opacity" => "`opacity()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "sepia" => "`sepia()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "drop-shadow" => "`drop-shadow()` is CSS, but exact2's `backdrop-filter` builds only `blur()` (LLP 1053.000 D1)",
        "url" => "`url()` is CSS, but exact2's `backdrop-filter` builds only `blur()`; an SVG filter on a backdrop is not built (LLP 1053.000 D1)",
        _ => EXPECTED,
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn none_and_one_blur_are_the_grammar() {
        assert_eq!(parse("none"), Ok(0.0));
        assert_eq!(parse(" blur(20px) "), Ok(20.0));
        assert_eq!(parse("blur(0)"), Ok(0.0));
        assert_eq!(parse("blur()"), Ok(0.0));
        assert_eq!(parse("BLUR(2.5px)"), Ok(2.5));
    }

    #[test]
    fn the_rest_of_css_is_refused_by_name() {
        assert!(parse("blur(20px) saturate(180%)")
            .unwrap_err()
            .contains("`saturate()` is CSS"));
        assert!(parse("url(#f)").unwrap_err().contains("`url()` is CSS"));
        assert!(parse("blur(1px) blur(2px)")
            .unwrap_err()
            .contains("one `blur()`"));
        assert!(parse("blur(-1px)").is_err());
        assert!(parse("blur(2em)").is_err());
        assert!(parse("20px").is_err());
        assert!(parse("").is_err());
    }
}
