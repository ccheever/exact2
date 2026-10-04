//! Attribute values checked against what they set, at compile time: a
//! literal style value by the kernel's own row parser, a computed one and a
//! prop by type (LLP 1017 P1a).

use crate::{err, media, tags, FontUse, LowerError};
use contract_syntax::{Attr, Expr, Span, UnOp};
use contract_types::Ty;
use exact_kernel::{PropId, StyleId, StyleProps, StyleValue, StyleValueError};

/// A literal, as an author wrote it, for a message.
fn literal_text(e: &Expr) -> String {
    match e {
        Expr::Number(n, _) => format!("{n}"),
        Expr::Str(s, _) => format!("\"{s}\""),
        Expr::Bool(b, _) => format!("{b}"),
        _ => "…".into(),
    }
}

/// `glassGroup`'s value to compile (LLP 1053.000.000 D1, 1053.000.000.000
/// D1): each literal spacing, at the top and in every arm of a choice, is 0 to
/// 10,000 points; `"auto"` is rewritten to the reserved `-1`; any other string
/// literal is refused.
pub(crate) fn glass_group(e: &Expr) -> Result<Expr, LowerError> {
    match e {
        Expr::Str(s, span) if s == "auto" => Ok(Expr::Number(-1.0, *span)),
        Expr::Str(s, span) => err(
            "lower-attr-value",
            format!("`glassGroup` takes a spacing in points or `\"auto\"`; given `\"{s}\"`"),
            *span,
        ),
        Expr::Ternary(c, a, b, span) => Ok(Expr::Ternary(
            c.clone(),
            Box::new(glass_group(a)?),
            Box::new(glass_group(b)?),
            *span,
        )),
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => Ok(Expr::Match {
            subject: subject.clone(),
            var: var.clone(),
            some: Box::new(glass_group(some)?),
            none: Box::new(glass_group(none)?),
            span: *span,
        }),
        // A shared derive's `let`: its value rewritten only when it is a
        // spacing as written (a condition it binds is not), its body always.
        Expr::Let {
            name,
            value,
            body,
            span,
        } => Ok(Expr::Let {
            name: name.clone(),
            value: Box::new(if spacing_tree(value) {
                glass_group(value)?
            } else {
                (**value).clone()
            }),
            body: Box::new(glass_group(body)?),
            span: *span,
        }),
        _ => {
            if let Some(spacing) = numeric_literal(e) {
                if !(0.0..=10_000.0).contains(&spacing) {
                    return err(
                        "lower-attr-value",
                        format!("`glassGroup` takes a spacing from 0 to 10000 points, or `\"auto\"`; given {spacing}"),
                        e.span(),
                    );
                }
            }
            Ok(e.clone())
        }
    }
}

/// Whether a value is a spacing as written: `"auto"`, a number, or a choice
/// of them.
fn spacing_tree(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s == "auto",
        Expr::Ternary(_, a, b, _) => spacing_tree(a) && spacing_tree(b),
        Expr::Match { some, none, .. } => spacing_tree(some) && spacing_tree(none),
        _ => numeric_literal(e).is_some(),
    }
}

pub(crate) fn numeric_literal(e: &Expr) -> Option<f64> {
    match e {
        Expr::Number(n, _) => Some(*n),
        Expr::Unary(UnOp::Neg, inner, _) => numeric_literal(inner).map(|n| -n),
        _ => None,
    }
}

fn whole_i64(n: f64) -> bool {
    n.is_finite() && n.fract() == 0.0 && n >= i64::MIN as f64 && n < -(i64::MIN as f64)
}

/// A CSS-text row's refusal with the kernel's own reason (LLP 1077).
fn named(e: &StyleValueError, v: &exact_kernel::StyleValue) -> Option<&'static str> {
    let exact_kernel::StyleValue::Text(t) = v else {
        return None;
    };
    match e {
        StyleValueError::BadTextStroke { reason, .. } => (*reason).into(),
        StyleValueError::BadBoxShadow { .. } => exact_kernel::style::BoxShadows::check(t).err(),
        StyleValueError::BadTextShadow { .. } => exact_kernel::style::TextShadow::check(t).err(),
        StyleValueError::BadCornerShape { .. } => exact_kernel::corner::CornerShape::check(t).err(),
        StyleValueError::BadMaskImage { .. } => {
            exact_kernel::gradient::BackgroundImage::check_mask(t).err()
        }
        _ => None,
    }
}

