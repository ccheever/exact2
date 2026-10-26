//! A tool call's arguments against the bound action's parameter types (LLP
//! 1111 D4 step 1): stricter than decoding, so that a caller's mistake is
//! named rather than guessed at. Unknown fields, missing fields and
//! non-finite numbers are refused; `null` is `none` for an `option<T>`,
//! which is still written. The first failing path is reported.

use exact_js_value::Shape;
use serde_json::Value as Json;

/// Whether `json` is a value of `shape`, else the first failing path and
/// why, as `page: a number, not "120"`.
pub fn check(json: &Json, shape: &Shape, path: &str) -> Result<(), String> {
    let at = |why: String| {
        Err(match path.is_empty() {
            true => why,
            false => format!("{path}: {why}"),
        })
    };
    match (shape, json) {
        (Shape::Option(_), Json::Null) => Ok(()),
        (Shape::Option(inner), _) => check(json, inner, path),
        (Shape::Number, Json::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => Ok(()),
        (Shape::Bool, Json::Bool(_)) | (Shape::String, Json::String(_)) => Ok(()),
        (Shape::Unit, Json::Null) => Ok(()),
        (Shape::List(inner), Json::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                check(item, inner, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        (Shape::Record(fields), Json::Object(given)) => {
            if let Some(extra) = given.keys().find(|k| !fields.iter().any(|(f, _)| f == *k)) {
                return at(format!("no field `{extra}`"));
            }
            for (name, field) in fields {
                let inner = match path.is_empty() {
                    true => name.clone(),
                    false => format!("{path}.{name}"),
                };
                match given.get(name) {
                    Some(value) => check(value, field, &inner)?,
                    None => return Err(format!("{inner}: missing")),
                }
            }
            Ok(())
        }
        _ => at(format!("{}, not {json}", describe(shape))),
    }
}

/// `shape` as a caller would read it.
fn describe(shape: &Shape) -> &'static str {
    match shape {
        Shape::Number => "a finite number",
        Shape::Bool => "true or false",
        Shape::String => "a string",
        Shape::Unit => "null",
        Shape::Option(_) => "a value or null",
        Shape::List(_) => "a list",
        Shape::Record(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_call_is_refused_at_its_first_bad_path() {
        let shape = Shape::Record(vec![
            ("id".into(), Shape::String),
            ("page".into(), Shape::Number),
            ("note".into(), Shape::Option(Box::new(Shape::String))),
        ]);
        assert!(check(&json!({"id": "a", "page": 3, "note": null}), &shape, "").is_ok());
        assert_eq!(
            check(&json!({"id": "a", "note": null}), &shape, ""),
            Err("page: missing".into())
        );
        assert_eq!(
            check(&json!({"id": "a", "page": 3}), &shape, ""),
            Err("note: missing".into()),
            "an option is still written"
        );
        assert_eq!(
            check(&json!({"id": "a", "page": "3", "note": null}), &shape, ""),
            Err("page: a finite number, not \"3\"".into())
        );
        assert_eq!(
            check(
                &json!({"id": "a", "page": 3, "note": null, "x": 1}),
                &shape,
                ""
            ),
            Err("no field `x`".into())
        );
        let list = Shape::List(Box::new(Shape::Bool));
        assert_eq!(
            check(&json!([true, 1]), &list, "flags"),
            Err("flags[1]: true or false, not 1".into())
        );
    }
}
