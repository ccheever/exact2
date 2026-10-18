//! @ref LLP 1121 D10 (stage 6 F) — an icon token's SVG, read at build: for
//! v1 one monochrome shape, its `<path>`s each filled (nonzero) under its own
//! viewBox, as a template, as the roles' paths are drawn; their `d`s travel
//! joined by `;`, which no path data holds.
//! Anything else (several colours, a gradient, an image, text, a stroke, a
//! transform, a shape that is not a path) is no template: the web may draw
//! it as the file, and a shipped target that is not the web refuses it.

/// An SVG read as an icon.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Svg {
    /// Its viewBox, as written (`0 -960 960 960`).
    pub view_box: String,
    /// Its paths' `d`, joined by `;`.
    pub path: String,
    /// Why it is no one-colour shape, when it is not.
    pub not_template: Option<String>,
    /// What to change in the file so that it is one, beside `not_template`.
    pub fix: String,
}

/// The fix for a property a template loses.
fn fix_lost(property: &str) -> &'static str {
    match property {
        "transform" => "apply the transform to the path's coordinates and remove it (an editor's Object to Path, or SVGO's `convertTransform`, does)",
        "fill-rule" => "remove it: a template fills nonzero, so a hole is drawn in the opposite direction to its outline (an editor's path union or Combine makes it so)",
        "opacity" | "fill-opacity" => "remove it: the host paints the shape opaque, in the text's colour",
        "display" | "visibility" => "delete the hidden element",
        _ => "remove it, cutting the shape itself instead (an editor's Intersect or Difference)",
    }
}

/// The fix for an element that is not a path.
const FIX_SHAPE: &str = "convert it to a `<path>` (an editor's Object to Path; `<rect x=\"2\" y=\"2\" width=\"20\" height=\"20\"/>` is `<path d=\"M2 2h20v20H2Z\"/>`)";

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
    // The last declaration wins, an `!important` one over any other.
    let styled = attrs
        .iter()
        .find(|(k, _)| *k == "style")
        .and_then(|(_, s)| {
            let declared = s.split(';').filter_map(|d| {
                let (k, v) = d.split_once(':')?;
                (k.trim() == name).then(|| (v.trim().ends_with("!important"), clean(v)))
            });
            declared.fold(
                None,
                |won: Option<(bool, String)>, (important, v)| match won {
                    Some((true, _)) if !important => won,
                    _ => Some((important, v)),
                },
            )
        });
    let styled = styled.map(|(_, v)| v);
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