/// The kernel's refusal of a style value, in an author's words.
pub(crate) fn describe(e: &StyleValueError) -> String {
    match e {
        StyleValueError::WrongKind { expected, .. } => format!("expected {expected}"),
        StyleValueError::BadEnv { refusal, .. } => refusal.reason().into(),
        StyleValueError::UnknownEnumValue { style } => format!(
            "expected one of {}",
            style.enum_names().iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ")
        ),
        StyleValueError::AutoNotAdmitted { .. } => "`auto` is not admitted here".into(),
        StyleValueError::OutOfRange { .. } => "out of the row's range".into(),
        StyleValueError::BadColor { .. } => "a color is a CSS colour: hex, `rgb()`, `hsl()`, `hwb()`, `lab()`, `oklch()`, a named colour, `transparent`, or `light-dark(a, b)` of two".into(),
        StyleValueError::BadShapeOutside { .. } => "expected none, circle(), ellipse(), inset() with one round radius, or polygon() with at most 64 vertices; lengths are points/px or percentages".into(),
        StyleValueError::BadClipPath { .. } => "expected none or path() with explicit absolute M/L/Q/C/Z commands and separated finite coordinates".into(),
        StyleValueError::BadAspectRatio { .. } => "expected auto, a ratio (`16 / 9`, or a number), or both (`auto 4 / 3`); numbers are nonnegative".into(),
        StyleValueError::BadBackgroundImage { .. } => "expected none, or up to four of linear-gradient(…), radial-gradient(…) and conic-gradient(…)".into(),
        StyleValueError::BadMaskImage { .. } => "expected none, or one linear-gradient(…), radial-gradient(…) or conic-gradient(…)".into(),
        StyleValueError::BadTextShadow { .. } => "expected none, or one shadow: <offset-x> <offset-y> [<blur>] and an optional colour".into(),
        StyleValueError::BadCornerShape { .. } => "expected one to four of round, squircle, square, bevel, scoop, notch, superellipse(<number>) or -apple-continuous".into(),
        StyleValueError::BadDragTimeline { .. } => "expected none, or a `--name` and an optional axis (`x` or `y`)".into(),
        StyleValueError::BadAnimationTimeline { .. } => "expected auto or a `--name`".into(),
        StyleValueError::BadAnimationRange { .. } => "expected normal, or two distinct lengths (`0px 300px`)".into(),
        StyleValueError::BadTimelineScope { .. } => "expected none, all, or `--name`s separated by commas".into(),
        StyleValueError::BadTransition { .. } => "unsupported transition property or invalid timing components; exact2 supports transform components, opacity, paint and SVG properties, and admitted numeric height transitions; general layout interpolation is not implemented".into(),
        StyleValueError::BadPaint { .. } => "SVG paint is `none`, `currentcolor`, or a colour (hex, `rgb()`, `hsl()`, `hwb()`, a named colour, `light-dark()`); paint servers (`url(#…)`) are refused (LLP 1055 D12)".into(),
        StyleValueError::BadDashArray { .. } => "`stroke-dasharray` is `none` or non-negative numbers separated by spaces or commas".into(),
        StyleValueError::BadTransform { .. } => "`transform` is `none` or transform functions: matrix, translate, translateX/Y, scale, scaleX/Y, rotate (with SVG's optional centre), skew, skewX/Y; lengths in user units or px, angles in deg, rad, grad or turn".into(),
        StyleValueError::BadMarker { .. } => "a marker or a mask is `none` or `url(#id)`, naming a `marker` or a `mask`".into(),
        StyleValueError::BadFilter { .. } => "`filter` is `none`, or `url(#id)` naming a `filter` and the filter functions (blur, brightness, contrast, drop-shadow, grayscale, hue-rotate, invert, opacity, saturate, sepia), in order".into(),
        StyleValueError::BadPaintOrder { .. } => "`paint-order` is `normal`, or `fill`, `stroke` and `markers` in the order they paint".into(),
        StyleValueError::BadTransformOrigin { .. } => "`transform-origin` is one or two of left, center, right, top, bottom, a length or a percentage".into(),
        StyleValueError::BadAnimation { .. } => "not a CSS `animation` shorthand: `<name> <duration> [<easing>] [<delay>] [<count>|infinite] [<direction>] [<fill-mode>] [<play-state>]`".into(),
        StyleValueError::Unsupported { .. } => "this row has no dynamic form".into(),
        StyleValueError::BadGridTracks { .. } => "expected at most 10,000 CSS grid tracks the kernel can lay out: px, %, fr, auto, min-content, max-content, fit-content(), minmax(), repeat() (including auto-fill/auto-fit), and named lines".into(),
        StyleValueError::BadGridPlacement { .. } => "expected `auto`, a nonzero or named line through 10,000, or `span N` through 10,000, optionally followed by `/` and a second line".into(),
        StyleValueError::BadTextStroke { reason, .. } => (*reason).into(),
        StyleValueError::BadSymbolPalette { .. } => "expected none, or one to three colours".into(),
        StyleValueError::BadRotateAxis { .. } => "expected an angle, and optionally an axis: `x`, `y`, `z` or three numbers".into(),
        StyleValueError::BadBoxShadow { .. } => "expected none, or shadows separated by commas: `inset? <x> <y> [<blur> [<spread>]] <colour>`".into(),
        StyleValueError::BadBackdropFilter { reason, .. } => (*reason).into(),
    }
}

/// The rows CSS's `border-color` shorthand sets: top, right, bottom, left.
/// CSS's one-to-four-value box shorthands, rows in top, right, bottom, left order.
const FOUR_SIDED: [[StyleId; 4]; 6] = [
    [
        StyleId::PaddingTop,
        StyleId::PaddingRight,
        StyleId::PaddingBottom,
        StyleId::PaddingLeft,
    ],
    [
        StyleId::MarginTop,
        StyleId::MarginRight,
        StyleId::MarginBottom,
        StyleId::MarginLeft,
    ],
    [
        StyleId::BorderWidthTop,
        StyleId::BorderWidthRight,
        StyleId::BorderWidthBottom,
        StyleId::BorderWidthLeft,
    ],
    [
        StyleId::BorderStyleTop,
        StyleId::BorderStyleRight,
        StyleId::BorderStyleBottom,
        StyleId::BorderStyleLeft,
    ],
    [
        StyleId::BorderColorTop,
        StyleId::BorderColorRight,
        StyleId::BorderColorBottom,
        StyleId::BorderColorLeft,
    ],
    [StyleId::Top, StyleId::Right, StyleId::Bottom, StyleId::Left],
];

