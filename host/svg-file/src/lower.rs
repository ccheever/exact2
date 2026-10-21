//! The document's elements as the kernel's SVG nodes (LLP 1011.001 D2):
//! each tag the compiler lowers an inline `svg` tag to, each attribute the
//! row or prop the compiler gives the same attribute (`contract/lower`'s
//! tag and attribute tables, mirrored here for the file's names), the
//! cascade of a `style` element and a `style=""` folded into the element's
//! attributes, and what a browser drops from a picture dropped with a
//! warning: scripts, animation, links and external references.

use crate::css;
use crate::xml::Event;
use crate::{Document, Error, Natural};
use exact_kernel::{NodeType, Op, PropId, PropKind, PropValue, StyleId, StyleProps, StyleValue};

/// Elements past this count are a file no picture needs (D2's bound).
pub const MAX_ELEMENTS: usize = 50_000;
/// Nesting past this depth is a file no picture needs.
pub const MAX_DEPTH: usize = 256;

/// Lower the events of a document to its ops.
pub fn lower(events: Vec<Event>) -> Result<Document, Error> {
    let mut l = Lowerer {
        ops: Vec::new(),
        next: 1,
        warnings: Vec::new(),
        rules: Vec::new(),
        elements: 0,
        text: false,
    };
    // The sheet applies to every element, those before it too: read the
    // whole document's `style` elements first.
    l.read_sheets(&events);
    let mut i = 0;
    let mut root = None;
    while i < events.len() {
        match &events[i] {
            Event::Start {
                name, attrs, line, ..
            } => {
                if root.is_some() {
                    return Err(Error::at(*line, "a second root element"));
                }
                if local(name) != "svg" {
                    return Err(Error::at(
                        *line,
                        format!("the root element is `<{name}>`, not `<svg>`"),
                    ));
                }
                let natural = Natural::read(attrs);
                let id = l.element(&events, &mut i, None, 0)?;
                root = Some((id, natural));
            }
            Event::End { line, .. } => return Err(Error::at(*line, "an end tag at the root")),
            Event::Text(_) => i += 1,
        }
    }
    let Some((svg, natural)) = root else {
        return Err(Error::at(1, "no `<svg>` element"));
    };
    // The svg under a view, the shape every plan has (the kernel's root).
    let view = l.next;
    l.ops.push(Op::CreateView {
        id: view,
        node_type: NodeType::View,
    });
    l.ops.push(Op::SetChildren {
        id: view,
        children: vec![svg],
    });
    l.ops.push(Op::AttachRoot { id: view });
    Ok(Document {
        ops: l.ops,
        root: view,
        svg,
        natural,
        warnings: l.warnings,
        text: l.text,
    })
}

/// A name without its namespace prefix (`svg:path` is `path`); `None` for
/// a foreign prefix (`inkscape:`, `sodipodi:`), which a browser ignores.
fn local(name: &str) -> &str {
    match name.split_once(':') {
        Some(("svg", rest)) => rest,
        Some(_) => "",
        None => name,
    }
}

struct Lowerer {
    ops: Vec<Op>,
    next: u32,
    warnings: Vec<String>,
    rules: Vec<css::Rule>,
    elements: usize,
    text: bool,
}

