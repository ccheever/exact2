//! Inline atoms' declarations and a Markdown node's `atoms` and `atomFaces`
//! (LLP 1045.000 D3): the forms and paint as compiled tables, the faces as
//! a typed record list.
use super::*;
use exact_kernel::PropId;
use serde_json::{json, Map, Value};

/// A declared kind: its table row and its paint rows.
pub(crate) struct Declared {
    row: Map<String, Value>,
    paint: Value,
}

/// Refuse `atoms` and `atomFaces` except on a `text` or `textarea` with a
/// literal `markup="markdown"` (`lower-atom-field`).
pub(super) fn check_field(tag: &str, attrs: &[Attr]) -> Result<(), LowerError> {
    let Some(atom) = attrs
        .iter()
        .find(|a| a.name == "atoms" || a.name == "atomFaces")
    else {
        return Ok(());
    };
    let markdown = attrs.iter().any(|a| {
        a.name == "markup" && matches!(&a.value, Expr::Str(word, _) if word == "markdown")
    });
    if !matches!(tag, "text" | "textarea") || !markdown {
        return err(
            "lower-atom-field",
            format!(
                "`{}` belongs on a `text` or `textarea` with `markup=\"markdown\"`: an atom is a span of Markdown source (LLP 1045.000)",
                atom.name
            ),
            atom.span,
        );
    }
    Ok(())
}

fn style_err<T>(name: &str, attr: &Attr) -> Result<T, LowerError> {
    err(
        "lower-atom-style",
        format!(
            "`atom {name}` takes one form (`link=\"scheme:\"`, `link=\"relative\"` or `prefix=\"@\"`), an optional `role` (`button` or `link`), and `color` and `background-color`; `{}` is none of these: a pill's metrics are the platform's (LLP 1045.000 D3)",
            attr.name
        ),
        attr.span,
    )
}

