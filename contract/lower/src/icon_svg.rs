//! @ref LLP 1121 D10 (stage 6 F) — an icon token's SVG, read at build: for
//! v1 one monochrome shape, its `<path>`s joined into one `d` (subpaths) and
//! drawn as a template under its own viewBox, as the roles' paths are drawn.
//! Anything else (several colours, a gradient, an image, text, a stroke, a
//! transform, a shape that is not a path) is no template: the web may draw
//! it as the file, and a shipped target that is not the web refuses it.

/// An SVG read as an icon.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Svg {
    /// Its viewBox, as written (`0 -960 960 960`).
    pub view_box: String,
    /// Its paths' `d`, joined.
    pub path: String,
    /// Why it is no one-colour shape, when it is not.
    pub not_template: Option<String>,
}

/// Each open element's fill and stroke, inherited, and whether it is inside
/// something that draws nothing itself (`defs`).
type Open = (Option<String>, Option<String>, bool);

/// A tag's name and attributes, entities decoded; `None` for a closing tag.
#[allow(clippy::type_complexity)]
fn tag(text: &str) -> Result<Option<(&str, Vec<(&str, String)>)>, String> {
    if text.starts_with('/') {
        return Ok(None);
    }
    let text = text.strip_suffix('/').unwrap_or(text);
    let name_end = text.find(|c: char| c.is_whitespace()).unwrap_or(text.len());
    let (name, mut rest) = (&text[..name_end], &text[name_end..]);
    let mut attrs = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return Ok(Some((name, attrs)));
        }
        let eq = rest
            .find('=')
            .ok_or_else(|| format!("`<{name}>` has an attribute with no value"))?;
        let key = rest[..eq].trim();
        let value = rest[eq + 1..].trim_start();
        let q = value
            .chars()
            .next()
            .filter(|q| *q == '"' || *q == '\'')
            .ok_or_else(|| format!("`<{name}>`'s `{key}` is not quoted"))?;
        let end = value[1..]
            .find(q)
            .ok_or_else(|| format!("`<{name}>`'s `{key}` is not closed"))?;
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(format!("`<{name}>` has a malformed attribute"));
        }
        attrs.push((key, decode(&value[1..1 + end])));
        rest = &value[end + 2..];
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return Err(format!("`<{name}>`'s attributes are not apart"));
        }
    }
}

