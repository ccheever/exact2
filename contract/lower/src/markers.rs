//! `marker` declarations and the `marker-*` attributes that name them.
//!
//! @ref LLP 1065 D11
//!
//! A declaration resolves once, here, to the kernel's [`Marker`]: SVG's
//! `<marker>` attributes and its paths' painting, each value checked as SVG
//! reads it. A `marker-start`, `marker-mid`, `marker-end` or `marker`
//! value's every literal — `url(#name)` or `none` — is then rewritten to the
//! row's own text, the marker whole, as an `animation` literal carries its
//! keyframes (LLP 1057 D3): the plan, the runner and every host carry the
//! marker, never a name to look up. A name nobody declared is refused here.

use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, File, MarkerDecl};
use exact_kernel::vector::{
    parse_view_box, Marker, MarkerDef, MarkerShape, Orient, PathData, PreserveAspectRatio,
    ShapePaint,
};
use exact_kernel::{FillRule, StrokeLinecap, StrokeLinejoin};
use std::collections::BTreeMap;

/// The file's markers, by name, as their row text.
#[derive(Debug, Default)]
pub(crate) struct Table(BTreeMap<String, String>);

/// Resolve every `marker` declaration. A refused one is left out and
/// reported; the rest still resolve.
pub(crate) fn resolve(file: &File) -> (Table, Vec<LowerError>) {
    let mut table = Table::default();
    let mut errors = Vec::new();
    for decl in &file.markers {
        match marker(decl) {
            Ok(text) => {
                table.0.insert(decl.name.clone(), text);
            }
            Err(e) => errors.push(e),
        }
    }
    (table, errors)
}

fn text(a: &Attr) -> Result<&str, LowerError> {
    match &a.value {
        Expr::Str(s, _) => Ok(s),
        _ => err(
            "lower-marker",
            format!("`{}` in a marker is text in quotes", a.name),
            a.span,
        ),
    }
}

fn number(a: &Attr, min: f32) -> Result<f32, LowerError> {
    match crate::values::numeric_literal(&a.value) {
        Some(n) if n.is_finite() && n as f32 >= min => Ok(n as f32),
        _ => err(
            "lower-marker",
            format!("`{}` in a marker is a number of at least {min}", a.name),
            a.span,
        ),
    }
}

fn bad<T>(a: &Attr, expected: &str) -> Result<T, LowerError> {
    err(
        "lower-marker",
        format!("`{}` in a marker is {expected}", a.name),
        a.span,
    )
}

