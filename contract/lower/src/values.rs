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

/// A CSS-text row's refusal with the kernel's own reason (LLP 1076).
fn named(e: &StyleValueError, v: &exact_kernel::StyleValue) -> Option<&'static str> {
    let exact_kernel::StyleValue::Text(t) = v else {
        return None;
    };
    match e {
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
        StyleValueError::UnknownEnumValue { style } => format!(
            "expected one of {}",
            style.enum_names().iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ")
        ),
        StyleValueError::AutoNotAdmitted { .. } => "`auto` is not admitted here".into(),
        StyleValueError::OutOfRange { .. } => "out of the row's range".into(),
        StyleValueError::BadColor { .. } => "a color is `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, `rgba(r, g, b, a)`, or `transparent`".into(),
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
        StyleValueError::BadTransition { .. } => "not a CSS `transition` shorthand".into(),
        StyleValueError::BadPaint { .. } => "SVG paint is `none`, `currentcolor`, or a colour (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `light-dark()`); paint servers (`url(#…)`) are refused (LLP 1055 D12)".into(),
        StyleValueError::BadDashArray { .. } => "`stroke-dasharray` is `none` or non-negative numbers separated by spaces or commas".into(),
        StyleValueError::BadTransform { .. } => "`transform` is `none` or transform functions: matrix, translate, translateX/Y, scale, scaleX/Y, rotate (with SVG's optional centre), skew, skewX/Y; lengths in user units or px, angles in deg, rad, grad or turn".into(),
        StyleValueError::BadMarker { .. } => "a marker or a mask is `none` or `url(#id)`, naming a `marker` or a `mask`".into(),
        StyleValueError::BadFilter { .. } => "`filter` is `none`, or `url(#id)` naming a `filter` and the filter functions (blur, brightness, contrast, drop-shadow, grayscale, hue-rotate, invert, opacity, saturate, sepia), in order".into(),
        StyleValueError::BadPaintOrder { .. } => "`paint-order` is `normal`, or `fill`, `stroke` and `markers` in the order they paint".into(),
        StyleValueError::BadTransformOrigin { .. } => "`transform-origin` is one or two of left, center, right, top, bottom, a length or a percentage".into(),
        StyleValueError::BadAnimation { .. } => "not a CSS `animation` shorthand: `<name> <duration> [<easing>] [<delay>] [<count>|infinite] [<direction>] [<fill-mode>] [<play-state>]`".into(),
        StyleValueError::Unsupported { .. } => "this row has no dynamic form".into(),
        StyleValueError::BadBoxShadow { .. } => "expected none, or shadows separated by commas: `inset? <x> <y> [<blur> [<spread>]] <colour>`".into(),
        StyleValueError::BadBackdropFilter { reason, .. } => (*reason).into(),
    }
}

/// The rows CSS's `border-color` shorthand sets: top, right, bottom, left.
pub(crate) const BORDER_COLORS: [StyleId; 4] = [
    StyleId::BorderColorTop,
    StyleId::BorderColorRight,
    StyleId::BorderColorBottom,
    StyleId::BorderColorLeft,
];

/// A `border-color` value's one to four colours, split where CSS splits
/// them: at white space outside parentheses, so `light-dark(#fff, #000)` is
/// one colour.
fn border_color_values(text: &str) -> Vec<&str> {
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

/// CSS's `border-color: <color>{1,4}` as four longhand expressions (top,
/// right, bottom, left): every literal the value can produce — a string, or
/// an arm of a conditional — is split into its sides, and a computed leaf is
/// one colour for all four. `None` when no literal names more than one
/// colour, so the shorthand stays one binding for four rows.
pub(crate) fn border_color_sides(value: &Expr) -> Result<Option<[Expr; 4]>, LowerError> {
    fn widest(e: &Expr) -> Result<usize, LowerError> {
        Ok(match e {
            Expr::Str(s, span) => {
                let n = border_color_values(s).len();
                if n > 4 {
                    return err(
                        "lower-attr-value",
                        format!("`border-color` takes one to four colours (top, right, bottom, left); \"{s}\" has {n}"),
                        *span,
                    );
                }
                n
            }
            Expr::Ternary(_, yes, no, _) => widest(yes)?.max(widest(no)?),
            Expr::Match { some, none, .. } => widest(some)?.max(widest(none)?),
            Expr::Let { body, .. } => widest(body)?,
            _ => 1,
        })
    }
    fn side(e: &Expr, i: usize) -> Expr {
        match e {
            Expr::Str(s, span) => {
                let v = border_color_values(s);
                // CSS: top; right = top; bottom = top; left = right.
                let pick = match (v.len(), i) {
                    (0, _) => return e.clone(),
                    (1, _) => 0,
                    (2, _) => i % 2,
                    (3, 3) => 1,
                    (_, i) => i,
                };
                Expr::Str(v[pick].to_string(), *span)
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
    if widest(value)? < 2 {
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
    if rows == BORDER_COLORS {
        if let Some(sides) = border_color_sides(&a.value)? {
            for (row, value) in BORDER_COLORS.iter().zip(sides) {
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
        // @ref LLP 1043.000 §3 D1 — keep the full wire vocabulary, narrow authoring.
        if let Expr::Str(v, _) = value {
            if rows.contains(&StyleId::WrapFlow) && !matches!(v.as_str(), "auto" | "both") {
                return err("lower-attr-value", "unsupported `wrap-flow` value: CSS Exclusions defines it; exact2 v1 implements `both` (or `auto`)", span);
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
            // @ref LLP 1066, LLP 1076 — the kernel's parse says why, by name.
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

/// A prop attribute's value by the prop's type: text for most, a bool for
/// `disabled`, a whole number for `aria-level`.
pub(crate) fn check_prop_value(
    name: &str,
    value: &Expr,
    span: Span,
    prop: PropId,
    ty: &Ty,
) -> Result<(), LowerError> {
    media::check(name, value, span)?;
    let want = tags::prop_ty(prop);
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
            && matches!(&a.value, Expr::Str(v, _) if v == "scroll")
    });
    if tag.node_type.scrolls_by_default() || scrolls {
        return refuse("on an element that scrolls: put the group on a child inside the scroll");
    }
    if tag.node_type == exact_kernel::NodeType::Canvas {
        return refuse("on a `canvas`: put the group on a child of the canvas");
    }
    Ok(())
}
