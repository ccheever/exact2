//! Literal declaration rows and the typed field record (LLP 1114.007).
use super::*;
use exact_kernel::PropId;
use serde_json::{json, Map, Value};

/// Reject a competing editor owner even when its markup is computed.
pub(super) fn check_field(tag: &str, attrs: &[Attr]) -> Result<(), LowerError> {
    let Some(decoration) = attrs.iter().find(|a| a.name == "decorations") else {
        return Ok(());
    };
    if tag != "textarea"
        || attrs.iter().any(|a| {
            a.name == "markup" && !matches!(&a.value, Expr::Str(word, _) if word == "none")
        })
    {
        return err(
            "lower-decoration-field",
            "`decorations` belongs to a plain `textarea`; remove `markup` or write `markup=\"none\"`",
            decoration.span,
        );
    }
    Ok(())
}

impl Lowerer<'_> {
    pub(super) fn declare_decorations(&mut self, file: &File) -> Result<(), LowerError> {
        for declaration in &file.decorations {
            let mut rows = Vec::new();
            for attr in &declaration.attrs {
                let expanded: Vec<(StyleId, Expr)> = match tags::attr(&attr.name) {
                    Some(tags::AttrTarget::Styles(ids)) => {
                        let sides = if values::four_sided(ids) {
                            values::sides(&attr.name, &attr.value)?
                        } else {
                            None
                        };
                        ids.iter()
                            .enumerate()
                            .map(|(index, &id)| {
                                (
                                    id,
                                    sides
                                        .as_ref()
                                        .map_or_else(|| attr.value.clone(), |s| s[index].clone()),
                                )
                            })
                            .collect()
                    }
                    Some(tags::AttrTarget::Shorthand) => shorthands::rows(&attr.name)
                        .iter()
                        .enumerate()
                        .map(|(index, &id)| {
                            shorthands::component(&attr.value, &attr.name, index)
                                .map(|value| (id, value))
                        })
                        .collect::<Result<_, _>>()?,
                    _ => return unsupported(attr),
                };
                for (id, mut value) in expanded {
                    if !supported(id) {
                        return unsupported(attr);
                    }
                    if id == StyleId::FontFamily {
                        value = self.family_stacks(&value)?;
                    }
                    let ty = match value {
                        Expr::Number(..) => Ty::Number,
                        Expr::Str(..) => Ty::String,
                        _ => return unsupported(attr),
                    };
                    let component = Attr {
                        value: value.clone(),
                        ..attr.clone()
                    };
                    // Font families have already been resolved into plan stack ids.
                    if id != StyleId::FontFamily {
                        self.check_style(&component, &[id], &ty, &[], &Scope::default())?;
                    }
                    let literal = match value {
                        Expr::Number(number, _) => json!(number),
                        Expr::Str(text, _) => json!(text),
                        _ => unreachable!("checked literal"),
                    };
                    rows.push(json!([id as u16, literal]));
                }
            }
            if self
                .decorations
                .insert(declaration.name.clone(), Value::Array(rows))
                .is_some()
            {
                return err(
                    "lower-decoration-duplicate",
                    format!("`decoration {}` is declared twice", declaration.name),
                    declaration.span,
                );
            }
        }
        Ok(())
    }

    pub(super) fn bind_decorations(
        &mut self,
        tag: &str,
        attr: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        if tag != "textarea" {
            return err(
                "lower-decoration-field",
                "`decorations` belongs to a plain `textarea`",
                attr.span,
            );
        }
        let (code, ty) = self.typed_code(&attr.value, scope, locals)?;
        if ty != Ty::Record("TextDecorations".into()) {
            return err(
                "lower-attr-type",
                "`decorations` takes a `TextDecorations` record",
                attr.span,
            );
        }
        bindings.push(prop_row(PropId::Decorations, code, attr.written));
        let mut table = Map::new();
        match self.decoration_scopes.get(&attr.span.source_id) {
            Some(aliases) => {
                for (alias, target) in aliases {
                    if let Some(rows) = self.decorations.get(target) {
                        table.insert(alias.clone(), rows.clone());
                    }
                }
            }
            None if self.decoration_scopes.is_empty() => {
                table.extend(
                    self.decorations
                        .iter()
                        .map(|(name, rows)| (name.clone(), rows.clone())),
                );
            }
            None => {}
        }
        let value = Expr::Str(Value::Object(table).to_string(), attr.span);
        let (code, _) = self.typed_code(&value, scope, locals)?;
        bindings.push(prop_row(PropId::DecorationStyles, code, false));
        Ok(())
    }
}

fn unsupported<T>(attr: &Attr) -> Result<T, LowerError> {
    err("lower-decoration-style", format!("`{}` is not a decoration row: use literal font, colour, text-decoration, block margin, left padding/border, radius or line-height properties", attr.name), attr.span)
}

fn supported(id: StyleId) -> bool {
    use StyleId::*;
    matches!(
        id,
        FontFamily
            | FontSize
            | FontWeight
            | FontStyle
            | FontVariantNumeric
            | TextColor
            | BackgroundColor
            | TextDecorationLine
            | LineHeight
            | MarginTop
            | MarginBottom
            | MarginLeft
            | MarginRight
            | PaddingLeft
            | BorderWidthLeft
            | BorderStyleLeft
            | BorderColorLeft
            | BorderRadiusTopLeft
            | BorderRadiusTopRight
            | BorderRadiusBottomLeft
            | BorderRadiusBottomRight
    )
}
