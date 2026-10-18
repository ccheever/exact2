//! Snapshot paint values and family limits. @ref LLP 1126 D4 / 1126.000 WP2.
use super::{admits, refusal, CompileError};
use contract_syntax::{Attr, Expr, File, Node};
use exact_kernel::{
    gradient::{BackgroundImage, Gradient, GradientKind, StopUnit},
    style::{sides, wide},
    Color, ColorValue, StyleId,
};

const BORDER: &str = "use one border for all four sides, with one width, one colour and a solid, dashed, dotted or none style";
const RADIUS: &str = "write a radius in px or %, without calc() or env()";
const SLASH_RADIUS: &str = "a corner takes one radius, not `h / v`; a percentage (border-radius=\"50%\") makes an elliptical corner on a box that is not square";
const LITERAL: &str =
    "write a literal, or `cond ? A : B` of literals, so the bake can check each value";
const COLOR: &str = "use an sRGB colour, a system colour role or a named colour asset";
const GRADIENT: &str = "use a nonrepeating linear-gradient or radial-gradient in srgb or srgb-linear, with strictly increasing stops; use one stop unit when their order depends on the box";
const OVERFLOW: &str = "write the same overflow on both axes: visible, hidden or clip";

// A restricted string cannot hide an unsupported value behind a branch or helper.
// Ordinary numeric layout and text expressions still go through the type checker.
fn choices(e: &Expr, accepts: &impl Fn(&Expr) -> bool) -> bool {
    match e {
        Expr::Ternary(_, a, b, _)
        | Expr::Match {
            some: a, none: b, ..
        } => choices(a, accepts) && choices(b, accepts),
        Expr::Let { body, .. } => choices(body, accepts),
        _ => accepts(e),
    }
}
/// Whether every branch of `e` is a literal, so a refusal names the value.
fn literal(e: &Expr) -> bool {
    choices(e, &|e| {
        matches!(
            e,
            Expr::Str(..) | Expr::Number(..) | Expr::Bool(..) | Expr::None(..)
        )
    })
}
fn words(e: &Expr, allowed: &[&str]) -> bool {
    choices(
        e,
        &|e| matches!(e, Expr::Str(s, _) if allowed.contains(&s.as_str())),
    )
}
fn color_value(c: ColorValue) -> bool {
    match c {
        ColorValue::Wide(id) => wide::wide(id).is_some_and(|w| {
            [Some(w.light), w.dark]
                .into_iter()
                .flatten()
                .all(|c| matches!(c.space.name(), "srgb" | "srgb-linear") && c.in_srgb())
        }),
        ColorValue::Profiled(_) | ColorValue::Moving(..) => false,
        _ => true,
    }
}
fn color(s: &str) -> bool {
    s.eq_ignore_ascii_case("currentcolor")
        || ColorValue::parse_light_dark(s)
            .or_else(|| Color::parse(s).map(ColorValue::Fixed))
            .is_some_and(color_value)
}
fn radius(e: &Expr) -> bool {
    match e {
        Expr::Number(n, _) => n.is_finite() && *n >= 0.0,
        Expr::Str(s, _) => s
            .strip_suffix("px")
            .or_else(|| s.strip_suffix('%'))
            .unwrap_or(s)
            .parse::<f32>()
            .is_ok_and(|v| v.is_finite() && v >= 0.0),
        _ => false,
    }
}
fn increasing(g: &Gradient) -> bool {
    // Each stop is a * line-length + b. Prove strict order for every positive
    // box size, including omitted stops; mixed units may cross when resized.
    let last = g.stops.len() - 1;
    let mut positions: Vec<Option<(f64, f64)>> = g
        .stops
        .iter()
        .enumerate()
        .map(|(i, s)| match s.unit {
            StopUnit::Percent => Some((f64::from(s.at) / 100.0, 0.0)),
            StopUnit::Px => Some((0.0, f64::from(s.at))),
            StopUnit::Auto if i == 0 => Some((0.0, 0.0)),
            StopUnit::Auto if i == last => Some((1.0, 0.0)),
            StopUnit::Auto => None,
        })
        .collect();
    let mut from = 0;
    for to in 1..positions.len() {
        if let Some(b) = positions[to] {
            let a = positions[from].unwrap();
            for (i, p) in positions.iter_mut().enumerate().take(to).skip(from + 1) {
                let t = (i - from) as f64 / (to - from) as f64;
                *p = Some((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
            }
            from = to;
        }
    }
    positions.windows(2).all(|p| {
        let (a, b) = (p[0].unwrap(), p[1].unwrap());
        b.0 >= a.0 && b.1 >= a.1 && (b.0 > a.0 || b.1 > a.1)
    })
}
fn gradient(s: &str) -> bool {
    BackgroundImage::check(s).is_ok_and(|image| {
        image.layers().iter().all(|g| {
            !g.repeating
                && !matches!(g.kind, GradientKind::Conic { .. })
                && increasing(g)
                && g.stops.iter().all(|s| color_value(s.color))
                && [false, true].into_iter().all(|dark| {
                    g.interpolation(dark)
                        .is_none_or(|i| matches!(i.css().as_str(), "in srgb" | "in srgb-linear"))
                })
        })
    })
}

pub(super) fn attribute(a: &Attr, errors: &mut Vec<CompileError>) {
    let fail = |id, edit: &str| refusal(id, format!("`{}`: {edit}", a.name), a.span);
    let border = a.name == "border" || a.name.starts_with("border-") && !a.name.ends_with("radius");
    if border
        && !matches!(
            a.name.as_str(),
            "border" | "border-width" | "border-color" | "border-style"
        )
    {
        errors.push(fail("snapshot-border", BORDER));
        return;
    }
    let rows = match contract_lower::style_values(&a.name, &a.value) {
        Ok(rows) => rows,
        Err(_) if border => {
            errors.push(fail("snapshot-border", BORDER));
            return;
        }
        Err(_) if a.name.ends_with("radius") => {
            let slash = matches!(&a.value, Expr::Str(s, _) if s.contains('/'));
            errors.push(fail(
                "snapshot-radius",
                if slash { SLASH_RADIUS } else { RADIUS },
            ));
            return;
        }
        Err(_) => return, // The ordinary compiler reports its grammar error.
    };
    if border
        && a.name != "border"
        && !choices(&a.value, &|e| match e {
            Expr::Str(s, _) => {
                let v = sides::values(s);
                v.iter().all(|p| Some(p) == v.first())
            }
            Expr::Number(_, _) => true,
            _ => false,
        })
    {
        errors.push(fail("snapshot-border", BORDER));
    }
    for (row, value) in rows {
        use StyleId::*;
        if !admits(row) {
            let edit = match row {
                Visibility => "use display: none or a condition",
                ZIndex => "use document order to set the paint order",
                ColorScheme => "remove color-scheme; the widget follows the system's colour scheme",
                _ => "remove this row from the entry; the snapshot driver cannot draw it",
            };
            errors.push(fail("snapshot-row", edit));
            continue;
        }
        let rule = match row {
            PositionType => Some((words(&value, &["static", "relative", "absolute"]), "snapshot-value", "a snapshot cannot scroll or stick; write static, relative or absolute as literal choices")),
            OverflowX | OverflowY => Some((words(&value, &["visible", "hidden", "clip"]), "snapshot-value", "a snapshot cannot scroll; write visible, hidden or clip on both axes")),
            TextAlign => Some((words(&value, &["start", "center", "end"]), "snapshot-text-align", "use text-align: start, center or end")),
            // The driver decodes a bounded thumbnail, which cannot show an image at its own scale.
            ObjectFit => Some((words(&value, &["fill", "contain", "cover", "scale-down"]), "snapshot-object-fit", "use object-fit: fill, contain, cover or scale-down; a widget draws a bounded thumbnail, never the image at its own scale")),
            TextOverflow => Some((words(&value, &["clip"]), "snapshot-text-overflow", "use line-clamp instead of text-overflow: ellipsis")),
            BorderStyleTop | BorderStyleRight | BorderStyleBottom | BorderStyleLeft => Some((words(&value, &["solid", "dashed", "dotted", "none", "hidden"]), "snapshot-border", BORDER)),
            BorderRadiusTopLeft | BorderRadiusTopRight | BorderRadiusBottomLeft | BorderRadiusBottomRight => Some((choices(&value, &radius), "snapshot-radius", RADIUS)),
            BackgroundColor | TextColor | BorderColorTop | BorderColorRight | BorderColorBottom | BorderColorLeft => Some((choices(&value, &|e| matches!(e, Expr::Str(s, _) if color(s))), "snapshot-color", COLOR)),
            BackgroundImage => Some((choices(&value, &|e| matches!(e, Expr::Str(s, _) if gradient(s))), "snapshot-gradient", GRADIENT)),
            _ => None,
        };
        if let Some((false, id, edit)) = rule {
            // A computed value cannot be checked at bake, whatever it holds.
            if literal(&value) {
                errors.push(fail(id, edit));
            } else {
                errors.push(fail("snapshot-literal", LITERAL));
            }
        }
    }
}

/// Each named class alternative, with the node's own attributes winning as in
/// lowering. Cross-row checks must see a complete alternative, not their union.
pub(super) fn variants<'a>(file: &'a File, attrs: &'a [Attr]) -> Vec<Vec<&'a Attr>> {
    let class = attrs.iter().find(|a| a.name == "class");
    let names = match class.map(|a| &a.value) {
        Some(Expr::Ident(n, _)) => vec![n],
        Some(Expr::Ternary(_, a, b, _)) => [a, b]
            .into_iter()
            .filter_map(|e| match &**e {
                Expr::Ident(n, _) => Some(n),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    if names.is_empty() {
        return vec![attrs.iter().collect()];
    }
    names
        .into_iter()
        .map(|name| {
            file.styles
                .iter()
                .filter(|s| &s.name == name)
                .flat_map(|s| &s.attrs)
                .filter(|a| !attrs.iter().any(|own| own.name == a.name))
                .chain(attrs)
                .collect()
        })
        .collect()
}

pub(super) fn tree(
    nodes: &[Node],
    file: &File,
    families: &[String],
    errors: &mut Vec<CompileError>,
) {
    super::visit_nodes(nodes, &mut |n| {
        let Node::Element {
            tag,
            attrs,
            positional,
            span,
            ..
        } = n
        else {
            return;
        };
        for attrs in variants(file, attrs) {
            let mut overflow = [None, None];
            for a in &attrs {
                if matches!(a.name.as_str(), "overflow" | "overflow-x" | "overflow-y") {
                    if let Ok(rows) = contract_lower::style_values(&a.name, &a.value) {
                        for (row, value) in rows {
                            overflow[usize::from(row == StyleId::OverflowY)] = Some(value);
                        }
                    }
                }
            }
            if !same_overflow(overflow[0].as_ref(), overflow[1].as_ref()) {
                errors.push(refusal("snapshot-overflow", OVERFLOW, *span));
            }
            if tag == "image" {
                let src = attrs
                    .iter()
                    .find(|a| a.name == "src")
                    .map(|a| &a.value)
                    .or(positional.first());
                let symbol = src.is_some_and(|e| {
                    choices(
                        e,
                        &|e| matches!(e, Expr::Str(s,_) if s.starts_with("symbol:")),
                    )
                });
                if !symbol
                    && !["width", "height"].iter().all(|name| {
                        attrs.iter().any(|a| {
                            a.name == *name
                                && choices(&a.value, &|e| {
                                    !matches!(e, Expr::None(_))
                                        && !matches!(e, Expr::Str(s,_) if s == "auto")
                                })
                        })
                    })
                {
                    errors.push(refusal("snapshot-image-size", "author both width and height on the raster image, so the snapshot can decode a bounded thumbnail", *span));
                }
            }
        }
    });
    if families.iter().any(|f| {
        !matches!(
            f.as_str(),
            "systemMedium" | "systemLarge" | "systemExtraLarge"
        )
    }) && links(nodes) > 1
    {
        let span = nodes.first().map(Node::span).unwrap_or_default();
        errors.push(refusal("snapshot-links", "keep at most one link when declaring a small or accessory family, or declare only medium, large and extra large families", span));
    }
}
fn same_overflow(a: Option<&Expr>, b: Option<&Expr>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Expr::Str(s, _)), None) | (None, Some(Expr::Str(s, _))) => s == "visible",
        (Some(Expr::Str(a, _)), Some(Expr::Str(b, _))) => a == b,
        // A shorthand assigns both axes the identical expression. Comparing
        // spans here deliberately refuses separately authored conditional axes.
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
fn links(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Element {
                attrs, children, ..
            } => usize::from(attrs.iter().any(|a| a.name == "href")) + links(children),
            Node::When {
                then, otherwise, ..
            } => links(then).max(links(otherwise)),
            Node::Match { some, none, .. } => links(&some.1).max(links(none)),
            Node::Each { body, list, .. } => {
                links(body)
                    * match list {
                        Expr::List(v, _) => v.len().min(2),
                        _ => 2,
                    }
            }
            _ => 0,
        })
        .sum::<usize>()
        .min(2)
}
