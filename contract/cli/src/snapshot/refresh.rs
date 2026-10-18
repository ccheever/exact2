//! The refresh uses the data seam's source types, without a render resource.
use super::{compile_widgets, Entry};
use crate::{BakeError, DataSource, Manifest};
use exact_plan::{builder::PlanBuilder, Plan, TypeKind, TypesId, Value};
use std::{collections::BTreeMap, path::Path};

fn entries(app: &Path) -> Result<BTreeMap<String, Entry>, String> {
    compile_widgets(app, &Manifest::read(app)?).map_err(|errors| {
        errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Bind each refresh's declared signature and check its bake-time answer.
/// Epoch zero is the bake's unknown date, as on the app's data seam.
pub fn check_refreshes(app: &Path, data: &mut impl DataSource) -> Result<(), String> {
    for (kind, entry) in entries(app)? {
        let plan = &entry.plan;
        let signature = plan.source(plan.snapshot_refresh.expect("refresh signature"));
        let name = plan.str(signature.name);
        data.bind(plan);
        let value = data.query(name, &[Value::Number(0.0)])
            .map_err(|e| format!("widget {kind}: refresh `{name}` failed: {e:?}; implement the declared refresh source"))?;
        if !value.conforms(plan, signature.ty) {
            return Err(format!("widget {kind}: refresh `{name}` must return {{date: number, props: {}}}[] in that field order; repair the data module or entry props shape", entry.widget.props));
        }
    }
    Ok(())
}

/// Bake the app after checking its entries against the same data implementation.
/// Entry signatures never enter the app plan's encoded bytes.
pub fn bake_app(app: &Path, plan: Plan, mut data: impl DataSource) -> Result<Plan, BakeError> {
    check_refreshes(app, &mut data).map_err(|message| BakeError::Lint {
        id: "snapshot-refresh",
        message,
        site: None,
    })?;
    crate::bake(plan, data)
}

/// The combined data contract for tooling such as TypeScript declarations.
/// This copy is not the app's shipped render plan.
pub fn source_plan(app: &Path, plan: &Plan) -> Result<Plan, String> {
    let mut builder = PlanBuilder::from_plan(plan.clone());
    let mut signatures = BTreeMap::new();
    for source in &plan.sources {
        signatures.insert(plan.str(source.name).to_string(), signature(plan, source));
    }
    for entry in entries(app)?.values() {
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
