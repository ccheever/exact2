//! Icon tokens (LLP 1121 D10; stage 6 F, stage 9's brought forward): an
//! `icons` block's tokens, each naming a symbol that is not a role by a
//! source per platform, `icon(sf "bubble.left", material "chat_bubble",
//! svg "icons/reply.svg")`: an SF Symbol on Apple, an SVG under the
//! `assets/` of the root whose file writes it (the app's, or a
//! package's: LLP 1121 D11, stage 13) everywhere else, and a Material
//! Symbol's name kept as an annotation. What the files hold, and which
//! targets need which source, is the build's to check: this module reads
//! the text.

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
    /// The SVG other hosts draw, relative to the `assets/` of the root
    /// whose file writes its sources (the app's directory, or a package's
    /// root: LLP 1121 D11, stage 13).
    pub svg: Option<String>,
    /// Where it was declared.
    pub line: u32,
    /// The line its sources are written on: its own value's, or, for one
    /// naming another, that one's (whose file its SVG is read beside).
    pub written: u32,
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

/// Each icon whose value names another icon takes that one's sources, to a
/// fixed point: the icons block folds only literals. One naming no icon, an
/// icon whose sources don't read (its own error says why), or in a cycle,
/// keeps its name, which [`check`] refuses.
pub(crate) fn aliased(icons: &mut [Token]) {
    // Each alias names an icon; followed once, iteratively, every icon on
    // the way kept (Astra's round 2 on the checker bound: a pass per alias,
    // each scanning every icon, was cubic in a chain). A name two icons
    // share (which the check refuses) takes the first that is an icon.
    let valid: Vec<bool> = icons
        .iter()
        .map(|t| sources(&t.value.value).is_ok())
        .collect();
    let mut named: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (k, t) in icons.iter().enumerate() {
        let first = named.entry(t.name.value.as_str()).or_insert(k);
        if !valid[*first] && valid[k] {
            *first = k;
        }
    }
    // The icon each one's value comes from: known, or none (a cycle, or a
    // name no icon has).
    let mut source: Vec<Option<Option<usize>>> = vec![None; icons.len()];
    let mut on_path = vec![false; icons.len()];
    for start in 0..icons.len() {
        let mut path = Vec::new();
        let mut at = start;
        let found = loop {
            if let Some(known) = source[at] {
                break known;
            }
            if valid[at] {
                break Some(at);
            }
            let name = icons[at].value.value.trim();
            if name.contains('(') || on_path[at] {
                break None;
            }
            on_path[at] = true;
            path.push(at);
            match named.get(name) {
                Some(&next) => at = next,
                None => break None,
            }
        };
        for k in path {
            on_path[k] = false;
            source[k] = Some(found);
        }
        source[start].get_or_insert(found);
    }
    let values: Vec<_> = icons.iter().map(|t| t.value.clone()).collect();
    for (k, t) in icons.iter_mut().enumerate() {
        // Its line too: the SVG is read beside the file it is written in.
        if let Some(Some(from)) = source[k] {
            if from != k {
                t.value = values[from].clone();
            }
        }
    }
}

/// Check the `icons` block: each an `icon(…)` with one value, its name
/// unique among the design's tokens and icons.
pub(crate) fn check(icons: &[Token], tokens: &[Token]) -> (Vec<Icon>, Vec<Diagnostic>) {
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    let at =
        |t: &Token, id: &'static str, m: String| Diagnostic::new(id, m, t.name.line, t.name.col);
    // The names declared so far, once each (linear: round 3).
    let mut names: std::collections::HashSet<&str> =
        tokens.iter().map(|x| x.name.value.as_str()).collect();
    for t in icons {
        let name = &t.name.value;
        if !names.insert(name.as_str()) {
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
                written: t.value.line,
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

    /// An icon naming another, through a third too and declared before
    /// either, is that one's sources wherever a design is checked; one
    /// naming no icon, a theme token or an icon whose sources don't read,
    /// or a cycle, is refused.
    #[test]
    fn an_icon_naming_another_checks_as_that_one() {
        let vocab = crate::Vocab::default();
        let icons = "tamber t\n\nicons\n  nav.home = icon.home\n  icon.home = icon.house\n  icon.house = icon(sf \"house\")\n";
        let folding = crate::check(icons, &vocab).unwrap_or_else(|e| panic!("{e:?}"));
        let nav = folding.icons.iter().find(|i| i.name == "nav.home").unwrap();
        assert_eq!(nav.sf.as_deref(), Some("house"));
        for refused in [
            "  icon.home = icon.nowhere\n",
            "  icon.a = icon.b\n  icon.b = icon.a\n",
            "  icon.home = ink\n",
        ] {
            let text = format!("tamber t\n\ntheme\n  ink = #000000\n\nicons\n{refused}");
            let e = crate::check(&text, &vocab).unwrap_err();
            assert!(e.iter().all(|d| d.id == "tamber-kind"), "{e:?}");
        }
        // One naming an icon its sources refuse says what its line says;
        // that icon's line says why.
        let text = "tamber t\n\nicons\n  icon.bad = icon(png \"x\")\n  icon.home = icon.bad\n";
        let e = crate::check(text, &vocab).unwrap_err();
        let said: Vec<(u32, &str)> = e.iter().map(|d| (d.line, d.message.as_str())).collect();
        assert!(said[0].0 == 4 && said[0].1.contains("`png`"), "{said:?}");
        assert!(
            said[1].0 == 5 && said[1].1.starts_with("an icon is"),
            "{said:?}"
        );
    }
}