fn marker(decl: &MarkerDecl) -> Result<String, LowerError> {
    let mut def = MarkerDef {
        name: decl.name.clone(),
        ..MarkerDef::default()
    };
    for a in &decl.attrs {
        match a.name.as_str() {
            "viewBox" => {
                def.view_box = Some(parse_view_box(text(a)?).map_or_else(
                    || bad(a, "`min-x min-y width height` with a positive width and height"),
                    Ok,
                )?)
            }
            "preserveAspectRatio" => {
                def.aspect = PreserveAspectRatio::parse(text(a)?)
                    .map_or_else(|| bad(a, "`none` or an alignment like `xMidYMid`, then `meet` or `slice`"), Ok)?
            }
            "refX" => def.reference[0] = number(a, f32::MIN)?,
            "refY" => def.reference[1] = number(a, f32::MIN)?,
            "markerWidth" => def.size[0] = number(a, 0.0)?,
            "markerHeight" => def.size[1] = number(a, 0.0)?,
            "markerUnits" => {
                def.stroke_units = match text(a)? {
                    "strokeWidth" => true,
                    "userSpaceOnUse" => false,
                    _ => return bad(a, "`strokeWidth` or `userSpaceOnUse`"),
                }
            }
            "orient" => {
                def.orient = match &a.value {
                    Expr::Str(s, _) if s == "auto" => Orient::Auto,
                    Expr::Str(s, _) if s == "auto-start-reverse" => Orient::AutoStartReverse,
                    Expr::Str(s, _) => match s.strip_suffix("deg").unwrap_or(s).trim().parse::<f32>() {
                        Ok(n) if n.is_finite() => Orient::Angle(n),
                        _ => return bad(a, "`auto`, `auto-start-reverse`, or an angle"),
                    },
                    _ => Orient::Angle(number(a, f32::MIN)?),
                }
            }
            other => {
                return err(
                    "lower-marker",
                    format!("a marker has no attribute `{other}`: it takes `viewBox`, `preserveAspectRatio`, `refX`, `refY`, `markerWidth`, `markerHeight`, `markerUnits` and `orient`, and `path` lines"),
                    a.span,
                )
            }
        }
    }
    for path in &decl.paths {
        let mut shape = MarkerShape::default();
        let mut drawn = false;
        for a in &path.attrs {
            let paint = |a: &Attr| {
                ShapePaint::parse(text(a)?).map_or_else(
                    || {
                        bad(
                            a,
                            "`none`, `currentcolor`, `context-fill`, `context-stroke`, or a colour",
                        )
                    },
                    Ok,
                )
            };
            match a.name.as_str() {
                "d" => {
                    let d = text(a)?;
                    shape.data = PathData::parse(d);
                    if let Some(e) = shape.data.error() {
                        return err(
                            "lower-attr-value",
                            format!("`d` is not SVG path data from byte {}", e.at),
                            a.span,
                        );
                    }
                    drawn = true;
                }
                "fill" => shape.fill = paint(a)?,
                "stroke" => shape.stroke = paint(a)?,
                "stroke-width" => shape.width = number(a, 0.0)?,
                "stroke-miterlimit" => shape.miter = number(a, 1.0)?,
                "stroke-linecap" => {
                    shape.cap = StrokeLinecap::from_name(text(a)?)
                        .map_or_else(|| bad(a, "`butt`, `round` or `square`"), Ok)?
                }
                "stroke-linejoin" => {
                    shape.join = StrokeLinejoin::from_name(text(a)?)
                        .map_or_else(|| bad(a, "`miter`, `round` or `bevel`"), Ok)?
                }
                "fill-rule" => {
                    shape.rule = FillRule::from_name(text(a)?)
                        .map_or_else(|| bad(a, "`nonzero` or `evenodd`"), Ok)?
                }
                other => {
                    return err(
                        "lower-marker",
                        format!("a marker's `path` has no attribute `{other}`: it takes `d`, `fill`, `stroke`, `stroke-width`, `stroke-linecap`, `stroke-linejoin`, `stroke-miterlimit` and `fill-rule`"),
                        a.span,
                    )
                }
            }
        }
        if !drawn {
            return err("lower-marker", "a marker's `path` needs `d`", path.span);
        }
        def.shapes.push(shape);
    }
    let text = Marker(Some(Box::new(def))).css();
    debug_assert!(Marker::parse(&text).is_some(), "{text}");
    Ok(text)
}

/// A `marker-*` value with every literal resolved to the row's text: `none`,
/// or `url(#name)` naming a declared marker. A computed value is refused; a
/// condition may still choose between literals.
pub(crate) fn marker_value(value: &Expr, table: &Table, attr: &str) -> Result<Expr, LowerError> {
    Ok(match value {
        Expr::Str(text, span) => {
            let text = text.trim();
            if text == "none" {
                return Ok(Expr::Str("none".into(), *span));
            }
            let name = text
                .strip_prefix("url(")
                .and_then(|t| t.strip_suffix(')'))
                .map(|t| t.trim().trim_matches(['"', '\'']))
                .and_then(|t| t.strip_prefix('#'));
            let Some(name) = name else {
                return err(
                    "lower-attr-value",
                    format!("`{attr}=\"{text}\"` is `none` or `url(#name)` naming a `marker`"),
                    *span,
                );
            };
            match table.0.get(name) {
                Some(row) => Expr::Str(row.clone(), *span),
                None => {
                    let known: Vec<&str> = table.0.keys().map(String::as_str).collect();
                    return err(
                        "lower-marker-unknown",
                        format!(
                            "`{attr}` names `{name}`, which no `marker` declares{}",
                            if known.is_empty() {
                                String::new()
                            } else {
                                format!(" (declared: {})", known.join(", "))
                            }
                        ),
                        *span,
                    );
                }
            }
        }
        Expr::None(_) => value.clone(),
        Expr::Ternary(c, yes, no, span) => Expr::Ternary(
            c.clone(),
            Box::new(marker_value(yes, table, attr)?),
            Box::new(marker_value(no, table, attr)?),
            *span,
        ),
        other => {
            return err(
                "lower-marker-literal",
                format!("`{attr}` is `none` or `url(#name)`, or a condition choosing between them: its marker is resolved when the app compiles"),
                other.span(),
            )
        }
    })
}
