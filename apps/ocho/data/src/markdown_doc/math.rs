//! LaTeX in Markdown (markdown_math.rs and pulldown-cmark's `ENABLE_MATH`),
//! as data: the parser finds the four delimiters and keeps each formula's
//! source and whether it is display math; the host typesets it.
//!
//! Upstream lays a formula out with RaTeX (`ratex_layout`) and paints it as
//! an SVG with KaTeX glyph outlines: inline formulas at text style, display
//! formulas at display style, 14 px font, 2 px padding (so the box is the
//! layout's `width × 14 + 4` by `(height + depth) × 14 + 4`, the baseline
//! `height × 14 + 2` from the top), all scaled by `font size / 14`, painted
//! in `text` (`muted` in progress or compact text). A display formula is a
//! full-width row, `my_2` (8 px above and below), centred, scrolling
//! sideways when wider than the column; an inline formula sits on the text
//! baseline at its own width (at most the column's, scrolling inside).
//!
//! Recognised: `$…$` and `$$…$$` with pulldown-cmark's rules (an opening
//! `$` not followed by whitespace, a closing `$` not preceded by it, the
//! two at the same brace depth, no other `$` between), and `\(…\)` and
//! `\[…\]`, which upstream rewrites to `$…$` and `$$…$$` before parsing
//! (`normalize_delimiters`), outside code, links and existing math.

/// Upstream bounds a formula's source (`Formula::parse`).
pub const MAX_SOURCE: usize = 16_384;

/// A formula the host typesets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formula {
    /// The TeX between the delimiters, as written.
    pub source: String,
    /// `$$…$$` or `\[…\]`: display style, on its own row.
    pub display: bool,
}

impl Formula {
    /// The formula, when its source would typeset; `None` means upstream
    /// falls back to the literal text (`$src$` / `$$src$$`).
    pub fn parse(source: &str, display: bool) -> Option<Formula> {
        if !well_formed(source) {
            return None;
        }
        Some(Formula {
            source: source.to_string(),
            display,
        })
    }

    // Upstream shows the delimited source on compact surfaces, which strip Markdown here.
    #[allow(dead_code)]
    /// The source with its delimiters, as compact surfaces show it.
    pub fn literal(&self) -> String {
        literal(&self.source, self.display)
    }
}

/// `source` between `$` or `$$`.
pub fn literal(source: &str, display: bool) -> String {
    let delimiter = if display { "$$" } else { "$" };
    format!("{delimiter}{source}{delimiter}")
}

/// A stand-in for RaTeX's parser: the checks it would fail on that a parser
/// without a macro table can make. Empty or oversized sources, unbalanced
/// braces, `\begin`/`\end` and `\left`/`\right` that do not pair, and a
/// dangling backslash fall back; unknown commands do not (RaTeX rejects
/// them, so a host renderer should fall back to the literal too).
pub fn well_formed(source: &str) -> bool {
    if source.len() > MAX_SOURCE || source.trim().is_empty() {
        return false;
    }
    let chars: Vec<char> = source.chars().collect();
    let mut depth = 0i32;
    let mut environments: Vec<String> = Vec::new();
    let mut lefts = 0i32;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                let Some(&next) = chars.get(i + 1) else {
                    return false;
                };
                if !next.is_ascii_alphabetic() {
                    i += 2;
                    continue;
                }
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end].is_ascii_alphabetic() {
                    end += 1;
                }
                let name: String = chars[start..end].iter().collect();
                match name.as_str() {
                    "begin" | "end" => {
                        let Some((env, after)) = group(&chars, end) else {
                            return false;
                        };
                        if name == "begin" {
                            environments.push(env);
                        } else if environments.pop().as_deref() != Some(env.as_str()) {
                            return false;
                        }
                        i = after;
                        continue;
                    }
                    "left" => lefts += 1,
                    "right" => {
                        lefts -= 1;
                        if lefts < 0 {
                            return false;
                        }
                    }
                    _ => {}
                }
                i = end;
                continue;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
        i += 1;
    }
    depth == 0 && environments.is_empty() && lefts == 0
}

/// `{name}` at `chars[at..]` (spaces allowed before it).
fn group(chars: &[char], at: usize) -> Option<(String, usize)> {
    let mut i = at;
    while chars.get(i).is_some_and(|c| *c == ' ') {
        i += 1;
    }
    if chars.get(i) != Some(&'{') {
        return None;
    }
    let close = chars[i + 1..].iter().position(|c| *c == '}')? + i + 1;
    Some((chars[i + 1..close].iter().collect(), close + 1))
}