/// An attribute value with its entities decoded.
fn decode(v: &str) -> String {
    if !v.contains('&') {
        return v.to_string();
    }
    let mut out = String::new();
    let mut rest = v;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = tail.find(';').unwrap_or(0);
        let entity = &tail[1..end.max(1)];
        let ch = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch.filter(|_| end > 0) {
            Some(c) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// A property's value on an element: its `style` declaration first, else
/// its presentation attribute (SVG's cascade).
fn property(attrs: &[(&str, String)], name: &str) -> Option<String> {
    let styled = attrs
        .iter()
        .find(|(k, _)| *k == "style")
        .and_then(|(_, s)| {
            s.split(';').rev().find_map(|d| {
                let (k, v) = d.split_once(':')?;
                (k.trim() == name).then(|| clean(v))
            })
        });
    styled.or_else(|| {
        attrs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| clean(v))
    })
}

/// A paint's value, lowercase, without `!important`.
fn clean(v: &str) -> String {
    let v = v.trim();
    v.strip_suffix("!important")
        .unwrap_or(v)
        .trim()
        .to_ascii_lowercase()
}

/// Whether `d` is path data alone, so it travels in the source form.
fn path_data(d: &str) -> bool {
    d.chars().all(|c| {
        c.is_ascii_digit() || c.is_ascii_whitespace() || "MmZzLlHhVvCcSsQqTtAaEe.,+-".contains(c)
    })
}

/// `d` to join after another path: a leading relative moveto is absolute
/// (SVG's rule for a path's first command), its further pairs relative
/// linetos, so `m2 2h1` is `M2 2h1` and `m2 2 3 3` is `M2 2 l3 3`.
fn standalone(d: &str) -> String {
    let d = d.trim_start();
    let Some(rest) = d.strip_prefix('m') else {
        return d.to_string();
    };
    // The first coordinate pair: two numbers.
    let b = rest.as_bytes();
    let (mut numbers, mut at) = (0, 0);
    while numbers < 2 && at < b.len() {
        while at < b.len() && (b[at].is_ascii_whitespace() || b[at] == b',') {
            at += 1;
        }
        let start = at;
        if at < b.len() && (b[at] == b'-' || b[at] == b'+') {
            at += 1;
        }
        let mut dot = false;
        while at < b.len() && (b[at].is_ascii_digit() || (b[at] == b'.' && !dot)) {
            dot |= b[at] == b'.';
            at += 1;
        }
        if at < b.len() && (b[at] == b'e' || b[at] == b'E') {
            at += 1;
            if at < b.len() && (b[at] == b'-' || b[at] == b'+') {
                at += 1;
            }
            while at < b.len() && b[at].is_ascii_digit() {
                at += 1;
            }
        }
        if at == start {
            break;
        }
        numbers += 1;
    }
    let (pair, tail) = rest.split_at(at);
    let more = tail.trim_start().trim_start_matches(',').trim_start();
    let implicit = more.starts_with(|c: char| c.is_ascii_digit() || "+-.".contains(c));
    match implicit {
        true => format!("M{pair} l{}", tail.trim_start().trim_start_matches(',')),
        false => format!("M{pair}{tail}"),
    }
}

/// Read `text` as an icon's SVG, or why it is none.
pub(crate) fn read(text: &str) -> Result<Svg, String> {
    let (mut view_box, mut paths, mut fill, mut why) =
        (None, Vec::<String>::new(), None::<String>, None);
    let mut stack: Vec<Open> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        rest = &rest[open..];
        // Comments, CDATA, declarations and processing instructions draw nothing.
        let skip = [
            ("<!--", "-->"),
            ("<![CDATA[", "]]>"),
            ("<?", "?>"),
            ("<!", ">"),
        ]
        .into_iter()
        .find(|(start, _)| rest.starts_with(start));
        if let Some((start, end)) = skip {
            let close = rest[start.len()..]
                .find(end)
                .ok_or("an SVG comment or declaration is not closed")?;
            rest = &rest[start.len() + close + end.len()..];
            continue;
        }
        let end = rest.find('>').ok_or("an SVG tag is not closed")?;
        let raw = &rest[1..end];
        rest = &rest[end + 1..];
        let Some((name, attrs)) = tag(raw)? else {
            stack.pop();
            continue;
        };
        let (inherited_fill, inherited_stroke, in_defs) =
            stack.last().cloned().unwrap_or((None, None, false));
        let fill_here = property(&attrs, "fill").or(inherited_fill);
        let stroke_here = property(&attrs, "stroke").or(inherited_stroke);
        let in_defs =
            in_defs || matches!(name, "defs" | "symbol" | "clipPath" | "mask" | "pattern");
        if attrs.iter().any(|(k, _)| *k == "transform") && !in_defs {
            why.get_or_insert_with(|| format!("its `<{name}>` has a transform"));
        }
        match name {
            "svg" => {
                let attr = |a: &str| attrs.iter().find(|(k, _)| *k == a).map(|(_, v)| v.clone());
                view_box = attr("viewBox").or_else(|| {
                    let n = |a| attr(a).map(|v: String| v.trim_end_matches("px").to_string());
                    Some(format!("0 0 {} {}", n("width")?, n("height")?))
                });
            }
            "path" if !in_defs => {
                let d = attrs
                    .iter()
                    .find(|(k, _)| *k == "d")
                    .map(|(_, v)| v.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .ok_or("a `<path>` has no `d`")?;
                if !path_data(&d) {
                    return Err("a `<path>`'s `d` is not path data".into());
                }
                let painted = fill_here.clone().unwrap_or_else(|| "black".into());
                if stroke_here.as_deref().is_some_and(|s| s != "none") {
                    why.get_or_insert_with(|| "it has a stroke".to_string());
                }
                if painted == "none" {
                    why.get_or_insert_with(|| "a path is not filled".to_string());
                } else if fill.as_ref().is_some_and(|f| *f != painted) {
                    why.get_or_insert_with(|| "it fills in more than one colour".to_string());
                } else {
                    fill = Some(painted);
                }
                paths.push(if paths.is_empty() { d } else { standalone(&d) });
            }
            "path" | "g" | "title" | "desc" | "defs" | "metadata" => {}
            _ if in_defs => {}
            other => {
                why.get_or_insert_with(|| format!("it has a `<{other}>`, not only paths"));
            }
        }
        if !raw.ends_with('/') {
            stack.push((fill_here, stroke_here, in_defs));
        }
    }
    let view_box = view_box.ok_or("it has no `<svg>` with a viewBox (or a width and height)")?;
    let numbers: Vec<f64> = view_box
        .split([' ', ','])
        .filter(|s| !s.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| format!("its viewBox `{view_box}` is not four numbers"))?;
    if numbers.len() != 4 || numbers[2] <= 0.0 || numbers[3] <= 0.0 {
        return Err(format!(
            "its viewBox `{view_box}` is not four numbers, a positive size"
        ));
    }
    if paths.is_empty() && why.is_none() {
        return Err("it has no `<path>`".into());
    }
    Ok(Svg {
        view_box: numbers
            .iter()
            .map(|n| exact_num::Shortest(*n).to_string())
            .collect::<Vec<_>>()
            .join(" "),
        path: paths.join(" "),
        not_template: why,
    })
}

#[cfg(test)]
mod tests {
    use super::{read, standalone};

    #[test]
    fn a_material_symbol_is_one_path_under_its_viewbox() {
        let svg = read(r#"<svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="M280-80 120-240Z"/></svg>"#).unwrap();
        assert_eq!(svg.view_box, "0 -960 960 960");
        assert_eq!(svg.path, "M280-80 120-240Z");
        assert_eq!(svg.not_template, None);
    }

    #[test]
    fn several_paths_of_one_fill_join_and_others_are_no_template() {
        let svg = read(r##"<svg viewBox="0 0 24 24"><g fill="#000"><path d="M0 0h1"/><path d="m2 2h1"/></g></svg>"##).unwrap();
        assert_eq!(svg.path, "M0 0h1 M2 2h1");
        assert_eq!(svg.not_template, None);
        let two = read(r#"<svg viewBox="0 0 24 24"><path fill="red" d="M0 0h1"/><path fill="blue" d="M2 2h1"/></svg>"#).unwrap();
        assert_eq!(
            two.not_template.as_deref(),
            Some("it fills in more than one colour")
        );
        let gradient =
            read(r#"<svg viewBox="0 0 24 24"><linearGradient id="a"/><path d="M0 0h1"/></svg>"#)
                .unwrap();
        assert!(gradient.not_template.unwrap().contains("linearGradient"));
        assert!(read(r#"<svg viewBox="0 0 0 24"><path d="M0 0"/></svg>"#).is_err());
        assert!(read(r#"<svg><path d="M0 0"/></svg>"#).is_err());
    }

    /// Review round 1: comments and CDATA draw nothing; the style beats the
    /// attribute; an inherited fill is the path's; a transform, a stroke and
    /// an unfilled path are no template; `|` is no path data; `defs` draw
    /// nothing; entities are decoded; attributes are apart.
    #[test]
    fn the_svg_is_read_as_xml_and_its_cascade() {
        let commented = read(r#"<svg viewBox="0 0 24 24"><!-- > <path d="M9 9"/> --><![CDATA[<path d="M8 8"/>]]><path d="M0 0h1"/></svg>"#).unwrap();
        assert_eq!(commented.path, "M0 0h1");
        let styled = read(r#"<svg viewBox="0 0 24 24"><path fill="red" style="fill:none;stroke:black" d="M0 0h1"/></svg>"#).unwrap();
        assert!(styled.not_template.is_some());
        let important = read(r#"<svg viewBox="0 0 24 24"><path style="fill : red !important" d="M0 0h1"/><path fill="red" d="M2 2h1"/></svg>"#).unwrap();
        assert_eq!(important.not_template, None);
        let nested = read(r#"<svg viewBox="0 0 24 24"><g fill="red"><g fill="blue"><path d="M0 0h1"/><path d="M2 2h1"/></g></g></svg>"#).unwrap();
        assert_eq!(nested.not_template, None);
        let moved =
            read(r#"<svg viewBox="0 0 24 24"><path transform="scale(2)" d="M0 0h1"/></svg>"#)
                .unwrap();
        assert!(moved.not_template.unwrap().contains("transform"));
        assert!(read(r#"<svg viewBox="0 0 24 24"><path d="M0 0|L24 24"/></svg>"#).is_err());
        let defs = read(
            r#"<svg viewBox="0 0 24 24"><defs><path d="M9 9h1"/></defs><path d="M0 0h1"/></svg>"#,
        )
        .unwrap();
        assert_eq!(defs.path, "M0 0h1");
        let entity = read(r#"<svg viewBox="0&#32;0 24 24"><path d="M0&#x20;0h1"/></svg>"#).unwrap();
        assert_eq!(
            (entity.view_box.as_str(), entity.path.as_str()),
            ("0 0 24 24", "M0 0h1")
        );
        assert!(read(r#"<svg viewBox="0 0 24 24"><path d="M0 0"fill="red"/></svg>"#).is_err());
    }

    #[test]
    fn a_relative_moveto_joins_as_absolute() {
        assert_eq!(standalone("m2 2h1"), "M2 2h1");
        assert_eq!(standalone("m2,2 3 3z"), "M2,2 l3 3z");
        assert_eq!(standalone("M1 1"), "M1 1");
    }
}
