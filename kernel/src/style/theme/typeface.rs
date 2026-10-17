//! Type and family tokens' entries (LLP 1121 D3, D6; stage 6 C). A type
//! token is CSS `font` syntax, `[style] [weight] size[/line-height]
//! [family]`, and names only the longhands it writes; `font="type.body"`
//! lowers to one row a longhand, each naming the token, and each reads its
//! own part here. A family is a font stack. In a bound table the family is
//! the plan's stack id, `#12`, as the runner and the compiler write it
//! ([`entry`]), since `font-family` holds a stack.

use crate::StyleId;

/// A type entry's parts, as their rows read them.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeParts<'a> {
    /// `italic`.
    pub style: Option<&'a str>,
    /// The weight's text, a number.
    pub weight: Option<&'a str>,
    /// The size's text: px, `rem`, `em` or a text style.
    pub size: &'a str,
    /// The line height's text.
    pub leading: Option<&'a str>,
    /// The family: a stack's text, or `#<id>` in a bound table.
    pub family: Option<&'a str>,
}

/// Whether `word` reads as a type value's size: a dimension or a named
/// text style.
fn is_size(word: &str) -> bool {
    let size = word.split('/').next().unwrap_or(word).to_ascii_lowercase();
    size.starts_with("-exact-")
        || size.starts_with("-apple-system-")
        || ["px", "rem", "em"].iter().any(|u| {
            size.strip_suffix(u)
                .is_some_and(|n| n.parse::<f64>().is_ok())
        })
}

/// A type value's parts, as the crate writes it (`Type::css`): the style
/// and the weight before the size, the line height after a `/`, the family
/// the rest. `None` for a text that is no type value.
pub fn parts(text: &str) -> Option<TypeParts<'_>> {
    let text = text.trim();
    let (mut style, mut weight) = (None, None);
    let mut rest = text;
    loop {
        let rest_trim = rest.trim_start();
        let end = rest_trim
            .find(char::is_whitespace)
            .unwrap_or(rest_trim.len());
        let word = &rest_trim[..end];
        if word.is_empty() {
            return None;
        }
        if is_size(word) {
            let (size, leading) = match word.split_once('/') {
                Some((s, l)) => (s, Some(l)),
                None => (word, None),
            };
            let family = rest_trim[end..].trim();
            return Some(TypeParts {
                style,
                weight,
                size,
                leading,
                family: (!family.is_empty()).then_some(family),
            });
        }
        match word {
            "italic" if style.is_none() => style = Some(word),
            w if weight.is_none() && w.parse::<u16>().is_ok() => weight = Some(w),
            _ => return None,
        }
        rest = &rest_trim[end..];
    }
}

/// The rows a type token may write, and a family token's.
pub fn admits_type(id: StyleId) -> bool {
    matches!(
        id,
        StyleId::FontStyle
            | StyleId::FontWeight
            | StyleId::FontSize
            | StyleId::LineHeight
            | StyleId::FontFamily
    )
}

/// Row `id`'s part of type entry `value`; `None` when it writes none.
pub fn part(id: StyleId, value: &str) -> Option<super::super::StyleValue<'static>> {
    use super::super::StyleValue;
    let p = parts(value)?;
    let text = match id {
        StyleId::FontStyle => p.style?,
        StyleId::FontWeight => {
            return Some(StyleValue::Number(exact_num::parse_f64(p.weight?).ok()?))
        }
        StyleId::FontSize => p.size,
        StyleId::LineHeight => {
            let l = p.leading?;
            return Some(match exact_num::parse_f64(l) {
                Ok(n) => StyleValue::Number(n),
                Err(_) => StyleValue::Text(String::from(l).into()),
            });
        }
        StyleId::FontFamily => return family(p.family?),
        _ => return None,
    };
    Some(StyleValue::Text(String::from(text).into()))
}

/// A bound family's stack id, `#12`, as `font-family` holds it.
pub fn family(value: &str) -> Option<super::super::StyleValue<'static>> {
    let id: u16 = value.trim().strip_prefix('#')?.parse().ok()?;
    Some(super::super::StyleValue::Number(f64::from(id)))
}