/// The node type an element lowers to; `None` for one that is skipped
/// whole (with the subtree), with the warning to give.
fn node_type(tag: &str, nested_svg: bool) -> Result<NodeType, Option<&'static str>> {
    let name = match tag {
        "svg" if nested_svg => "SvgViewport",
        "svg" => "Svg",
        "g" | "a" | "switch" => "SvgGroup",
        "path" => "SvgPath",
        "polyline" => "SvgPolyline",
        "polygon" => "SvgPolygon",
        "circle" => "SvgCircle",
        "ellipse" => "SvgEllipse",
        "line" => "SvgLine",
        "rect" => "SvgRect",
        "text" => "SvgText",
        "tspan" => "SvgTSpan",
        "defs" => "SvgDefs",
        "linearGradient" => "SvgLinearGradient",
        "radialGradient" => "SvgRadialGradient",
        "stop" => "SvgStop",
        "use" => "SvgUse",
        "clipPath" => "SvgClipPath",
        "mask" => "SvgMask",
        "pattern" => "SvgPattern",
        "symbol" => "SvgSymbol",
        "marker" => "SvgMarker",
        "filter" => "SvgFilter",
        t if t.starts_with("fe") => "SvgFe",
        // Read elsewhere, nothing a picture draws, or another namespace's:
        // silently skipped, as a browser skips them.
        "" | "title" | "desc" | "metadata" | "style" => return Err(None),
        "script" => return Err(Some("does not run in an image")),
        "animate" | "animateTransform" | "animateMotion" | "set" | "mpath" | "discard" => {
            return Err(Some(
                "animation inside an image is ignored (LLP 1011.001 §3): the still values draw",
            ))
        }
        "image" => {
            return Err(Some(
                "is dropped from an SVG image (an external reference): the file still draws",
            ))
        }
        "foreignObject" => {
            return Err(Some(
                "`foreignObject` renders on the web only (LLP 1055.000 D13)",
            ))
        }
        "textPath" => {
            return Err(Some(
                "`textPath` in an SVG image is not read yet: its text is dropped",
            ))
        }
        "font" | "font-face" | "glyph" | "cursor" | "view" => {
            return Err(Some("has no place in a picture"))
        }
        _ => return Err(Some("is not an SVG element this host draws")),
    };
    Ok(NodeType::from_name(name).unwrap_or_else(|| panic!("the kernel has no node type {name}")))
}

impl Lowerer {
    fn read_sheets(&mut self, events: &[Event]) {
        let mut i = 0;
        while i < events.len() {
            if let Event::Start { name, empty, .. } = &events[i] {
                if local(name) == "style" && !*empty {
                    let mut text = String::new();
                    let mut j = i + 1;
                    while let Some(Event::Text(t)) = events.get(j) {
                        text.push_str(t);
                        j += 1;
                    }
                    let mut dropped = Vec::new();
                    self.rules.extend(css::rules(&text, &mut dropped));
                    for selector in dropped {
                        self.warnings.push(format!("the stylesheet's `{selector}` is not read: an SVG image takes type, class and id selectors with no combinators"));
                    }
                }
            }
            i += 1;
        }
    }

    /// Skip the subtree from the start event at `*i`, leaving `*i` past it.
    fn skip(events: &[Event], i: &mut usize) {
        let mut depth = 0usize;
        loop {
            match &events[*i] {
                Event::Start { empty, .. } => {
                    *i += 1;
                    if !*empty {
                        depth += 1;
                    }
                }
                Event::End { .. } => {
                    *i += 1;
                    depth -= 1;
                }
                Event::Text(_) => *i += 1,
            }
            if depth == 0 {
                return;
            }
        }
    }

    /// Lower the element whose start event is at `*i`, leaving `*i` past
    /// its end; its node id.
    fn element(
        &mut self,
        events: &[Event],
        i: &mut usize,
        parent: Option<&str>,
        depth: usize,
    ) -> Result<u32, Error> {
        let Event::Start {
            name,
            attrs,
            empty,
            line,
        } = &events[*i]
        else {
            unreachable!("element starts at a start event");
        };
        if depth > MAX_DEPTH {
            return Err(Error::at(
                *line,
                format!("elements nested deeper than {MAX_DEPTH}"),
            ));
        }
        self.elements += 1;
        if self.elements > MAX_ELEMENTS {
            return Err(Error::at(
                *line,
                format!("more than {MAX_ELEMENTS} elements"),
            ));
        }
        let tag = local(name).to_string();
        let nested_svg = tag == "svg" && parent.is_some();
        let node_type = match node_type(&tag, nested_svg) {
            Ok(t) => t,
            Err(why) => {
                if let Some(why) = why {
                    self.warnings.push(format!("line {line}: `<{name}>` {why}"));
                }
                Self::skip(events, i);
                return Ok(0);
            }
        };
        let id = self.next;
        self.next += 1;
        self.ops.push(Op::CreateView { id, node_type });
        if tag == "text" || tag == "tspan" {
            self.text = true;
        }
        let in_text = tag == "text" || tag == "tspan";
        let is_fe = tag.starts_with("fe");
        self.attributes(id, &tag, attrs, in_text, is_fe);
        if is_fe {
            self.ops.push(Op::SetProp {
                id,
                prop: PropId::from_name("fe").expect("the kernel has the fe prop"),
                value: PropValue::Str(tag.as_str().into()),
            });
        }
        let empty = *empty;
        *i += 1;
        let mut children = Vec::new();
        let mut text = String::new();
        if !empty {
            let switched = tag == "switch";
            let mut taken = false;
            loop {
                match &events[*i] {
                    Event::End { .. } => {
                        *i += 1;
                        break;
                    }
                    Event::Text(t) => {
                        if in_text {
                            text.push_str(t);
                        }
                        *i += 1;
                    }
                    Event::Start { attrs, .. } => {
                        // `switch`: the first child whose conditions pass
                        // draws; the rest are not read (SVG 2 §5.8).
                        if switched
                            && (taken
                                || attrs.iter().any(|(k, _)| {
                                    k == "requiredExtensions" || k == "requiredFeatures"
                                }))
                        {
                            Self::skip(events, i);
                            continue;
                        }
                        let child = self.element(events, i, Some(&tag), depth + 1)?;
                        if child != 0 {
                            children.push(child);
                            taken = true;
                        }
                    }
                }
            }
        }
        if in_text {
            // Character data as the compiler's positional text (LLP
            // 1055.000 D11); a run of white space collapses, as the
            // default `xml:space` collapses it.
            let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !collapsed.is_empty() {
                self.ops.push(Op::SetProp {
                    id,
                    prop: PropId::from_name("text").expect("the kernel has the text prop"),
                    value: PropValue::Str(collapsed.as_str().into()),
                });
            }
        }
        if !children.is_empty() {
            self.ops.push(Op::SetChildren { id, children });
        }
        Ok(id)
    }

