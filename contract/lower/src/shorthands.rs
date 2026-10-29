//! CSS shorthands project literal choices into existing longhand bindings.
use super::*;
use contract_syntax::TemplatePart;
use exact_kernel::style::sides as box_sides;

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
        // LLP 1115.000 D12.2: only what it writes; the style's weight and
        // leading are the kernel's, per platform (`fontShorthand`).
        "font" => &[FontStyle, FontSize],
        // CSS Inline Layout 3 §4: one row holds both edges (`cap-alphabetic`
        // for CSS's `cap alphabetic`); `text-box` is trim, then edge.
        "text-box-edge" => &[TextBoxEdge],
        "text-box" => &[TextBoxTrim, TextBoxEdge],
        // CSS Inline Layout 3 §4: `alignment-baseline` and `baseline-shift`.
        "vertical-align" => &[VerticalAlign, BaselineShift],
        _ => unreachable!("known shorthand"),
    }
}

pub(crate) fn component(value: &Expr, name: &str, index: usize) -> Result<Expr, LowerError> {
    let mut out = value.clone();
    // A longhand whose keywords the row does not hold: a literal keyword
    // becomes its number, anything else is the row's own value.
    let longhand = matches!(name, "column-count" | "column-rule-width");
    match &mut out {
        // An unset arm of a conditional class clears each expanded longhand.
        Expr::None(_) => {},
        Expr::Str(text, span) if longhand => out = columns_longhand(name, text, *span)?,
        Expr::Str(text, span) if name == "vertical-align" => out = vertical_align(text, *span)?[index].clone(),
        Expr::Number(n, span) if name == "vertical-align" => {
            out = if index == 0 { Expr::Str("baseline".into(), *span) } else { Expr::Number(*n, *span) };
        }
        Expr::Str(text, span) if name == "font" => out = font(text, *span)?[index].clone(),
        Expr::Number(_, span) if name == "font" => return font("", *span).map(|_| out),
        Expr::Number(n, span) if name == "column-count" && *n < 1.0 => {
            return err("lower-attr-value", "`column-count` is a positive integer or `auto`", *span);
        }
        _ if longhand && !matches!(value, Expr::Ternary(..) | Expr::Match { .. } | Expr::Let { .. }) => {}
        Expr::Str(text, span) if name == "columns" => {
            out = columns(text, *span)?[index].clone();
        }
        Expr::Str(text, span) if name == "text-box-edge" || name == "text-box" => {
            // `text-box-edge`'s one row is the shorthand's second.
            let i = if name == "text-box-edge" { 1 } else { index };
            out = Expr::Str(text_box(name, text, *span)?[i].clone(), *span);
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
                // A named width (`-exact-hairline`) stays its name: the kernel resolves it.
                out = if index.is_multiple_of(3) && exact_kernel::style::relative::named_length(&part).is_none() {
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
                (true, false) => format!(", as CSS writes it: `\"{n}px solid -exact-separator\"`"),
                _ => ", as CSS writes it".to_string(),
            };
            return err("lower-css-shorthand", format!("`{name}`: `{n}` is a number, and a CSS shorthand is a string{like}"), *span);
        }
        _ => return err("lower-css-shorthand", format!("`{name}` takes a literal CSS shorthand or a choice of literals; computed strings cannot be split into longhands"), value.span()),
    }
    Ok(out)
}

