//! Admission, wire transport, and reserved-source default descriptors.
use exact_plan::Plan;

/// A type for the agent's typed JSON: `"n"`, `"b"`, `"s"`, `"u"`,
/// `["?",T]`, `["[",T]`, `{"field":T,…}` in field order.
pub(crate) fn type_json(plan: &Plan, ty: exact_plan::TypesId) -> String {
    type_json_defaults(plan, ty, false)
}

pub(crate) fn surface_type_json(plan: &Plan, ty: exact_plan::TypesId) -> String {
    type_json_defaults(plan, ty, true)
}

fn type_json_defaults(plan: &Plan, ty: exact_plan::TypesId, defaults: bool) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "\"n\"".into(),
        exact_plan::TypeKind::Bool => "\"b\"".into(),

        exact_plan::TypeKind::String if defaults && !t.fields.is_empty() => format!(
            "[\"e\",{}]",
            serde_json::to_string(
                &t.fields
                    .iter()
                    .map(|f| plan.str(plan.field(f).name))
                    .collect::<Vec<_>>()
            )
            .unwrap()
        ),
        exact_plan::TypeKind::String => "\"s\"".into(),
        exact_plan::TypeKind::Unit => "\"u\"".into(),
        exact_plan::TypeKind::Option => format!(
            "[\"?\",{}]",
            type_json_defaults(plan, t.elem.expect("option element"), defaults)
        ),
        exact_plan::TypeKind::List => {
            format!(
                "[\"[\",{}]",
                type_json_defaults(plan, t.elem.expect("list element"), defaults)
            )
        }
        exact_plan::TypeKind::Record => format!(
            "{{{}}}",
            t.fields
                .iter()
                .map(|f| {
                    let f = &plan.fields[f.0 as usize];
                    format!(
                        "{}:{}",
                        serde_json::to_string(plan.str(f.name)).unwrap(),
                        type_json_defaults(plan, f.ty, defaults)
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// A type as the data module client reads it: `n` number, `b` bool, `s`
/// string, `u` unit, `?T` option, `[T` list, `{T…}` record.
pub(crate) fn type_code(plan: &Plan, ty: exact_plan::TypesId) -> String {
    type_code_numeric(plan, ty, false)
}

pub(crate) fn type_code_numeric(plan: &Plan, ty: exact_plan::TypesId, nonfinite: bool) -> String {
    type_code_kind(plan, ty, nonfinite, false)
}

pub(crate) fn type_code_wire(plan: &Plan, ty: exact_plan::TypesId) -> String {
    type_code_kind(plan, ty, false, true)
}

fn type_code_kind(plan: &Plan, ty: exact_plan::TypesId, nonfinite: bool, wire: bool) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => if nonfinite { "N" } else { "n" }.into(),
        exact_plan::TypeKind::Bool => "b".into(),
        exact_plan::TypeKind::String if !wire && !t.fields.is_empty() => {
            let values = serde_json::to_string(
                &t.fields
                    .iter()
                    .map(|f| plan.str(plan.field(f).name))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
            format!("e{}:{values}", values.encode_utf16().count())
        }
        exact_plan::TypeKind::String => "s".into(),
        exact_plan::TypeKind::Unit => "u".into(),
        exact_plan::TypeKind::Option => {
            format!(
                "?{}",
                type_code_kind(plan, t.elem.expect("option element"), nonfinite, wire)
            )
        }
        exact_plan::TypeKind::List => {
            format!(
                "[{}",
                type_code_kind(plan, t.elem.expect("list element"), nonfinite, wire)
            )
        }
        exact_plan::TypeKind::Record => {
            let fields: String = t
                .fields
                .iter()
                .map(|f| type_code_kind(plan, plan.fields[f.0 as usize].ty, nonfinite, wire))
                .collect();
            format!("{{{fields}}}")
        }
    }
}
