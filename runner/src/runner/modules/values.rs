//! Completing a module's answer: a field it left out, by its declaration
//! (`fill`) or by its shape (`absent_is_none`); a permission's state written
//! into a status at its path (`set_path`).

use serde_json::{Map, Value as Json};

/// Set `path` (`coords.accuracy`) in `value` to `filler` where it is absent.
pub(super) fn fill(value: &mut Json, path: &str, filler: Json) {
    fill_with(value, path, &|_| Some(filler.clone()));
}

/// [`fill`] with what `made` makes of the object the path ends in; a part
/// `list[]` fills each entry of that list.
pub(super) fn fill_with(
    value: &mut Json,
    path: &str,
    made: &dyn Fn(&Map<String, Json>) -> Option<Json>,
) {
    let (part, rest) = path
        .split_once('.')
        .map_or((path, None), |(p, r)| (p, Some(r)));
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if let Some(list) = part.strip_suffix("[]") {
        let entries = object.get_mut(list).and_then(Json::as_array_mut);
        for entry in entries.into_iter().flatten() {
            fill_with(entry, rest.unwrap_or_default(), made);
        }
        return;
    }
    match rest {
        None if !object.contains_key(part) => {
            if let Some(filler) = made(object) {
                object.insert(part.to_owned(), filler);
            }
        }
        None => {}
        Some(rest) => {
            let next = (object.entry(part)).or_insert_with(|| Json::Object(Map::new()));
            fill_with(next, rest, made);
        }
    }
}

/// Set `path` in `value` to `to` where every record before its last part
/// exists: a path into a record this target's status leaves out sets nothing.
pub(super) fn set_path(value: &mut Json, path: &str, to: Json) {
    let (parents, last) = path.rsplit_once('.').unwrap_or(("", path));
    let parent = parents
        .split('.')
        .filter(|p| !p.is_empty())
        .try_fold(value, |at, part| at.get_mut(part));
    if let Some(object) = parent.and_then(Json::as_object_mut) {
        object.insert(last.into(), to);
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
