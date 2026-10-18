//! The refresh uses the data seam's source types, without a render resource.
use super::{compile_widgets, Entry};
use crate::{BakeError, DataSource, Manifest};
use exact_plan::{builder::PlanBuilder, Plan, TypeKind, TypesId, Value};
use exact_runner::{Answer, DataError, Store};
use std::{collections::BTreeMap, path::Path};

/// Compile the entries once, for declarations, validation and the bundle receipt.
pub fn entries(app: &Path) -> Result<BTreeMap<String, Entry>, String> {
    compile_widgets(app, &Manifest::read(app)?).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Check immediate refresh answers; pending and unavailable answers are checked
/// by their source declarations. No host executes requests during this check.
/// Epoch zero is the bake's unknown date, as on the app's data seam.
pub fn check_refreshes(
    entries: &BTreeMap<String, Entry>,
    data: &mut impl DataSource,
) -> Result<(), String> {
    for (kind, entry) in entries {
        let plan = &entry.plan;
        let signature = plan.source(plan.snapshot_refresh.expect("refresh signature"));
        let name = plan.str(signature.name);
        data.bind(plan);
        let mut store = Store::new(data.grants(), []);
        match data.answer(&mut store, name, &[Value::Number(0.0)]) {
            Ok(Answer::Now(value)) => {
                if let Some(mismatch) = mismatch(plan, signature.ty, &value, "result") {
                    return Err(format!("widget {kind}: refresh `{name}` {mismatch}; repair the data module to match its declared source signature"));
                }
            }
            Err(DataError::UnknownSource(_)) => {
                return Err(format!(
                "widget {kind}: unknown refresh `{name}`; implement the declared refresh source"
            ))
            }
            Ok(Answer::Later(request)) => {
                if let Some(token) = request.continuation {
                    data.discard(token);
                }
            }
            // A refresh that fails at bake fails on the device too: nothing
            // declares a bad argument, a broken seam or a thrown failure away.
            Err(
                DataError::BadArguments(why)
                | DataError::Interface(why)
                | DataError::Failed(_, why, _),
            ) => {
                return Err(format!(
                    "widget {kind}: refresh `{name}` failed at bake: {why}; repair the data module"
                ));
            }
            // An async source or a Rust source seeded away in the dev producer
            // is checked by the emitted Sources signature, not a fake response.
            // A Rust source's own error is also `Unavailable`; the device sees it.
            Err(_) => {}
        }
    }
    Ok(())
}

/// Bake the app with a separate data instance from the refresh check. A refresh
/// can mutate module state without changing the app's compiled resource values.
pub fn bake_app(
    entries: &BTreeMap<String, Entry>,
    plan: Plan,
    data: impl DataSource,
    mut refresh_data: impl DataSource,
) -> Result<Plan, BakeError> {
    check_refreshes(entries, &mut refresh_data).map_err(|message| BakeError::Lint {
        id: "snapshot-refresh",
        message,
        site: None,
    })?;
    drop(refresh_data);
    crate::bake(plan, data)
}

fn mismatch(plan: &Plan, ty: TypesId, value: &Value, at: &str) -> Option<String> {
    if value.conforms(plan, ty) {
        return None;
    }
    let row = plan.type_(ty);
    match (row.kind, value) {
        (TypeKind::List, Value::List(items)) => items
            .iter()
            .enumerate()
            .find_map(|(i, value)| mismatch(plan, row.elem.unwrap(), value, &format!("{at}[{i}]"))),
        (TypeKind::Option, Value::Option(Some(value))) => {
            mismatch(plan, row.elem.unwrap(), value, at)
        }
        (TypeKind::Record, Value::Record(values)) => {
            let fields: Vec<_> = row.fields.iter().map(|id| plan.field(id)).collect();
            if values.len() < fields.len() {
                return Some(format!(
                    "{at} is missing field `{}` (expected {} fields, got {})",
                    plan.str(fields[values.len()].name),
                    fields.len(),
                    values.len()
                ));
            }
            if values.len() > fields.len() {
                return Some(format!(
                    "{at} has {} fields, expected {}",
                    values.len(),
                    fields.len()
                ));
            }
            // Values are positional at the data seam. Diagnose order only if
            // a permutation of these values satisfies every declared field.
            let mut remaining: Vec<_> = values.iter().collect();
            let reordered = fields.iter().all(|f| {
                if let Some(i) = remaining.iter().position(|v| v.conforms(plan, f.ty)) {
                    remaining.remove(i);
                    true
                } else {
                    false
                }
            });
            if reordered {
                return Some(format!(
                    "{at} has the wrong field order; expected {{{}}}",
                    fields
                        .iter()
                        .map(|f| plan.str(f.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            fields.iter().zip(values.iter()).find_map(|(f, value)| {
                mismatch(plan, f.ty, value, &format!("{at}.{}", plan.str(f.name)))
            })
        }
        _ => Some(format!(
            "{at} expected {:?}, got {}",
            row.kind,
            match value {
                Value::Number(n) if !n.is_finite() => "a non-finite number",
                Value::Number(_) => "number",
                Value::Bool(_) => "bool",
                Value::HeapStr(_) | Value::InlineStr(_) => "string",
                Value::Unit => "unit",
                Value::Option(_) => "option",
                Value::List(_) => "list",
                Value::Record(_) => "record",
            }
        )),
    }
}

/// The combined data contract for tooling such as TypeScript declarations.
/// This copy is not the app's shipped render plan.
pub fn source_plan(entries: &BTreeMap<String, Entry>, plan: &Plan) -> Result<Plan, String> {
    let mut builder = PlanBuilder::from_plan(plan.clone());
    let mut signatures = BTreeMap::new();
    for source in &plan.sources {
        signatures.insert(plan.str(source.name).to_string(), signature(plan, source));
    }
    for entry in entries.values() {
        let from = &entry.plan;
        let source = from.source(from.snapshot_refresh.expect("refresh signature"));
        let name = from.str(source.name);
        let wanted = signature(from, source);
        if let Some(existing) = signatures.get(name) {
            if existing != &wanted {
                return Err(format!("refresh `{name}` and the app declare different source signatures; make their parameters and result agree"));
            }
            continue;
        }
        signatures.insert(name.into(), wanted);
        let params: Vec<_> = source
            .params
            .iter()
            .map(|id| copy_type(&mut builder, from, from.source_param(id).ty))
            .collect();
        let result = copy_type(&mut builder, from, source.ty);
        builder.source(name, &params, result);
    }
    builder
        .finish()
        .map_err(|e| format!("invalid entry source contract: {e:?}"))
}

fn signature(plan: &Plan, source: &exact_plan::SourcesRow) -> String {
    fn shape(plan: &Plan, id: TypesId) -> String {
        let ty = plan.type_(id);
        format!(
            "{:?}({})[{}]",
            ty.kind,
            ty.elem.map(|id| shape(plan, id)).unwrap_or_default(),
            ty.fields
                .iter()
                .map(|id| {
                    let f = plan.field(id);
                    format!("{:?}:{}", plan.str(f.name), shape(plan, f.ty))
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    }
    format!(
        "({})->{}",
        source
            .params
            .iter()
            .map(|id| shape(plan, plan.source_param(id).ty))
            .collect::<Vec<_>>()
            .join(","),
        shape(plan, source.ty)
    )
}

fn copy_type(builder: &mut PlanBuilder, from: &Plan, id: TypesId) -> TypesId {
    let ty = from.type_(id);
    match ty.kind {
        TypeKind::Record => {
            let fields: Vec<_> = ty
                .fields
                .iter()
                .map(|id| {
                    let f = from.field(id);
                    (from.str(f.name), copy_type(builder, from, f.ty))
                })
                .collect();
            builder.record(from.str(ty.name), &fields)
        }
        TypeKind::List | TypeKind::Option => {
            let inner = copy_type(builder, from, ty.elem.expect("element"));
            if ty.kind == TypeKind::List {
                builder.list(inner)
            } else {
                builder.option(inner)
            }
        }
        TypeKind::String if !ty.fields.is_empty() => builder.string_enum(
            &ty.fields
                .iter()
                .map(|id| from.str(from.field(id).name).to_owned())
                .collect::<Vec<_>>(),
        ),
        primitive => builder.primitive(primitive),
    }
}
