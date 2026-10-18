//! Type and family tokens (LLP 1121 D3, D6; stage 6 C). A `type` is CSS
//! `font` syntax, `[style] [weight] size[/line-height] [family]`, and it
//! names only what it writes: `-exact-body/1.35 "InterVariable"` writes the
//! size, the leading and the family, and the text style's own weight
//! applies. A `family` is a font-family stack.

use crate::fold::{fail, FoldError, Named, Names};
use crate::measure::{num, Length};

/// A folded type token: the longhands it writes, each as its row reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    /// `italic`, when written (the kernel's `font-style` holds `normal`
    /// and `italic`).
    pub style: Option<String>,
    /// The weight, 1 to 1000, when written.
    pub weight: Option<u16>,
    /// The size: px, `rem`, `em`, or a platform text style.
    pub size: Length,
    /// The line height as `line-height` reads it (a number, a length,
    /// `normal`), when written.
    pub leading: Option<String>,
    /// The family stack, when written ([`family`]'s form).
    pub family: Option<String>,
}

impl Type {
    /// The text the plan carries: the parts written, in `font`'s order.
    pub fn css(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        out.extend(self.style.clone());
        out.extend(self.weight.map(|w| w.to_string()));
        out.push(match &self.leading {
            Some(l) => format!("{}/{l}", self.size.css()),
            None => self.size.css(),
        });
        out.extend(self.family.clone());
        out.join(" ")
    }

    /// The longhands it writes, as `font`'s rows name them: `font-style`,
    /// `font-weight`, `font-size`, `line-height`, `font-family`.
    pub fn longhands(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.style.is_some() {
            out.push("font-style");
        }
        if self.weight.is_some() {
            out.push("font-weight");
        }
        out.push("font-size");
        if self.leading.is_some() {
            out.push("line-height");
        }
        if self.family.is_some() {
            out.push("font-family");
        }
        out
    }
}

/// Whether `word` is a text style's name a font size takes, by the
/// vocabulary's named lengths (not `-exact-hairline`).
fn text_style(word: &str, named: &dyn Fn(&str) -> bool) -> bool {
    named(word) && word != "-exact-hairline"
}

/// Fold a type token's value.
pub(crate) fn fold(
    text: &str,
    names: &mut dyn Names,
    named: &dyn Fn(&str) -> bool,
) -> Result<Type, FoldError> {
    let text = text.trim();
    // Another type token's name: its value.
    if is_name(text) && !text_style(text, named) {
        return match names.named(text)? {
            Named::Type(t) => Ok(t),
            Named::Unknown => fail("tamber-unknown-token", format!("`{text}` is no token")),
            _ => fail("tamber-kind", format!("`{text}` is not a type token")),
        };
    }
    let (mut style, mut weight) = (None, None);
    let mut rest = text;
    let size_word = loop {
        let (word, tail) = split_word(rest);
        if word.is_empty() {
            return fail(
                "tamber-kind",
                format!("`{text}`: a type token needs a size (px, `rem`, `em` or a text style)"),
            );
        }
        match word {
            "italic" if style.is_none() && weight.is_none() => style = Some(word.to_string()),
            "oblique" => {
                return fail("tamber-kind", format!("`{text}`: `font-style` is `normal` or `italic` on every host; write `italic`"));
            }
            "bold" if weight.is_none() => weight = Some(700),
            "normal" => {
                return fail("tamber-kind", format!("`{text}`: a type token writes only what it names, so `normal` says nothing; leave the style or weight out, or write the weight as a number"));
            }
            w if weight.is_none() && w.parse::<f64>().is_ok() => {
                let n: f64 = w.parse().unwrap_or_default();
                if !(1.0..=1000.0).contains(&n) || n.fract() != 0.0 {
                    return fail(
                        "tamber-kind",
                        format!("`{w}`: a weight is a whole number from 1 to 1000"),
                    );
                }
                weight = Some(n as u16);
            }
            _ => break word,
        }
        rest = tail;
    };
    rest = split_word(rest).1;
    // The leading: `size/leading`, or `size / leading`.
    let (size_text, mut leading_text) = match size_word.split_once('/') {
        Some((s, l)) => (s, Some(l.to_string())),
        None => (size_word, None),
    };
    if leading_text.as_deref() == Some("")
        || leading_text.is_none() && rest.trim_start().starts_with('/')
    {
        let after = rest.trim_start().trim_start_matches('/').trim_start();
        let (word, tail) = split_word(after);
        if word.is_empty() {
            return fail(
                "tamber-kind",
                format!("`{text}`: `/` is followed by the line height"),
            );
        }
        leading_text = Some(word.to_string());
        rest = tail;
    }
    let size = size(size_text, names, named)?;
    let leading = leading_text.map(|l| leading(&l, names)).transpose()?;
    let rest = rest.trim();
    let family = (!rest.is_empty())
        .then(|| family(rest, names))
        .transpose()?;
    Ok(Type {
        style,
        weight,
        size,
        leading,
        family,
    })
}

