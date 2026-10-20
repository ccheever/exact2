//! Type and family tokens (LLP 1121 D3, D6; stage 6 C). `font="type.body"`
//! writes only the longhands the token names: one row each, every row
//! naming the token, and the kernel reads each row's part of its value
//! (`exact_kernel::style::theme::typeface`). `font-family="family.text"` is
//! one row naming the token. A token's family is a stack the plan declares,
//! as a literal `font-family`'s is, and a bound table holds its id.
use super::*;
use exact_kernel::style::theme::typeface;
use exact_tamber as tamber;

/// The rows a type value writes, in `font`'s order.
fn longhands(value: &str) -> Option<Vec<StyleId>> {
    let p = typeface::parts(value)?;
    let mut out = Vec::new();
    if p.style.is_some() {
        out.push(StyleId::FontStyle);
    }
    if p.weight.is_some() {
        out.push(StyleId::FontWeight);
    }
    out.push(StyleId::FontSize);
    if p.leading.is_some() {
        out.push(StyleId::LineHeight);
    }
    if p.family.is_some() {
        out.push(StyleId::FontFamily);
    }
    Some(out)
}

impl Lowerer<'_> {
    /// Each theme entry's value as the compiler's bound table holds it: a
    /// type or family token's family as the stack it declares here (`#12`),
    /// every tuple's declared so the runner finds each in the plan.
    pub(super) fn theme_stacks(&mut self) -> Vec<String> {
        let entries: Vec<(tamber::Kind, Vec<String>)> = self
            .theme
            .iter()
            .map(|e| {
                (
                    e.kind,
                    std::iter::once(&e.value)
                        .chain(&e.slices)
                        .cloned()
                        .collect(),
                )
            })
            .collect();
        let mut stacks: Vec<(String, u16)> = Vec::new();
        let mut out = Vec::new();
        for (kind, values) in entries {
            let kernel = match kind {
                tamber::Kind::Type => exact_kernel::style::theme::Kind::Type,
                tamber::Kind::Family => exact_kernel::style::theme::Kind::Family,
                _ => {
                    out.push(values[0].clone());
                    continue;
                }
            };
            for value in &values {
                let family = match kind {
                    tamber::Kind::Family => Some(value.trim()),
                    _ => typeface::parts(value).and_then(|p| p.family),
                };
                let Some(family) = family else { continue };
                if stacks.iter().any(|(f, _)| f == family) {
                    continue;
                }
                let mut stack = Expr::Str(family.to_string(), Span::default());
                match self.replace_family(&mut stack) {
                    Ok(()) => {
                        if let Expr::Number(n, _) = stack {
                            stacks.push((family.to_string(), n as u16));
                        }
                    }
                    Err(e) => self.push(e),
                }
            }
            let found = |f: &str| stacks.iter().find(|(s, _)| s == f).map(|(_, id)| *id);
            out.push(
                typeface::entry(kernel, &values[0], found).unwrap_or_else(|| values[0].clone()),
            );
        }
        out
    }

    /// `font=` naming type tokens: one row a longhand they write, each
    /// naming the token, and `fontShorthand` so a text style's weight and
    /// leading apply where they write none. `false` when the value names no
    /// type token (the shorthand's own path).
    pub(super) fn type_font(
        &mut self,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        font: &[FontUse],
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<bool, LowerError> {
        let Some(rows) = self.tokens_named(a, tamber::Kind::Type, font)? else {
            return Ok(false);
        };
        let (code, _) = self.typed_code(&a.value, scope, locals)?;
        for row in rows {
            bindings.push(crate::style_row(row, code, a.written));
        }
        let marked = self.b.constant(&Value::Bool(true));
        bindings.push(crate::prop_row(
            exact_kernel::PropId::FontShorthand,
            marked,
            crate::IMPLIED,
        ));
        Ok(true)
    }

    /// `font-family=` naming family tokens: the row names the token, which
    /// the kernel reads as its stack. `false` when the value names none.
    pub(super) fn family_token(
        &mut self,
        a: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<bool, LowerError> {
        if self.tokens_named(a, tamber::Kind::Family, &[])?.is_none() {
            return Ok(false);
        }
        let (code, _) = self.typed_code(&a.value, scope, locals)?;
        bindings.push(crate::style_row(StyleId::FontFamily, code, a.written));
        Ok(true)
    }

    /// A type token's written style and weight, where its family, or the
    /// node's `font-family` when it names none (`font`), is a font the app
    /// ships: a face of it, as `font-weight` and `font-style` beside
    /// `font-family` are checked (v1 never synthesizes one).
    fn bundled_faces(
        &self,
        name: &str,
        value: &str,
        font: &[FontUse],
        span: Span,
    ) -> Result<(), LowerError> {
        let Some(p) = typeface::parts(value) else {
            return Ok(());
        };
        // The token's style, else the node's `font-style` where it is known
        // (a style neither says is not checked).
        let italic = match p.style {
            Some(style) => Some(style == "italic"),
            None => font.iter().find_map(|u| u.italic),
        };
        let weight: u16 = p.weight.and_then(|w| w.parse().ok()).unwrap_or(400);
        let shipped: Vec<(String, &DeclaredFont)> = match p.family {
            Some(family) => typeface::family_members(family)
                .into_iter()
                .filter(|(family, _)| *family)
                .filter_map(|(_, m)| {
                    self.declared_fonts
                        .iter()
                        .find(|(declared, _)| declared.eq_ignore_ascii_case(&m))
                        .map(|(declared, f)| (format!("`{declared}`"), f))
                })
                .collect(),
            // Only what the token writes: an unwritten style or weight is
            // the node's, checked where it is written.
            None if p.style.is_some() || p.weight.is_some() => font
                .iter()
                .map(|u| ("its `font-family`".to_string(), &u.font))
                .collect(),
            None => Vec::new(),
        };
        for (member, font) in shipped {
            let face = font
                .faces
                .iter()
                .any(|(w, i)| italic.is_none_or(|it| *i == it) && (weight < 600 || *w >= 600));
            if !face {
                let style = match italic {
                    Some(true) => "italic",
                    Some(false) => "normal",
                    None => "",
                };
                return err(
                    "lower-font-face",
                    format!("`{name}` is `{value}`: {member} has no real {}{style} face; v1 never synthesizes one", if weight >= 600 { "bold " } else { "" }),
                    span,
                );
            }
        }
        Ok(())
    }

    /// The tokens of `kind` `a`'s value names, through `?:`, `match` and
    /// `let`: `None` when it names none; else the longhands a type token
    /// writes, the same for every token and tuple (none for a family).
    /// A choice of a token and anything else is refused, and so is a token
    /// that varies by an app axis on a node with element children, which
    /// would inherit its rows (theme2's ruling: a pin re-selects a colour,
    /// not inherited type).
    fn tokens_named(
        &mut self,
        a: &Attr,
        kind: tamber::Kind,
        font: &[FontUse],
    ) -> Result<Option<Vec<StyleId>>, LowerError> {
        let mut leaves = Vec::new();
        crate::theme::leaves(&a.value, &mut Vec::new(), &mut leaves);
        let named: Vec<(String, Vec<String>, Span)> = leaves
            .iter()
            .filter_map(|e| match e {
                Expr::Str(text, span) => crate::theme::measure_values(text)
                    .filter(|(k, _)| *k == kind)
                    .map(|(_, values)| (text.trim().to_string(), values, *span)),
                _ => None,
            })
            .collect();
        // A token of the other kind, undotted (`heading`), is no family's
        // name or text style here: refused, naming the row that names it.
        for e in &leaves {
            if let Expr::Str(text, span) = e {
                if let Some((other @ (tamber::Kind::Type | tamber::Kind::Family), _)) =
                    crate::theme::measure_values(text)
                {
                    if other != kind {
                        let row = if other == tamber::Kind::Type {
                            "font"
                        } else {
                            "font-family"
                        };
                        return err(
                            "tamber-kind",
                            format!(
                                "`{}=\"{}\"`: `{}` is a {} token, which `{row}` names",
                                a.name,
                                text.trim(),
                                text.trim(),
                                other.name()
                            ),
                            *span,
                        );
                    }
                }
            }
        }
        if named.is_empty() {
            return Ok(None);
        }
        let what = if kind == tamber::Kind::Type {
            "type"
        } else {
            "family"
        };
        if named.len() != leaves.len() {
            return err(
                "tamber-kind",
                format!("`{}` names a {what} token in one arm and something else in another: each arm names a {what} token", a.name),
                a.value.span(),
            );
        }
        let mut rows: Option<Vec<StyleId>> = None;
        for (name, values, span) in &named {
            if kind == tamber::Kind::Type {
                for value in values {
                    let written = longhands(value);
                    if written.is_none() {
                        return err(
                            "tamber-kind",
                            format!("`{name}` is `{value}`, which no `font` longhand reads as a type value"),
                            *span,
                        );
                    }
                    self.bundled_faces(name, value, font, *span)?;
                    if rows.is_some() && written != rows {
                        return err(
                            "tamber-type-longhands",
                            format!("`{name}` writes other longhands than another value `{}` may be (`{value}`): a type token's tuples, and the tokens one row chooses between, write the same longhands, since which rows a token writes is lowered (D6)", a.name),
                            *span,
                        );
                    }
                    rows = written;
                }
            }
        }
        let varies = named.iter().any(|(_, values, _)| values.len() > 1);
        if varies && self.element_children {
            let name = &named
                .iter()
                .find(|(_, v, _)| v.len() > 1)
                .expect("varies")
                .0;
            return err(
                "tamber-inherited-axis",
                format!("`{}=\"{name}\"`: `{name}` varies by an app axis, and its rows are inherited by this node's children, which a mode boundary would not choose again: write it on the text, not its container (a pin would not reach inherited type)", a.name),
                a.value.span(),
            );
        }
        Ok(Some(rows.unwrap_or_default()))
    }
}
