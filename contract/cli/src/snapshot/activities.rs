//! One state and one plan for all of an activity's placements (LLP 1126.001 WP8).
use super::{entry::RenderEntry, refusal, Declaration, Entry, STATE_SLOT};
use crate::{manifest::Activity, CompileError};
use contract_syntax::{Expr, File, Node, Span};
use exact_plan::{ActivitySlotsRow, NodesId, Plan, TypeKind, TypesId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

/// ActivityKit's placement identities, never an app-visible switching value.
pub const PLACEMENTS: [&str; 8] = [
    "lockScreen",
    "compactLeading",
    "compactTrailing",
    "minimal",
    "expanded.leading",
    "expanded.trailing",
    "expanded.center",
    "expanded.bottom",
];

/// Compile a source fixture under the activity profile used by the bake.
pub fn compile_activity(src: &str, activity: &Activity) -> Result<Plan, Vec<CompileError>> {
    compile_file(
        contract_syntax::parse(src).map_err(CompileError::from)?,
        activity,
        None,
    )
}

pub(super) fn compile_file(
    mut file: File,
    activity: &Activity,
    app: Option<&Path>,
) -> Result<Plan, Vec<CompileError>> {
    let fail = |m| refusal("snapshot-placement", m, Span::default());
    let root = file
        .components
        .iter_mut()
        .find(|c| c.name == activity.component)
        .ok_or_else(|| {
            fail(format!(
                "activity {}: declare component `{}` in app.contract",
                activity.kind, activity.component
            ))
        })?;
    // `slot` is HTML's placement attribute. It is consumed here, before ordinary
    // lowering, and never reaches the kernel as a presentation property.
    fn slots(
        nodes: Vec<Node>,
        prefix: &str,
        found: &mut BTreeMap<String, Node>,
    ) -> Result<(), CompileError> {
        for mut node in nodes {
            let Node::Element {
                tag,
                attrs,
                children,
                positional,
                span,
                ..
            } = &mut node
            else {
                return Err(refusal("snapshot-placement", "declare each placement with `view slot=\"lockScreen\"` (and `view slot=\"expanded\"` with its four regions)", node.span()));
            };
            let names: Vec<_> = attrs.iter().filter(|a| a.name == "slot").collect();
            let name = match names.as_slice() {
                [a] => match &a.value {
                    Expr::Str(s, _) => s.clone(),
                    _ => String::new(),
                },
                _ => String::new(),
            };
            let full = format!("{prefix}{name}");
            if tag != "view"
                || !positional.is_empty()
                || (full != "expanded" && !PLACEMENTS.contains(&full.as_str()))
            {
                return Err(refusal("snapshot-placement", format!("unknown placement `{full}`; use lockScreen, compactLeading, compactTrailing, minimal or expanded's leading, trailing, center and bottom"), *span));
            }
            attrs.retain(|a| a.name != "slot");
            if full == "expanded" {
                if !attrs.is_empty() || children.is_empty() {
                    return Err(refusal(
                        "snapshot-placement",
                        "expanded only groups regions; put layout attributes on each region's view",
                        *span,
                    ));
                }
                slots(std::mem::take(children), "expanded.", found)?;
            } else if found.insert(full.clone(), node).is_some() {
                return Err(refusal(
                    "snapshot-placement",
                    format!("placement `{full}` repeats; keep one view per slot"),
                    Span::default(),
                ));
            }
        }
        Ok(())
    }
    let mut found = BTreeMap::new();
    slots(std::mem::take(&mut root.view), "", &mut found)?;
    for name in PLACEMENTS {
        root.view.push(found.remove(name).ok_or_else(|| {
            fail(format!(
                "missing placement `{name}`; add its `view slot=…` to the activity component"
            ))
        })?);
    }
    let mut container = contract_syntax::parse("component Slots\n  view\n    view\n")
        .expect("slot container")
        .components
        .remove(0)
        .view
        .remove(0);
    if let Node::Element { children, .. } = &mut container {
        *children = std::mem::take(&mut root.view);
    }
    root.view.push(container);
    let plan = super::entry::compile_file(
        file,
        &RenderEntry {
            kind: &activity.kind,
            component: &activity.component,
            props: &activity.state,
            preview: &activity.preview,
            families: &[],
            refresh: None,
        },
        app,
    )?;
    let shape = shape_digest(&plan);
    payload_bytes(&activity.kind, &"0".repeat(64), &shape, &activity.preview).map_err(|m| {
        refusal(
            "snapshot-activity-size",
            format!("activity {} preview: {m}", activity.kind),
            Span::default(),
        )
    })?;
    Ok(plan)
}

pub(super) fn placements(plan: &mut Plan) -> Result<(), Vec<CompileError>> {
    let roots: Vec<_> = plan
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent == Some(NodesId(0)) && n.arm.is_none())
        .map(|(i, _)| NodesId(i as u32))
        .collect();
    if roots.len() != PLACEMENTS.len() {
        return Err(vec![refusal(
            "snapshot-placement",
            "each activity slot needs one view root",
            Span::default(),
        )]);
    }
    for (name, root) in PLACEMENTS.into_iter().zip(roots) {
        let name = if let Some(i) = plan.strings.iter().position(|s| s == name) {
            exact_plan::StrId(i as u32)
        } else {
            let i = exact_plan::StrId(plan.strings.len() as u32);
            plan.strings.push(name.into());
            i
        };
        plan.activity_slots.push(ActivitySlotsRow { name, root });
    }
    Ok(())
}

