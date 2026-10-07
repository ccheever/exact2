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
        // @ref LLP 1093 §1
        "columns" => &[ColumnWidth, ColumnCount],
        "column-rule" => &[ColumnRuleWidth, ColumnRuleStyle, ColumnRuleColor],
        "column-count" => &[ColumnCount],
        "column-rule-width" => &[ColumnRuleWidth],
        _ => unreachable!("known shorthand"),
    }
}

pub(crate) fn component(value: &Expr, name: &str, index: usize) -> Result<Expr, LowerError> {
    let mut out = value.clone();
    // A longhand whose keywords the row does not hold: a literal keyword
    // becomes its number, anything else is the row's own value.
    let longhand = matches!(name, "column-count" | "column-rule-width");
    match &mut out {
        Expr::Str(text, span) if longhand => out = columns_longhand(name, text, *span)?,
        Expr::Number(n, span) if name == "column-count" && *n < 1.0 => {
            return err("lower-attr-value", "`column-count` is a positive integer or `auto`", *span);
        }
        _ if longhand && !matches!(value, Expr::Ternary(..) | Expr::Match { .. } | Expr::Let { .. }) => {}
        Expr::Str(text, span) if name == "columns" => {
            out = columns(text, *span)?[index].clone();
        }
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
                let part = border(name, text, *span)?[index % 3].clone();
                out = if index.is_multiple_of(3) {
                    let pt = part.ends_with("pt");
                    let value: f64 = part.strip_suffix("px").or_else(|| part.strip_suffix("pt")).unwrap_or(&part).parse().unwrap();
                    Expr::Number(value * if pt { 4.0 / 3.0 } else { 1.0 }, *span)
                } else { Expr::Str(part, *span) };
            }
        }
        // A number where CSS writes a shorthand string (authoring bench: `border=0`, three builders).
        // Names the literal, not the attribute: it may be one arm of a choice.
        Expr::Number(n, span) => {
            let width = name.starts_with("border") && *n >= 0.0 && (*n as f32).is_finite();
            let like = match (width, *n == 0.0) {
                (true, true) => ", so write `\"0\"`".to_string(),
                (true, false) => format!(", as CSS writes it: `\"{n}px solid #ccc\"`"),
                _ => ", as CSS writes it".to_string(),
            };
            return err("lower-css-shorthand", format!("`{name}`: `{n}` is a number, and a CSS shorthand is a string{like}"), *span);
        }
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

fn border(name: &str, text: &str, span: Span) -> Result<[String; 3], LowerError> {
    let mut width = None;
    let mut style = None;
    let mut color = None;
    for word in words(text) {
        let lower = word.to_ascii_lowercase();
        // A border paints `inset`; a column rule does not (LLP 1093 §1).
        let rule = name == "column-rule";
        if matches!(lower.as_str(), "none" | "hidden" | "solid") || (lower == "inset" && !rule) {
            if style.replace(lower).is_some() {
                return err(
                    "lower-css-shorthand",
                    format!("`{name}` has more than one line style"),
                    span,
                );
            }
        } else if matches!(
            lower.as_str(),
            "dotted" | "dashed" | "double" | "groove" | "ridge" | "inset" | "outset"
        ) {
            let supported = if rule {
                "none, hidden and solid"
            } else {
                "none, hidden, solid and inset"
            };
            return err("lower-css-shorthand", format!("CSS line style `{word}` in `{name}` is not implemented by native painters; supported styles are {supported}"), span);
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
                    format!("`{name}` has more than one line width"),
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
                return err("lower-css-shorthand", format!("`{word}` is not an admitted `{name}` width, style or color; widths are nonnegative px/pt lengths or thin/medium/thick{}", crate::values::role_hint(word).unwrap_or_default()), span);
            }
            if color.replace(word.into()).is_some() {
                return err(
                    "lower-css-shorthand",
                    format!("`{name}` has more than one color"),
                    span,
                );
            }
        }
    }
    if text.trim().is_empty() {
        return err(
            "lower-css-shorthand",
            format!("`{name}` needs a width, style or color"),
            span,
        );
    }
    Ok([
        width.unwrap_or("3px".into()),
        style.unwrap_or("none".into()),
        color.unwrap_or("currentcolor".into()),
    ])
}

// `columns`: `<column-width> || <column-count>`, each `auto` when not given.
fn columns(text: &str, span: Span) -> Result<[Expr; 2], LowerError> {
    let mut width = None;
    let mut count = None;
    let mut autos = 0;
    let parts = words(text);
    for word in &parts {
        if *word == "auto" {
            autos += 1;
        } else if let Ok(n) = word.parse::<u16>() {
            if n == 0 || count.replace(n).is_some() {
                return err(
                    "lower-css-shorthand",
                    "`columns` takes one positive column count",
                    span,
                );
            }
        } else if !word.ends_with('%') && width.is_none() {
            width = Some(
                match word.strip_suffix("px").unwrap_or(word).parse::<f64>() {
                    Ok(n) if n >= 0.0 => Expr::Number(n, span),
                    _ => Expr::Str((*word).into(), span),
                },
            );
        } else {
            return err("lower-css-shorthand", format!("`{word}` is not a column width (a length) or count (a positive integer) for `columns`"), span);
        }
    }
    if parts.is_empty()
        || parts.len() > 2
        || autos + width.is_some() as usize + count.is_some() as usize != parts.len()
    {
        return err(
            "lower-css-shorthand",
            "`columns` is `<column-width> || <column-count>`, each `auto` when left out",
            span,
        );
    }
    Ok([
        width.unwrap_or_else(|| Expr::Str("auto".into(), span)),
        Expr::Number(f64::from(count.unwrap_or(0)), span),
    ])
}

// `column-count` or `column-rule-width` as a literal: its row's number.
fn columns_longhand(name: &str, text: &str, span: Span) -> Result<Expr, LowerError> {
    let word = text.trim();
    let n = match (name, word) {
        ("column-count", "auto") => Some(0.0),
        ("column-count", _) => word.parse::<u16>().ok().filter(|n| *n > 0).map(f64::from),
        (_, "thin") => Some(1.0),
        (_, "medium") => Some(3.0),
        (_, "thick") => Some(5.0),
        _ => word
            .strip_suffix("px")
            .unwrap_or(word)
            .parse::<f64>()
            .ok()
            .filter(|n| *n >= 0.0),
    };
    match n {
        Some(n) => Ok(Expr::Number(n, span)),
        None if name == "column-count" => err(
            "lower-attr-value",
            format!("`column-count=\"{text}\"`: a positive integer or `auto`"),
            span,
        ),
        None => err(
            "lower-attr-value",
            format!(
                "`column-rule-width=\"{text}\"`: a length in px, or `thin`, `medium` or `thick`"
            ),
            span,
        ),
    }
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
            "underline-line-through" => return err("lower-css-shorthand", "`underline-line-through` is spelled `underline line-through`, CSS's two keywords (LLP 1081)", span),
            _ => return err("lower-css-shorthand", format!("CSS text-decoration component `{word}` is not implemented; native text painters support underline and line-through with solid currentcolor at the platform's default thickness{}", crate::values::role_hint(word).unwrap_or_default()), span),
        }
    }
    match (underline, strike) {
        (true, true) => Ok("underline line-through".into()),
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