fn size(
    word: &str,
    names: &mut dyn Names,
    named: &dyn Fn(&str) -> bool,
) -> Result<Length, FoldError> {
    if text_style(word, named) {
        return Ok(Length::Named(word.to_string()));
    }
    if let Some(l) = dimension(word) {
        return l;
    }
    if named(word) {
        return fail(
            "tamber-kind",
            format!("`{word}` is a named length no font size takes"),
        );
    }
    if is_name(word) {
        return match names.named(word)? {
            Named::Length(Length::Named(n)) if !text_style(&n, named) => fail(
                "tamber-kind",
                format!("`{word}` is `{n}`, which no font size takes"),
            ),
            Named::Length(l) => Ok(l),
            Named::Unknown => fail(
                "tamber-unknown-token",
                format!("`{word}` is no token or text style"),
            ),
            _ => fail("tamber-kind", format!("`{word}` is not a length")),
        };
    }
    fail(
        "tamber-kind",
        format!("`{word}`: a type token's size is px, `rem`, `em` or a text style"),
    )
}

/// `word` as px, `rem` or `em`; `None` when it is no dimension.
fn dimension(word: &str) -> Option<Result<Length, FoldError>> {
    for (unit, make) in [
        ("px", Length::Px as fn(f64) -> Length),
        ("rem", Length::Rem),
        ("em", Length::Em),
    ] {
        if let Some(n) = word.strip_suffix(unit).and_then(|n| n.parse::<f64>().ok()) {
            return Some(if n > 0.0 && n.is_finite() {
                Ok(make(n))
            } else {
                fail("tamber-kind", format!("`{word}`: a size is positive"))
            });
        }
    }
    None
}

fn leading(word: &str, names: &mut dyn Names) -> Result<String, FoldError> {
    if word == "normal" {
        return Ok(word.into());
    }
    if let Ok(n) = word.parse::<f64>() {
        return if n >= 0.0 && n.is_finite() {
            Ok(num(n))
        } else {
            fail(
                "tamber-kind",
                format!("`{word}`: a line height is not negative"),
            )
        };
    }
    if let Some(l) = dimension(word) {
        return l.map(|l| l.css());
    }
    if is_name(word) {
        return match names.named(word)? {
            Named::Number(n) if n >= 0.0 => Ok(num(n)),
            Named::Length(l @ (Length::Px(_) | Length::Rem(_) | Length::Em(_))) => Ok(l.css()),
            Named::Unknown => fail("tamber-unknown-token", format!("`{word}` is no token")),
            _ => fail("tamber-kind", format!("`{word}` is no line height")),
        };
    }
    fail(
        "tamber-kind",
        format!("`{word}`: a line height is a number, a length or `normal`"),
    )
}