fn state_type(plan: &Plan) -> TypesId {
    plan.slots
        .iter()
        .find(|s| plan.str(s.name) == STATE_SLOT && s.owner.is_none())
        .expect("activity state")
        .ty
}

/// JSON Schema 2020-12 for the state carried in the universal envelope.
pub fn state_schema(plan: &Plan) -> Value {
    fn schema(plan: &Plan, id: TypesId) -> Value {
        let row = plan.type_(id);
        match row.kind {
            TypeKind::Number => json!({"type":"number"}),
            TypeKind::Bool => json!({"type":"boolean"}),
            TypeKind::Unit => json!({"type":"null"}),
            TypeKind::String if row.fields.is_empty() => json!({"type":"string"}),
            TypeKind::String => {
                json!({"type":"string", "enum":row.fields.iter().map(|i| plan.str(plan.field(i).name)).collect::<Vec<_>>()})
            }
            TypeKind::Option => json!({"anyOf":[schema(plan, row.elem.unwrap()), {"type":"null"}]}),
            TypeKind::List => json!({"type":"array", "items":schema(plan, row.elem.unwrap())}),
            TypeKind::Record => {
                let fields: Vec<_> = row.fields.iter().map(|i| plan.field(i)).collect();
                let properties: serde_json::Map<_, _> = fields
                    .iter()
                    .map(|f| (plan.str(f.name).to_string(), schema(plan, f.ty)))
                    .collect();
                json!({"type":"object", "properties":properties, "required":fields.iter().map(|f| plan.str(f.name)).collect::<Vec<_>>(), "additionalProperties":false})
            }
        }
    }
    let mut schema = schema(plan, state_type(plan));
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema
}

/// Hash the canonical schema, including each record's ordered `required` list.
pub fn shape_digest(plan: &Plan) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&state_schema(plan)).unwrap())
    )
}

/// The encoded static attributes and dynamic state share ActivityKit's 4 KiB limit.
/// Use UTF-8 bytes, including JSON escaping and both copies of the shape digest.
pub fn payload_bytes(
    kind: &str,
    generation: &str,
    shape: &str,
    state: &Value,
) -> Result<usize, String> {
    let attributes = json!({"kind":kind, "generation":generation, "shape":shape});
    let content = json!({"shape":shape, "state":state});
    let bytes = serde_json::to_vec(&attributes).unwrap().len()
        + serde_json::to_vec(&content).unwrap().len();
    if bytes > 4096 {
        Err(format!("encoded attributes and state use {bytes} bytes; the limit is 4096 bytes; shorten the state"))
    } else {
        Ok(bytes)
    }
}

fn typescript(plan: &Plan, id: TypesId) -> String {
    let row = plan.type_(id);
    match row.kind {
        TypeKind::Number => "number".into(),
        TypeKind::Bool => "boolean".into(),
        TypeKind::Unit => "null".into(),
        TypeKind::String if row.fields.is_empty() => "string".into(),
        TypeKind::String => row
            .fields
            .iter()
            .map(|i| serde_json::to_string(plan.str(plan.field(i).name)).unwrap())
            .collect::<Vec<_>>()
            .join(" | "),
        TypeKind::Option => format!("({} | null)", typescript(plan, row.elem.unwrap())),
        TypeKind::List => format!("({})[]", typescript(plan, row.elem.unwrap())),
        TypeKind::Record => format!(
            "{{ {} }}",
            row.fields
                .iter()
                .map(|i| {
                    let f = plan.field(i);
                    format!(
                        "{}: {};",
                        serde_json::to_string(plan.str(f.name)).unwrap(),
                        typescript(plan, f.ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

pub(super) fn write(entries: &BTreeMap<String, Entry>, out: &Path) -> Result<(), String> {
    let dir = out.join("activities");
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    let mut registry = Vec::new();
    for (kind, entry) in entries {
        let Declaration::Activity(a) = &entry.declaration else {
            continue;
        };
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let schema = state_schema(&entry.plan);
        let digest = shape_digest(&entry.plan);
        std::fs::write(
            dir.join(format!("{kind}.schema.json")),
            serde_json::to_vec_pretty(&schema).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        let ts = format!("// @generated from Contract; do not edit.\nexport type State = {};\nexport type Attributes = {{ kind: {}; generation: string; shape: {} }};\nexport type ContentState = {{ shape: {}; state: State }};\n", typescript(&entry.plan, state_type(&entry.plan)), serde_json::to_string(kind).unwrap(), serde_json::to_string(&digest).unwrap(), serde_json::to_string(&digest).unwrap());
        std::fs::write(dir.join(format!("{kind}.d.ts")), ts).map_err(|e| e.to_string())?;
        registry.push(
            json!({"kind":kind, "state":a.state, "shape":digest, "preview":a.preview,
            "push":a.push, "frequent":a.frequent, "title":a.title, "description":a.description,
            "slots":PLACEMENTS}),
        );
    }
    if !registry.is_empty() {
        std::fs::write(
            out.join("plans/activities.json"),
            serde_json::to_vec(&registry).unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
