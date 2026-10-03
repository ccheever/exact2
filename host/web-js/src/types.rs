//! The plan's types as the JavaScript target writes them: a type's zero,
//! the agent's typed JSON, and the type codes the runtime checks values by.

use exact_plan::Plan;

/// The type's zero, as the runner's `zero` makes it.
pub(crate) fn zero(plan: &Plan, ty: exact_plan::TypesId) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "0".into(),
        exact_plan::TypeKind::Bool => "!1".into(),
        // A choice's zero is its first literal (LLP 1035.005.000 D4a).
        exact_plan::TypeKind::String => match t.fields.iter().next() {
            Some(f) => serde_json::to_string(plan.str(plan.fields[f.0 as usize].name)).unwrap(),
            None => "\"\"".into(),
        },
        exact_plan::TypeKind::Unit | exact_plan::TypeKind::Option => "null".into(),
        exact_plan::TypeKind::List => "[]".into(),
        exact_plan::TypeKind::Record => format!(
            "[{}]",
            t.fields
                .iter()
                .map(|f| zero(plan, plan.fields[f.0 as usize].ty))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// A type for the agent's typed JSON: `"n"`, `"b"`, `"s"`, `"u"`,
/// `["?",T]`, `["[",T]`, `{"field":T,…}` in field order.
pub(crate) fn type_json(plan: &Plan, ty: exact_plan::TypesId) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "\"n\"".into(),
        exact_plan::TypeKind::Bool => "\"b\"".into(),
        exact_plan::TypeKind::String => "\"s\"".into(),
        exact_plan::TypeKind::Unit => "\"u\"".into(),
        exact_plan::TypeKind::Option => format!(
            "[\"?\",{}]",
            type_json(plan, t.elem.expect("option element"))
        ),
        exact_plan::TypeKind::List => {
            format!("[\"[\",{}]", type_json(plan, t.elem.expect("list element")))
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
                        type_json(plan, f.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// A type as the data module client reads it: `n` number, `b` bool, `s`
/// string, `(a|b…)` a choice of strings (LLP 1035.005.000 D4a: a literal is
/// spelled without `|` or `)`), `u` unit, `?T` option, `[T` list, `{T…}`
/// record.
pub(crate) fn type_code(plan: &Plan, ty: exact_plan::TypesId) -> String {
    let t = &plan.types[ty.0 as usize];
    match t.kind {
        exact_plan::TypeKind::Number => "n".into(),
        exact_plan::TypeKind::Bool => "b".into(),
        exact_plan::TypeKind::String if t.fields.len > 0 => {
            let literals: Vec<&str> = t
                .fields
                .iter()
                .map(|f| plan.str(plan.fields[f.0 as usize].name))
                .collect();
            format!("({})", literals.join("|"))
        }
        exact_plan::TypeKind::String => "s".into(),
        exact_plan::TypeKind::Unit => "u".into(),
        exact_plan::TypeKind::Option => {
            format!("?{}", type_code(plan, t.elem.expect("option element")))
        }
        exact_plan::TypeKind::List => {
            format!("[{}", type_code(plan, t.elem.expect("list element")))
        }
        exact_plan::TypeKind::Record => {
            let fields: String = t
                .fields
                .iter()
                .map(|f| type_code(plan, plan.fields[f.0 as usize].ty))
                .collect();
            format!("{{{fields}}}")
        }
    }
}