    /// The element's attributes, cascaded: presentation attributes, then the
    /// sheet's matching rules, then `style=""`, each later one overriding.
    fn attributes(
        &mut self,
        id: u32,
        tag: &str,
        attrs: &[(String, String)],
        in_text: bool,
        is_fe: bool,
    ) {
        let mut merged: Vec<(String, String)> = Vec::new();
        let mut put = |name: String, value: String| {
            if let Some(slot) = merged.iter_mut().find(|(n, _)| *n == name) {
                slot.1 = value;
            } else {
                merged.push((name, value));
            }
        };
        let mut style_attr = None;
        let mut class = None;
        let mut element_id = None;
        for (name, value) in attrs {
            let key = match name.as_str() {
                "xlink:href" => "href",
                n if n.starts_with("xmlns") => continue,
                "xml:space" | "xml:lang" | "xml:base" => continue,
                n => {
                    if n.contains(':') {
                        continue;
                    }
                    n
                }
            };
            match key {
                "style" => style_attr = Some(value.clone()),
                "class" => class = Some(value.clone()),
                "id" => {
                    element_id = Some(value.clone());
                    put("id".into(), value.clone());
                }
                _ => put(key.into(), value.clone()),
            }
        }
        let classes: Vec<&str> = class
            .as_deref()
            .map(|c| c.split_whitespace().collect())
            .unwrap_or_default();
        for (name, value) in css::matching(&self.rules, tag, element_id.as_deref(), &classes) {
            put(name.clone(), value.clone());
        }
        if let Some(style) = &style_attr {
            for (name, value) in css::declarations(style) {
                put(name, value);
            }
        }
        let mut patch = StyleProps::default();
        let mut styled = false;
        for (name, value) in merged {
            match self.attribute(tag, &name, &value, in_text, is_fe) {
                Attr::Prop(prop, value) => self.ops.push(Op::SetProp { id, prop, value }),
                Attr::Style(row, value) => match patch.set_dynamic(row, &value) {
                    Ok(()) => styled = true,
                    // A unitless number where the row's grammar wants a
                    // length (`stroke-width="0.5"`): SVG's user units.
                    Err(_)
                        if matches!(&value, StyleValue::Text(t) if exact_num::parse_f64(t).is_ok_and(f64::is_finite))
                            && patch
                                .set_dynamic(
                                    row,
                                    &StyleValue::Number(
                                        exact_num::parse_f64(value_text(&value)).unwrap_or(0.0),
                                    ),
                                )
                                .is_ok() =>
                    {
                        styled = true
                    }
                    Err(_) => self.warnings.push(format!(
                        "`{name}=\"{}\"` on `<{tag}>` is not a value the row takes",
                        clip(&value)
                    )),
                },
                Attr::Ignored => {}
                Attr::Unknown => self.warnings.push(format!(
                    "`{name}` on `<{tag}>` is not an attribute this host draws"
                )),
            }
        }
        if tag == "svg" || tag == "symbol" || tag == "marker" {
            // The tag's fixed rows, as the compiler sets them: these
            // viewports clip to their box.
            for row in ["overflow_x", "overflow_y"] {
                let _ = patch.set_dynamic(
                    StyleId::from_name(row).expect("overflow rows"),
                    &StyleValue::Text("hidden".into()),
                );
            }
            styled = true;
        }
        if styled {
            self.ops.push(Op::SetStyle {
                id,
                patch: Box::new(patch),
            });
        }
    }

