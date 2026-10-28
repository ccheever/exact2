//! What the client reads from a backend (`GET /schema`'s JSON) itself.

use serde_json::Value as Json;
use std::collections::BTreeSet;

/// Whether the device can predict `op`: every table it or its rules touch is
/// synced to the device (Snapback's `operationTables`, `admission.ts`).
pub fn predictable(backend: &Json, op: &str) -> bool {
    let schema = &backend["schema"];
    let Some(program) = backend["programs"]
        .as_array()
        .and_then(|programs| programs.iter().find(|p| p["name"] == op))
    else {
        return false;
    };
    let tables = operation_tables(program, schema);
    tables
        .iter()
        .all(|table| schema["tables"][table]["sync"].is_object())
}

/// Whether `op` touches an ephemeral table (`ephemeral 4s`: rows held in
/// one server process). This client does not send ephemeral writes yet.
pub fn touches_ephemeral(backend: &Json, op: &str) -> bool {
    let schema = &backend["schema"];
    let Some(program) = backend["programs"]
        .as_array()
        .and_then(|programs| programs.iter().find(|p| p["name"] == op))
    else {
        return false;
    };
    operation_tables(program, schema)
        .iter()
        .any(|table| schema["tables"][table]["ephemeral"].is_object())
}

/// The visible tables the device does not sync (`online only`), sorted and
/// bounded, for the change poll's `tables=`: the server wakes this device
/// for their writes, which log no partition (Snapback LLP 3000.041 §4.1).
pub fn online_only_tables(backend: &Json) -> Vec<String> {
    let Some(tables) = backend["schema"]["tables"].as_object() else { return Vec::new() };
    let mut names: Vec<String> = tables
        .iter()
        .filter(|(_, table)| !table["sync"].is_object() && !table["ephemeral"].is_object())
        .map(|(name, _)| name.clone())
        .collect();
    names.sort();
    names.truncate(256);
    names
}

/// The tables an operation reads or writes, its rules' included, and a
/// timeline's sources for its posts table.
pub fn operation_tables(program: &Json, schema: &Json) -> BTreeSet<String> {
    fn walk(value: &Json, tables: &mut BTreeSet<String>, found: &mut Vec<String>) {
        match value {
            Json::Object(map) => {
                for (key, child) in map {
                    match child {
                        Json::String(table) if key == "table" => {
                            if tables.insert(table.clone()) {
                                found.push(table.clone());
                            }
                        }
                        _ => walk(child, tables, found),
                    }
                }
            }
            Json::Array(items) => items.iter().for_each(|item| walk(item, tables, found)),
            _ => {}
        }
    }
    let mut tables = BTreeSet::new();
    let mut work = Vec::new();
    walk(&program["body"], &mut tables, &mut work);
    while let Some(table) = work.pop() {
        walk(&schema["tables"][&table]["rules"], &mut tables, &mut work);
        for timeline in schema["timelines"].as_array().into_iter().flatten() {
            if timeline["name"]
                .as_str()
                .map(|name| format!("{name}Posts"))
                .as_deref()
                != Some(table.as_str())
            {
                continue;
            }
            for source in [&timeline["posts"][0], &timeline["reposts"][0]] {
                let Some(source) = source.as_str() else {
                    continue;
                };
                let sync = &schema["tables"][source]["sync"];
                if (!sync.is_object() || !sync["horizon"].is_null()) && tables.insert(source.into())
                {
                    work.push(source.into());
                }
            }
        }
    }
    tables
}