/// Whether these rows are one of CSS's `<value>{1,4}` box shorthands.
pub(crate) fn four_sided(rows: &[StyleId]) -> bool {
    FOUR_SIDED.iter().any(|sides| rows == sides)
}

/// A box shorthand's one to four values, split where CSS splits them: at
/// white space outside parentheses, so `light-dark(#fff, #000)` and
/// `calc(50% - 4px)` are one value.
fn side_values(text: &str) -> Vec<&str> {
    let (mut values, mut depth, mut start) = (Vec::new(), 0usize, None);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    values.push(&text[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        values.push(&text[s..]);
    }
    values
}

/// A box shorthand (`padding: <length>{1,4}`, `border-color: <color>{1,4}`,
/// …) as four longhand expressions (top, right, bottom, left): every literal
/// the value can produce — a string, or an arm of a conditional — is split
/// into its sides, and a computed leaf is one value for all four. `None` when
/// no literal names more than one value, so the shorthand stays one binding
/// for four rows.
pub(crate) fn sides(name: &str, value: &Expr) -> Result<Option<[Expr; 4]>, LowerError> {
    fn widest(name: &str, e: &Expr) -> Result<usize, LowerError> {
        Ok(match e {
            Expr::Str(s, span) => {
                let n = side_values(s).len();
                if n > 4 {
                    return err(
                        "lower-attr-value",
                        format!("`{name}` takes one to four values (top, right, bottom, left); \"{s}\" has {n}"),
                        *span,
                    );
                }
                n
            }
            Expr::Ternary(_, yes, no, _) => widest(name, yes)?.max(widest(name, no)?),
            Expr::Match { some, none, .. } => widest(name, some)?.max(widest(name, none)?),
            Expr::Let { body, .. } => widest(name, body)?,
            _ => 1,
        })
    }
    fn side(e: &Expr, i: usize) -> Expr {
        match e {
            Expr::Str(s, span) => {
                let v = side_values(s);
                // CSS: top; right = top; bottom = top; left = right.
                let pick = match (v.len(), i) {
                    (0, _) => return e.clone(),
                    (1, _) => 0,
                    (2, _) => i % 2,
                    (3, 3) => 1,
                    (_, i) => i,
                };
                // A length in pixels is the number it would be on its own
                // (`border-width` takes only numbers).
                let piece = v[pick];
                match piece.strip_suffix("px").unwrap_or(piece).parse::<f64>() {
                    Ok(n) if n.is_finite() => Expr::Number(n, *span),
                    _ => Expr::Str(piece.to_string(), *span),
                }
            }
            Expr::Ternary(c, yes, no, span) => Expr::Ternary(
                c.clone(),
                Box::new(side(yes, i)),
                Box::new(side(no, i)),
                *span,
            ),
            Expr::Match {
                subject,
                var,
                some,
                none,
                span,
            } => Expr::Match {
                subject: subject.clone(),
                var: var.clone(),
                some: Box::new(side(some, i)),
                none: Box::new(side(none, i)),
                span: *span,
            },
            Expr::Let {
                name,
                value,
                body,
                span,
            } => Expr::Let {
                name: name.clone(),
                value: value.clone(),
                body: Box::new(side(body, i)),
                span: *span,
            },
            other => other.clone(),
        }
    }
    if widest(name, value)? < 2 {
        return Ok(None);
    }
    Ok(Some([0, 1, 2, 3].map(|i| side(value, i))))
}

/// A literal style value is checked now by the kernel's own parser
/// (`StyleProps::set_dynamic`), so `width=true` and `align-items="middle"`
/// are refused at compile time, not at the first frame; a computed value
/// is checked by type — a number or a string (LLP 1017 P1a).
/// A colour chosen by branching on the system's scheme (LLP 1069.000 D1,
/// amending LLP 1034 D3): refused, naming the `light-dark()` pair that
/// does the same as a host repaint instead of a recommit on every
/// appearance change — and that follows the app's own `setScheme`, which
/// `prefersColorScheme` does not.
fn scheme_colour(a: &Attr, rows: &[StyleId]) -> Result<(), LowerError> {
    use exact_kernel::StyleCodec;
    fn reads_scheme(e: &Expr) -> bool {
        match e {
            Expr::Member(_, field, _) if field == "prefersColorScheme" => true,
            Expr::Member(inner, _, _) | Expr::Unary(_, inner, _) | Expr::Some(inner, _) => {
                reads_scheme(inner)
            }
            Expr::Binary(_, l, r, _) => reads_scheme(l) || reads_scheme(r),
            Expr::Ternary(c, y, n, _) => reads_scheme(c) || reads_scheme(y) || reads_scheme(n),
            Expr::Call(_, args, _) => args.iter().any(reads_scheme),
            _ => false,
        }
    }
    let colour = rows.iter().any(|row| {
        matches!(
            row.codec(),
            StyleCodec::ColorValue
                | StyleCodec::KeywordColor
                | StyleCodec::Rgba8
                | StyleCodec::Color2
                | StyleCodec::Paint
        )
    });
    let Expr::Ternary(cond, yes, no, span) = &a.value else {
        return Ok(());
    };
    if !colour || !reads_scheme(cond) {
        return Ok(());
    }
    let pair = match (yes.as_ref(), no.as_ref()) {
        (Expr::Str(y, _), Expr::Str(n, _)) => {
            let (dark, light) = match cond.as_ref() {
                Expr::Binary(contract_syntax::BinOp::Eq, _, r, _) if matches!(r.as_ref(), Expr::Str(v, _) if v == "light") => {
                    (n, y)
                }
                Expr::Binary(contract_syntax::BinOp::Ne, _, r, _) if matches!(r.as_ref(), Expr::Str(v, _) if v == "dark") => {
                    (n, y)
                }
                _ => (y, n),
            };
            format!("`{}=\"light-dark({light}, {dark})\"`", a.name)
        }
        _ => format!("`{}=\"light-dark(<light>, <dark>)\"`", a.name),
    };
    err(
        "lower-scheme-color",
        format!(
            "`{}` is chosen by `prefersColorScheme`, which costs a recommit on every appearance change and ignores the app's `setScheme`; write {pair}, which the host resolves (LLP 1034 D3)",
            a.name
        ),
        *span,
    )
}

pub(crate) fn check_style_value(
    a: &Attr,
    rows: &[StyleId],
    ty: &Ty,
    fonts: &[FontUse],
) -> Result<(), LowerError> {
    if rows
        .iter()
        .any(|r| matches!(r, StyleId::Resize | StyleId::UserSelect))
    {
        super::shorthands::portable_literal(&a.value, &a.name)?;
    }
    if four_sided(rows) {
        if let Some(sides) = sides(&a.name, &a.value)? {
            for (row, value) in rows.iter().zip(sides) {
                let side = Attr { value, ..a.clone() };
                check_style_value(&side, std::slice::from_ref(row), ty, fonts)?;
            }
            return Ok(());
        }
    }
    scheme_colour(a, rows)?;
    // Validate every authored literal result, including inactive branches.
    // Only the whole expression is type-checked here: match arms bind their
    // own local names, which the type pass resolves in the proper scope.
    let mut pending: Vec<(&Expr, Span)> = Vec::new();
    let mut current = (&a.value, a.span);
    loop {
        let (value, span) = current;
        match value {
            Expr::Ternary(_, yes, no, _) => {
                pending.push((no, no.span()));
                pending.push((yes, yes.span()));
            }
            Expr::Match { some, none, .. } => {
                pending.push((none, none.span()));
                pending.push((some, some.span()));
            }
            Expr::Let { body, .. } => pending.push((body, body.span())),
            _ => {}
        }
        // A computed gradient is parsed where it is painted: a native host
        // drops one its parse refuses while a browser paints it (studio
        // diary R15). The functions a template's own text already names are
        // refused here, on every target, as a literal's are.
        if let Expr::Template(parts, _) = value {
            if rows.contains(&StyleId::BackgroundImage) || rows.contains(&StyleId::MaskImage) {
                let text: String = parts
                    .iter()
                    .map(|p| match p {
                        contract_syntax::TemplatePart::Text(t) => t.as_str(),
                        contract_syntax::TemplatePart::Expr(_) => " ",
                    })
                    .collect();
                if let Some(why) = exact_kernel::gradient::refused_function(&text) {
                    return err(
                        "lower-attr-value",
                        format!("`{}=…`: {why} — no host but the browser paints it, so the native ones would drop it", a.name),
                        span,
                    );
                }
            }
        }
        // @ref LLP 1043.000 §3 D1 — keep the full wire vocabulary, narrow authoring.
        if let Expr::Str(v, _) = value {
            if rows.contains(&StyleId::Resize)
                && matches!(
                    v.as_str(),
                    "both" | "horizontal" | "vertical" | "block" | "inline"
                )
            {
                return err("lower-css-resize", "CSS resize handles are not implemented by the native layout presenters; only `resize=\"none\"` is portable. Other values require user-controlled box geometry, not a different property name", span);
            }
            if rows.contains(&StyleId::UserSelect)
                && matches!(v.as_str(), "text" | "all" | "contain")
            {
                return err("lower-css-user-select", "CSS user-select text/all/contain require selectable text and selection ownership on iOS and Linux; those presenters do not implement it. Supported portable values are auto and none", span);
            }
            if rows.contains(&StyleId::WrapFlow) && !matches!(v.as_str(), "auto" | "both") {
                return err("lower-attr-value", "unsupported `wrap-flow` value: CSS Exclusions defines it; exact2 v1 implements `both` (or `auto`)", span);
            }
            if rows.contains(&StyleId::FlexBasis)
                && matches!(
                    v.trim(),
                    "content" | "min-content" | "max-content" | "fit-content"
                )
            {
                return err("lower-flex-basis", format!("CSS flex-basis `{v}` requires intrinsic basis sizing; exact2 dimension rows represent auto, lengths and percentages, and do not implement that intrinsic sizing mode. Use auto for a basis taken from the main-size property"), span);
            }
            if rows.contains(&StyleId::Transition) {
                if let Err(reason) = exact_motion::Transitions::parse(v) {
                    let supported = "translate, scale, rotate, opacity; color, background-color, border-color (and each side), tint-color, box-shadow; SVG fill, stroke, stroke-dashoffset, r, cx, cy, x, y, rx, ry; numeric height on admitted height owners";
                    let why = match reason {
                        exact_motion::ParseError::UnknownProperty(property) => {
                            let layout = matches!(property.as_str(), "width" | "min-width" | "max-width" | "min-height" | "max-height" | "top" | "right" | "bottom" | "left" | "margin" | "padding" | "flex-basis" | "gap");
                            format!("`{property}` {}: transitions animate {supported}. General layout-property interpolation would require layout per frame and is not implemented; `layout-transition` animates changes to the laid-out box", if layout { "is a CSS layout property, but exact2 cannot transition it" } else { "is not a supported transition property" })
                        }
                        other => format!("invalid transition components ({other:?}); supported properties: {supported}"),
                    };
                    return err(
                        "lower-attr-value",
                        format!("`transition=\"{v}\"`: {why}"),
                        span,
                    );
                }
            }
            // @ref LLP 1053 §0 G4 — the rest of CSS's list, refused by name.
            if rows.contains(&StyleId::FontVariantNumeric) {
                if let Some(word) = v.split_ascii_whitespace().find(|w| {
                    matches!(
                        *w,
                        "lining-nums"
                            | "oldstyle-nums"
                            | "proportional-nums"
                            | "diagonal-fractions"
                            | "stacked-fractions"
                            | "ordinal"
                            | "slashed-zero"
                    )
                }) {
                    return err("lower-attr-value", format!("`font-variant-numeric: {word}` is CSS, but exact2 implements only `normal` and `tabular-nums`"), span);
                }
            }
            // @ref LLP 1066, LLP 1077 — the kernel's parse says why, by name.
            let why = if rows.contains(&StyleId::BackgroundImage) {
                exact_kernel::gradient::BackgroundImage::check(v).err()
            } else if rows.contains(&StyleId::MaskImage) {
                exact_kernel::gradient::BackgroundImage::check_mask(v).err()
            } else if rows.contains(&StyleId::TextShadow) {
                exact_kernel::style::TextShadow::check(v).err()
            } else if rows.contains(&StyleId::CornerShape) {
                exact_kernel::corner::CornerShape::check(v).err()
            } else {
                None
            };
            if let Some(why) = why {
                return err(
                    "lower-attr-value",
                    format!("`{}=\"{v}\"`: {why}", a.name),
                    span,
                );
            }
            if rows.contains(&StyleId::ShapeMargin) && v.trim().ends_with('%') {
                return err("lower-attr-value", "percentage `shape-margin` is not implemented in exact2 v1; use a nonnegative length in points/px", span);
            }
        }
        // @ref LLP 1061 D1 — a press that makes a node vanish or flip is a
        // typo, not a feel.
        if rows.contains(&StyleId::PressScale) && numeric_literal(value).is_some_and(|n| n <= 0.0) {
            return err(
                "lower-attr-value",
                format!(
                    "`{}` takes a positive scale (0.97 is a button's, 1 is none)",
                    a.name
                ),
                span,
            );
        }
        // @ref LLP 1064 D1 — a shadow is text; a number is no shadow.
        if rows.contains(&StyleId::BoxShadow)
            && (numeric_literal(value).is_some()
                || (std::ptr::eq(value, &a.value) && matches!(ty, Ty::Number)))
        {
            return err(
                "lower-attr-type",
                format!("`{}` takes a string, as CSS writes a shadow", a.name),
                span,
            );
        }
        // The compiler checks every row's grammar (LLP 1053.000 §2).
        exact_kernel::style::link_backdrop_filter();
        exact_kernel::style::link_segments();
        exact_kernel::style::link_wide_colors();
        exact_kernel::timeline::link();
        let literal = match value {
            expr if numeric_literal(expr).is_some() => {
                Some(StyleValue::Number(numeric_literal(expr).unwrap()))
            }
            Expr::Str(s, _) => Some(
                // Enum keywords stay text, including `auto` (as in the runner).
                // Other codecs retain their existing dimension/keyword handling.
                if s == "auto"
                    && !rows
                        .iter()
                        .all(|row| row.codec() == exact_kernel::StyleCodec::Enum)
                {
                    StyleValue::Auto
                } else if let Some(pct) = s.strip_suffix('%').and_then(|p| p.parse::<f64>().ok()) {
                    StyleValue::Percent(pct)
                } else {
                    StyleValue::Text(s.clone())
                },
            ),
            Expr::Bool(b, _) => {
                return err(
                    "lower-attr-value",
                    format!(
                        "`{}={b}` — a style value is a number or a string, not a bool",
                        a.name
                    ),
                    span,
                )
            }
            _ => None,
        };
        match literal {
            Some(v) => {
                let mut probe = StyleProps::default();
                for row in rows {
                    // `unset`, or `inherit` on an inherited row: the row is
                    // cleared where it binds (feed F1).
                    if v.unsets(*row) {
                        continue;
                    }
                    if matches!(&v, StyleValue::Text(t) if t.trim().eq_ignore_ascii_case("inherit"))
                    {
                        return err(
                            "lower-attr-value",
                            format!(
                                "`{}=\"inherit\"`: `{}` does not inherit, and exact2 inherits only the rows CSS inherits; write the value",
                                a.name, a.name
                            ),
                            span,
                        );
                    }
                    if let Err(e) = probe.set_dynamic(*row, &v) {
                        // A number written as a pixel string: say the number.
                        let pixels = match (&e, value) {
                            (StyleValueError::WrongKind { .. }, Expr::Str(text, _)) => text
                                .trim()
                                .strip_suffix("px")
                                .and_then(|n| n.trim().parse::<f64>().ok())
                                .map(|n| format!("; write `{}={n}` (a number is pixels)", a.name)),
                            _ => None,
                        };
                        return err(
                            "lower-attr-value",
                            format!(
                                "`{}={}` is not a valid `{}`: {}{}",
                                a.name,
                                literal_text(value),
                                a.name,
                                named(&e, &v).map_or_else(|| describe(&e), String::from),
                                pixels.unwrap_or_default()
                            ),
                            span,
                        );
                    }
                }
            }
            None if std::ptr::eq(value, &a.value)
                && !matches!(ty, Ty::Number | Ty::String | Ty::Unknown) =>
            {
                return err(
                    "lower-attr-type",
                    format!(
                        "`{}` takes a number or a string; this expression is `{ty}`",
                        a.name
                    ),
                    span,
                );
            }
            _ => {}
        }
        for font in fonts {
            if rows.contains(&StyleId::FontStyle) {
                let requested = match value {
                    Expr::Str(s, _) if s == "normal" => Some(false),
                    Expr::Str(s, _) if s == "italic" => Some(true),
                    _ => None,
                };
                if let Some(italic) = requested {
                    if !font
                        .font
                        .faces
                        .iter()
                        .any(|(_, face_italic)| *face_italic == italic)
                    {
                        return err(
                            "lower-font-face",
                            format!(
                                "this family declares no real {} face; v1 never synthesizes one",
                                if italic { "italic" } else { "normal" }
                            ),
                            span,
                        );
                    }
                }
            }
            if rows.contains(&StyleId::FontWeight) {
                if let (Expr::Number(weight, _), Some(italic)) = (value, font.italic) {
                    if *weight >= 600.0
                        && !font.font.faces.iter().any(|(face_weight, face_italic)| {
                            *face_italic == italic && *face_weight >= 600
                        })
                    {
                        return err(
                            "lower-font-face",
                            format!(
                                "this family has no real {} face for font-weight={weight}; v1 never synthesizes one",
                                if italic { "italic bold" } else { "bold" }
                            ),
                            span,
                        );
                    }
                }
            }
        }
        let Some(next) = pending.pop() else { break };
        current = next;
    }
    Ok(())
}

/// HTML's enumerated attributes whose IDL attributes are bools, as the words
/// a bool is written: `spellcheck`'s `true`/`false`, `autocorrect`'s
/// `on`/`off`.
/// An ARIA state whose value is a word, a bool among them (`true`/`false`):
/// the words it takes, a bool expression written as one of the first two.
pub(crate) fn aria_words(prop: PropId) -> Option<(&'static str, &'static [&'static str])> {
    Some(match prop {
        PropId::AccessibilityPressed => ("aria-pressed", &["true", "false", "mixed"]),
        PropId::AccessibilityInvalid => ("aria-invalid", &["true", "false", "grammar", "spelling"]),
        PropId::AccessibilityHasPopup => (
            "aria-haspopup",
            &["true", "false", "menu", "listbox", "tree", "grid", "dialog"],
        ),
        _ => return None,
    })
}

pub(crate) fn bool_words(prop: PropId) -> Option<(&'static str, &'static str)> {
    match prop {
        PropId::Spellcheck => Some(("true", "false")),
        PropId::Autocorrect => Some(("on", "off")),
        _ => None,
    }
}

/// A prop attribute's value by the prop's type: text for most, a bool for
/// `disabled`, a whole number for `aria-level`, either for [`bool_words`].
pub(crate) fn check_prop_value(
    name: &str,
    value: &Expr,
    span: Span,
    prop: PropId,
    ty: &Ty,
) -> Result<(), LowerError> {
    media::check(name, value, span)?;
    let want = tags::prop_ty(prop);
    // An app's browsing contexts are its window and new ones: `_parent` and
    // `_top` name frames an app does not have, a name one it cannot open.
    if prop == PropId::Target
        && matches!(value, Expr::Str(s, _) if !matches!(s.as_str(), "_blank" | "_self"))
    {
        return err(
            "lower-attr-value",
            "`target` takes \"_blank\" or \"_self\"",
            span,
        );
    }
    if prop == PropId::AccessibilityLive
        && matches!(value, Expr::Str(s, _) if !matches!(s.as_str(), "off" | "polite" | "assertive"))
    {
        return err(
            "lower-attr-value",
            "`aria-live` takes \"off\", \"polite\" or \"assertive\"",
            span,
        );
    }
    // @ref LLP 1053.000 D4 — a material is a name in the schema's table.
    if prop == PropId::BackgroundMaterial {
        if let Expr::Str(name, _) = value {
            if exact_kernel::generated::material(name).is_none() {
                return err(
                    "lower-attr-value",
                    format!(
                        "`backgroundMaterial=\"{name}\"` is not a material; materials: {}",
                        exact_kernel::generated::MATERIALS.join(", ")
                    ),
                    span,
                );
            }
        }
    }
    if prop == PropId::ImageSource {
        if let Expr::Str(source, _) = value {
            if let Some(role) = source.strip_prefix("symbol:") {
                if !role.starts_with("sf/") && exact_kernel::generated::symbol(role).is_none() {
                    return err(
                        "lower-attr-value",
                        format!(
                            "symbol `{role}` is not a role; roles: {}",
                            exact_kernel::generated::SYMBOL_ROLES.join(", ")
                        ),
                        span,
                    );
                }
            }
        }
    }
    if want == tags::PropTy::Int {
        if let Some(number) = numeric_literal(value) {
            if !whole_i64(number) {
                return err(
                    "lower-attr-value",
                    format!(
                        "`{name}` takes a whole number in the signed 64-bit range; given {number}"
                    ),
                    span,
                );
            }
        }
    }
    // ARIA's word-valued states (`aria-pressed`'s `mixed`), or a bool.
    if let Some((attr, words)) = aria_words(prop) {
        if matches!(value, Expr::Str(s, _) if !words.contains(&s.as_str())) {
            let (last, rest) = words.split_last().unwrap();
            let rest: Vec<String> = rest.iter().map(|w| format!("\"{w}\"")).collect();
            return err(
                "lower-attr-value",
                format!("`{attr}` takes a bool or {} or \"{last}\"", rest.join(", ")),
                span,
            );
        }
        if matches!(ty, Ty::Bool) {
            return Ok(());
        }
    }
    if bool_words(prop).is_some() && matches!(ty, Ty::Bool) {
        return Ok(());
    }
    let ok = matches!(
        (want, ty),
        (_, Ty::Unknown)
            | (tags::PropTy::Str, Ty::String)
            | (tags::PropTy::Bool, Ty::Bool)
            | (tags::PropTy::Int | tags::PropTy::Float, Ty::Number)
    );
    if !ok {
        return err(
            "lower-attr-type",
            format!(
                "`{}` takes {}; this expression is `{}`{}",
                name,
                match want {
                    tags::PropTy::Str => "a string",
                    tags::PropTy::Bool => "a bool",
                    tags::PropTy::Int => "a whole number",
                    tags::PropTy::Float => "a number",
                },
                ty,
                // A number or bool shown as text is interpolated.
                if want == tags::PropTy::Str && matches!(ty, Ty::Number | Ty::Bool) {
                    ": interpolate it in a template, `${…}`, or write `toString(…)`"
                } else {
                    ""
                }
            ),
            span,
        );
    }
    Ok(())
}

/// @ref LLP 1053.000.000 D6 — where a glass group cannot be: beside the
/// element's own material (the group's glass would fuse with it), on a
/// scroll (its content belongs to the scroll and its rows), on a canvas
/// (whose overlay's direct children are captured and placed).
pub(crate) fn check_glass_group(tag: &tags::Tag, attrs: &[Attr]) -> Result<(), LowerError> {
    let Some(group) = attrs.iter().find(|a| a.name == "glassGroup") else {
        return Ok(());
    };
    let refuse = |why: &str| {
        err(
            "lower-glass-group",
            format!("`glassGroup` {why}"),
            group.span,
        )
    };
    if let Some(m) = attrs
        .iter()
        .find(|a| a.name == "backgroundMaterial" || a.name == "backdrop-filter")
    {
        return refuse(&format!(
            "and `{}` on one element: the group's glass would fuse with the element's own; put the group on the parent",
            m.name
        ));
    }
    let scrolls = attrs.iter().any(|a| {
        matches!(a.name.as_str(), "overflow" | "overflow-x" | "overflow-y")
            && matches!(&a.value, Expr::Str(v, _) if v == "scroll" || v == "auto")
    });
    if tag.node_type.scrolls_by_default() || scrolls {
        return refuse("on an element that scrolls: put the group on a child inside the scroll");
    }
    if tag.node_type == exact_kernel::NodeType::Canvas {
        return refuse("on a `canvas`: put the group on a child of the canvas");
    }
    Ok(())
}

/// CSS flex shorthand, projected into the three longhands before bytecode.
pub(crate) fn flex_component(value: &Expr, index: usize) -> Result<Expr, LowerError> {
    let mut out = value.clone();
    match &mut out {
        Expr::Ternary(_, yes, no, _) => {
            **yes = flex_component(yes, index)?;
            **no = flex_component(no, index)?;
        }
        Expr::Match { some, none, .. } => {
            **some = flex_component(some, index)?;
            **none = flex_component(none, index)?;
        }
        Expr::Let { body, .. } => **body = flex_component(body, index)?,
        Expr::Str(text, span) => {
            let number = |s: &str| s.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 0.0);
            let (grow, shrink, basis) = match text.trim() {
                word if word.eq_ignore_ascii_case("none") => (0.0, 0.0, "auto"),
                word if word.eq_ignore_ascii_case("auto") => (1.0, 1.0, "auto"),
                word if word.eq_ignore_ascii_case("initial") => (0.0, 1.0, "auto"),
                text => {
                    let mut depth = 0;
                    let words: Vec<_> = text
                        .split(|c: char| {
                            if c == '(' {
                                depth += 1;
                            }
                            if c == ')' {
                                depth -= 1;
                            }
                            c.is_ascii_whitespace() && depth == 0
                        })
                        .filter(|s| !s.is_empty())
                        .collect();
                    let mut factors = Vec::new();
                    let mut factor_positions = Vec::new();
                    let mut basis = None;
                    for (position, word) in words.into_iter().enumerate() {
                        if let Some(n) = number(word) {
                            factors.push(n);
                            factor_positions.push(position);
                        } else if basis.replace(word).is_some() {
                            return err(
                                "lower-attr-value",
                                "`flex` expects none, auto, or <grow> [<shrink>] [<basis>]",
                                *span,
                            );
                        }
                    }
                    // A third unitless zero is a zero length, not a factor.
                    if factors.len() == 3 && factors[2] == 0.0 && basis.is_none() {
                        factors.pop();
                        factor_positions.pop();
                        basis = Some("0px");
                    }
                    if factors.len() > 2
                        || (factors.is_empty() && basis.is_none())
                        || depth != 0
                        || (factor_positions.len() == 2
                            && factor_positions[1] != factor_positions[0] + 1)
                    {
                        return err(
                            "lower-attr-value",
                            "`flex` expects none, auto, or <grow> [<shrink>] [<basis>]",
                            *span,
                        );
                    }
                    (
                        factors.first().copied().unwrap_or(1.0),
                        factors.get(1).copied().unwrap_or(1.0),
                        basis.unwrap_or("0%"),
                    )
                }
            };
            out = match index {
                0 => Expr::Number(grow, *span),
                1 => Expr::Number(shrink, *span),
                _ => Expr::Str(basis.into(), *span),
            };
        }
        _ if index == 1 => out = Expr::Number(1.0, value.span()),
        _ if index == 2 => out = Expr::Str("0%".into(), value.span()),
        _ => {}
    }
    Ok(out)
}

/// A literal `flex` (a number or the CSS shorthand's text) as its three
/// longhands; `None` for a computed one, which the bake's measured-layout
/// lint judges.
pub(crate) fn flex_longhands(value: &Expr) -> Option<(f64, f64, String)> {
    if !matches!(value, Expr::Str(..)) && numeric_literal(value).is_none() {
        return None;
    }
    let part = |i| flex_component(value, i).ok();
    let grow = part(0).as_ref().and_then(numeric_literal)?;
    let shrink = part(1).as_ref().and_then(numeric_literal)?;
    let Some(Expr::Str(basis, _)) = part(2) else {
        return None;
    };
    Some((grow, shrink, basis))
}

/// Whether a `flex` bounds its box on the main axis by itself: a positive
/// grow (it takes the container's free space), or a definite length basis,
/// zero included (`flex="0 0 0px"` is an empty scrollport, as `height=0`
/// is). `none`, `initial`, `"0"` (grow 0 over a `0%` basis) and a zero grow
/// over `auto`, content or a zero percentage are not (Grok's batch 2
/// reviews). A computed value is left to the bake's lint.
pub(crate) fn flex_bounds(value: &Expr) -> bool {
    let Some((grow, _, basis)) = flex_longhands(value) else {
        return true;
    };
    let basis = basis.trim();
    let intrinsic = matches!(
        basis,
        "auto" | "content" | "min-content" | "max-content" | "fit-content"
    );
    let zero_percent = basis
        .strip_suffix('%')
        .is_some_and(|n| n.trim().parse::<f64>().is_ok_and(|n| n == 0.0));
    grow > 0.0 || !(intrinsic || zero_percent)
}

/// A shrinking flex item with a zero minimum fits a bounded flex column.
/// Its shrink is the `flex-shrink` longhand's, or else the `flex`
/// shorthand's (`flex="none"` does not shrink).
pub(crate) fn shrinking_scroll(attrs: &[Attr], bounded_column: bool) -> bool {
    let shrink = attrs
        .iter()
        .rev()
        .find(|a| a.name == "flex-shrink")
        .map(|a| numeric_literal(&a.value))
        .or_else(|| {
            let flex = attrs.iter().rev().find(|a| a.name == "flex")?;
            Some(flex_longhands(&flex.value).map(|(_, shrink, _)| shrink))
        });
    bounded_column
        && attrs
            .iter()
            .any(|a| a.name == "min-height" && numeric_literal(&a.value) == Some(0.0))
        && shrink.is_none_or(|n| n.is_none_or(|n| n > 0.0))
}

pub(crate) fn bounded_column(tag: &str, attrs: &[Attr], inherited: bool) -> bool {
    let word = |name: &str| {
        attrs
            .iter()
            .rev()
            .find(|a| a.name == name)
            .and_then(|a| match &a.value {
                Expr::Str(s, _) => Some(s.as_str()),
                _ => None,
            })
    };
    let column = word("display").map_or(tag == "column" || tag == "button", |v| v == "flex")
        && word("flex-direction").map_or(tag == "column" || tag == "button", |v| {
            v.starts_with("column")
        });
    column
        && (attrs.iter().any(|a| {
            matches!(a.name.as_str(), "height" | "max-height")
                && !matches!(&a.value, Expr::Str(s, _) if s == "auto")
        }) || shrinking_scroll(attrs, inherited))
}
