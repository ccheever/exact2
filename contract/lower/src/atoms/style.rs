//! An `atom` declaration's style (LLP 1045.000 D3, D4): the pill's inline
//! padding and border, which are measured and go in the kind's table row,
//! and its paint, by state, which goes in `atomStyles` as kernel rows.
use super::*;
use serde_json::{json, Map, Value};

/// The states an atom's paint may name: the pointer over it, and the
/// selection being exactly it (ProseMirror's node selection), which is
/// `:checked` as a selected `<option>` is (HTML's `:checked`).
pub(crate) const ATOM_STATES: [&str; 2] = ["hover", "checked"];

/// One atom's style as its declaration writes it.
#[derive(Default)]
pub(crate) struct AtomStyle {
    /// Paint rows by state: `""` the pill's own, then `hover`, `checked`.
    pub(crate) paint: Vec<(&'static str, Vec<Value>)>,
    padding: Option<String>,
    icon_size: Option<String>,
    gap: Option<String>,
    border_width: Option<(f64, Span)>,
    border_style: Option<(bool, Span)>,
}

/// CSS's `thin`, `medium` and `thick`, in px (CSS Backgrounds 3 §3.3).
fn line_width(word: &str) -> Option<f64> {
    match word {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        _ => px(word),
    }
}

/// A length in px: `0`, or a number and `px`.
fn px(text: &str) -> Option<f64> {
    let text = text.trim();
    if text == "0" {
        return Some(0.0);
    }
    text.strip_suffix("px")?
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite())
}

/// A length of 0 or more in px or em (`em` the label's size), as the row writes it.
fn px_or_em(text: &str) -> Option<String> {
    let em = text
        .strip_suffix("em")
        .and_then(|n| n.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n >= 0.0);
    match (px(text).filter(|n| *n >= 0.0), em) {
        (Some(n), _) => Some(format!("{n}px")),
        (None, Some(n)) => Some(format!("{n}em")),
        _ => None,
    }
}

