//! Inline SVG and CSS animations in Contract (LLP 1055).
//!
//! @ref LLP 1055 D1 (the subset and where each attribute belongs), D3 (the
//! content model), D5 (`keyframes` and the `animation` longhands), D12 (what
//! is refused, by name)
//!
//! `keyframes Name` lowers to one plan row of CSS text; `animation` and its
//! longhands lower to one `animation` row per node, composed here; an SVG
//! element may sit only in an `svg` or `g`, takes only what applies to it,
//! and handles no events (its `svg` is the one hit and accessibility box).

use crate::{err, tags, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, File, Span, TemplatePart};
use exact_motion::animation::LONGHANDS;
use exact_motion::{Animation, Animations, Keyframes, Property};

/// The filter primitives and their children (LLP 1055.000 D14), each a
/// `SvgFe` node whose `fe` prop is its tag.
pub(crate) const FE: [&str; 24] = [
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDropShadow",
    "feFlood",
    "feFuncR",
    "feFuncG",
    "feFuncB",
    "feFuncA",
    "feGaussianBlur",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "feSpecularLighting",
    "feTile",
    "feTurbulence",
    "feDistantLight",
    "fePointLight",
    "feSpotLight",
];

/// The SVG element tags inside an `svg`.
pub(crate) fn is_element(tag: &str) -> bool {
    FE.contains(&tag)
        || matches!(
            tag,
            "g" | "path"
                | "polyline"
                | "polygon"
                | "circle"
                | "ellipse"
                | "line"
                | "rect"
                | "defs"
                | "linearGradient"
                | "radialGradient"
                | "stop"
                | "use"
                | "symbol"
                | "clipPath"
                | "marker"
                | "mask"
                | "pattern"
                | "foreignObject"
                | "filter"
                | "tspan"
        )
}

/// Whether a node under `parent_tag` is inside an `svg`: `inside` says an
/// `svg` encloses it, for a `text` parent is a box outside one.
pub(crate) fn in_svg(inside: bool, parent_tag: Option<&str>) -> bool {
    inside
        && matches!(
            parent_tag,
            Some(
                "svg"
                    | "g"
                    | "defs"
                    | "symbol"
                    | "linearGradient"
                    | "radialGradient"
                    | "clipPath"
                    | "marker"
                    | "mask"
                    | "pattern"
                    | "filter"
                    | "feMerge"
                    | "feComponentTransfer"
                    | "feDiffuseLighting"
                    | "feSpecularLighting"
                    | "text"
                    | "tspan"
            )
        )
}

/// What an SVG container may hold (LLP 1055.000 D7): a gradient holds
/// `stop`s only, and a `stop` only sits in a gradient.
fn holds(parent: &str, child: &str) -> bool {
    match parent {
        // SVG text holds its runs.
        "text" | "tspan" => child == "tspan",
        "linearGradient" | "radialGradient" => child == "stop",
        "filter" => {
            FE.contains(&child)
                && !child.starts_with("feFunc")
                && !child.ends_with("Light")
                && child != "feMergeNode"
        }
        "feMerge" => child == "feMergeNode",
        "feComponentTransfer" => child.starts_with("feFunc"),
        "feDiffuseLighting" | "feSpecularLighting" => child.ends_with("Light"),
        "svg" | "g" | "defs" | "symbol" | "marker" | "mask" | "pattern" => {
            !FE.contains(&child)
                && child != "stop"
                && !(child == "foreignObject" && matches!(parent, "defs" | "mask" | "pattern"))
                && child != "tspan"
                && (is_element(child) || matches!(child, "svg" | "text"))
        }
        // A clip's children are its geometry: shapes and `use`.
        "clipPath" => matches!(
            child,
            "path" | "polyline" | "polygon" | "circle" | "ellipse" | "line" | "rect" | "use"
        ),
        _ => false,
    }
}

/// SVG length attributes CSS cannot set: props holding the authored text
/// (LLP 1055.000 D4), so a number and `"50%"` are both accepted.
pub(crate) fn is_length_prop(attr: &str) -> bool {
    matches!(
        attr,
        "x1" | "y1"
            | "x2"
            | "y2"
            | "fx"
            | "fy"
            | "fr"
            | "offset"
            | "textX"
            | "textY"
            | "textDx"
            | "textDy"
            | "markerWidth"
            | "markerHeight"
            | "refX"
            | "refY"
            | "orient"
    ) || FE_ATTRS.contains(&attr)
}

