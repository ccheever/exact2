//! Attribute values checked against what they set, at compile time: a
//! literal style value by the kernel's own row parser, a computed one and a
//! prop by type (LLP 1017 P1a).
use crate::style_names::press_refusal;
use crate::{edits, err, media, symbols, tags, vocab, FontUse, LowerError};
use contract_syntax::{Attr, Expr, Span, UnOp};
use contract_types::{Ref, Scope, Shapes, Ty};
use exact_kernel::style::sides as box_sides;
use exact_kernel::{PropId, StyleId, StyleProps, StyleValue, StyleValueError};
use std::{cell::Cell, rc::Rc};
/// A literal, as an author wrote it, for a message.
fn literal_text(e: &Expr) -> String {
    match e {
        Expr::Number(n, _) => format!("{n}"),
        Expr::Str(s, _) => format!("\"{s}\""),
        Expr::Bool(b, _) => format!("{b}"),
        _ => "…".into(),
    }
}
/// `navigationSplit`'s column width written as a number: its text, as the
/// prop carries it (LLP 1116.004 D1.2).
pub(crate) fn split_width(prop: PropId, e: &Expr) -> Option<Expr> {
    match e {
        Expr::Number(n, span) if prop == PropId::NavigationSplit => {
            Some(Expr::Str(n.to_string(), *span))
        }
        _ => None,
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
        StyleValueError::BadComparison { reason, .. } => (*reason).into(),
        StyleValueError::UnknownEnumValue { style } => format!(
            "expected one of {}",
            style.enum_names().iter().map(|name| format!("{name:?}")).collect::<Vec<_>>().join(", ")
        ),
        StyleValueError::AutoNotAdmitted { .. } => "`auto` is not admitted here".into(),
        StyleValueError::OutOfRange { .. } => "out of the row's range".into(),
        StyleValueError::BadColor { .. } => "a color is a CSS colour: hex, `rgb()`, `hsl()`, `hwb()`, a named colour, `transparent`, or one in its own space: `color(display-p3 1 0 0)`, `oklch()`, `oklab()`, `lab()`, `lch()` (LLP 1100) — `light-dark(a, b)` of two, a role (`\"-exact-secondary-label\"`, `\"CanvasText\"`: LLP 1095, LLP 1081), or `-exact-platform-color(ios <name>Color, …, <fallback>)` written whole as a string literal".into(),
        StyleValueError::BadShapeOutside { .. } => "expected none, circle(), ellipse(), inset() with one round radius, or polygon() with at most 64 vertices; lengths are points/px or percentages".into(),
        StyleValueError::BadClipPath { .. } => "expected none or path() with explicit absolute M/L/Q/C/Z commands and separated finite coordinates".into(),
        StyleValueError::BadAspectRatio { .. } => "expected auto, a ratio (`16 / 9`, or a number), or both (`auto 4 / 3`); numbers are nonnegative".into(),
        StyleValueError::BadBackgroundImage { .. } => "expected none, or up to four of linear-gradient(…), radial-gradient(…) and conic-gradient(…)".into(),
        StyleValueError::BadMaskImage { .. } => "expected none, or one linear-gradient(…), radial-gradient(…) or conic-gradient(…)".into(),
        StyleValueError::BadTextShadow { .. } => "expected none, or one shadow: <offset-x> <offset-y> [<blur>] and an optional colour".into(),
        StyleValueError::BadFontVariations { .. } => "expected normal, or up to eight axes as `\"wght\" 650, \"wdth\" 90`: a quoted four-character tag and a number".into(),
        StyleValueError::BadCornerShape { .. } => "expected one to four of round, squircle, square, bevel, scoop, notch, superellipse(<number>) or -exact-continuous".into(),
        StyleValueError::BadDragTimeline { .. } => "expected none, or a `--name` and an optional axis (`x` or `y`)".into(),
        StyleValueError::BadAnimationTimeline { .. } => "expected auto or a `--name`".into(),
        StyleValueError::BadAnimationRange { .. } => "expected normal, or two distinct lengths (`0px 300px`)".into(),
        StyleValueError::BadPositionTryFallbacks { .. } => "expected none, or comma-separated attempts, each one to three of flip-block, flip-inline and flip-start, applied in order; flip-x, flip-y and named rules are not implemented".into(),
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
        StyleValueError::BadGridTracks { .. } => "expected at most 10,000 CSS grid tracks the kernel can lay out: px, %, fr, auto, min-content, max-content, calc()/min()/max()/clamp() over px and %, fit-content(px or %), minmax(), repeat() (including auto-fill/auto-fit), and named lines; var() and env() are unsupported".into(),
        StyleValueError::BadGridPlacement { .. } => "expected `auto`, a nonzero or named line through 10,000, or `span N` through 10,000, optionally followed by `/` and a second line".into(),
        StyleValueError::BadTextStroke { reason, .. } => (*reason).into(),
        StyleValueError::BadSymbolPalette { .. } => "expected none, or one to three colours".into(),
        StyleValueError::BadRotateAxis { .. } => "expected an angle, and optionally an axis: `x`, `y`, `z` or three numbers".into(),
        StyleValueError::BadBoxShadow { .. } => "expected none, or shadows separated by commas: `inset? <x> <y> [<blur> [<spread>]] <colour>`".into(),
        StyleValueError::BadBackdropFilter { reason, .. } => (*reason).into(),
    }
}
/// Whether these rows are one of CSS's `<value>{1,4}` box shorthands or
/// its `<value>{1,2}` pairs, which are split into their rows the same way.
pub(crate) fn four_sided(rows: &[StyleId]) -> bool {
    box_sides::FAMILIES.iter().any(|f| rows == f)
        || super::shorthands::PAIRS.iter().any(|p| rows == p)
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
/// Whether `e` puts a `-exact-platform-color(` literal into a value it computes.
fn builds_platform_color(e: &Expr) -> bool {
    let named = |t: &str| t.contains("-exact-platform-color(");
    match e {
        Expr::Str(t, _) => named(t),
        Expr::Template(parts, _) => parts.iter().any(|p| match p {
            contract_syntax::TemplatePart::Text(t) => named(t),
            contract_syntax::TemplatePart::Expr(e) => builds_platform_color(e),
        }),
        Expr::Some(e, _)
        | Expr::Member(e, _, _)
        | Expr::NamedArg(_, e, _)
        | Expr::Unary(_, e, _)
        | Expr::Typed(e, _, _) => builds_platform_color(e),
        Expr::Arrow { body, .. } => builds_platform_color(body),
        Expr::Call(_, args, _) => args.iter().any(builds_platform_color),
        Expr::Binary(_, l, r, _) => builds_platform_color(l) || builds_platform_color(r),
        Expr::Ternary(c, y, n, _) => [c, y, n].iter().any(|e| builds_platform_color(e)),
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => [subject, some, none]
            .iter()
            .any(|e| builds_platform_color(e)),
        Expr::Let { value, body, .. } => {
            builds_platform_color(value) || builds_platform_color(body)
        }
        Expr::List(items, _) => items.iter().any(builds_platform_color),
        Expr::Number(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => false,
    }
}
/// `position-area` places a popover against the invoker that opens it (its
/// implicit anchor, LLP 1021 §5): there is no `anchor-name`, so on any other
/// node it would name nothing to place against.
pub(crate) fn check_position_area(attrs: &[Attr]) -> Result<(), LowerError> {
    let Some(a) = attrs
        .iter()
        .find(|a| matches!(a.name.as_str(), "position-area" | "position-try-fallbacks"))
    else {
        return Ok(());
    };
    if attrs.iter().any(|a| a.name == "popover") {
        return Ok(());
    }
    err(
        "lower-css-position-area",
        "`position-area` and `position-try-fallbacks` are admitted on a `popover` only: its anchor is the button whose `popovertarget` opens it. `anchor-name` and `position-anchor` are not implemented",
        a.span,
    )
}
impl crate::Lowerer<'_> {
    /// `check_style_value` with this lowering's shapes.
    pub(crate) fn check_style(
        &self,
        a: &Attr,
        rows: &[StyleId],
        ty: &Ty,
        fonts: &[FontUse],
        scope: &Scope,
    ) -> Result<(), LowerError> {
        check_style_value(a, rows, ty, fonts, scope, &self.types.shapes)
    }
}
pub(crate) fn check_style_value(
    a: &Attr,
    rows: &[StyleId],
    ty: &Ty,
    fonts: &[FontUse],
    scope: &Scope,
    shapes: &Shapes,
) -> Result<(), LowerError> {
    // A pin names the design's axes, which the runner resolves (LLP 1121 D5).
    if rows == [StyleId::Mode] {
        return crate::theme::mode_pins(a);
    }
    // A length or number token in a row of another kind (LLP 1121 D3).
    crate::theme::measure_row(a, rows, ty.is_string(), &|alt| {
        let ty = match alt.value {
            Expr::Number(..) => Ty::Number,
            _ => Ty::String,
        };
        style_value(alt, rows, &ty, fonts, scope, shapes)
    })?;
    // A name the design lacks, or a colour token in another row (LLP 1121 D3).
    style_value(a, rows, ty, fonts, scope, shapes)
        .map_err(|e| crate::theme::token_error(a, rows, e))
}
fn style_value(
    a: &Attr,
    rows: &[StyleId],
    ty: &Ty,
    fonts: &[FontUse],
    scope: &Scope,
    shapes: &Shapes,
) -> Result<(), LowerError> {
    if rows
        .iter()
        .any(|r| matches!(r, StyleId::Resize | StyleId::UserSelect))
    {
        super::shorthands::portable_literal(&a.value, &a.name)?;
    }
    if four_sided(rows) {
        if let Some(sides) = super::shorthands::sides(&a.name, &a.value)? {
            for (row, value) in rows.iter().zip(sides) {
                let side = Attr { value, ..a.clone() };
                check_style_value(&side, std::slice::from_ref(row), ty, fonts, scope, shapes)?;
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
            // @ref LLP 1095 D3 — a plan's platform colours are its literals:
            // never built from a template, a concatenation or a call. A
            // metric's name is the compiler's alone (LLP 1115.002 D6).
            Expr::Str(..) => crate::metrics::refuse_authored(a, value, span)?,
            other if builds_platform_color(other) => {
                return err(
                    "lower-platform-color-literal",
                    format!("`{}`: write `-exact-platform-color(…)` whole, as a string literal (a branch of `?:` or `match` may be one); it is never built from a template, a concatenation or data, so the platform colours a plan names are fixed when it compiles (LLP 1095 D3)", a.name),
                    span,
                );
            }
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
            // @ref LLP 1081 D2 — an old spelling is refused with its new one.
            if let Some((old, new)) = crate::style_names::renamed_token(v, rows) {
                return err(
                    "lower-attr-value",
                    format!(
                        "`{old}` is spelled `{new}` (LLP 1081): `{}=\"{v}\"`",
                        a.name
                    ),
                    span,
                );
            }
            // @ref LLP 1081 D5 — host-private `--exact-*` names are the web
            // host's own; a content owner's values (LLP 1123 D9) are read.
            if host_private(v) {
                return err(
                    "lower-attr-value",
                    format!("`--exact-*` names other than a content owner's values (`var(--exact-safe-area-inset-top)`, LLP 1123 D9) are the host's own; write the author's `-exact-` name: `{}=\"{v}\"`", a.name),
                    span,
                );
            }
            if rows.contains(&StyleId::Resize)
                && matches!(
                    v.as_str(),
                    "both" | "horizontal" | "vertical" | "block" | "inline"
                )
            {
                return err("lower-css-resize", "CSS resize handles are not implemented by the native layout presenters; only `resize=\"none\"` is portable. Other values require user-controlled box geometry, not a different property name", span);
            }
            // @ref LLP 1115.000 D11.6 — `text` and `all` are honoured where a
            // host has text selection (the web, macOS, iOS); `contain` names
            // a selection behaviour no native presenter has.
            if rows.contains(&StyleId::UserSelect) && v == "contain" {
                return err("lower-css-user-select", "CSS `user-select=\"contain\"` is not implemented by the native presenters: it keeps a selection from leaving its element, and their text selection ownership has no such bound. Supported values are auto, none, text and all", span);
            }
            if rows.contains(&StyleId::PositionArea)
                && !StyleId::PositionArea.enum_names().contains(&v.trim())
            {
                return err("lower-css-position-area", format!("`position-area=\"{v}\"`: exact2 places an invoker's popover in a subset of CSS `position-area`: {}. Logical areas and other spellings are not implemented by the native top layers", StyleId::PositionArea.enum_names().join(", ")), span);
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
                    let supported = "translate, scale, rotate, opacity; color, background-color, border-color (and each side), -exact-tint-color, box-shadow; SVG fill, stroke, stroke-dashoffset, r, cx, cy, x, y, rx, ry, d; numeric height on admitted height owners";
                    let why = match reason {
                        exact_motion::ParseError::UnknownProperty(property) => {
                            crate::motion::layout_property_refusal(&property, supported).unwrap_or_else(|| format!("`{property}` is not a supported transition property: transitions animate {supported}"))
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
            // A shadow token's name: its values were checked as this row's
            // (`theme::measure_row`), not the name's own text.
            let why = if crate::theme::measure_named(v).is_some() {
                None
            } else if rows.contains(&StyleId::BackgroundImage) {
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
                    format!(
                        "`{}=\"{v}\"`: {why}{}",
                        a.name,
                        edits::role_hint(v, false)
                            .map(|e| format!("; {}", e.clause()))
                            .unwrap_or_default()
                    ),
                    span,
                );
            }
            if rows.contains(&StyleId::TextIndent)
                && (v.trim().ends_with('%') || v.contains("hanging") || v.contains("each-line"))
            {
                return err("lower-attr-value", format!("`text-indent=\"{v}\"`: exact2 implements a length (a number of pixels, or `rem` or `em`; negative hangs the first line); a percentage of the containing block and the `hanging` and `each-line` keywords are not implemented. For a hanging indent write a negative length with the same `padding-left`"), span);
            }
            // @ref LLP 1093 §1 — each refused by what it would need.
            if let Some(why) = multicol_value(rows, v.trim()) {
                return err(
                    "lower-attr-value",
                    format!("`{}=\"{v}\"`: {why}", a.name),
                    span,
                );
            }
            // @ref LLP 1001 §6: CSS's `text-wrap-style` keywords the hosts lack.
            if rows.contains(&StyleId::TextWrapStyle) && matches!(v.trim(), "pretty" | "stable") {
                return err("lower-attr-value", format!("`{}=\"{v}\"`: exact2 implements `auto` and `balance`; Chrome's `pretty` and `stable` line breakers are not implemented on the native hosts. Use `auto` for ordinary wrapping", a.name), span);
            }
            // @ref LLP 1034 §8: `light` or `dark`. CSS's `normal` means the
            // page's schemes, not the parent's, and `light dark` and `only`
            // ask a browser to choose: none is implemented; leaving the
            // attribute off follows the surrounding scheme.
            if rows.contains(&StyleId::ColorScheme) && !matches!(v.trim(), "light" | "dark") {
                return err("lower-attr-value", format!("`color-scheme=\"{v}\"`: exact2 implements `light` and `dark` on a subtree (LLP 1034 §8); leave it off to follow the surrounding scheme. CSS's `normal`, `light dark` and `only` are not implemented. A Theme setting (Auto, Light, Dark) is `setScheme(\"system\" | \"light\" | \"dark\")` in the action; `color-scheme` fixes a subtree's scheme"), span);
            }
            if rows.contains(&StyleId::ShapeMargin) && v.trim().ends_with('%') {
                return err("lower-attr-value", "percentage `shape-margin` is not implemented in exact2 v1; use a nonnegative length in points/px", span);
            }
        }
        // @ref LLP 1093 D4 — a positive integer, as CSS says.
        if (rows.contains(&StyleId::Widows) || rows.contains(&StyleId::Orphans))
            && numeric_literal(value).is_some_and(|n| n < 1.0)
        {
            return err(
                "lower-attr-value",
                format!("`{}` is a positive integer", a.name),
                span,
            );
        }
        if let Some(why) = numeric_literal(value).and_then(|n| press_refusal(rows, n, &a.name)) {
            return err("lower-attr-value", why, span);
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
                // A keyword is ASCII case-insensitive, as the runner reads it.
                if s.trim_matches(['\t', '\n', '\u{c}', '\r', ' '])
                    .eq_ignore_ascii_case("auto")
                    && !rows
                        .iter()
                        .all(|row| row.codec() == exact_kernel::StyleCodec::Enum)
                {
                    StyleValue::Auto
                } else if let Some(pct) = s.strip_suffix('%').and_then(|p| p.parse::<f64>().ok()) {
                    StyleValue::Percent(pct)
                } else {
                    StyleValue::Text(s.as_str().into())
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
                        // @ref LLP 1116.014 D1 — the edit leads when the fix is one.
                        if let Some(edit) = edits::value_edit(&a.name, value, rows, &e) {
                            return err("lower-attr-value", edit, span);
                        }
                        // A viewport-pinned box (authoring bench, t6-todo-more). A pixel row
                        // takes `14px` as CSS does (LLP 1102 §3.10), and a maximum takes
                        // `none` (§3.11), so neither needs a hint here.
                        let hint = (a.name == "position"
                            && matches!(value, Expr::Str(t, _) if t.trim() == "fixed"))
                        .then(|| {
                            "; `fixed` is not a row (LLP 1001): pin a box to the viewport \
                             with `absolute`, directly inside a viewport-sized root that \
                             does not scroll (its content scrolls in a `scroll` beside it)"
                                .to_string()
                        })
                        .or_else(|| match value {
                            // A notification haptic reports a task's outcome, which a
                            // press has not had at touch-down.
                            Expr::Str(t, _) if rows == [StyleId::PressHaptic] && matches!(t.as_str(), "success" | "warning" | "error") => {
                                Some(format!("; \"{t}\" is an outcome, which a press has not had at touch-down: play it from the action that has it, `haptic(\"{t}\")`"))
                            }
                            Expr::Str(t, _) => {
                                edits::role_hint(t, false).map(|e| format!("; {}", e.clause()))
                            }
                            _ => None,
                        });
                        return err(
                            "lower-attr-value",
                            format!(
                                "`{}={}` is not a valid `{}`: {}{}",
                                a.name,
                                literal_text(value),
                                a.name,
                                named(&e, &v).map_or_else(|| describe(&e), String::from),
                                hint.unwrap_or_default()
                            ),
                            span,
                        );
                    }
                }
            }
            None if std::ptr::eq(value, &a.value)
                && !matches!(ty, Ty::Number | Ty::String | Ty::Enum(_) | Ty::Unknown) =>
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
    // A metric in a sheet the compiler wrote is a string that every host
    // reads as its platform's number (LLP 1115.002 D6).
    let metric = !a.written && crate::metrics::names(&a.value);
    // So is a number token's name, which the runner reads as its number
    // (LLP 1121 D3).
    // A computed string may name one too, when the design has one (D7).
    let token = crate::theme::names_measures(&a.value)
        || crate::theme::number_tokens()
            && rows.iter().all(|r| {
                exact_kernel::style::theme::admits(exact_kernel::style::theme::Kind::Number, *r)
            });
    if !metric && !token && !rows.iter().any(|row| exact_kernel::style::takes_text(*row)) {
        let offending = computed_string_arm(&a.value, scope, shapes, rows, &[]).or_else(|| {
            (ty.is_string() && !css_wide_branches(&a.value, rows)).then_some(a.value.span())
        });
        if let Some(span) = offending {
            return err(
                "lower-attr-type",
                format!(
                    "`{}` takes a number in every branch; this arm is `string`, which a native host does not read on this row",
                    a.name
                ),
                span,
            );
        }
    }
    Ok(())
}
fn css_wide_branches(e: &Expr, rows: &[StyleId]) -> bool {
    match e {
        Expr::Str(s, _) => rows
            .iter()
            .all(|row| StyleValue::Text(s.as_str().into()).unsets(*row)),
        Expr::Ternary(_, yes, no, _) => css_wide_branches(yes, rows) && css_wide_branches(no, rows),
        Expr::Match { some, none, .. } => {
            css_wide_branches(some, rows) && css_wide_branches(none, rows)
        }
        Expr::Let { body, .. } | Expr::Typed(body, _, _) => css_wide_branches(body, rows),
        _ => false,
    }
}
#[derive(Clone)]
struct Bound<'a> {
    name: &'a str,
    value: Option<&'a Expr>,
    scope: Scope,
    checked: Rc<Cell<Option<Option<Span>>>>,
}
fn computed_string_arm<'a>(
    e: &'a Expr,
    scope: &Scope,
    shapes: &Shapes,
    rows: &[StyleId],
    bound: &[Bound<'a>],
) -> Option<Span> {
    #[cfg(test)]
    tests::record_string_arm_visit();
    match e {
        Expr::Ternary(_, yes, no, _) => computed_string_arm(yes, scope, shapes, rows, bound)
            .or_else(|| computed_string_arm(no, scope, shapes, rows, bound)),
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            let ty = match contract_types::infer(subject, scope, shapes) {
                Ok(Ty::Option(inner)) => *inner,
                Err(e) if e.id == "type-branches" => return Some(e.span),
                _ => Ty::Unknown,
            };
            let mut inner = scope.clone();
            inner.push(vec![(var.clone(), Ref::Local(0), ty)]);
            let mut bound_some = bound.to_vec();
            bound_some.push(Bound {
                name: var,
                value: None,
                scope: scope.clone(),
                checked: Rc::new(Cell::new(None)),
            });
            computed_string_arm(some, &inner, shapes, rows, &bound_some)
                .or_else(|| computed_string_arm(none, scope, shapes, rows, bound))
        }
        Expr::Let {
            name, value, body, ..
        } => {
            let ty = contract_types::infer(value, scope, shapes).unwrap_or(Ty::Unknown);
            let mut inner = scope.clone();
            inner.push(vec![(name.clone(), Ref::Local(0), ty)]);
            let mut bound_body = bound.to_vec();
            bound_body.push(Bound {
                name,
                value: Some(value),
                scope: scope.clone(),
                checked: Rc::new(Cell::new(None)),
            });
            computed_string_arm(body, &inner, shapes, rows, &bound_body)
        }
        Expr::Ident(name, _) => {
            if let Some(index) = bound.iter().rposition(|binding| binding.name == name) {
                let binding = &bound[index];
                if let Some(value) = binding.value {
                    if let Some(result) = binding.checked.get() {
                        return result;
                    }
                    let result =
                        computed_string_arm(value, &binding.scope, shapes, rows, &bound[..index]);
                    binding.checked.set(Some(result));
                    return result;
                }
            }
            matches!(
                contract_types::infer(e, scope, shapes),
                Ok(Ty::String | Ty::Enum(_))
            )
            .then_some(e.span())
        }
        Expr::Typed(inner, _, _) => computed_string_arm(inner, scope, shapes, rows, bound),
        Expr::Str(s, _)
            if rows
                .iter()
                .all(|row| StyleValue::Text(s.as_str().into()).unsets(*row)) =>
        {
            None
        }
        _ => match contract_types::infer(e, scope, shapes) {
            Ok(Ty::String | Ty::Enum(_)) => Some(e.span()),
            Err(error) if error.id == "type-branches" => Some(error.span),
            _ => None,
        },
    }
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
        PropId::AccessibilityCurrent => (
            "aria-current",
            &["true", "false", "page", "step", "location", "date", "time"],
        ),
        _ => return None,
    })
}
pub(crate) fn aria_disabled_literal(prop: PropId, value: &Expr) -> Option<Expr> {
    match (prop, value) {
        (PropId::AccessibilityDisabled, Expr::Str(s, span)) if s == "true" || s == "false" => {
            Some(Expr::Bool(s == "true", *span))
        }
        _ => None,
    }
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
    if prop == PropId::Popover
        && matches!(ty, Ty::Option(inner) if matches!(**inner, Ty::String | Ty::Enum(_) | Ty::Unknown))
    {
        return Ok(());
    }
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
    if prop == PropId::StatusBarStyle
        && matches!(value, Expr::Str(s, _) if !matches!(s.as_str(), "light-content" | "dark-content" | "auto"))
    {
        return err(
            "lower-attr-value",
            "`status-bar-style` takes \"light-content\" (light text, for a dark surface), \"dark-content\" or \"auto\"",
            span,
        );
    }
    // @ref LLP 1075.003 §9.11 — a sheet's resting heights: UIKit's two named
    // detents, CSS's `fit-content`, a positive point height, or a percentage
    // of the sheet's maximum (§9.13).
    if prop == PropId::NavigationDetent {
        if let Expr::Str(s, _) = value {
            if let Some(word) = s
                .split(' ')
                .filter(|w| !w.is_empty())
                .find(|w| !vocab::detent_word(w))
            {
                return err(
                    "lower-attr-value",
                    format!(
                        "`navigationDetent` accepts {}; given \"{word}\"",
                        vocab::accepts(prop).unwrap()
                    ),
                    span,
                );
            }
            // UIKit's detents go from shortest to tallest; the content's
            // height changes, so only `large` is always above it.
            let words: Vec<&str> = s.split(' ').filter(|w| !w.is_empty()).collect();
            if words.contains(&"fit-content")
                && !matches!(words[..], ["fit-content"] | ["fit-content", "large"])
            {
                return err(
                    "lower-attr-value",
                    format!("`navigationDetent` accepts {}: a content height can pass any other detent, and UIKit's go from shortest to tallest; given \"{s}\"", vocab::accepts(prop).unwrap()),
                    span,
                );
            }
            if let Some(word) = vocab::detent_out_of_order(&words) {
                return err(
                    "lower-attr-value",
                    format!("`navigationDetent`'s heights go from shortest to tallest on every screen, as UIKit takes them; a point height and a share of the screen (`medium`, half, or a percentage) cross on some screen, so use one kind, each taller than the one before, with `large` last; given \"{word}\" in \"{s}\""),
                    span,
                );
            }
        }
    }
    // @ref LLP 1116.004 D1.2, LLP 1116.006 D2 — a stack's split on a wide window: off, or the list column's
    // width; or, on a box in the root route, the detail pane's placeholder.
    if prop == PropId::NavigationSplit
        && matches!(value, Expr::Str(s, _) if s != "none" && s != "placeholder" && !s.parse::<f64>().is_ok_and(|w| w.is_finite() && w > 0.0))
    {
        return err(
            "lower-attr-value",
            "`navigationSplit` takes \"none\" (the stack pushes on a wide window too) or the list column's width, a positive number, on the root route; or \"placeholder\" on a box in it, which the detail pane shows while nothing is selected",
            span,
        );
    }
    // A prop that takes one of a few words, and what its refusal says.
    let words: Option<(&[&str], &str)> = match prop {
        PropId::StatusBarAnimation => Some((&["none", "fade"], "`status-bar-animation` takes \"none\" or \"fade\"")),
        // @ref LLP 1115.000 D5.2 — no bar is something a route says, in one word.
        PropId::NavigationBar => Some((&["none", "auto", "transparent"], "`navigationBar` takes \"none\" (this route shows no navigation bar: a full-bleed screen that draws its own way back), \"transparent\" (the bar drawn clear at the top of the route's scroller, whose content starts under it: a full-bleed picture) or \"auto\" (the platform's bar, the default)")),
        // A sheet's grabber, said or left to the platform (LLP 1115 §2).
        PropId::NavigationGrabber => Some((&["visible", "hidden", "auto"], "`navigationGrabber` takes \"visible\", \"hidden\" or \"auto\" (the default: the platform's, a grabber on a sheet with several detents)")),
        PropId::FocusGuide => Some((&["auto"], "`focusGuide` takes \"auto\"")),
        PropId::Wakelock => Some((&["screen", "none"], "`wakelock` takes \"screen\" (the display stays awake while this element is displayed) or \"none\" (the default)")),
        PropId::Orientationlock => Some((&["portrait", "landscape", "portrait-primary", "portrait-secondary", "landscape-primary", "landscape-secondary", "natural", "any", "current", "none"], "`orientationlock` takes a Screen Orientation API lock type (\"portrait\", \"landscape\", \"portrait-primary\", \"portrait-secondary\", \"landscape-primary\", \"landscape-secondary\", \"natural\", \"any\"), \"current\" (the orientation the window has when the lock begins) or \"none\" (the default)")),
        PropId::AccessibilityLive => Some((&["off", "polite", "assertive"], "`aria-live` takes \"off\", \"polite\" or \"assertive\"")),
        _ => None,
    };
    if let (Some((allowed, why)), Expr::Str(s, _)) = (words, value) {
        if !allowed.contains(&s.as_str()) {
            return err("lower-attr-value", why, span);
        }
    }
    // @ref LLP 1053.000 D4 — a material is a name in the schema's table, or
    // `none`, no material; each string a computed value can take is one.
    if prop == PropId::BackgroundMaterial {
        let mut leaves = Vec::new();
        string_leaves(value, &mut leaves);
        for (name, at) in leaves {
            if name != "none" && exact_kernel::generated::material(name).is_none() {
                return err(
                    "lower-attr-value",
                    format!(
                        "`backgroundMaterial=\"{name}\"` is not a material; materials: {}, or `none` for no material",
                        exact_kernel::generated::MATERIALS.join(", ")
                    ),
                    if matches!(value, Expr::Str(..)) { span } else { at },
                );
            }
        }
    }
    if prop == PropId::ImageSource {
        symbols::check(value, span)?;
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
            | (tags::PropTy::Str, Ty::String | Ty::Enum(_))
            | (tags::PropTy::Bool, Ty::Bool)
            | (tags::PropTy::Int | tags::PropTy::Float, Ty::Number)
    );
    if !ok && prop == PropId::Datetime {
        return err(
            "lower-time-instant",
            "`datetime` is an instant in epoch milliseconds; parse a string with `date(…)`",
            span,
        );
    }
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
                    ": interpolate it in a template, `${…}`, or write `x.toString()`"
                } else {
                    ""
                }
            ),
            span,
        );
    }
    Ok(())
}
/// The string literals `e` can be: itself, or the arms of a `?:`, `match` or `let`.
fn string_leaves<'e>(e: &'e Expr, out: &mut Vec<(&'e str, Span)>) {
    match e {
        Expr::Str(s, span) => out.push((s, *span)),
        Expr::Ternary(_, a, b, _)
        | Expr::Match {
            some: a, none: b, ..
        } => {
            string_leaves(a, out);
            string_leaves(b, out);
        }
        Expr::Let { body: inner, .. } | Expr::Typed(inner, _, _) => string_leaves(inner, out),
        _ => {}
    }
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
    let scrolls = super::overflow::Overflow::of(attrs, "visible").may_scroll();
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
/// Why a multi-column or break value is refused (LLP 1093 §1), if it is.
fn multicol_value(rows: &[StyleId], v: &str) -> Option<&'static str> {
    let has = |row| rows.contains(&row);
    if has(StyleId::ColumnRuleStyle)
        && matches!(
            v,
            "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset"
        )
    {
        return Some("the native hosts paint only `none`, `hidden` and `solid` column rules; Chrome would paint this one and they would not");
    }
    if (has(StyleId::BreakBefore) || has(StyleId::BreakAfter))
        && matches!(
            v,
            "page"
                | "left"
                | "right"
                | "recto"
                | "verso"
                | "always"
                | "all"
                | "region"
                | "avoid-page"
                | "avoid-region"
        )
    {
        return Some("is a page or region break, and exact2 fragments only into columns (pages and regions stay out, LLP 1093 §5); use `column`, `avoid-column`, `avoid` or `auto`");
    }
    if has(StyleId::BreakInside) && matches!(v, "avoid-page" | "avoid-region") {
        return Some("is a page or region value, and exact2 fragments only into columns; use `avoid-column`, `avoid` or `auto`");
    }
    if has(StyleId::ColumnFill) && v == "balance-all" {
        return Some("balances every fragment of paged media, which exact2 does not have; use `balance` or `auto`");
    }
    if has(StyleId::ColumnWidth) && v.ends_with('%') {
        return Some("CSS `column-width` is a length or `auto`, never a percentage; use `column-count` to divide the width");
    }
    None
}
#[cfg(test)]
mod tests;

/// Whether `v` names an `--exact-*` property anywhere but as a content
/// owner's value read alone by `var()` (LLP 1123 D9).
fn host_private(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    lower.match_indices("--exact-").any(|(at, _)| {
        let read = lower[..at].trim_end().ends_with("var(");
        let name = lower[at..].split([')', ',', ' ']).next().unwrap_or("");
        !(read && exact_kernel::OwnerVar::from_name(name).is_some())
    })
}