/// Each reason an SVG is no template, with its fix.
type Why = Vec<(String, &'static str)>;

/// Add a reason to `why`, once.
fn note(why: &mut Why, reason: impl FnOnce() -> (String, &'static str)) {
    let (said, fix) = reason();
    if !why.iter().any(|(r, _)| *r == said) {
        why.push((said, fix));
    }
}

/// Read `text` as an icon's SVG, or why it is none.
pub(crate) fn read(text: &str) -> Result<Svg, String> {
    // Every reason it is no template, so one build names each fix.
    let (mut view_box, mut paths, mut fill, mut why) =
        (None, Vec::<String>::new(), None::<String>, Why::new());
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
        // The tag ends at the first `>` outside its attributes' quotes.
        let mut quote = None;
        let end = rest
            .char_indices()
            .find(|&(_, c)| match quote {
                Some(q) if c == q => {
                    quote = None;
                    false
                }
                Some(_) => false,
                None if c == '"' || c == '\'' => {
                    quote = Some(c);
                    false
                }
                None => c == '>',
            })
            .map(|(i, _)| i)
            .ok_or("an SVG tag is not closed")?;
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
        // What a template drawn filled, nonzero, at full opacity would lose.
        if !in_defs {
            let lost = [
                "transform",
                "fill-rule",
                "opacity",
                "fill-opacity",
                "clip-path",
                "mask",
                "filter",
            ]
            .into_iter()
            .chain(["display", "visibility"])
            .find(|p| {
                property(&attrs, p).is_some_and(|v| match *p {
                    "fill-rule" => v != "nonzero",
                    "opacity" | "fill-opacity" => v.parse::<f64>().map_or(true, |o| o < 1.0),
                    "display" => v == "none",
                    "visibility" => v != "visible",
                    _ => v != "none",
                })
            });
            if let Some(p) = lost {
                note(&mut why, || {
                    (format!("its `<{name}>` has a `{p}`"), fix_lost(p))
                });
            }
        }
        match name {
            // A nested viewport places and scales what it holds.
            "svg" if !stack.is_empty() => {
                note(&mut why, || {
                    (
                        "it nests an `<svg>`".to_string(),
                        "move the inner `<svg>`'s paths into the outer one, in the outer viewBox's coordinates",
                    )
                });
            }
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
                if painted.starts_with("url(") {
                    note(&mut why, || {
                        (
                            "it fills with a gradient or a pattern".to_string(),
                            "fill every path with one plain colour (`fill=\"black\"`): the host paints it in the text's colour",
                        )
                    });
                }
                // A colour with its own alpha (`rgba()`, `#rrggbbaa`) is no opaque template.
                let hex = painted.strip_prefix('#').map_or(0, str::len);
                if painted.contains('/')
                    || painted.starts_with("rgba")
                    || painted.starts_with("hsla")
                    || painted == "transparent"
                    || hex == 4
                    || hex == 8
                {
                    note(&mut why, || {
                        (
                            "it fills in a translucent colour".to_string(),
                            "fill with an opaque colour (`fill=\"black\"`): the host paints it in the text's colour",
                        )
                    });
                }
                // An outline icon: its lines are strokes, which a template does not draw.
                let outline = "outline the stroke into a filled path and remove the `stroke` (Inkscape's Path > Stroke to Path, Figma's and Illustrator's Outline Stroke)";
                if stroke_here.as_deref().is_some_and(|s| s != "none") {
                    note(&mut why, || ("it has a stroke".to_string(), outline));
                }
                if painted == "none" {
                    note(&mut why, || ("a path is not filled".to_string(), outline));
                } else if fill.as_ref().is_some_and(|f| *f != painted) {
                    note(&mut why, || {
                        (
                            "it fills in more than one colour".to_string(),
                            "fill every path in the same colour: the host paints the whole shape in one",
                        )
                    });
                } else {
                    fill = Some(painted);
                }
                paths.push(d);
            }
            "path" | "g" | "title" | "desc" | "defs" | "metadata" => {}
            _ if in_defs => {}
            other => {
                note(&mut why, || {
                    (format!("it has a `<{other}>`, not only paths"), FIX_SHAPE)
                });
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
    if paths.is_empty() && why.is_empty() {
        return Err("it has no `<path>`".into());
    }
    let mut fixes: Vec<&str> = Vec::new();
    for (_, fix) in &why {
        if !fixes.contains(fix) {
            fixes.push(fix);
        }
    }
    Ok(Svg {
        view_box: numbers
            .iter()
            .map(|n| exact_num::Shortest(*n).to_string())
            .collect::<Vec<_>>()
            .join(" "),
        path: paths.join(";"),
        fix: fixes.join("; "),
        not_template: (!why.is_empty()).then(|| {
            why.into_iter()
                .map(|(r, _)| r)
                .collect::<Vec<_>>()
                .join("; ")
        }),
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
        let svg = read(r##"<svg viewBox="0 0 24 24"><g fill="#000"><path d="M0 0h1"/><path d="m2 2h1"/></g></svg>"##).unwrap();
        assert_eq!(svg.path, "M0 0h1;m2 2h1");
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

    /// An outline icon (Lucide's shape: strokes, a `<rect>`) names each reason
    /// once and each fix, so one build says everything to change.
    #[test]
    fn every_reason_is_named_with_its_fix() {
        let svg = read(r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor"><path d="M12 20h9"/><path d="M3 3h1"/><rect x="2" y="2" width="4" height="4"/></svg>"#).unwrap();
        assert_eq!(
            svg.not_template.as_deref(),
            Some("it has a stroke; a path is not filled; it has a `<rect>`, not only paths")
        );
        assert!(svg.fix.contains("Stroke to Path") && svg.fix.contains("Object to Path"));
        assert_eq!(svg.fix.matches("Stroke to Path").count(), 1);
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
        let gradient = read(r#"<svg viewBox="0 0 24 24"><defs><linearGradient id="g"/></defs><path fill="url(#g)" d="M0 0h1"/></svg>"#).unwrap();
        assert!(gradient.not_template.is_some());
        for hidden in [
            r#"display="none""#,
            r#"visibility="hidden""#,
            r##"fill="#ff000080""##,
            r#"fill="rgba(0,0,0,.5)""#,
            r#"fill="transparent""#,
            r#"style="opacity:.5 !important; opacity:1""#,
        ] {
            let svg = read(&format!(
                r#"<svg viewBox="0 0 24 24"><path {hidden} d="M0 0h1"/></svg>"#
            ))
            .unwrap();
            assert!(svg.not_template.is_some(), "{hidden}");
        }
        let nested = read(r#"<svg viewBox="0 0 24 24"><svg x="12" viewBox="0 0 10 10"><path d="M0 0h1"/></svg></svg>"#).unwrap();
        assert!(nested.not_template.is_some());
        let important = read(r#"<svg viewBox="0 0 24 24"><path style="fill : red !important" d="M0 0h1"/><path fill="red" d="M2 2h1"/></svg>"#).unwrap();
        assert_eq!(important.not_template, None);
        let nested = read(r#"<svg viewBox="0 0 24 24"><g fill="red"><g fill="blue"><path d="M0 0h1"/><path d="M2 2h1"/></g></g></svg>"#).unwrap();
        assert_eq!(nested.not_template, None);
        let moved =
            read(r#"<svg viewBox="0 0 24 24"><path transform="scale(2)" d="M0 0h1"/></svg>"#)
                .unwrap();
        assert!(moved.not_template.unwrap().contains("transform"));
        for lost in [
            r#"fill-rule="evenodd""#,
            r#"style="opacity:0.5""#,
            r#"clip-path="url(#a)""#,
            r#"style="transform:scale(2)""#,
        ] {
            let svg = read(&format!(
                r#"<svg viewBox="0 0 24 24"><path {lost} d="M0 0h1"/></svg>"#
            ))
            .unwrap();
            assert!(svg.not_template.is_some(), "{lost}");
        }
        let fine = read(
            r#"<svg viewBox="0 0 24 24"><path fill-rule="nonzero" opacity="1" d="M0 0h1"/></svg>"#,
        )
        .unwrap();
        assert_eq!(fine.not_template, None);
        let quoted =
            read(r#"<svg aria-label="A > B" viewBox="0 0 24 24"><path d="M0 0h1"/></svg>"#)
                .unwrap();
        assert_eq!(quoted.path, "M0 0h1");
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
}
