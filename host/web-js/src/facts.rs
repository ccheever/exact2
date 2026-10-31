//! The runner's reserved sources the JS runtime answers itself (facts.js):
//! each declared reader's fields, checked against the runner's names.

use crate::emit::{surface_type_json, value_js};
use exact_plan::{Plan, Value};
use std::fmt::Write as _;

/// The reserved sources facts.js answers (LLP 1071 §7): each declared
/// record's fields checked against the runner's names and passed by name;
/// `exactSurface` readers with their surface and shape. Returns the import.
pub fn reserved(plan: &Plan, body: &mut String) -> Result<String, String> {
    use exact_runner::{delivery, page, surface_record, viewport};
    let mut imports = Vec::new();
    for (source, fields, name) in [
        ("exactViewport", viewport::FIELDS, "viewport"),
        ("exactPage", page::FIELDS, "page"),
        ("exactDelivery", &delivery::FIELDS[..], "delivery"),
        ("exactSurface", &[][..], "surfaces"),
        (
            "exactActivities",
            &["activities", "frequentPushesEnabled", "linked"][..],
            "activities",
        ),
    ] {
        // Each reader by its resource name: its fields (by its own shape),
        // with what the build baked for delivery, or its surface and shape.
        let mut readers = Vec::new();
        for r in plan
            .resources
            .iter()
            .filter(|r| plan.str(r.source) == source)
        {
            let resource = plan.str(r.name);
            let t = &plan.types[r.ty.0 as usize];
            if t.kind != exact_plan::TypeKind::Record {
                return Err(format!("resource {resource}: `{source}` answers a record"));
            }
            let entry = if source == "exactSurface" {
                let surface = surface_record::surface_name(plan, r).ok_or_else(|| {
                    format!(
                        "resource {resource}: `exactSurface` takes one string-literal surface name"
                    )
                })?;
                format!(
                    "{},{}",
                    serde_json::to_string(surface).unwrap(),
                    surface_type_json(plan, r.ty)
                )
            } else {
                let mut names = Vec::new();
                for f in t.fields.iter() {
                    let field = plan.str(plan.fields[f.0 as usize].name);
                    if !fields.contains(&field) {
                        return Err(format!("resource {resource}: `{source}` has no `{field}`"));
                    }
                    names.push(serde_json::to_string(field).unwrap());
                }
                let baked = plan.bytes(r.initial);
                if source == "exactDelivery" && !baked.is_empty() {
                    let v = Value::from_bytes(baked).map_err(|e| e.to_string())?;
                    format!("[{}],{}", names.join(","), value_js(&v))
                } else {
                    format!("[{}]", names.join(","))
                }
            };
            readers.push(format!(
                "{}:[{entry}]",
                serde_json::to_string(resource).unwrap()
            ));
        }
        if !readers.is_empty() {
            let _ = write!(body, "${name}({{{}}});", readers.join(","));
            imports.push(format!("{name} as ${name}"));
        }
    }
    let facts = if imports.is_empty() {
        String::new()
    } else {
        format!("import{{{}}}from\"./facts.js\";", imports.join(","))
    };
    // The tools after the modules, whose origins they stand beside (calls.js).
    Ok(facts + &modules(plan, body) + &crate::calls::tools(plan, body))
}