/// A filter primitive's attributes, as props holding authored text.
const FE_ATTRS: [&str; 47] = [
    "in",
    "in2",
    "result",
    "stdDeviation",
    "feDx",
    "feDy",
    "operator",
    "k1",
    "k2",
    "k3",
    "k4",
    "mode",
    "values",
    "edgeMode",
    "feRadius",
    "tableValues",
    "slope",
    "intercept",
    "amplitude",
    "exponent",
    "baseFrequency",
    "numOctaves",
    "seed",
    "stitchTiles",
    "feScale",
    "xChannelSelector",
    "yChannelSelector",
    "order",
    "kernelMatrix",
    "divisor",
    "bias",
    "targetX",
    "targetY",
    "preserveAlpha",
    "surfaceScale",
    "diffuseConstant",
    "specularConstant",
    "specularExponent",
    "azimuth",
    "elevation",
    "lightX",
    "lightY",
    "lightZ",
    "pointsAtX",
    "pointsAtY",
    "pointsAtZ",
    "limitingConeAngle",
];

/// The elements an SVG-specific attribute belongs to.
fn owners(attr: &str) -> Option<&'static [&'static str]> {
    Some(match attr {
        "viewBox" | "preserveAspectRatio" => &["svg", "symbol", "marker", "pattern"],
        "markerWidth" | "markerHeight" | "refX" | "refY" | "orient" | "markerUnits" => &["marker"],
        "points" => &["polyline", "polygon"],
        "d" => &["path"],
        "pathLength" => &[
            "path", "polyline", "polygon", "circle", "ellipse", "line", "rect",
        ],
        "x" | "y" => &[
            "rect",
            "svg",
            "use",
            "mask",
            "pattern",
            "foreignObject",
            "filter",
            "feBlend",
            "feColorMatrix",
            "feComponentTransfer",
            "feComposite",
            "feConvolveMatrix",
            "feDiffuseLighting",
            "feDisplacementMap",
            "feDropShadow",
            "feFlood",
            "feGaussianBlur",
            "feMerge",
            "feMorphology",
            "feOffset",
            "feSpecularLighting",
            "feTile",
            "feTurbulence",
        ],
        "filterUnits" | "primitiveUnits" => &["filter"],
        "rx" | "ry" => &["rect", "ellipse"],
        "x1" | "y1" | "x2" | "y2" => &["line", "linearGradient"],
        "cx" | "cy" => &["circle", "ellipse", "radialGradient"],
        "r" => &["circle", "radialGradient"],
        "fx" | "fy" | "fr" => &["radialGradient"],
        "gradientUnits" | "gradientTransform" | "spreadMethod" => {
            &["linearGradient", "radialGradient"]
        }
        "stop-color" | "stop-opacity" => &["stop"],
        "offset" => &["stop", "feFuncR", "feFuncG", "feFuncB", "feFuncA"],
        "clipPathUnits" => &["clipPath"],
        "maskUnits" | "maskContentUnits" | "mask-type" => &["mask"],
        "patternUnits" | "patternContentUnits" | "patternTransform" => &["pattern"],
        "textX" | "textY" | "textDx" | "textDy" | "text-anchor" | "dominant-baseline" => {
            &["text", "tspan"]
        }
        "vector-effect" => &[
            "path", "polyline", "polygon", "circle", "ellipse", "line", "rect",
        ],
        _ => return None,
    })
}

/// What an SVG element takes besides its own geometry: paint, opacity,
/// motion, identity.
fn shared(attr: &str) -> bool {
    matches!(
        attr,
        "fill"
            | "stroke"
            | "stroke-width"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-miterlimit"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "fill-opacity"
            | "stroke-opacity"
            | "fill-rule"
            | "opacity"
            | "color"
            | "transform"
            | "transform-origin"
            | "transform-box"
            | "translate"
            | "rotate"
            | "scale"
            | "visibility"
            | "display"
            | "paint-order"
            | "clip-path"
            | "clip-rule"
            | "mask"
            | "filter"
            | "flood-color"
            | "flood-opacity"
            | "lighting-color"
            | "color-interpolation-filters"
            | "font-size"
            | "font-weight"
            | "font-style"
            | "font-family"
            | "letter-spacing"
            | "text-anchor"
            | "dominant-baseline"
            | "pointer-events"
            | "marker"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
            | "animation"
            | "transition"
            | "testId"
            | "id"
    )
}

