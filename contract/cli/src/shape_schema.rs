//! A shape named where a module's declaration takes a `shape-name` (LLP
//! 1133.016 D8, D15 item 4): the literal names one of the app's shapes, and
//! the call carries that shape's JSON Schema, in the subset every platform's
//! constrained decoding takes. A field is required unless it is an
//! `option<…>`; an `enum<…>` is a string's `enum`; a shape a field names is
//! written in place.

use crate::CompileError;
use contract_syntax::{Expr, ShapeDecl, Span, TypeExpr};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// The JSON Schema of shape `name`, or why there is none.
pub(crate) fn schema(name: &str, shapes: &HashMap<String, ShapeDecl>) -> Result<Value, String> {
    let mut path = Vec::new();
    record(name, shapes, &mut path)
}

fn record(
    name: &str,
    shapes: &HashMap<String, ShapeDecl>,
    path: &mut Vec<String>,
) -> Result<Value, String> {
    let Some(shape) = shapes.get(name) else {
        return Err(format!("`{name}` is not one of this app's shapes"));
    };
    if path.iter().any(|p| p == name) {
        return Err(format!(
            "`{name}` names itself, which no schema here can hold"
        ));
    }
    path.push(name.to_owned());
    let mut properties = Map::new();
    let mut required = Vec::new();
    for f in &shape.fields {
        let (ty, optional) = match &f.ty {
            TypeExpr::Option(inner, _) => (inner.as_ref(), true),
            ty => (ty, false),
        };
        properties.insert(f.name.clone(), of(ty, shapes, path)?);
        if !optional {
            required.push(Value::String(f.name.clone()));
        }
    }
    path.pop();
    Ok(json!({ "type": "object", "properties": properties, "required": required }))
}

fn of(
    ty: &TypeExpr,
    shapes: &HashMap<String, ShapeDecl>,
    path: &mut Vec<String>,
) -> Result<Value, String> {
    Ok(match ty {
        TypeExpr::Enum(members, _) => json!({ "type": "string", "enum": members }),
        TypeExpr::List(item, _) => json!({ "type": "array", "items": of(item, shapes, path)? }),
        TypeExpr::Option(inner, _) => of(inner, shapes, path)?,
        TypeExpr::Named(n, _) => match n.as_str() {
            "string" => json!({ "type": "string" }),
            "number" => json!({ "type": "number" }),
            "bool" => json!({ "type": "boolean" }),
            shape => record(shape, shapes, path)?,
        },
    })
}

/// Each literal shape name `options` write at `paths` (`responseConstraint`,
/// `tools[].inputSchema`) replaced by its schema's JSON text; a computed one,
/// or a name that is no shape, refused.
pub(crate) fn rewrite(
    declared: &str,
    paths: &[String],
    options: &mut [(String, Expr)],
    shapes: &HashMap<String, ShapeDecl>,
    errors: &mut Vec<CompileError>,
) {
    for path in paths {
        let (option, rest) = match path.split_once('.') {
            Some((head, rest)) => (head, Some(rest)),
            None => (path.as_str(), None),
        };
        let (option, each) = match option.strip_suffix("[]") {
            Some(o) => (o, true),
            None => (option, false),
        };
        let Some((_, value)) = options.iter_mut().find(|(n, _)| n == option) else {
            continue;
        };
        let mut at = |e: &mut Expr| at_field(e, rest, declared, path, shapes, errors);
        match (each, value) {
            (true, Expr::List(items, _)) => items.iter_mut().for_each(&mut at),
            (true, other) => errors.push(refusal(
                format!(
                    "`{declared}`'s `{option}` is a literal list, so the bake can read each `{}`",
                    rest.unwrap_or("")
                ),
                other.span(),
            )),
            (false, value) => at(value),
        }
    }
}

/// The shape name at `field` of `record` (a record constructor), or `record`
/// itself when there is no field.
fn at_field(
    e: &mut Expr,
    field: Option<&str>,
    declared: &str,
    path: &str,
    shapes: &HashMap<String, ShapeDecl>,
    errors: &mut Vec<CompileError>,
) {
    let target = match field {
        None => Some(e),
        Some(field) => {
            match e {
                Expr::Call(_, args, _) => args.iter_mut().find_map(|a| match a {
                    Expr::NamedArg(n, v, _) if n == field => Some(v.as_mut()),
                    _ => None,
                }),
                other => {
                    errors.push(refusal(
                    format!("`{declared}`'s `{path}` is written in a record literal the bake can read"),
                    other.span(),
                ));
                    None
                }
            }
        }
    };
    let Some(target) = target else { return };
    match target {
        Expr::Str(name, at) => match schema(name, shapes) {
            Ok(s) => *target = Expr::Str(s.to_string(), *at),
            Err(why) => errors.push(refusal(format!("`{declared}`'s `{path}`: {why}"), *at)),
        },
        // An omitted option stays omitted.
        Expr::None(_) => {}
        other => errors.push(refusal(
            format!("`{declared}`'s `{path}` names a shape by a literal, `\"Title\"`, so the bake can write its schema"),
            other.span(),
        )),
    }
}

fn refusal(message: String, span: Span) -> CompileError {
    CompileError {
        pass: "use",
        id: "contract-module-shape-name".into(),
        message,
        span,
        file: None,
        related: Box::new([]),
    }
}