/// A `$` math span opening at `chars[at] == '$'`: the content's range and
/// the index after the closing delimiter, and whether it is `$$` display
/// math. pulldown-cmark's rules: an inline opener has a non-space after
/// it; the closer is the next `$` at the same brace depth, which must have
/// a non-space before it and not touch the opener (any other `$` at that
/// depth makes the span invalid); a display span closes at the next `$$`.
/// An unmatched `}` ends the search. Backslash-escaped punctuation is
/// skipped.
pub fn dollar_span(chars: &[char], at: usize) -> Option<(usize, usize, usize, bool)> {
    if chars.get(at) != Some(&'$') {
        return None;
    }
    let display = chars.get(at + 1) == Some(&'$');
    let start = if display { at + 2 } else { at + 1 };
    if !display && chars.get(start).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let mut depth = 0i32;
    let mut j = start;
    while j < chars.len() {
        match chars[j] {
            '\\' => {
                if chars.get(j + 1).is_some_and(|c| c.is_ascii_punctuation()) {
                    j += 2;
                } else {
                    j += 1;
                }
                continue;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            '$' if depth == 0 => {
                if display {
                    return (chars.get(j + 1) == Some(&'$')).then_some((start, j, j + 2, true));
                }
                let can_close = j > start && !chars[j - 1].is_whitespace();
                return can_close.then_some((start, j, j + 1, false));
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// A `\(…\)` or `\[…\]` span at `chars[at] == '\\'`: the trimmed source,
/// whether it is display math, and the index after `\)` / `\]`. Upstream
/// converts it only when the body is not blank, spans no blank line and
/// overlaps no code span or other math (here: holds no backtick or `$`).
pub fn bracket_span(chars: &[char], at: usize) -> Option<(String, bool, usize)> {
    if chars.get(at) != Some(&'\\') {
        return None;
    }
    let (close, display) = match chars.get(at + 1) {
        Some('(') => (')', false),
        Some('[') => (']', true),
        _ => return None,
    };
    let mut j = at + 2;
    while j + 1 < chars.len() {
        if chars[j] == '\\' && chars[j + 1] == close {
            let body: String = chars[at + 2..j].iter().collect();
            if body.trim().is_empty()
                || body.contains("\n\n")
                || body.contains('`')
                || body.contains('$')
            {
                return None;
            }
            return Some((body.trim().to_string(), display, j + 2));
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn span(s: &str) -> Option<(String, bool)> {
        let c = chars(s);
        dollar_span(&c, 0).map(|(a, b, _, d)| (c[a..b].iter().collect(), d))
    }

    #[test]
    fn dollar_rules_follow_pulldown_cmark() {
        assert_eq!(span("$x^2$"), Some(("x^2".into(), false)));
        assert_eq!(span("$$\\sqrt{x}$$"), Some(("\\sqrt{x}".into(), true)));
        // Currency: the closer has a space before it, so nothing opens.
        assert_eq!(span("$5 and $10"), None);
        assert_eq!(span("$ x$"), None);
        assert_eq!(span("$x $"), None);
        assert_eq!(span("$x^2"), None);
        // A `$` inside braces belongs to another depth.
        assert_eq!(span("$a{$b}$"), Some(("a{$b}".into(), false)));
        // An unbalanced brace ends the search.
        assert_eq!(span("$\\frac{a}{$"), None);
        assert_eq!(span("$}$2+2$"), None);
        assert_eq!(span("$a\\$b$"), Some(("a\\$b".into(), false)));
        assert_eq!(span("$$x$ y$$"), None);
    }

    #[test]
    fn bracket_delimiters_convert_like_normalize_delimiters() {
        let c = chars("\\( x^2 \\) rest");
        assert_eq!(bracket_span(&c, 0), Some(("x^2".into(), false, 9)));
        let c = chars("\\[x^2\\].");
        assert_eq!(bracket_span(&c, 0), Some(("x^2".into(), true, 7)));
        assert_eq!(bracket_span(&chars("\\(  \\)"), 0), None);
        assert_eq!(bracket_span(&chars("\\(a `b` c\\)"), 0), None);
        assert_eq!(bracket_span(&chars("\\(unclosed"), 0), None);
    }

    #[test]
    fn malformed_and_oversized_math_falls_back() {
        assert!(Formula::parse(r"\frac{a}{", true).is_none());
        assert!(Formula::parse(&"x".repeat(MAX_SOURCE + 1), true).is_none());
        assert!(Formula::parse("  ", false).is_none());
        assert!(Formula::parse(r"\begin{pmatrix}a\end{bmatrix}", true).is_none());
        assert!(Formula::parse(r"\left( x", true).is_none());
        assert!(Formula::parse("x\\", false).is_none());
        for source in [
            r"\frac{a^2+b^2}{\sqrt{n}}",
            r"\sum_{i=1}^{n} i = \frac{n(n+1)}{2}",
            r"\begin{pmatrix}a & b \\ c & d\end{pmatrix}",
            r"\left(\frac{1}{2}\right)",
            r"\{x\}",
            r"\text{divisibility} \;\longrightarrow\; \gcd\text{ and Bézout}",
        ] {
            assert!(Formula::parse(source, true).is_some(), "{source}");
        }
        assert_eq!(
            Formula::parse("x^2", false).unwrap().literal(),
            "$x^2$".to_string()
        );
        assert_eq!(literal("y", true), "$$y$$");
    }
}
