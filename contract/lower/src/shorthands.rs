//! CSS shorthands project literal choices into existing longhand bindings.
use super::*;

/// The shorthand's longhands, in component order.
pub(crate) fn rows(name: &str) -> &'static [StyleId] {
    use StyleId::*;
    match name {
        "border" => &[
            BorderWidthTop,
            BorderStyleTop,
            BorderColorTop,
            BorderWidthRight,
            BorderStyleRight,
            BorderColorRight,
            BorderWidthBottom,
            BorderStyleBottom,
            BorderColorBottom,
            BorderWidthLeft,
            BorderStyleLeft,
            BorderColorLeft,
        ],
        "border-top" => &[BorderWidthTop, BorderStyleTop, BorderColorTop],
        "border-right" => &[BorderWidthRight, BorderStyleRight, BorderColorRight],
        "border-bottom" => &[BorderWidthBottom, BorderStyleBottom, BorderColorBottom],
        "border-left" => &[BorderWidthLeft, BorderStyleLeft, BorderColorLeft],
        "text-decoration" => &[TextDecorationLine],
        _ => unreachable!("known shorthand"),
    }
}

pub(crate) fn component(value: &Expr, name: &str, index: usize) -> Result<Expr, LowerError> {
    let mut out = value.clone();
    match &mut out {
        Expr::Ternary(_, yes, no, _) => {
            **yes = component(yes, name, index)?;
            **no = component(no, name, index)?;
        }
        Expr::Match { some, none, .. } => {
            **some = component(some, name, index)?;
            **none = component(none, name, index)?;
        }
        Expr::Let { body, .. } => **body = component(body, name, index)?,
        Expr::Str(text, span) => {
            if name == "text-decoration" { out = Expr::Str(decoration(text, *span)?, *span); }
            else {
                let part = border(text, *span)?[index % 3].clone();
                out = if index.is_multiple_of(3) {
                    let pt = part.ends_with("pt");
                    let value: f64 = part.strip_suffix("px").or_else(|| part.strip_suffix("pt")).unwrap_or(&part).parse().unwrap();
                    Expr::Number(value * if pt { 4.0 / 3.0 } else { 1.0 }, *span)
                } else { Expr::Str(part, *span) };
            }
        }
        // A number where CSS writes a shorthand string (authoring bench: `border=0`, three builders).
        Expr::Number(n, span) => return err("lower-css-shorthand", format!("`{name}={n}`: a CSS shorthand is a string, so write `{name}=\"{n}\"`"), *span),
        _ => return err("lower-css-shorthand", format!("`{name}` takes a literal CSS shorthand or a choice of literals; computed strings cannot be split into longhands"), value.span()),
    }
    Ok(out)
}

fn words(text: &str) -> Vec<&str> {
    let mut depth = 0;
    text.split(|c: char| {
        if c == '(' {
            depth += 1;
        }
        if c == ')' {
            depth -= 1;
        }
        c.is_ascii_whitespace() && depth == 0
    })
    .filter(|s| !s.is_empty())
    .collect()
}

fn border(text: &str, span: Span) -> Result<[String; 3], LowerError> {
    let mut width = None;
    let mut style = None;
    let mut color = None;
    for word in words(text) {
        let lower = word.to_ascii_lowercase();
        if matches!(lower.as_str(), "none" | "hidden" | "solid" | "inset") {
            if style.replace(lower).is_some() {
                return err(
                    "lower-css-shorthand",
                    "border has more than one line style",
                    span,
                );
            }
        } else if matches!(
            lower.as_str(),
            "dotted" | "dashed" | "double" | "groove" | "ridge" | "outset"
        ) {
            return err("lower-css-shorthand", format!("CSS border style `{word}` is not implemented by native painters; supported styles are none, hidden, solid and inset"), span);
        } else if matches!(lower.as_str(), "thin" | "medium" | "thick")
            || word
                .strip_suffix("px")
                .or_else(|| word.strip_suffix("pt"))
                .unwrap_or(word)
                .parse::<f32>()
                .is_ok_and(|x| {
                    x.is_finite() && x >= 0.0 && (x == 0.0 || word.ends_with(['x', 't']))
                })
        {
            let value = match lower.as_str() {
                "thin" => "1px",
                "medium" => "3px",
                "thick" => "5px",
                _ => word,
            };
            if width.replace(value.into()).is_some() {
                return err(
                    "lower-css-shorthand",
                    "border has more than one line width",
                    span,
                );
            }
        } else {
            let mut probe = exact_kernel::StyleProps::default();
            if probe
                .set_dynamic(
                    StyleId::BorderColorTop,
                    &exact_kernel::StyleValue::Text(word.into()),
                )
                .is_err()
            {
                return err("lower-css-shorthand", format!("`{word}` is not an admitted border width, style or color; widths are nonnegative px/pt lengths or thin/medium/thick"), span);
            }
            if color.replace(word.into()).is_some() {
                return err(
                    "lower-css-shorthand",
                    "border has more than one color",
                    span,
                );
            }
        }
    }
    if text.trim().is_empty() {
        return err(
            "lower-css-shorthand",
            "border needs a width, style or color",
            span,
        );
    }
    Ok([
        width.unwrap_or("3px".into()),
        style.unwrap_or("none".into()),
        color.unwrap_or("currentcolor".into()),
    ])
}

fn decoration(text: &str, span: Span) -> Result<String, LowerError> {
    let tokens = words(text);
    if tokens.is_empty() {
        return err(
            "lower-css-shorthand",
            "text-decoration needs a CSS component",
            span,
        );
    }
    let mut none = false;
    let mut underline = false;
    let mut strike = false;
    for word in tokens {
        match word.to_ascii_lowercase().as_str() {
            "none" if !none && !underline && !strike => none = true,
            "underline" if !underline && !none => underline = true,
            "line-through" if !strike && !none => strike = true,
            "solid" | "currentcolor" | "auto" => {},
            _ => return err("lower-css-shorthand", format!("CSS text-decoration component `{word}` is not implemented; native text painters support underline and line-through with solid currentcolor at the platform's default thickness"), span),
        }
    }
    match (underline, strike) {
        (true, true) => Ok("underline-line-through".into()),
        (true, false) => Ok("underline".into()),
        (false, true) => Ok("line-through".into()),
        _ => Ok("none".into()),
    }
}

impl Lowerer<'_> {
    pub(super) fn bind_shorthand(
        &mut self,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        font: &[FontUse],
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        for (index, row) in rows(&a.name).iter().copied().enumerate() {
            let value = component(&a.value, &a.name, index)?;
            let component = Attr { value, ..a.clone() };
            let (code, ty) = self.typed_code(&component.value, scope, locals)?;
            values::check_style_value(&component, &[row], &ty, font)?;
            bindings.push(BindingsRow {
                kind: BindingKind::Style,
                id: row as u16,
                expr: code,
            });
        }
        Ok(())
    }
}

/// Native interaction limits must also cover computed expressions, not only literals.
pub(crate) fn portable_literal(value: &Expr, name: &str) -> Result<(), LowerError> {
    match value {
        Expr::Str(_, _) => Ok(()),
        Expr::Ternary(_, yes, no, _) | Expr::Match { some: yes, none: no, .. } => { portable_literal(yes, name)?; portable_literal(no, name) },
        Expr::Let { body, .. } => portable_literal(body, name),
        _ => err("lower-css-interaction", format!("`{name}` takes a CSS keyword literal or a choice of literals so native support can be checked; computed strings cannot be checked"), value.span()),
    }
}