/// A URL scheme with its colon, by RFC 3986 §3.1.
fn scheme(text: &str) -> bool {
    let Some(name) = text.strip_suffix(':') else {
        return false;
    };
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

impl Lowerer<'_> {
    pub(super) fn declare_atoms(&mut self, file: &File) -> Result<(), LowerError> {
        for declaration in &file.atoms {
            let name = &declaration.name;
            let mut row = Map::new();
            row.insert("name".into(), json!(name));
            let mut paint = Vec::new();
            let mut form = None;
            for attr in &declaration.attrs {
                let text = match &attr.value {
                    Expr::Str(text, _) => Some(text.as_str()),
                    _ => None,
                };
                match attr.name.as_str() {
                    "link" | "prefix" => {
                        if let Some(other) = form.replace(attr) {
                            return err(
                                "lower-atom-style",
                                format!(
                                    "`atom {name}` takes one form; it has `{}` and `{}`",
                                    other.name, attr.name
                                ),
                                attr.span,
                            );
                        }
                        let valid = match (attr.name.as_str(), text) {
                            ("link", Some(t)) => t == "relative" || scheme(t),
                            ("prefix", Some(t)) => {
                                !t.is_empty() && !t.chars().any(|c| c.is_whitespace() || c == '"')
                            }
                            _ => false,
                        };
                        if !valid {
                            return err(
                                "lower-atom-style",
                                format!(
                                    "`atom {name}`'s `{}` takes {}",
                                    attr.name,
                                    if attr.name == "link" {
                                        "a scheme with its colon (`link=\"t3-citation:\"`) or `relative`"
                                    } else {
                                        "the characters that begin a token, none of them white space or `\"` (`prefix=\"@\"`)"
                                    }
                                ),
                                attr.span,
                            );
                        }
                        let value = text.unwrap_or_default();
                        let value = if attr.name == "link" && value != "relative" {
                            value.to_ascii_lowercase()
                        } else {
                            value.to_string()
                        };
                        row.insert(attr.name.clone(), json!(value));
                    }
                    "role" => match text {
                        Some(role @ ("button" | "link")) => {
                            row.insert("role".into(), json!(role));
                        }
                        _ => {
                            return err(
                                "lower-atom-style",
                                format!(
                                    "`atom {name}`'s `role` is `button` (the default) or `link`"
                                ),
                                attr.span,
                            )
                        }
                    },
                    "color" | "background-color" => {
                        let Some(tags::AttrTarget::Styles(ids)) = tags::attr(&attr.name) else {
                            return style_err(name, attr);
                        };
                        if text.is_none() {
                            return err(
                                "lower-atom-style",
                                format!("`atom {name}`'s `{}` takes a literal colour", attr.name),
                                attr.span,
                            );
                        }
                        self.check_style(attr, ids, &Ty::String, &[], &Scope::default())?;
                        for id in ids {
                            paint.push(json!([*id as u16, text]));
                        }
                    }
                    _ => return style_err(name, attr),
                }
            }
            if form.is_none() {
                return err(
                    "lower-atom-style",
                    format!("`atom {name}` needs its form: `link=\"scheme:\"`, `link=\"relative\"` or `prefix=\"@\"`"),
                    declaration.span,
                );
            }
            let declared = Declared {
                row,
                paint: Value::Array(paint),
            };
            if self.atoms.insert(name.clone(), declared).is_some() {
                return err(
                    "lower-atom-duplicate",
                    format!("`atom {name}` is declared twice"),
                    declaration.span,
                );
            }
        }
        Ok(())
    }

    /// `atoms="a b"`: the named kinds' table (`atoms`) and paint
    /// (`atomStyles`), each name resolved as the field's file sees it.
    pub(super) fn bind_atoms(
        &mut self,
        attr: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        let Expr::Str(names, _) = &attr.value else {
            return err(
                "lower-attr-type",
                "`atoms` takes a literal list of `atom` names: `atoms=\"citation mention\"`",
                attr.span,
            );
        };
        let visible = self.atom_scopes.get(&attr.span.source_id);
        let mut rows = Vec::new();
        let mut paint = Map::new();
        // (form, value) of each kind so far, for the duplicate rule.
        let mut forms: Vec<(String, String, String)> = Vec::new();
        for name in names.split_whitespace() {
            let target = match visible {
                Some(aliases) => aliases
                    .iter()
                    .find(|(alias, _)| alias == name)
                    .map(|(_, target)| target.as_str()),
                None if self.atom_scopes.is_empty() => Some(name),
                None => None,
            };
            let Some(declared) = target.and_then(|t| self.atoms.get(t)) else {
                let mut known: Vec<_> = match visible {
                    Some(aliases) => aliases.iter().map(|(a, _)| format!("`{a}`")).collect(),
                    None => self.atoms.keys().map(|a| format!("`{a}`")).collect(),
                };
                known.sort();
                return err(
                    "lower-atom-unknown",
                    format!(
                        "no `atom {name}` is visible here{}",
                        if known.is_empty() {
                            String::new()
                        } else {
                            format!("; visible: {}", known.join(", "))
                        }
                    ),
                    attr.span,
                );
            };
            let mut row = declared.row.clone();
            row.insert("name".into(), json!(name));
            for (form, value) in [("link", row.get("link")), ("prefix", row.get("prefix"))]
                .into_iter()
                .filter_map(|(f, v)| Some((f, v?.as_str()?.to_string())))
            {
                let clash = forms.iter().find(|(f, v, _)| {
                    f == form
                        && if form == "prefix" {
                            v.chars().any(|c| value.contains(c))
                        } else {
                            *v == value
                        }
                });
                if let Some((_, _, other)) = clash {
                    return err(
                        "lower-atom-duplicate",
                        format!(
                            "`{other}` and `{name}` in one `atoms` list both match `{form}=\"{value}\"`: every span has at most one kind (LLP 1045.000 D1)"
                        ),
                        attr.span,
                    );
                }
                forms.push((form.to_string(), value, name.to_string()));
            }
            paint.insert(name.to_string(), declared.paint.clone());
            rows.push(Value::Object(row));
        }
        for (prop, value) in [
            (PropId::Atoms, Value::Array(rows).to_string()),
            (PropId::AtomStyles, Value::Object(paint).to_string()),
        ] {
            let (code, _) = self.typed_code(&Expr::Str(value, attr.span), scope, locals)?;
            bindings.push(prop_row(prop, code, false));
        }
        Ok(())
    }

    /// `atomFaces=list`: the app's `AtomFace` records.
    pub(super) fn bind_atom_faces(
        &mut self,
        attr: &Attr,
        scope: &Scope,
        locals: u16,
        bindings: &mut Vec<BindingsRow>,
    ) -> Result<(), LowerError> {
        let (code, ty) = self.typed_code(&attr.value, scope, locals)?;
        if ty != Ty::List(Box::new(Ty::Record("AtomFace".into()))) {
            return err(
                "lower-attr-type",
                "`atomFaces` takes a list of `AtomFace` records: `AtomFace(source, label, icon, title, color, backgroundColor, direction, ariaLabel)`",
                attr.span,
            );
        }
        bindings.push(prop_row(PropId::AtomFaces, code, attr.written));
        Ok(())
    }
}