/// A shorthand's words, a function's arguments kept with it
/// (`2px solid light-dark(#a, #b)` is three).
fn words(text: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, None);
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c.is_whitespace() && depth == 0 => {
                if let Some(s) = start.take() {
                    out.push(&text[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if let Some(s) = start {
        out.push(&text[s..]);
    }
    out
}

fn refuse<T>(name: &str, attr: &Attr, why: impl std::fmt::Display) -> Result<T, LowerError> {
    err(
        "lower-atom-style",
        format!("`atom {name}`'s `{}` {why}", attr.name),
        attr.span,
    )
}

impl AtomStyle {
    fn rows(&mut self, state: &'static str) -> &mut Vec<Value> {
        if let Some(at) = self.paint.iter().position(|(s, _)| *s == state) {
            return &mut self.paint[at].1;
        }
        self.paint.push((state, Vec::new()));
        &mut self.paint.last_mut().unwrap().1
    }

    /// The measured part, into the kind's table row: `padding`, a CSS length
    /// on each inline side, `iconSize` and `gap`, CSS lengths, the icon's
    /// side and the space after it, and `border`, its width in px on every
    /// side.
    pub(crate) fn finish(
        &self,
        name: &str,
        row: &mut Map<String, Value>,
    ) -> Result<(), LowerError> {
        if let Some(padding) = &self.padding {
            row.insert("padding".into(), json!(padding));
        }
        if let Some(size) = &self.icon_size {
            row.insert("iconSize".into(), json!(size));
        }
        if let Some(gap) = &self.gap {
            row.insert("gap".into(), json!(gap));
        }
        match (self.border_style, self.border_width) {
            (Some((true, _)), width) => {
                row.insert("border".into(), json!(width.map_or(3.0, |(w, _)| w)));
            }
            (None, Some((w, span))) if w > 0.0 => {
                return err(
                    "lower-atom-style",
                    format!("`atom {name}` has a border width and no style: a border draws only with `border-style=\"solid\"` (CSS's initial `none` draws none), or write `border=\"{w}px solid <colour>\"`"),
                    span,
                );
            }
            _ => {}
        }
        Ok(())
    }
}

impl Lowerer<'_> {
    /// One style row of `atom name`, its own or a state's.
    pub(crate) fn atom_style_row(
        &self,
        name: &str,
        attr: &Attr,
        style: &mut AtomStyle,
    ) -> Result<(), LowerError> {
        let (state, property) = match contract_syntax::state::state_of(&attr.name) {
            Some((state, row)) => {
                let Some(state) = ATOM_STATES.iter().find(|s| **s == state) else {
                    return err(
                        "lower-atom-style",
                        format!("`atom {name}` takes the `:hover` and `:checked` states (the pointer over it, the selection exactly it); `:{state}` is not one"),
                        attr.span,
                    );
                };
                (*state, row)
            }
            None => ("", attr.name.as_str()),
        };
        let row = Attr {
            name: property.to_string(),
            ..attr.clone()
        };
        let Expr::Str(text, _) = &attr.value else {
            return refuse(name, &row, "takes a literal string");
        };
        let text = text.trim();
        let colour = |ids: &[StyleId], text: &str| -> Result<(), LowerError> {
            let probe = Attr {
                value: Expr::Str(text.to_string(), attr.span),
                ..row.clone()
            };
            self.check_style(&probe, ids, &Ty::String, &[], &Scope::default())
        };
        let measured = |what: &str| -> Result<(), LowerError> {
            if state.is_empty() {
                return Ok(());
            }
            refuse(name, &row, format_args!("is {what}, which moves the text; it belongs on the atom's own line, and `:{state}` takes paint: `color`, `background-color`, `border-color`, `border-radius`, `outline`, `outline-offset` and `text-decoration-line`"))
        };
        match property {
            "color" | "background-color" => {
                let id = if property == "color" {
                    StyleId::TextColor
                } else {
                    StyleId::BackgroundColor
                };
                colour(&[id], text)?;
                style.rows(state).push(json!([id as u16, text]));
            }
            "border-color" => {
                let ids = [
                    StyleId::BorderColorTop,
                    StyleId::BorderColorRight,
                    StyleId::BorderColorBottom,
                    StyleId::BorderColorLeft,
                ];
                colour(&ids, text)?;
                style
                    .rows(state)
                    .extend(ids.iter().map(|id| json!([*id as u16, text])));
            }
            "border-radius" => {
                let Some(r) = px(text).filter(|r| *r >= 0.0) else {
                    return refuse(
                        name,
                        &row,
                        "takes one length in px, every corner the same (`border-radius=\"4px\"`)",
                    );
                };
                let ids = [
                    StyleId::BorderRadiusTopLeft,
                    StyleId::BorderRadiusTopRight,
                    StyleId::BorderRadiusBottomRight,
                    StyleId::BorderRadiusBottomLeft,
                ];
                style
                    .rows(state)
                    .extend(ids.iter().map(|id| json!([*id as u16, format!("{r}px")])));
            }
            "text-decoration-line" => {
                if !matches!(text, "underline" | "none") {
                    return refuse(
                        name,
                        &row,
                        "takes `underline` or `none`: the label's underline",
                    );
                }
                style
                    .rows(state)
                    .push(json!([StyleId::TextDecorationLine as u16, text]));
            }
            "outline" => {
                let (mut width, mut solid, mut ink) = (None, None, None);
                for word in words(text) {
                    match word {
                        "solid" | "none" if solid.is_none() => solid = Some(word == "solid"),
                        _ if width.is_none() && line_width(word).is_some() => width = line_width(word),
                        _ if ink.is_none() => ink = Some(word),
                        _ => return refuse(name, &row, format_args!("takes a width, `solid` and a colour (`outline=\"2px solid #2563eb\"`), or `none`; \"{text}\" is not that")),
                    }
                }
                let rows = style.rows(state);
                match solid {
                    Some(true) => {
                        let width = width.unwrap_or(3.0);
                        if width < 0.0 {
                            return refuse(name, &row, "takes a width of 0 or more");
                        }
                        if let Some(ink) = ink {
                            colour(&[StyleId::OutlineColor], ink)?;
                            rows.push(json!([StyleId::OutlineColor as u16, ink]));
                        }
                        rows.push(json!([StyleId::OutlineWidth as u16, width]));
                    }
                    Some(false) if width.is_none() && ink.is_none() => rows.push(json!([StyleId::OutlineWidth as u16, 0.0])),
                    _ => return refuse(name, &row, "draws only with `solid` (CSS's initial `outline-style` is `none`): `outline=\"2px solid #2563eb\"`, or `none`"),
                }
            }
            "outline-offset" => {
                let Some(offset) = px(text) else {
                    return refuse(name, &row, "takes one length in px, negative inside the pill (`outline-offset=\"-2px\"`)");
                };
                style
                    .rows(state)
                    .push(json!([StyleId::OutlineOffset as u16, offset]));
            }
            "outline-width" | "outline-color" | "outline-style" => {
                return refuse(
                    name,
                    &row,
                    "is written through `outline`: `outline=\"2px solid #2563eb\"`",
                );
            }
            "padding-inline" => {
                measured("padding")?;
                let Some(padding) = px_or_em(text) else {
                    return refuse(name, &row, "takes one length in px or em, both sides the same (`padding-inline=\"0\"`)");
                };
                style.padding = Some(padding);
            }
            "-exact-icon-size" => {
                measured("its icon's size")?;
                let Some(size) = px_or_em(text) else {
                    return refuse(name, &row, "takes one length in px or em, the icon's side (`-exact-icon-size=\"1em\"`)");
                };
                style.icon_size = Some(size);
            }
            "gap" => {
                measured("the gap after its icon")?;
                let Some(gap) = px_or_em(text) else {
                    return refuse(name, &row, "takes one length in px or em, the space between its icon and its label (`gap=\"0.2em\"`)");
                };
                style.gap = Some(gap);
            }
            "border-width" => {
                measured("a border's width")?;
                let Some(width) = line_width(text).filter(|w| *w >= 0.0) else {
                    return refuse(name, &row, "takes one width in px, every side the same");
                };
                style.border_width = Some((width, attr.span));
            }
            "border-style" => {
                measured("a border's style")?;
                if !matches!(text, "solid" | "none") {
                    return refuse(name, &row, "takes `solid` or `none`");
                }
                style.border_style = Some((text == "solid", attr.span));
            }
            "border" => {
                measured("a border")?;
                let (mut width, mut solid, mut ink) = (None, None, None);
                for word in words(text) {
                    match word {
                        "solid" | "none" if solid.is_none() => solid = Some(word == "solid"),
                        _ if width.is_none() && line_width(word).is_some() => width = line_width(word),
                        _ if ink.is_none() => ink = Some(word),
                        _ => return refuse(name, &row, format_args!("takes a width, `solid` and a colour (`border=\"1px solid #d4d4d8\"`), or `none`; \"{text}\" is not that")),
                    }
                }
                if solid.is_none() && (width.is_some() || ink.is_some()) {
                    return refuse(
                        name,
                        &row,
                        "draws only with `solid` (CSS's initial `border-style` is `none`)",
                    );
                }
                style.border_style = Some((solid == Some(true), attr.span));
                style.border_width = width.map(|w| (w, attr.span));
                if let Some(ink) = ink {
                    let ids = [
                        StyleId::BorderColorTop,
                        StyleId::BorderColorRight,
                        StyleId::BorderColorBottom,
                        StyleId::BorderColorLeft,
                    ];
                    colour(&ids, ink)?;
                    style
                        .rows(state)
                        .extend(ids.iter().map(|id| json!([*id as u16, ink])));
                }
            }
            _ => {
                return refuse(name, &row, "is not an atom's: an atom takes one form (`link=\"scheme:\"`, `link=\"relative\"` or `prefix=\"@\"`), an optional `role` (`button` or `link`), `color`, `background-color`, `padding-inline`, `-exact-icon-size`, `gap`, `border`, `border-radius`, `outline`, `outline-offset` and `text-decoration-line`, and the `:hover` and `:checked` states; its height and font are its line's (LLP 1045.000 D3, D4)");
            }
        }
        Ok(())
    }
}