/// A type or family token's value as a bound table holds it: its family's
/// text replaced by the stack `stack` gives it (`#12`). `None` when the
/// family names no stack the plan has, or the text is no type value.
pub fn entry(
    kind: super::Kind,
    value: &str,
    stack: impl Fn(&str) -> Option<u16>,
) -> Option<String> {
    match kind {
        super::Kind::Family => Some(format!("#{}", stack(value.trim())?)),
        super::Kind::Type => {
            let p = parts(value)?;
            let Some(family) = p.family else {
                return Some(value.trim().to_string());
            };
            let at = value.trim().len() - family.len();
            Some(format!("{}#{}", &value.trim()[..at], stack(family)?))
        }
        _ => Some(value.to_string()),
    }
}

/// The generic families a plan's stack names as such, not as a family.
const GENERICS: [&str; 8] = [
    "system-ui",
    "ui-sans-serif",
    "sans-serif",
    "ui-serif",
    "serif",
    "ui-monospace",
    "monospace",
    "ui-rounded",
];

/// A family stack's members, split at the commas outside quotes: each a
/// family (`true`) or a generic family by its name. A member is generic
/// only unquoted, as the compiler reads it (`-apple-system` is
/// `system-ui`); a quoted one, or any other name, is a family.
pub fn family_members(text: &str) -> Vec<(bool, String)> {
    let (mut members, mut start, mut quote) = (Vec::new(), 0, None);
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, ',') => {
                members.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    members.push(&text[start..]);
    members
        .into_iter()
        .map(|m| {
            let m = m.trim();
            let quoted = m.len() >= 2
                && (m.starts_with('"') && m.ends_with('"')
                    || m.starts_with('\'') && m.ends_with('\''));
            if quoted {
                return (true, m[1..m.len() - 1].to_string());
            }
            let lower = m.to_ascii_lowercase();
            match lower.as_str() {
                "-apple-system" => (false, "system-ui".into()),
                g if GENERICS.contains(&g) => (false, lower),
                _ => (true, m.to_string()),
            }
        })
        .collect()
}

/// A family stack's text as a key ([`family_key`]), by its members
/// ([`family_members`]).
pub fn family_key_of(text: &str) -> String {
    let members = family_members(text);
    family_key(members.iter().map(|(f, n)| (*f, n.as_str())))
}

/// A family stack's members as a key, each a family (`true`) or a generic
/// family by its name: a family's name quoted, each lower-cased with its
/// spaces collapsed, joined by `,` (a plan's stacks are matched by it).
pub fn family_key<'a>(members: impl IntoIterator<Item = (bool, &'a str)>) -> String {
    members
        .into_iter()
        .map(|(family, m)| {
            let name = m
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase();
            if family {
                format!("\"{name}\"")
            } else {
                name
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StyleValue;

    #[test]
    fn each_row_reads_its_part() {
        let v = "italic 600 -exact-body/1.35 #12";
        assert_eq!(
            part(StyleId::FontStyle, v),
            Some(StyleValue::Text("italic".into()))
        );
        assert_eq!(
            part(StyleId::FontWeight, v),
            Some(StyleValue::Number(600.0))
        );
        assert_eq!(
            part(StyleId::FontSize, v),
            Some(StyleValue::Text("-exact-body".into()))
        );
        assert_eq!(part(StyleId::LineHeight, v), Some(StyleValue::Number(1.35)));
        assert_eq!(part(StyleId::FontFamily, v), Some(StyleValue::Number(12.0)));
        assert_eq!(part(StyleId::FontWeight, "16px/20px"), None);
        assert_eq!(
            part(StyleId::LineHeight, "16px/20px"),
            Some(StyleValue::Text("20px".into()))
        );
        let e = entry(
            super::super::Kind::Type,
            "600 1rem \"Inter\", sans-serif",
            |f| (family_key_of(f) == "\"inter\",sans-serif").then_some(9),
        );
        assert_eq!(e.as_deref(), Some("600 1rem #9"));
        assert_eq!(parts("Inter"), None);
        assert_ne!(family_key_of("\"serif\""), family_key_of("serif"));
        assert_ne!(family_key_of("\"A,B\""), family_key_of("A, B"));
        assert_eq!(
            family_members("\"A,B\", serif"),
            [(true, "A,B".to_string()), (false, "serif".to_string())]
        );
        assert_eq!(
            family_key_of("-apple-system, Georgia"),
            family_key_of("system-ui, \"georgia\"")
        );
    }
}