/// Fold a family token's value, or a type token's family: a stack, its
/// members as CSS writes them (a quoted name, or identifiers), joined by
/// `, `; or another family token's name.
pub(crate) fn family(text: &str, names: &mut dyn Names) -> Result<String, FoldError> {
    let text = text.trim();
    // A family token's name (`textFace`, `family.text`): its stack. Another
    // one word is a family's name, unless the design names it otherwise.
    if is_name(text) {
        match names.named(text)? {
            Named::Family(f) => return Ok(f),
            Named::Unknown if text.contains('.') => {
                return fail("tamber-unknown-token", format!("`{text}` is no token"))
            }
            Named::Unknown => {}
            _ => return fail("tamber-kind", format!("`{text}` is not a family token")),
        }
    }
    let mut members = Vec::new();
    for member in split_commas(text) {
        let member = member.trim();
        let quoted = member.len() >= 2
            && (member.starts_with('"') && member.ends_with('"')
                || member.starts_with('\'') && member.ends_with('\''));
        let ok = if quoted {
            !member[1..member.len() - 1].contains(['"', '\''])
        } else {
            !member.is_empty()
                && member.split_whitespace().all(|w| {
                    w.chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                        && !w.starts_with(|c: char| c.is_ascii_digit())
                })
        };
        if !ok {
            return fail(
                "tamber-kind",
                format!(
                    "`{text}`: `{member}` is no family name (quote a name with other characters)"
                ),
            );
        }
        members.push(if quoted {
            member.to_string()
        } else {
            member.split_whitespace().collect::<Vec<_>>().join(" ")
        });
    }
    Ok(members.join(", "))
}

/// The kind a value's text makes it, where it says (§3): CSS `font` syntax
/// with more than a size (a line height, a family, a style or a weight
/// before it) is a type; a stack with a quoted name or a comma, a family.
pub(crate) fn inferred(text: &str, named: &dyn Fn(&str) -> bool) -> Option<crate::Kind> {
    let text = text.trim();
    if text.contains('(') {
        return None;
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let sized = |w: &str| {
        let s = w.split('/').next().unwrap_or(w);
        named(s) || dimension(s).is_some()
    };
    let at = words.iter().position(|w| sized(w));
    let before_ok = at.is_some_and(|i| {
        words[..i].iter().all(|w| {
            matches!(*w, "italic" | "oblique" | "bold" | "normal") || w.parse::<f64>().is_ok()
        })
    });
    // What follows the size and its line height is a family: no number, no
    // colour (a shadow's offsets and colour are not a type).
    let after_ok = at.is_some_and(|i| {
        let mut rest = &words[i + 1..];
        if !words[i].contains('/') && rest.first() == Some(&"/") {
            rest = rest.get(2..).unwrap_or(&[]);
        }
        rest.iter().all(|w| {
            !w.starts_with(|c: char| c.is_ascii_digit() || c == '#' || c == '-' || c == '.')
        })
    });
    if before_ok && after_ok && (words.len() > 1 || words[0].contains('/')) {
        return Some(crate::Kind::Type);
    }
    (text.contains(['"', '\'']) || split_commas(text).len() > 1).then_some(crate::Kind::Family)
}

/// A token's or dial's name, or a keyword: one word with no unit.
fn is_name(text: &str) -> bool {
    !text.is_empty()
        && !text.contains([' ', '/', ',', '"', '\''])
        && !text.starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

/// The first whitespace-separated word of `text` and what follows it.
fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    (&text[..end], &text[end..])
}

/// `text` split at its commas outside quotes.
fn split_commas(text: &str) -> Vec<&str> {
    let (mut out, mut start, mut quote) = (Vec::new(), 0, None);
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, ',') => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&text[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct None_;
    impl Names for None_ {
        fn named(&mut self, _: &str) -> Result<Named, FoldError> {
            Ok(Named::Unknown)
        }
    }

    fn named(w: &str) -> bool {
        w.starts_with("-exact-")
    }

    #[test]
    fn a_type_names_only_what_it_writes() {
        let t = fold("-exact-body/1.35 \"InterVariable\"", &mut None_, &named).unwrap();
        assert_eq!(t.longhands(), ["font-size", "line-height", "font-family"]);
        assert_eq!(t.css(), "-exact-body/1.35 \"InterVariable\"");
        let t = fold(
            "italic 600 16px / 20px Inter,  sans-serif",
            &mut None_,
            &named,
        )
        .unwrap();
        assert_eq!(t.css(), "italic 600 16px/20px Inter, sans-serif");
        assert_eq!(t.longhands().len(), 5);
        assert_eq!(
            fold("bold 1.25rem", &mut None_, &named).unwrap().weight,
            Some(700)
        );
        for bad in ["normal 16px", "Inter", "1200 16px", "16px/x y"] {
            assert!(fold(bad, &mut None_, &named).is_err(), "{bad}");
        }
    }
}
