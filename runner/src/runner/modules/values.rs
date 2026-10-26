//! Completing a module's answer: a field it left out, by its declaration
//! (`fill`) or by its shape (`absent_is_none`).

use serde_json::{Map, Value as Json};

/// Set `path` (`coords.accuracy`) in `value` to `filler` where it is absent.
pub(super) fn fill(value: &mut Json, path: &str, filler: Json) {
    let mut at = value;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        let Some(object) = at.as_object_mut() else {
            return;
        };
        if parts.peek().is_none() {
            object.entry(part).or_insert(filler);
            return;
        }
        at = object
            .entry(part)
            .or_insert_with(|| Json::Object(Map::new()));
    }
}

/// A module's answer leaves out an option field it has nothing for (a
/// platform's words where another platform answers, a declared default):
/// absent is `none`, as an omitted word is (LLP 1133.002 D2).
pub(super) fn absent_is_none(json: &mut Json, shape: &exact_js_value::Shape) {
    use exact_js_value::Shape;
    match (json, shape) {
        (Json::Object(map), Shape::Record(fields)) => {
            for (name, field) in fields {
                match map.get_mut(name) {
                    Some(value) => absent_is_none(value, field),
                    None if matches!(field, Shape::Option(_)) => {
                        map.insert(name.clone(), Json::Null);
                    }
                    None => {}
                }
            }
        }
        (Json::Array(items), Shape::List(item)) => {
            items.iter_mut().for_each(|v| absent_is_none(v, item))
        }
        (value, Shape::Option(inner)) if !value.is_null() => absent_is_none(value, inner),
        _ => {}
    }
}