    /// One attribute as the kernel takes it.
    fn attribute(&self, tag: &str, name: &str, value: &str, in_text: bool, is_fe: bool) -> Attr {
        let value = value.trim();
        // The compiler's renames: a text's positions and a primitive's own
        // numbers are props under other names, since the plain names are rows.
        let name = if in_text {
            match name {
                "x" => "textX",
                "y" => "textY",
                "dx" => "textDx",
                "dy" => "textDy",
                n => n,
            }
        } else if is_fe {
            let light = tag.ends_with("Light");
            match name {
                "dx" => "feDx",
                "dy" => "feDy",
                "scale" => "feScale",
                "radius" => "feRadius",
                "order" => "feOrder",
                "x" if light => "lightX",
                "y" if light => "lightY",
                "z" if light => "lightZ",
                n => n,
            }
        } else {
            name
        };
        if matches!(
            name,
            "requiredExtensions"
                | "requiredFeatures"
                | "systemLanguage"
                | "version"
                | "baseProfile"
                | "xml:space"
                | "lang"
                | "tabindex"
                | "role"
                | "focusable"
                | "aria-hidden"
                | "aria-label"
                | "data-name"
                | "externalResourcesRequired"
                | "class"
        ) {
            return Attr::Ignored;
        }
        if name == "href" {
            // A reference into this file; anything else is external and
            // dropped, as a browser drops it from a picture.
            return if value.starts_with('#') {
                Attr::Prop(
                    PropId::from_name("href").expect("href"),
                    PropValue::Str(value.into()),
                )
            } else {
                Attr::Ignored
            };
        }
        if let Some(prop) = PropId::from_name(name) {
            let value = match prop.kind() {
                PropKind::Str => PropValue::Str(value.into()),
                PropKind::Float => match exact_num::parse_f64(value) {
                    Ok(n) if n.is_finite() => PropValue::Float(n),
                    _ => return Attr::Unknown,
                },
                PropKind::Int => match value.parse::<i64>() {
                    Ok(n) => PropValue::Int(n),
                    Err(_) => return Attr::Unknown,
                },
                PropKind::Bool => PropValue::Bool(value == "true"),
            };
            return Attr::Prop(prop, value);
        }
        let field = match name {
            "color" => "text_color".to_string(),
            "mask" => "svg_mask".to_string(),
            n => n.replace('-', "_"),
        };
        let Some(row) = StyleId::from_name(&field) else {
            return Attr::Unknown;
        };
        // A geometry attribute is a unitless number (`y="46"`), which the
        // row's CSS grammar would refuse; a percentage is its own value.
        let geometry = matches!(
            name,
            "x" | "y" | "rx" | "ry" | "cx" | "cy" | "r" | "width" | "height"
        );
        if geometry {
            if let Ok(n) = exact_num::parse_f64(value) {
                if n.is_finite() {
                    return Attr::Style(row, StyleValue::Number(n));
                }
            }
            if let Some(p) = value
                .strip_suffix('%')
                .and_then(|p| exact_num::parse_f64(p.trim()).ok())
            {
                return Attr::Style(row, StyleValue::Percent(p));
            }
        }
        Attr::Style(row, StyleValue::Text(value.to_string().into()))
    }
}

enum Attr {
    Prop(PropId, PropValue),
    Style(StyleId, StyleValue<'static>),
    Ignored,
    Unknown,
}

fn value_text<'v>(value: &'v StyleValue<'_>) -> &'v str {
    match value {
        StyleValue::Text(t) => t,
        _ => "",
    }
}

fn clip(value: &StyleValue<'_>) -> String {
    let text = match value {
        StyleValue::Text(t) => t.to_string(),
        StyleValue::Number(n) => n.to_string(),
        StyleValue::Percent(p) => format!("{p}%"),
        StyleValue::Auto => "auto".into(),
        StyleValue::Vec2(a, b) => format!("{a} {b}"),
    };
    if text.len() > 40 {
        format!("{}…", &text[..40])
    } else {
        text
    }
}
