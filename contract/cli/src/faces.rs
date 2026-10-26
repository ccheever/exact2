//! A tool's signature, as a face compiled into a binary depends on it (LLP
//! 1133.023 D8): what the generated door compiles from the plan. A plan
//! whose faced tool changed signature has another compatibility id, and the
//! digest the door carries is checked against the plan's at each call.

use exact_plan::{Plan, TypeKind, TypesId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Tool `i`'s signature: its name, description, consequence, modifiers,
/// parameters by name and type, and the type of what it returns.
pub fn tool_signature(plan: &Plan, i: usize) -> Value {
    let t = &plan.tools[i];
    let action = plan.action(t.action);
    let params: Vec<Value> = (action.params.iter())
        .map(|p| {
            let p = plan.param(p);
            json!([plan.str(p.name), type_text(plan, p.ty)])
        })
        .collect();
    let returns = t
        .returns
        .map(|d| type_text(plan, plan.derives[d.0 as usize].ty));
    json!({
        "name": plan.str(t.name),
        "description": plan.str(t.description),
        "consequence": plan.str(t.consequence),
        "modifiers": [t.additive, t.idempotent, t.closed],
        "parameters": params,
        "returns": returns,
    })
}

/// The digest of tool `i`'s signature with `face`, the manifest's face
/// for it: hex SHA-256 of the two as canonical JSON.
pub fn face_digest(plan: &Plan, i: usize, face: &Value) -> String {
    let mut face = face.clone();
    if let Some(f) = face.as_object_mut() {
        f.remove("digest");
    }
    let both = json!({ "signature": tool_signature(plan, i), "face": face });
    let digest = Sha256::digest(canonical(&both).as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Type `ty` as text: `number`, `option<string>`, `Book{id:string}[]`.
pub fn type_text(plan: &Plan, ty: TypesId) -> String {
    let row = &plan.types[ty.0 as usize];
    let elem = || row.elem.map_or_else(|| "?".into(), |e| type_text(plan, e));
    match row.kind {
        TypeKind::Number => "number".into(),
        TypeKind::Bool => "bool".into(),
        TypeKind::String => "string".into(),
        TypeKind::Unit => "unit".into(),
        TypeKind::Option => format!("option<{}>", elem()),
        TypeKind::List => format!("{}[]", elem()),
        TypeKind::Record => {
            let start = row.fields.start as usize;
            let fields: Vec<String> = (plan.fields[start..start + row.fields.len as usize].iter())
                .map(|f| format!("{}:{}", plan.str(f.name), type_text(plan, f.ty)))
                .collect();
            format!("{}{{{}}}", plan.str(row.name), fields.join(","))
        }
    }
}

/// `v` as JSON with every object's keys sorted.
fn canonical(v: &Value) -> String {
    match v {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .into_iter()
                .map(|k| format!("{}:{}", Value::String(k.clone()), canonical(&map[k])))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}
