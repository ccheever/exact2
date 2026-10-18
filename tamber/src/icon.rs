//! Icon tokens (LLP 1121 D10; stage 6 F, stage 9's brought forward): an
//! `icons` block's tokens, each naming a symbol that is not a role by a
//! source per platform, `icon(sf "bubble.left", material "chat_bubble",
//! svg "icons/reply.svg")`: an SF Symbol on Apple, an SVG under the app's
//! `assets/` everywhere else, and a Material Symbol's name kept as an
//! annotation. What the files hold, and which targets need which source, is
//! the build's to check: this module reads the text.

use crate::syntax::Token;
use crate::Diagnostic;

/// One icon token, as declared.
#[derive(Debug, Clone, PartialEq)]
pub struct Icon {
    /// Its name.
    pub name: String,
    /// The SF Symbol Apple draws.
    pub sf: Option<String>,
    /// A Material Symbol's name: an annotation, which nothing resolves.
    pub material: Option<String>,
    /// The SVG under the app's `assets/` other hosts draw, relative to it.
    pub svg: Option<String>,
    /// Where it was declared.
    pub line: u32,
    /// Its column.
    pub col: u32,
}

/// The `icon(…)` value's sources, or why it is no icon.
fn sources(value: &str) -> Result<[Option<String>; 3], String> {
    let inner = value
        .trim()
        .strip_prefix("icon(")
        .and_then(|v| v.strip_suffix(')'))
        .ok_or("an icon is `icon(sf \"…\", material \"…\", svg \"…\")`")?;
    let mut out: [Option<String>; 3] = [None, None, None];
    if inner.trim().is_empty() {
        return Err("an icon has an `sf` or an `svg` source: `icon(sf \"bubble.left\")`".into());
    }
    // Its sources: commas outside the quotes.
    let mut parts = Vec::new();
    let (mut quoted, mut start) = (false, 0);
    for (i, c) in inner.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&inner[start..]);
    for part in parts {
        let part = part.trim();
        let (key, rest) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
        let rest = rest.trim();
        let text = rest
            .strip_prefix('"')
            .and_then(|r| r.strip_suffix('"'))
            .filter(|t| !t.is_empty() && !t.contains('"'))
            .ok_or_else(|| {
                format!("`{part}`: a source is a key and a quoted name, `sf \"bubble.left\"`")
            })?;
        let (slot, ok) = match key {
            "sf" => (
                0,
                text.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.'),
            ),
            "material" => (
                1,
                text.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            ),
            "svg" => (
                2,
                text.ends_with(".svg")
                    && !text.starts_with('/')
                    // Nor a Windows separator or drive, which leave `assets/` there.
                    && !text.contains(['\\', ':'])
                    && !text
                        .split('/')
                        .any(|s| s.is_empty() || s == "." || s == ".."),
            ),
            _ => {
                return Err(format!(
                    "`{key}`: an icon's sources are `sf`, `material` and `svg`"
                ))
            }
        };
        if !ok {
            return Err(match key {
                "sf" => format!("`{text}` is no SF Symbol's name (`bubble.left`)"),
                "material" => format!("`{text}` is no Material Symbol's name (`chat_bubble`)"),
                _ => format!("`{text}`: an SVG is a `.svg` file under the app's `assets/`, by its path there (`icons/reply.svg`)"),
            });
        }
        if out[slot].replace(text.to_string()).is_some() {
            return Err(format!("`{key}` is written twice"));
        }
    }
    if out[0].is_none() && out[2].is_none() {
        return Err("an icon has an `sf` or an `svg` source: a `material` name is an annotation, which no host draws".into());
    }
    Ok(out)
}

/// A `theme` token that is an icon, which belongs under `icons`.
pub(crate) fn misplaced(t: &Token) -> bool {
    t.kind.as_deref() == Some("icon") || t.value.value.trim_start().starts_with("icon(")
}

/// Check the `icons` block: each an `icon(…)` with one value, its name
/// unique among the design's tokens and icons.
pub(crate) fn check(icons: &[Token], tokens: &[Token]) -> (Vec<Icon>, Vec<Diagnostic>) {
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    let at =
        |t: &Token, id: &'static str, m: String| Diagnostic::new(id, m, t.name.line, t.name.col);
    for (i, t) in icons.iter().enumerate() {
        let name = &t.name.value;
        if tokens.iter().any(|x| x.name.value == *name)
            || icons[..i].iter().any(|x| x.name.value == *name)
        {
            errors.push(at(
                t,
                "tamber-duplicate",
                format!("`{name}` is declared twice"),
            ));
            continue;
        }
        // Dotted, so no dial, role, CSS colour or keyword is spelled as one.
        if !name.contains('.') {
            errors.push(at(
                t,
                "tamber-icon-name",
                format!("`{name}`: an icon's name is dotted (`icon.{name}`), so no dial, role or CSS word is spelled as one"),
            ));
            continue;
        }
        if t.kind.as_deref().is_some_and(|k| k != "icon") {
            errors.push(at(
                t,
                "tamber-kind",
                format!("`{name}`: an `icons` block holds icon tokens"),
            ));
            continue;
        }
        if !t.whens.is_empty() {
            errors.push(at(t, "tamber-icon-axis", format!("`{name}`: an icon has one source per platform, so no `when` (vary the row's symbol instead)")));
            continue;
        }
        match sources(&t.value.value) {
            Ok([sf, material, svg]) => out.push(Icon {
                name: name.clone(),
                sf,
                material,
                svg,
                line: t.name.line,
                col: t.name.col,
            }),
            Err(m) => errors.push(Diagnostic::new("tamber-kind", m, t.value.line, t.value.col)),
        }
    }
    for t in tokens.iter().filter(|t| misplaced(t)) {
        errors.push(at(
            t,
            "tamber-kind",
            format!(
                "`{}`: an icon token is declared under `icons`",
                t.name.value
            ),
        ));
    }
    (out, errors)
}

#[cfg(test)]
mod tests {
    use super::sources;

    #[test]
    fn an_icon_reads_its_sources() {
        let [sf, material, svg] =
            sources(r#"icon(sf "bubble.left", material "chat_bubble", svg "icons/reply.svg")"#)
                .unwrap();
        assert_eq!(sf.as_deref(), Some("bubble.left"));
        assert_eq!(material.as_deref(), Some("chat_bubble"));
        assert_eq!(svg.as_deref(), Some("icons/reply.svg"));
        assert!(sources(r#"icon(material "chat")"#).is_err());
        assert!(sources(r#"icon(svg "../x.svg")"#).is_err());
        assert!(sources(r#"icon(sf "a", sf "b")"#).is_err());
        assert!(sources(r#"icon(png "a")"#).is_err());
        assert!(sources("icon()").is_err());
        assert!(sources(r#"icon(svg "icons/a,b.svg")"#).is_ok());
        assert!(sources(r#"icon(svg "..\\x.svg")"#).is_err());
        assert!(sources(r#"icon(svg "C:/x.svg")"#).is_err());
    }
}
