use super::{BakeError, CompileError};
use contract_syntax::Span;
use exact_plan::Plan;
use std::{collections::BTreeMap, path::Path};

pub(super) fn arguments(
    app_root: &Path,
) -> Result<Option<BTreeMap<String, Vec<String>>>, CompileError> {
    let path = app_root.join(".shells/surfaces.json");
    let refusal = |message| CompileError {
        pass: "analyze",
        id: "analyze-surface-declaration".into(),
        message,
        span: Span::default(),
        file: Some(path.as_path().into()),
        related: Box::new([]),
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(refusal(e.to_string())),
    };
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| refusal(e.to_string()))?;
    let declarations = json.as_object().and_then(|surfaces| {
        surfaces
            .iter()
            .map(|(name, args)| {
                let fields: Option<Vec<_>> = args
                    .as_array()?
                    .iter()
                    .map(|arg| Some(arg.get("name")?.as_str()?.to_owned()))
                    .collect();
                Some((name.clone(), fields?))
            })
            .collect()
    });
    declarations.map(Some).ok_or_else(|| {
        refusal(
            "expected surface names mapped to argument declarations with a `name` string".into(),
        )
    })
}

/// Surface records are runner-owned and default before the module loads.
pub(super) fn shape(plan: &Plan) -> Result<(), BakeError> {
    use exact_runner::surface_record::{surface_name, SOURCE};
    for row in plan
        .resources
        .iter()
        .filter(|r| plan.str(r.source) == SOURCE)
    {
        let name = plan.str(row.name);
        let fail = |message: String| BakeError::Lint {
            site: None,
            id: "bake-surface-record",
            message: format!("resource `{name}`: {message}"),
        };
        let surface = surface_name(plan, row).ok_or_else(|| {
            fail(format!(
                "{SOURCE} takes exactly one string-literal surface name"
            ))
        })?;
        if plan.type_(row.ty).kind != exact_plan::TypeKind::Record {
            return Err(fail(format!("{SOURCE} must be `as shape` a record")));
        }
        if !plan.surfaces.iter().any(|s| plan.str(s.name) == surface) {
            return Err(fail(format!("surface `{surface}` is not used by a canvas")));
        }
    }
    Ok(())
}