/// `vertical-align`'s two rows (LLP 1011.002 D3): a keyword and no shift,
/// or `baseline` raised by a length.
fn vertical_align(text: &str, span: Span) -> Result<[Expr; 2], LowerError> {
    let word = text.trim().to_ascii_lowercase();
    if matches!(
        word.as_str(),
        "baseline" | "middle" | "text-top" | "text-bottom"
    ) {
        return Ok([Expr::Str(word, span), Expr::Number(0.0, span)]);
    }
    if matches!(word.as_str(), "sub" | "super" | "top" | "bottom" | "center") {
        return err(
            "lower-css-shorthand",
            format!("`vertical-align: {word}` is not implemented; an image in text takes `baseline`, `middle`, `text-top`, `text-bottom` or a length"),
            span,
        );
    }
    let mut probe = exact_kernel::StyleProps::default();
    if word.contains('%')
        || probe
            .set_dynamic(
                StyleId::BaselineShift,
                &exact_kernel::StyleValue::Text(word.as_str().into()),
            )
            .is_err()
    {
        return err(
            "lower-css-shorthand",
            format!("`vertical-align` takes `baseline`, `middle`, `text-top`, `text-bottom` or a length such as `-0.2em`, not `{text}`"),
            span,
        );
    }
    Ok([Expr::Str("baseline".into(), span), Expr::Str(word, span)])
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
        if matches!(lower.as_str(), "none" | "hidden" | "solid")
            || (!rule && matches!(lower.as_str(), "inset" | "dotted" | "dashed"))
        {
            if style.replace(lower).is_some() {
                return err(
                    "lower-css-shorthand",
                    format!("`{name}` has more than one line style"),
                    span,
                );
            }
        } else if matches!(
            lower.as_str(),
            "double" | "groove" | "ridge" | "outset" | "dotted" | "dashed" | "inset"
        ) {
            let supported = if rule {
                "none, hidden and solid"
            } else {
                "none, hidden, solid, inset, dotted and dashed"
            };
            return err("lower-css-shorthand", format!("CSS line style `{word}` in `{name}` is not implemented by native painters; supported styles are {supported}"), span);
        } else if matches!(lower.as_str(), "thin" | "medium" | "thick")
            // @ref LLP 1115.000 D10.3 — one device pixel, by name.
            || exact_kernel::style::relative::named_length(word).is_some()
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
                // @ref LLP 1116.014 D1 — a role meant is named first.
                if let Some(edit) = crate::edits::role_hint(word, true) {
                    let edit = edit.shifted(text.find(word).unwrap_or(0));
                    return err("lower-css-shorthand", edit.lead(name, text), span);
                }
                // A dotted word no token has, in a colour row's words (LLP
                // 1121 stage 12 B3, QUEUE's item); a base name or a length
                // token is a length, which the refusal below names.
                use exact_kernel::style::{scale, theme};
                let length = scale::is_base(word)
                    || crate::theme::measure_named(word) == Some(exact_tamber::Kind::Length);
                if crate::theme::scan::dotted_name(word)
                    && theme::bound()
                    && !theme::names(word)
                    && !length
                {
                    return err("tamber-unknown-token", format!("`{name}=\"{text}\"`: `{word}` is no token of this app's design and no role"), span);
                }
                return err("lower-css-shorthand", format!("`{word}` is not an admitted `{name}` width, style or color; widths are nonnegative px/pt lengths, `-exact-hairline` or thin/medium/thick"), span);
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

/// `font`: a platform text style only (LLP 1115 D3): a `normal` style and
/// the style's size, which the kernel reads from the platform's ramp at the
/// root font size. Its weight and leading are the style's on each platform
/// (LLP 1115.000 D12.2): the node is marked `fontShorthand`, so the kernel
/// gives them as the shorthand resets them, beating an inherited value. The
/// family stays what it was.
fn font(text: &str, span: Span) -> Result<[Expr; 2], LowerError> {
    use exact_kernel::{style::relative::text_style, TEXT_STYLES};
    let Some(id) = text_style(text) else {
        let names: Vec<String> = TEXT_STYLES
            .iter()
            .map(|s| format!("`-exact-{}`", s.name))
            .collect();
        return err(
            "lower-css-shorthand",
            format!("`font` takes a platform text style ({}, or WebKit's `-apple-system-*` spelling); anything else is its longhands, `font-size`, `font-weight`, `font-style`, `line-height`", names.join(", ")),
            span,
        );
    };
    let style = &TEXT_STYLES[usize::from(id)];
    Ok([
        Expr::Str("normal".into(), span),
        Expr::Str(format!("-exact-{}", style.name), span),
    ])
}

/// `text-box` (`normal`, or a trim and an edge in either order, each
/// optional, Inline Layout 3 §4) or `text-box-edge` (`auto`, `text`, or an
/// over edge then an under edge): the trim and the edge as their rows spell
/// them, by CSS's grammar. A lone `cap` or `ex` is not an edge (CSS needs its
/// under edge: `cap text`). A shorthand with no trim trims both ends; with no
/// edge, its edge is `auto`.
fn text_box(name: &str, text: &str, span: Span) -> Result<[String; 2], LowerError> {
    let refuse = |why: String| {
        err(
            "lower-css-shorthand",
            format!("`{name}`: {why}; write e.g. `trim-both cap alphabetic`"),
            span,
        )
    };
    let words: Vec<String> = words(text).iter().map(|w| w.to_ascii_lowercase()).collect();
    if words.is_empty() {
        return refuse("an empty value".into());
    }
    if name == "text-box" && words.len() == 1 && words[0] == "normal" {
        return Ok(["none".into(), "auto".into()]);
    }
    if let Some(w) = words.iter().find(|w| w.starts_with("ideographic")) {
        return err(
            "lower-css-shorthand",
            format!("`{name}`: `{w}` edges are not implemented; use text, cap, ex or alphabetic"),
            span,
        );
    }
    let is_trim = |w: &str| matches!(w, "none" | "trim-start" | "trim-end" | "trim-both");
    let mut trim = None;
    let mut edge: Vec<&str> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        if name == "text-box" && is_trim(w) {
            // The trim stands before or after the edge's words, never between.
            if trim.is_some() || (!edge.is_empty() && i < words.len() - 1) {
                return refuse(format!("`{w}` is out of place"));
            }
            trim = Some(w.clone());
        } else {
            edge.push(w);
        }
    }
    let edge = match edge.as_slice() {
        [] => "auto".to_string(),
        ["auto"] => "auto".to_string(),
        ["text"] => "text".to_string(),
        [over @ ("text" | "cap" | "ex"), under @ ("text" | "alphabetic")] => {
            if *under == "alphabetic" {
                format!("{over}-alphabetic")
            } else {
                over.to_string()
            }
        }
        [one @ ("cap" | "ex")] => {
            return refuse(format!(
                "`{one}` needs its under edge (`{one} text` or `{one} alphabetic`)"
            ))
        }
        other => {
            return refuse(format!(
                "`{}` is not a text-box value here",
                other.join(" ")
            ))
        }
    };
    Ok([
        trim.unwrap_or_else(|| {
            if name == "text-box" {
                "trim-both".into()
            } else {
                "none".into()
            }
        }),
        edge,
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
            "underline-line-through" => return err("lower-css-shorthand", "`underline-line-through` is spelled `underline line-through`, CSS's two keywords (LLP 1081)", span),
            _ => return err("lower-css-shorthand", format!("CSS text-decoration component `{word}` is not implemented; native text painters support underline and line-through with solid currentcolor at the platform's default thickness{}", crate::edits::role_hint(word, false).map(|e| format!("; {}", e.clause())).unwrap_or_default()), span),
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
        // Type tokens write only their longhands (stage 6 C).
        if a.name == "font" && self.type_font(a, scope, locals, font, bindings)? {
            return Ok(());
        }
        for (index, row) in rows(&a.name).iter().copied().enumerate() {
            let value = component(&a.value, &a.name, index)?;
            let component = Attr { value, ..a.clone() };
            let (code, ty) = self.typed_code(&component.value, scope, locals)?;
            values::check_style_value(&component, &[row], &ty, font, scope, &self.types.shapes)?;
            bindings.push(crate::style_row(row, code, a.written));
        }
        if a.name == "font" {
            let marked = self.b.constant(&Value::Bool(true));
            bindings.push(crate::prop_row(
                exact_kernel::PropId::FontShorthand,
                marked,
                crate::IMPLIED,
            ));
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

/// CSS `text-box-trim` on a block cuts its first and last formatted lines,
/// which the native hosts measure on the `text` itself: the trim (and the
/// `text-box` shorthand) is a `text`'s, and its edge, inherited, may sit
/// anywhere.
pub(crate) fn text_box_trim_refusal(name: &str, tag: &str, module: bool) -> Option<String> {
    (matches!(name, "text-box-trim" | "text-box") && tag != "text" && !module).then(|| {
        format!("`{name}` trims a `text`, not `{tag}`; write it on the `text` inside (`text-box-edge` may stay here: it is inherited)")
    })
}

/// A trim Exact cannot draw yet, refused rather than drawn wrong (Inline
/// Layout 3 §4.2): on an inline run, whose own box is cut to its edges
/// rather than its paragraph's first and last lines; and on a paragraph
/// that is itself a multi-column container, where CSS trims every column's
/// first and last lines. A paragraph merely split across an ancestor's
/// columns is trimmed at its outer ends, as `box-decoration-break: slice`
/// has it, and stays admitted.
pub(crate) fn check_text_box(
    tag: &str,
    parent_tag: Option<&str>,
    attrs: &[Attr],
) -> Result<(), LowerError> {
    // The effective trim, columns and display, as the declarations in order leave them: a literal, or each
    // literal a choice may take, is read for what it projects; any other bound value may trim, make columns or
    // lay the text out as flex or grid. Any column count or width other than `auto` makes a multi-column
    // container, a count of one included (its overflow makes more columns).
    let (mut trim, mut count, mut width, mut display) = (None, None, None, None);
    for a in attrs {
        let values = literals(&a.value);
        // `unset` clears a row to its initial value, as a choice that declares nothing does
        let may = |does: &dyn Fn(&str, Span) -> bool| {
            values.as_ref().is_none_or(|v| {
                v.iter()
                    .any(|&(s, span)| !s.trim().eq_ignore_ascii_case("unset") && does(s, span))
            })
        };
        let number = |w: &str| w.parse::<f64>().is_ok();
        let words = |s: &str, count: bool| {
            s.split_whitespace()
                .any(|w| w != "auto" && number(w) == count)
        };
        match a.name.as_str() {
            "text-box" => {
                trim =
                    may(&|s, span| text_box("text-box", s, span).map_or(true, |[t, _]| t != "none"))
                        .then_some(a)
            }
            "text-box-trim" => trim = may(&|s, _| s.trim() != "none").then_some(a),
            "columns" => {
                count = may(&|s, _| words(s, true)).then_some(a);
                width = may(&|s, _| words(s, false)).then_some(a);
            }
            "column-count" => count = may(&|s, _| s.trim() != "auto").then_some(a),
            "column-width" => width = may(&|s, _| s.trim() != "auto").then_some(a),
            "display" => {
                display =
                    may(&|s, _| matches!(s.trim(), "flex" | "grid" | "inline-flex" | "inline-grid"))
                        .then_some(a)
            }
            _ => {}
        }
    }
    let Some(trim) = trim.filter(|_| tag == "text") else {
        return Ok(());
    };
    if parent_tag == Some("text") {
        return err(
            "lower-attr-tag",
            format!("`{}` on an inline `text` run: Exact trims a paragraph, not a run's own box; set it on the paragraph", trim.name),
            trim.span,
        );
    }
    if let Some(c) = count.or(width) {
        return err(
            "lower-attr-tag",
            format!(
                "`{}` with `{}`: CSS trims the first and last lines of every column, which Exact does not draw yet; put the columns on a `view` around the `text`",
                trim.name, c.name
            ),
            trim.span,
        );
    }
    if let Some(d) = display {
        return err(
            "lower-attr-tag",
            format!(
                "`{}` with `{}`: CSS does not trim a flex or grid container's text, which Exact's native hosts would trim; put the flex or grid on a `view` around the `text`",
                trim.name, d.name
            ),
            trim.span,
        );
    }
    Ok(())
}

/// Each literal a value may take, through `?:` choices, with its span; none
/// when a choice is not a literal. A choice that declares nothing (a
/// conditional class's absent row) takes the row's initial value, which
/// trims nothing, makes no columns and lays nothing out as flex or grid.
fn literals(e: &Expr) -> Option<Vec<(&str, Span)>> {
    match e {
        Expr::Str(s, span) => Some(vec![(s.as_str(), *span)]),
        Expr::None(_) => Some(Vec::new()),
        Expr::Ternary(_, yes, no, _) => Some([literals(yes)?, literals(no)?].concat()),
        _ => None,
    }
}

/// CSS's two-value shorthands, each value its own row in this order, one
/// value both rows': `gap` (row, column; CSS Box Alignment 3 §8.3),
/// `overflow` (x, y; CSS Overflow 3 §3.1) and `overscroll-behavior` (x, y;
/// CSS Overscroll Behavior 1 §2).
pub(crate) const PAIRS: [[StyleId; 2]; 3] = [
    [StyleId::RowGap, StyleId::ColumnGap],
    [StyleId::OverflowX, StyleId::OverflowY],
    [StyleId::OverscrollBehaviorX, StyleId::OverscrollBehaviorY],
];
/// The longhands a shorthand's values go to, as an author writes them.
pub(crate) fn longhands(name: &str) -> Vec<String> {
    let sides = ["top", "right", "bottom", "left"];
    match name {
        "border-radius" => ["top-left", "top-right", "bottom-right", "bottom-left"]
            .map(|c| format!("border-{c}-radius"))
            .to_vec(),
        "inset" => sides.map(String::from).to_vec(),
        "gap" => vec!["row-gap".into(), "column-gap".into()],
        "overflow" | "overscroll-behavior" => vec![format!("{name}-x"), format!("{name}-y")],
        n => match n.strip_prefix("border-") {
            Some(part) => sides.map(|s| format!("border-{s}-{part}")).to_vec(),
            None => sides.map(|s| format!("{n}-{s}")).to_vec(),
        },
    }
}

/// Project an authored style into the same rows and expressions lowering binds.
/// Compiler profiles use this to check shorthand values without a second CSS parser.
pub fn style_values(name: &str, value: &Expr) -> Result<Vec<(StyleId, Expr)>, LowerError> {
    match tags::attr_valued(name, value) {
        Some(tags::AttrTarget::Styles(rows)) => {
            let split = if rows.len() > 1 {
                sides(name, value)?
            } else {
                None
            };
            Ok(rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    (
                        *row,
                        split
                            .as_ref()
                            .map_or_else(|| value.clone(), |v| v[i].clone()),
                    )
                })
                .collect())
        }
        Some(tags::AttrTarget::Shorthand) => super::shorthands::rows(name)
            .iter()
            .enumerate()
            .map(|(i, row)| Ok((*row, super::shorthands::component(value, name, i)?)))
            .collect(),
        _ => Ok(Vec::new()),
    }
}

/// A computed value that builds a list for a box shorthand (a
/// concatenation, or a `fn` returning one): no host splits a list at run
/// time, since an overriding longhand, an `env()` and the rows' own
/// grammars would each split it differently (LLP 1001 §1). Refused, naming
/// the longhands. A literal (or a literal arm) and a template
/// (`` `${pad}px 0` ``) are split in [`sides`]; a computed single value
/// (`padding=pad`) is every side's, and a computed list that reaches a row
/// is refused there.
pub(crate) fn computed_list<'e>(
    name: &str,
    value: &'e Expr,
    body: &dyn Fn(&str) -> Option<&'e Expr>,
) -> Result<(), LowerError> {
    /// The `let`s in scope: each name and its value.
    type Env<'e> = Vec<(&'e str, &'e Expr)>;
    /// The texts a computed value can spell, one per way its `?:`, `match`
    /// and the `fn`s it calls can go, a computed part as one word; `None`
    /// when they are too many or the calls too deep to tell.
    fn texts<'e>(
        e: &'e Expr,
        body: &dyn Fn(&str) -> Option<&'e Expr>,
        env: &mut Env<'e>,
        depth: u8,
    ) -> Option<Vec<String>> {
        const MOST: usize = 64;
        // Every text of `a` followed by every text of `b`.
        let join = |a: Vec<String>, b: Vec<String>| -> Option<Vec<String>> {
            (a.len() * b.len() <= MOST).then(|| {
                a.iter()
                    .flat_map(|x| b.iter().map(move |y| format!("{x}{y}")))
                    .collect()
            })
        };
        let either = |mut a: Vec<String>, b: Vec<String>| -> Option<Vec<String>> {
            a.extend(b);
            (a.len() <= MOST).then_some(a)
        };
        Some(match e {
            Expr::Str(s, _) => vec![s.clone()],
            Expr::Template(parts, _) => {
                let mut out = vec![String::new()];
                for p in parts {
                    let part = match p {
                        TemplatePart::Text(t) => vec![t.clone()],
                        TemplatePart::Expr(e) => texts(e, body, env, depth)?,
                    };
                    out = join(out, part)?;
                }
                out
            }
            Expr::Binary(contract_syntax::BinOp::Add, l, r, _) => {
                join(texts(l, body, env, depth)?, texts(r, body, env, depth)?)?
            }
            Expr::Ternary(_, yes, no, _) => {
                either(texts(yes, body, env, depth)?, texts(no, body, env, depth)?)?
            }
            Expr::Match { some, none, .. } => either(
                texts(some, body, env, depth)?,
                texts(none, body, env, depth)?,
            )?,
            Expr::Let {
                name,
                value,
                body: b,
                ..
            } => {
                env.push((name, value));
                let out = texts(b, body, env, depth);
                env.pop();
                out?
            }
            Expr::Ident(name, _) => match env.iter().rev().find(|(n, _)| n == name) {
                Some(&(_, value)) => texts(value, body, env, depth)?,
                None => vec!["x".into()],
            },
            Expr::Call(f, _, _) => match body(f) {
                // A `fn`'s parameters are computed parts; its `let`s its own.
                Some(b) if depth < 8 => texts(b, body, &mut Vec::new(), depth + 1)?,
                Some(_) => return None,
                None => vec!["x".into()],
            },
            _ => vec!["x".into()],
        })
    }
    // Whether a computed value may hold more than `most` values, or cannot
    // be told not to.
    fn more<'e>(
        e: &'e Expr,
        body: &dyn Fn(&str) -> Option<&'e Expr>,
        env: &mut Env<'e>,
        most: usize,
    ) -> bool {
        texts(e, body, env, 0)
            .is_none_or(|all| all.iter().any(|t| box_sides::values(t).len() > most))
    }
    // Where the value builds a list that is not split at compile time: a
    // literal arm and a template's own white space are ([`sides`]); a
    // `fn`'s result, a `let`'s value and a concatenation are not.
    fn lists<'e>(
        e: &'e Expr,
        body: &dyn Fn(&str) -> Option<&'e Expr>,
        env: &mut Env<'e>,
    ) -> Option<Span> {
        match e {
            Expr::Str(..) => None,
            // Refused only where an interpolation adds a value: its white
            // space outside the template's parentheses.
            Expr::Template(parts, at) => {
                let words = template_words(parts, *at).map_or(1, |w| w.len());
                more(e, body, env, words).then(|| e.span())
            }
            Expr::Ternary(_, yes, no, _) => lists(yes, body, env).or_else(|| lists(no, body, env)),
            Expr::Match { some, none, .. } => {
                lists(some, body, env).or_else(|| lists(none, body, env))
            }
            Expr::Let {
                name,
                value,
                body: b,
                ..
            } => {
                env.push((name, value));
                let out = lists(b, body, env);
                env.pop();
                out
            }
            e => more(e, body, env, 1).then(|| e.span()),
        }
    }
    let Some(span) = lists(value, body, &mut Vec::new()) else {
        return Ok(());
    };
    let longhands: Vec<String> = longhands(name).iter().map(|l| format!("`{l}`")).collect();
    err(
        "lower-css-shorthand",
        format!(
            "`{name}` is computed here as a list of values, which no host splits at run time; write {} (each may be computed), or a literal `{name}`",
            longhands.join("/")
        ),
        span,
    )
}