/// The extensions the plan's module names and package views belong to, each
/// once, in order.
pub fn extensions(plan: &Plan) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let named = plan.modules.iter().map(|m| m.extension);
    for e in named.chain(plan.views.iter().map(|v| v.extension)) {
        let e = plan.str(e).to_owned();
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// A native module's sources and sends (LLP 1133.002 D13, §6), which
/// modules.js answers as the runner's `modules.rs` does: each by its
/// source's name, `[extension.name, send, its substitute, its type, when it
/// asks for permission, its options record's type or null, live]`, an enum's
/// members spelled out, so a preferred value outside them is refused; then a
/// loader of each extension's web implementation (`ext-<name>.js`, which
/// the build writes beside the app, LLP 1133.002 D15); then each package
/// view, by its tag, `[extension, displayed, binding props]` (views.js);
/// then the events the root hears.
fn modules(plan: &Plan, body: &mut String) -> String {
    if plan.modules.is_empty() && plan.views.is_empty() && plan.module_events.is_empty() {
        return String::new();
    }
    let rows: Vec<String> = plan
        .modules
        .iter()
        .map(|m| {
            let source = &plan.sources[m.source.0 as usize];
            let key = format!("{}.{}", plan.str(m.extension), plan.str(m.declared));
            let options = (plan.source_params.get(source.params.start as usize))
                .filter(|_| source.params.len > 0)
                .map_or("null".into(), |p| surface_type_json(plan, p.ty));
            format!(
                "{}:[{},{},{},{},{},{options},{}]",
                serde_json::to_string(plan.str(source.name)).unwrap(),
                serde_json::to_string(&key).unwrap(),
                m.send,
                plan.str(m.substitute),
                surface_type_json(plan, source.ty),
                serde_json::to_string(plan.str(m.request)).unwrap(),
                m.live,
            )
        })
        .collect();
    let loaders: Vec<String> = extensions(plan)
        .iter()
        .map(|e| {
            format!(
                "{}:()=>import({})",
                serde_json::to_string(e).unwrap(),
                serde_json::to_string(&format!("./ext-{e}.js")).unwrap()
            )
        })
        .collect();
    let views: Vec<String> = (plan.views.iter())
        .map(|v| {
            let binds: Vec<&str> = plan.str(v.binds).split_whitespace().collect();
            format!(
                "{}:[{},{},{}]",
                serde_json::to_string(plan.str(v.tag)).unwrap(),
                serde_json::to_string(plan.str(v.extension)).unwrap(),
                v.displayed,
                serde_json::to_string(&binds).unwrap()
            )
        })
        .collect();
    // Each event the root hears (LLP 1133.004 D6): its action, taken when
    // the event is heard (the actions are declared after this call), how it
    // is raised, its hold, its parameters' types, its name and a wake's
    // budget (LLP 1133.017 D5).
    let events: Vec<String> = (plan.module_events.iter())
        .map(|e| {
            let action = plan.action(e.action);
            let params: Vec<String> = (action.params.iter())
                .map(|p| surface_type_json(plan, plan.param(p).ty))
                .collect();
            format!(
                "{}:[()=>a_{},{},{},[{}],{},{}]",
                serde_json::to_string(&format!(
                    "{}.{}",
                    plan.str(e.extension),
                    plan.str(e.declared)
                ))
                .unwrap(),
                e.action.0,
                serde_json::to_string(plan.str(e.raised)).unwrap(),
                e.hold,
                params.join(","),
                serde_json::to_string(plan.str(action.name)).unwrap(),
                e.budget
            )
        })
        .collect();
    let _ = write!(
        body,
        "$modules({{{}}},{{{}}},{{{}}},{{{}}});",
        rows.join(","),
        loaders.join(","),
        views.join(","),
        events.join(",")
    );
    "import{modules as $modules}from\"./modules.js\";".into()
}

/// The plan's viewport comparisons for the renderers (LLP 1048.007 D2):
/// `[[query, v => v.<fact> <op> <constant>], …]`, each predicate read
/// against the viewport a render saw; `[]` for a plan that reads no viewport,
/// or that reads it in a way no media query tells (`activation`'s `kept`).
pub fn viewport_media(plan: &Plan) -> String {
    use exact_plan::Opcode;
    use exact_runner::viewport_reads::Constant;
    let Some(media) = exact_runner::activation::wake(plan) else {
        return "[]".into();
    };
    let rows: Vec<String> = media
        .iter()
        .map(|m| {
            let op = match m.op {
                Opcode::Lt => "<",
                Opcode::Le => "<=",
                Opcode::Gt => ">",
                Opcode::Ge => ">=",
                Opcode::Eq => "===",
                _ => "!==",
            };
            let constant = match &m.constant {
                Constant::Number(n) => value_js(&Value::Number(*n)),
                Constant::Str(s) => serde_json::to_string(s).unwrap(),
                Constant::Bool(b) => b.to_string(),
            };
            format!(
                "[{},v=>v.{}{op}{constant}]",
                serde_json::to_string(&m.query).unwrap(),
                m.fact
            )
        })
        .collect();
    format!("[{}]", rows.join(","))
}

/// What an undeclared route's inferred activation asks of a render (LLP
/// 1048.007 D1, `exact_runner::activation`), for `render.mjs`.
pub fn activation(plan: &Plan) -> String {
    use exact_runner::activation::{runtime_kept, REPLAYED, WOKEN};
    let waits: Vec<&str> = REPLAYED.iter().chain(&WOKEN).map(|k| k.name()).collect();
    format!(
        "{{waits:{},kept:{}}}",
        serde_json::to_string(&waits).unwrap(),
        runtime_kept(plan) || exact_runner::activation::wake(plan).is_none()
    )
}
