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

/// The kernel's refusal of a style value, in an author's words.
fn describe(e: &StyleValueError) -> String {
    match e {
        StyleValueError::WrongKind { expected, .. } => format!("expected {expected}"),
        StyleValueError::UnknownEnumValue { style } => format!(
            "expected one of {}",
            style.enum_names().iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ")
        ),
        StyleValueError::AutoNotAdmitted { .. } => "`auto` is not admitted here".into(),
        StyleValueError::OutOfRange { .. } => "out of the row's range".into(),
        StyleValueError::BadColor { .. } => "a color is `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb(r, g, b)`, or `rgba(r, g, b, a)`".into(),
        StyleValueError::BadShapeOutside { .. } => "expected none, circle(), ellipse(), inset() with one round radius, or polygon() with at most 64 vertices; lengths are points/px or percentages".into(),
        StyleValueError::BadClipPath { .. } => "expected none or path() with explicit absolute M/L/Q/C/Z commands and separated finite coordinates".into(),
        StyleValueError::BadAspectRatio { .. } => "expected auto, a ratio (`16 / 9`, or a number), or both (`auto 4 / 3`); numbers are nonnegative".into(),
        StyleValueError::BadTransition { .. } => "not a CSS `transition` shorthand".into(),
        StyleValueError::Unsupported { .. } => "this row has no dynamic form".into(),
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
            if rows.contains(&StyleId::ShapeMargin) && v.trim().ends_with('%') {
                return err("lower-attr-value", "percentage `shape-margin` is not implemented in exact2 v1; use a nonnegative length in points/px", span);
            }
        }
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
                                describe(&e),
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
    if prop == PropId::ImageSource {
        if let Expr::Str(source, _) = value {
            if let Some(role) = source.strip_prefix("symbol:") {
                if exact_kernel::generated::symbol(role).is_none() {
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