/// A named refusal for an SVG tag outside the subset (LLP 1055 D12).
pub(crate) fn refused_tag(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "text" | "tspan" | "textPath" => {
            "text inside `svg` is refused (LLP 1055 D12); put a `text` beside the `svg`"
        }
        "image" | "feImage" => {
            "not in exact2's SVG yet: LLP 1055.000 §4 builds it in a later stage"
        }
        "hatch" | "hatchpath" | "mesh" | "meshgradient" | "solidcolor" => {
            "refused (LLP 1055.000 §7): Chrome does not implement it"
        }
        "animate" | "animateTransform" | "animateMotion" | "set" => {
            "SMIL is refused (LLP 1055 D12): declare `keyframes` and set `animation`"
        }
        _ => return None,
    })
}

impl Lowerer<'_> {
    /// Every `keyframes` declaration to a plan row of CSS text, validated by
    /// the evaluator that will sample it.
    pub(crate) fn declare_keyframes(&mut self, file: &File) -> Vec<LowerError> {
        let mut errors = Vec::new();
        for decl in &file.keyframes {
            if self.keyframes.contains_key(&decl.name) {
                errors.push(LowerError {
                    id: "lower-keyframes-duplicate",
                    message: format!("`keyframes {}` is declared twice", decl.name),
                    span: decl.span,
                });
                continue;
            }
            let mut css = String::new();
            let mut ok = true;
            for frame in &decl.frames {
                let selectors: Vec<String> = frame
                    .selectors
                    .iter()
                    .map(|s| format!("{}%", exact_num::Shortest(*s)))
                    .collect();
                css.push_str(&selectors.join(","));
                css.push('{');
                for a in &frame.attrs {
                    let known = a.name == "animation-timing-function"
                        || Property::from_name(&a.name).is_some_and(|p| p != Property::Height);
                    if !known {
                        ok = false;
                        errors.push(LowerError {
                            id: "lower-keyframe-property",
                            message: format!(
                                "`{}` cannot animate: keyframes take opacity, translate, scale, rotate, stroke-dashoffset, r, color, background-color, fill, stroke and animation-timing-function (LLP 1055.000 D6)",
                                a.name
                            ),
                            span: a.span,
                        });
                        continue;
                    }
                    let value = match &a.value {
                        Expr::Number(n, _) => exact_num::Shortest(*n).to_string(),
                        Expr::Str(s, _) => s.clone(),
                        _ => unreachable!("the parser admits literals only"),
                    };
                    css.push_str(&format!("{}:{value};", a.name));
                }
                css.push('}');
            }
            if !ok {
                continue;
            }
            match Keyframes::parse(&css) {
                Ok(rule) => {
                    self.b.keyframes(&decl.name, &rule.css());
                    self.keyframes.insert(decl.name.clone(), rule.properties());
                }
                Err(e) => errors.push(LowerError {
                    id: "lower-keyframes",
                    message: format!("`keyframes {}`: {e:?}", decl.name),
                    span: decl.span,
                }),
            }
        }
        errors
    }

    /// The content model, where SVG attributes go, and the `animation` names
    /// a node names, checked before its attributes lower.
    pub(crate) fn check_svg(
        &self,
        tag: &str,
        parent_tag: Option<&str>,
        attrs: &[Attr],
        span: Span,
    ) -> Result<(), LowerError> {
        let in_svg = in_svg(self.svg_depth > 0, parent_tag);
        // A nested `svg` is an SVG element (a new viewport, LLP 1055.000 D4),
        // and so is `text` inside one (D11).
        let element = is_element(tag) || (matches!(tag, "svg" | "text") && in_svg);
        if let Some(parent) = parent_tag.filter(|_| in_svg) {
            if !holds(parent, tag) {
                return err(
                    "lower-svg-content",
                    format!("`{parent}` does not hold `{tag}`"),
                    span,
                );
            }
        }
        if tag != "svg" && is_element(tag) && !in_svg {
            return err(
                "lower-svg-content",
                format!("`{tag}` is an SVG element: it goes inside an `svg` or a `g`"),
                span,
            );
        }
        if in_svg && !element {
            return err(
                "lower-svg-content",
                format!(
                    "an `svg` or `g` holds SVG elements (svg, g, path, polyline, polygon, circle, ellipse, line, rect), not `{tag}`"
                ),
                span,
            );
        }
        for a in attrs {
            // A `clipPath` by reference clips SVG elements (LLP 1055.000 D10);
            // a box's `clip-path` is a path.
            if matches!(tag, "text" | "tspan") && element && a.name == "rotate" {
                return err(
                    "lower-svg-attr",
                    "per-glyph `rotate` on SVG text is a later stage (LLP 1055.000 §4); rotate the `text` with `transform`",
                    a.span,
                );
            }
            if a.name == "clip-path"
                && !element
                && matches!(&a.value, Expr::Str(v, _) if v.trim_start().starts_with("url("))
            {
                return err(
                    "lower-attr-value",
                    "`clip-path: url(#…)` clips SVG elements; a box takes `path(\"…\")`",
                    a.span,
                );
            }
            if let Some(owners) = owners(&a.name) {
                if !owners.contains(&tag) {
                    return err(
                        "lower-attr-tag",
                        format!(
                            "`{}` belongs to {}, not `{tag}`",
                            a.name,
                            owners.join(" or ")
                        ),
                        a.span,
                    );
                }
            } else if element
                && !shared(&a.name)
                // @ref LLP 1055.000 D17 — an element that renders handles
                // events: the host hits it by `pointer-events`.
                && !(matches!(tags::attr(&a.name), Some(tags::AttrTarget::Handler("press")))
                    && !matches!(
                        tag,
                        "defs"
                            | "linearGradient"
                            | "radialGradient"
                            | "stop"
                            | "symbol"
                            | "clipPath"
                            | "marker"
                            | "mask"
                            | "pattern"
                            | "filter"
                            | "tspan"
                    ) && !FE.contains(&tag))
                && !((matches!(tag, "rect" | "svg" | "use" | "mask" | "pattern" | "foreignObject" | "filter")
                    || (FE.contains(&tag) && !tag.starts_with("feFunc") && !tag.ends_with("Light") && tag != "feMergeNode"))
                    && matches!(a.name.as_str(), "width" | "height"))
                && !(matches!(tag, "use" | "linearGradient" | "radialGradient" | "pattern")
                    && a.name == "href")
                && !(tag == "svg"
                    && matches!(a.name.as_str(), "overflow" | "overflow-x" | "overflow-y"))
                && !(FE.contains(&tag) && (is_length_prop(&a.name) || a.name == "type"))
            {
                let why = if matches!(
                    tags::attr(&a.name),
                    Some(tags::AttrTarget::Handler("press"))
                ) {
                    "a definition handles no events: it renders only where it is referenced (LLP 1055.000 D17)"
                } else if matches!(tags::attr(&a.name), Some(tags::AttrTarget::Handler(_))) {
                    "an SVG element takes `press` so far; its other handlers are a later stage (LLP 1055.000 D17)"
                } else {
                    "it does not apply to an SVG element"
                };
                return err(
                    "lower-svg-attr",
                    format!("`{}` on `{tag}`: {why}", a.name),
                    a.span,
                );
            }
            if a.name == "animation" {
                self.check_animation_names(tag, &a.value)?;
            }
        }
        Ok(())
    }

    fn check_animation_names(&self, tag: &str, value: &Expr) -> Result<(), LowerError> {
        match value {
            Expr::Str(text, span) => {
                let Ok(list) = Animations::parse(text) else {
                    return Ok(()); // the row's own check names the grammar
                };
                for a in &list.0 {
                    let Some(properties) = self.keyframes.get(&a.name) else {
                        return err(
                            "lower-animation-name",
                            format!(
                                "`animation` names `{}`, and no `keyframes {}` is declared",
                                a.name, a.name
                            ),
                            *span,
                        );
                    };
                    // An SVG element's transform animates as CSS says (LLP
                    // 1055.000 D5); `r` is a circle's alone.
                    if properties.contains(&Property::R)
                        && !matches!(tag, "circle")
                        && is_element(tag)
                    {
                        return err(
                            "lower-animation-target",
                            format!(
                                "`keyframes {}` animates `r`, which only a `circle` has",
                                a.name
                            ),
                            *span,
                        );
                    }
                }
                Ok(())
            }
            Expr::Ternary(_, yes, no, _) => {
                self.check_animation_names(tag, yes)?;
                self.check_animation_names(tag, no)
            }
            _ => Ok(()),
        }
    }

    /// `animation` and its longhands as one `animation` attribute (LLP 1055
    /// D5), or `None` when no longhand is set. A longhand overrides the
    /// shorthand's part whatever the order (a Contract rule; CSS goes by
    /// declaration order). Literal lists follow CSS: the names set the count
    /// and other lists repeat to it. A computed part makes one animation.
    pub(crate) fn compose_animation(
        &self,
        attrs: &[Attr],
    ) -> Result<Option<Vec<Attr>>, LowerError> {
        let longs: Vec<&Attr> = attrs
            .iter()
            .filter(|a| LONGHANDS.contains(&a.name.as_str()))
            .collect();
        let Some(first) = longs.first() else {
            return Ok(None);
        };
        let short = attrs.iter().find(|a| a.name == "animation");
        let literal = |e: &Expr| matches!(e, Expr::Str(..) | Expr::Number(..));
        let text = |e: &Expr| match e {
            Expr::Str(s, _) => s.clone(),
            Expr::Number(n, _) => exact_num::Shortest(*n).to_string(),
            _ => String::new(),
        };
        let bad = |span: Span, message: String| LowerError {
            id: "lower-animation-longhand",
            message,
            span,
        };
        let base: Vec<Animation> = match short {
            Some(s) if literal(&s.value) => Animations::parse(&text(&s.value))
                .map_err(|e| bad(s.span, format!("`animation`: {e:?}")))?
                .0,
            Some(s) => {
                return Err(bad(
                    s.span,
                    "a computed `animation` cannot be combined with longhands: write every part as a longhand".into(),
                ))
            }
            None => Vec::new(),
        };
        let value = if short.is_none_or(|s| literal(&s.value))
            && longs.iter().all(|a| literal(&a.value))
        {
            let mut list = base;
            if let Some(names) = longs.iter().find(|a| a.name == "animation-name") {
                let names: Vec<String> = text(&names.value)
                    .split(',')
                    .map(|n| n.trim().to_string())
                    .collect();
                list = names
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        let mut a = list.get(i).cloned().unwrap_or_default();
                        a.name = n.clone();
                        a
                    })
                    .collect();
            }
            for a in longs.iter().filter(|a| a.name != "animation-name") {
                let items: Vec<String> = text(&a.value)
                    .split(',')
                    .map(|v| v.trim().to_string())
                    .collect();
                for (i, entry) in list.iter_mut().enumerate() {
                    entry
                        .set_longhand(&a.name, &items[i % items.len()])
                        .map_err(|e| bad(a.span, format!("`{}`: {e:?}", a.name)))?;
                }
            }
            list.retain(|a| a.name != "none");
            Expr::Str(Animations(list).css(), first.span)
        } else {
            // One animation, its parts in shorthand order; a literal list
            // cannot be combined with a computed part.
            let defaults = base.into_iter().next().unwrap_or_default();
            let mut parts = Vec::new();
            for (i, name) in LONGHANDS.iter().enumerate() {
                if i > 0 {
                    parts.push(TemplatePart::Text(" ".into()));
                }
                match longs.iter().find(|a| a.name == *name) {
                    Some(a) if literal(&a.value) && text(&a.value).contains(',') => {
                        return Err(bad(a.span, format!("`{}` is a list, and another part is computed: a computed animation is one animation", a.name)))
                    }
                    Some(a) if literal(&a.value) => parts.push(TemplatePart::Text(text(&a.value))),
                    Some(a) => parts.push(TemplatePart::Expr(a.value.clone())),
                    None => parts.push(TemplatePart::Text(default_part(&defaults, i))),
                }
            }
            Expr::Template(parts, first.span)
        };
        let mut out: Vec<Attr> = attrs
            .iter()
            .filter(|a| a.name != "animation" && !LONGHANDS.contains(&a.name.as_str()))
            .cloned()
            .collect();
        out.push(Attr {
            name: "animation".into(),
            value,
            span: first.span,
        });
        Ok(Some(out))
    }
}

