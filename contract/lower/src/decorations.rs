//! Literal declaration rows and the typed field record (LLP 1114.007).
use super::*;
use exact_kernel::PropId;
use serde_json::{json, Map, Value};

/// Refuse `decorations` on any field but a plain `textarea`, even when its
/// `markup` is computed.
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
            "`decorations` paints a plain `textarea`; an `input` takes none, and a `markup=\"markdown\"` field's paint is its Markdown editor's (LLP 1045)",
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
                if !PAINT.contains(&attr.name.as_str()) {
                    return unsupported(attr);
                }
                let expanded: Vec<(StyleId, Expr)> = match tags::attr(&attr.name) {
                    Some(tags::AttrTarget::Styles(ids)) => {
                        ids.iter().map(|&id| (id, attr.value.clone())).collect()
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
                for (id, value) in expanded {
                    let ty = match value {
                        Expr::Number(..) => Ty::Number,
                        Expr::Str(..) => Ty::String,
                        _ => {
                            return err(
                                "lower-decoration-style",
                                format!(
                                    "`decoration {}`'s `{}` takes a literal value",
                                    declaration.name, attr.name
                                ),
                                attr.span,
                            )
                        }
                    };
                    let component = Attr {
                        value: value.clone(),
                        ..attr.clone()
                    };
                    self.check_style(&component, &[id], &ty, &[], &Scope::default())?;
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
                "`decorations` paints a plain `textarea`",
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

/// What CSS's `::highlight()` admits (CSS Pseudo-Elements 4 §3.2) and the
/// kernel has rows for: nothing that moves a line break, so a decorated field
/// measures, wraps and edits as it does without decorations.
const PAINT: [&str; 5] = [
    "color",
    "background-color",
    "text-decoration",
    "text-decoration-line",
    "text-shadow",
];

fn unsupported<T>(attr: &Attr) -> Result<T, LowerError> {
    err(
        "lower-decoration-style",
        format!(
            "`{}` cannot style a decoration: a decoration paints and never moves text, so it takes what CSS's `::highlight()` admits: `color`, `background-color`, `text-decoration`, `text-decoration-line` or `text-shadow` (LLP 1114.007)",
            attr.name
        ),
        attr.span,
    )
}
