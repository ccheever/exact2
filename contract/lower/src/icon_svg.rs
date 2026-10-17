//! @ref LLP 1121 D10 (stage 6 F) — an icon token's SVG, read at build: for
//! v1 one monochrome shape, its `<path>`s joined into one `d` (subpaths) and
//! drawn as a template under its own viewBox, as the roles' paths are drawn.
//! Anything else (several colours, a gradient, an image, text, a stroke, a
//! shape that is not a path) is no template: the web may draw it as the file
//! (`multicolor`), and a shipped target that is not the web refuses it.

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

/// The value of attribute `name` in a tag's text.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().last();
        let after = &rest[at + name.len()..];
        let after = after.trim_start();
        if before.is_some_and(char::is_whitespace) {
            if let Some(v) = after.strip_prefix('=') {
                let v = v.trim_start();
                let q = v.chars().next()?;
                if q == '"' || q == '\'' {
                    let end = v[1..].find(q)?;
                    return Some(&v[1..1 + end]);
                }
            }
        }
        rest = &rest[at + name.len()..];
    }
    None
}

/// Read `text` as an icon's SVG, or why it is none.
pub(crate) fn read(text: &str) -> Result<Svg, String> {
    let (mut view_box, mut paths, mut fills, mut why) = (None, Vec::new(), Vec::new(), None);
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        rest = &rest[open + 1..];
        let end = rest.find('>').ok_or("an SVG tag is not closed")?;
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        if tag.starts_with(['/', '?', '!']) {
            continue;
        }
        let name = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let fill = attr(tag, "fill").or_else(|| {
            attr(tag, "style").and_then(|s| {
                s.split(';')
                    .find_map(|d| d.trim().strip_prefix("fill:").map(str::trim))
            })
        });
        if let Some(f) = fill.filter(|f| !f.eq_ignore_ascii_case("none")) {
            let f = f.to_ascii_lowercase();
            if !fills.contains(&f) {
                fills.push(f);
            }
        }
        if attr(tag, "stroke").is_some_and(|s| !s.eq_ignore_ascii_case("none")) {
            why.get_or_insert_with(|| "it has a stroke".to_string());
        }
        match name {
            "svg" => {
                view_box = attr(tag, "viewBox").map(str::to_string).or_else(|| {
                    let n = |a| attr(tag, a).map(|v: &str| v.trim_end_matches("px").to_string());
                    Some(format!("0 0 {} {}", n("width")?, n("height")?))
                });
            }
            "path" => match attr(tag, "d") {
                Some(d) if !d.trim().is_empty() => paths.push(d.trim().to_string()),
                _ => return Err("a `<path>` has no `d`".into()),
            },
            "g" | "title" | "desc" | "defs" | "metadata" => {}
            other => {
                why.get_or_insert_with(|| format!("it has a `<{other}>`, not only paths"));
            }
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
    let colours = fills.iter().filter(|f| *f != "currentcolor").count();
    if colours > 1 {
        why.get_or_insert_with(|| format!("it fills in {colours} colours"));
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
    use super::read;

    #[test]
    fn a_material_symbol_is_one_path_under_its_viewbox() {
        let svg = read(r#"<svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="M280-80 120-240Z"/></svg>"#).unwrap();
        assert_eq!(svg.view_box, "0 -960 960 960");
        assert_eq!(svg.path, "M280-80 120-240Z");
        assert_eq!(svg.not_template, None);
    }

    #[test]
    fn several_paths_of_one_fill_join_and_others_are_no_template() {
        let svg = read(r##"<svg viewBox="0 0 24 24"><g fill="#000"><path d="M0 0h1"/><path d="M2 2h1"/></g></svg>"##).unwrap();
        assert_eq!(svg.path, "M0 0h1 M2 2h1");
        assert_eq!(svg.not_template, None);
        let two = read(r#"<svg viewBox="0 0 24 24"><path fill="red" d="M0 0h1"/><path fill="blue" d="M2 2h1"/></svg>"#).unwrap();
        assert_eq!(two.not_template.as_deref(), Some("it fills in 2 colours"));
        let gradient =
            read(r#"<svg viewBox="0 0 24 24"><linearGradient id="a"/><path d="M0 0h1"/></svg>"#)
                .unwrap();
        assert!(gradient.not_template.unwrap().contains("linearGradient"));
        assert!(read(r#"<svg viewBox="0 0 0 24"><path d="M0 0"/></svg>"#).is_err());
        assert!(read(r#"<svg><path d="M0 0"/></svg>"#).is_err());
    }
}