/// A box shorthand (`padding: <length>{1,4}`, `border-color: <color>{1,4}`,
/// …) as four longhand expressions (top, right, bottom, left), or a pair
/// (`gap`, `overflow`, `overscroll-behavior`) as two: every literal the
/// value can produce — a string, or an arm of a conditional — is split into
/// its rows, as is a template ([`template_words`]), and another computed
/// leaf is one value for all of them. `None` when no literal or template
/// names more than one value, so the shorthand stays one binding for its rows.
pub(crate) fn sides(name: &str, value: &Expr) -> Result<Option<Vec<Expr>>, LowerError> {
    let rows = longhands(name).len();
    fn widest(name: &str, rows: usize, e: &Expr) -> Result<usize, LowerError> {
        Ok(match e {
            Expr::Str(s, span) => {
                let n = box_sides::values(s).len();
                if name == "border-radius" && box_sides::elliptical(s) {
                    return err(
                        "lower-attr-value",
                        format!("`border-radius` takes one to four corners, each one length (top-left, top-right, bottom-right, bottom-left); \"{s}\" is an elliptical radius (`/`), which no host draws"),
                        *span,
                    );
                }
                if n > rows {
                    let (count, order) = match name {
                        "border-radius" => ("one to four", "top-left, top-right, bottom-right, bottom-left; no `/` elliptical radii"),
                        "gap" => ("one or two", "row, column"),
                        _ if rows == 2 => ("one or two", "x, y"),
                        _ => ("one to four", "top, right, bottom, left"),
                    };
                    return err(
                        "lower-attr-value",
                        format!("`{name}` takes {count} values ({order}); \"{s}\" has {n}"),
                        *span,
                    );
                }
                n
            }
            // Checked as the text it spells, each interpolation one value.
            Expr::Template(parts, span) => match template_words(parts, *span) {
                Some(w) if w.len() > 1 => {
                    let text = parts.iter().map(|p| match p {
                        TemplatePart::Text(t) => t.as_str(),
                        TemplatePart::Expr(_) => "${…}",
                    });
                    widest(name, rows, &Expr::Str(text.collect(), *span))?
                }
                _ => 1,
            },
            Expr::Ternary(_, yes, no, _) => widest(name, rows, yes)?.max(widest(name, rows, no)?),
            Expr::Match { some, none, .. } => {
                widest(name, rows, some)?.max(widest(name, rows, none)?)
            }
            Expr::Let { body, .. } => widest(name, rows, body)?,
            _ => 1,
        })
    }
    fn side(e: &Expr, i: usize) -> Expr {
        match e {
            Expr::Str(s, span) => {
                let v = box_sides::values(s);
                if v.is_empty() {
                    return e.clone();
                }
                let pick = box_sides::pick(v.len(), i);
                // A length in pixels is the number it would be on its own
                // (`border-width` takes only numbers).
                let piece = v[pick];
                match piece.strip_suffix("px").unwrap_or(piece).parse::<f64>() {
                    Ok(n) if n.is_finite() => Expr::Number(n, *span),
                    _ => Expr::Str(piece.to_string(), *span),
                }
            }
            Expr::Template(parts, span) => match template_words(parts, *span) {
                Some(w) if w.len() > 1 => match &w[box_sides::pick(w.len(), i)] {
                    literal @ Expr::Str(..) => side(literal, 0),
                    word => word.clone(),
                },
                _ => e.clone(),
            },
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
    if widest(name, rows, value)? < 2 {
        return Ok(None);
    }
    Ok(Some((0..rows).map(|i| side(value, i)).collect()))
}

/// A template's values, split as a literal's are, at CSS white space outside
/// parentheses in its text: each the template of its pieces, or the literal
/// it is with no interpolation. `None`, so the template is not split, unless
/// each interpolation is plainly part of one value: outside parentheses, a
/// literal one a single token (non-empty, no white space or parentheses),
/// and in a value that also holds text or only such literals. A computed
/// interpolation holding white space reaches its row as a list, which the
/// row refuses as it does `padding=pad` holding one.
fn template_words(parts: &[TemplatePart], span: Span) -> Option<Vec<Expr>> {
    const MARK: char = '\u{e000}';
    let (mut text, mut exprs, mut depth) = (String::new(), Vec::new(), 0usize);
    for p in parts {
        match p {
            TemplatePart::Text(t) if t.contains(MARK) => return None,
            TemplatePart::Text(t) => {
                for c in t.chars() {
                    match c {
                        '(' => depth += 1,
                        ')' => depth = depth.checked_sub(1)?,
                        _ => {}
                    }
                }
                text.push_str(t);
            }
            TemplatePart::Expr(e) => {
                let token = |s: &str| {
                    !s.is_empty() && !s.chars().any(|c| c.is_whitespace() || c == '(' || c == ')')
                };
                if depth > 0 || matches!(e, Expr::Str(s, _) if !token(s)) {
                    return None;
                }
                text.push(MARK);
                exprs.push(e);
            }
        }
    }
    let mut exprs = exprs.into_iter();
    let mut out = Vec::new();
    for word in box_sides::values(&text) {
        let mut pieces = Vec::new();
        for (i, chunk) in word.split(MARK).enumerate() {
            if i > 0 {
                pieces.push(TemplatePart::Expr(exprs.next()?.clone()));
            }
            if !chunk.is_empty() {
                pieces.push(TemplatePart::Text(chunk.into()));
            }
        }
        // A value of computed interpolations alone may be empty.
        let known = |p: &TemplatePart| match p {
            TemplatePart::Text(_) => true,
            TemplatePart::Expr(e) => matches!(e, Expr::Str(..) | Expr::Number(..)),
        };
        let has_text = pieces.iter().any(|p| matches!(p, TemplatePart::Text(_)));
        if !has_text && !pieces.iter().all(known) {
            return None;
        }
        out.push(match pieces.as_slice() {
            [TemplatePart::Text(t)] => Expr::Str(t.clone(), span),
            _ => Expr::Template(pieces, span),
        });
    }
    Some(out)
}