/// SVG length props take text (LLP 1055.000 D4): a number literal becomes
/// its text and any other expression is interpolated, so `x1=10`,
/// `x1="50%"` and `x1=gridX` all lower. `None` when nothing changes.
pub(crate) fn coerce_lengths(tag: &str, in_svg: bool, attrs: &[Attr]) -> Option<Vec<Attr>> {
    // SVG text's `x`, `y`, `dx`, `dy` are position lists, not geometry
    // rows (LLP 1055.000 D11): they lower to their own props.
    let text = tag == "tspan" || (tag == "text" && in_svg);
    // @ref LLP 1055.000 D14 — a primitive's `dx`, `dy` and `scale`, and a
    // light's `x`, `y`, `z`, are props: the plain names are rows.
    if FE.contains(&tag) {
        let light = tag.ends_with("Light");
        let renamed: Vec<Attr> = attrs
            .iter()
            .map(|a| {
                let name = match a.name.as_str() {
                    "dx" => "feDx",
                    "dy" => "feDy",
                    "scale" => "feScale",
                    "radius" => "feRadius",
                    "x" if light => "lightX",
                    "y" if light => "lightY",
                    "z" if light => "lightZ",
                    other => other,
                };
                Attr {
                    name: name.into(),
                    value: a.value.clone(),
                    span: a.span,
                }
            })
            .collect();
        let coerced = coerce_lengths("g", in_svg, &renamed);
        return Some(coerced.unwrap_or(renamed));
    }
    if text {
        let renamed: Vec<Attr> = attrs
            .iter()
            .map(|a| {
                let name = match a.name.as_str() {
                    "x" => "textX",
                    "y" => "textY",
                    "dx" => "textDx",
                    "dy" => "textDy",
                    other => other,
                };
                Attr {
                    name: name.into(),
                    value: a.value.clone(),
                    span: a.span,
                }
            })
            .collect();
        let coerced = coerce_lengths("", false, &renamed);
        return Some(coerced.unwrap_or(renamed));
    }
    let svg = is_element(tag) || tag == "svg";
    // A geometry property's presentation attribute is a unitless number
    // (`y="46"`), which CSS's grammar for the row would refuse.
    let unitless = |a: &Attr| {
        svg && matches!(
            a.name.as_str(),
            "x" | "y" | "rx" | "ry" | "cx" | "cy" | "r" | "width" | "height"
        ) && matches!(&a.value, Expr::Str(t, _) if exact_num::parse_f64(t.trim()).is_ok_and(f64::is_finite))
    };
    if !attrs
        .iter()
        .any(|a| unitless(a) || (is_length_prop(&a.name) && !matches!(a.value, Expr::Str(..))))
    {
        return None;
    }
    Some(
        attrs
            .iter()
            .map(|a| {
                if unitless(a) {
                    let Expr::Str(t, span) = &a.value else {
                        unreachable!("unitless is a string")
                    };
                    return Attr {
                        name: a.name.clone(),
                        value: Expr::Number(exact_num::parse_f64(t.trim()).unwrap_or(0.0), *span),
                        span: a.span,
                    };
                }
                if !is_length_prop(&a.name) {
                    return a.clone();
                }
                let value = match &a.value {
                    Expr::Str(..) => a.value.clone(),
                    Expr::Number(n, span) => Expr::Str(exact_num::Shortest(*n).to_string(), *span),
                    other => Expr::Template(vec![TemplatePart::Expr(other.clone())], a.span),
                };
                Attr {
                    name: a.name.clone(),
                    value,
                    span: a.span,
                }
            })
            .collect(),
    )
}

/// One longhand's text from an animation (for a composed template).
fn default_part(a: &Animation, index: usize) -> String {
    match index {
        0 => {
            if a.name.is_empty() {
                "none".into()
            } else {
                a.name.clone()
            }
        }
        1 => format!("{}s", exact_num::Shortest(a.duration)),
        2 => exact_motion::animation::easing_css(&a.easing),
        3 => format!("{}s", exact_num::Shortest(a.delay)),
        4 => {
            if a.iterations.is_infinite() {
                "infinite".into()
            } else {
                exact_num::Shortest(a.iterations).to_string()
            }
        }
        5 => a.direction.name().into(),
        6 => a.fill.name().into(),
        _ => if a.paused { "paused" } else { "running" }.into(),
    }
}
