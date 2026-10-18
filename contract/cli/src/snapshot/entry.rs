//! Compile a widget's reachable component tree with a typed, replaceable
//! props slot. No request, timer or handler enters this plan.

use super::{check, referenced_styles, refusal, visit_nodes};
use crate::{manifest::Widget, CompileError, Manifest};
use contract_syntax::{Attr, Binding, Expr, File, Node, Span, TypeExpr};
use exact_plan::Plan;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

/// A compiled entry and the manifest that supplies its placeholder and refresh.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The author declaration.
    pub widget: Widget,
    /// Pure render plan. `snapshot_props` is its typed root props slot.
    pub plan: Plan,
}

/// Compile every widget without changing or compiling the running app's plan.
pub fn compile_widgets(
    app: &Path,
    manifest: &Manifest,
) -> Result<BTreeMap<String, Entry>, Vec<CompileError>> {
    let widgets = manifest
        .widgets()
        .map_err(|m| refusal("snapshot-manifest", m, Span::default()))?;
    if widgets.is_empty() {
        return Ok(BTreeMap::new());
    }
    let app = app
        .canonicalize()
        .map_err(|e| refusal("snapshot-app", e.to_string(), Span::default()))?;
    let app = app.as_path();
    let path = app.join("app.contract");
    let src = crate::read_source(&path)?;
    let (file, sources) = crate::sources::load(&path, &src, app)?;
    let mut entries = BTreeMap::new();
    let mut errors = Vec::new();
    for widget in widgets {
        match compile_file(file.clone(), &widget, Some(app)) {
            Ok(mut plan) => {
                plan.app_id.clone_from(&manifest.id);
                entries.insert(widget.kind.clone(), Entry { widget, plan });
            }
            Err(all) => errors.extend(all.into_iter().map(|e| sources.resolve(e))),
        }
    }
    if errors.is_empty() {
        Ok(entries)
    } else {
        Err(errors)
    }
}

/// Write compiled entries beside the app plan, replacing the prior entry tree.
pub fn write_entries(entries: &BTreeMap<String, Entry>, out: &Path) -> Result<(), String> {
    let directory = out.join("plans");
    if directory.exists() {
        std::fs::remove_dir_all(&directory).map_err(|e| e.to_string())?;
    }
    if !entries.is_empty() {
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let registry: Vec<_> = entries
            .values()
            .map(|e| {
                serde_json::json!({
                    "kind":e.widget.kind, "families":e.widget.families, "preview":e.widget.preview
                })
            })
            .collect();
        std::fs::write(
            directory.join("widgets.json"),
            serde_json::to_vec(&registry).unwrap(),
        )
        .map_err(|e| e.to_string())?;
    }
    for (kind, entry) in entries {
        std::fs::write(directory.join(format!("{kind}.plan")), entry.plan.encode())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The synthesized `exactViewport()` resource is checked as a written one
/// is at bake: every field of the inject's shape is one the viewport fact
/// supplies, with its type (`runner::viewport::FIELDS`).
fn viewport_shape(shape: &TypeExpr, file: &File, at: Span) -> Result<(), Vec<CompileError>> {
    let fail = |m: String| vec![refusal("snapshot-viewport", m, at)];
    let TypeExpr::Named(name, _) = shape else {
        return Err(fail(format!(
            "`{}` takes a shape of the viewport's fields (`width`, `height`, …), not `{shape:?}`",
            super::VIEWPORT_SLOT
        )));
    };
    let Some(declared) = file.shapes.iter().find(|s| &s.name == name) else {
        return Err(fail(format!("`{}`: declare shape `{name}` with the viewport's fields it reads (`width: number`, `height: number`)", super::VIEWPORT_SLOT)));
    };
    for field in &declared.fields {
        let expected = match field.name.as_str() {
            "width" | "height" | "horizontalViewportSegments" | "verticalViewportSegments" => {
                "number"
            }
            "prefersReducedMotion" | "prefersReducedTransparency" => "bool",
            other if exact_runner::viewport::FIELDS.contains(&other) => "string",
            _ => {
                return Err(fail(format!("`{}`: `{name}` has a field `{}` the viewport fact does not supply; its fields are {}", super::VIEWPORT_SLOT, field.name, exact_runner::viewport::FIELDS.join(", "))));
            }
        };
        let written = match &field.ty {
            TypeExpr::Named(n, _) => n.as_str(),
            _ => "",
        };
        if written != expected {
            return Err(fail(format!("`{}`: `{name}.{}` is `{expected}` in the viewport fact; declare it `{}: {expected}`", super::VIEWPORT_SLOT, field.name, field.name)));
        }
    }
    Ok(())
}

/// Compile a source-text fixture's entry under the same profile as the bake.
pub fn compile(src: &str, widget: &Widget) -> Result<Plan, Vec<CompileError>> {
    let file = contract_syntax::parse(src).map_err(CompileError::from)?;
    compile_file(file, widget, None)
}

fn compile_file(
    mut file: File,
    widget: &Widget,
    app: Option<&Path>,
) -> Result<Plan, Vec<CompileError>> {
    let fail = |m: String| refusal("snapshot-entry", m, Span::default());
    let root = file
        .components
        .iter()
        .find(|c| c.name == widget.component)
        .ok_or_else(|| {
            fail(format!(
                "widget {}: {}; name its component in app.contract",
                widget.kind,
                file.unknown_component_message(&widget.component)
            ))
        })?
        .clone();
    let shape = file
        .shapes
        .iter()
        .find(|s| s.name == widget.props)
        .ok_or_else(|| {
            fail(format!(
                "widget {}: declare props shape `{}` in app.contract",
                widget.kind, widget.props
            ))
        })?;
    if shape.fields.len() != root.props.len()
        || shape.fields.iter().any(|f| {
            !root
                .props
                .iter()
                .any(|p| p.name == f.name && p.ty.as_ref().is_some_and(|ty| same_type(ty, &f.ty)))
        })
    {
        return Err(vec![fail(format!("widget {}: `{}` props must match every field and type of `{}`; edit the component props or manifest shape", widget.kind, root.name, widget.props))]);
    }
    let span = root.span;
    let preview = preview(
        &widget.preview,
        &TypeExpr::Named(widget.props.clone(), span),
        &file,
        0,
    )
    .map_err(|m| fail(format!("widget {} preview: {m}", widget.kind)))?;
    let mut reached = HashSet::from([root.name.clone()]);
    loop {
        let before = reached.len();
        for c in &file.components {
            if reached.contains(&c.name) {
                visit_nodes(&c.view, &mut |n| {
                    if let Node::Use { name, .. } = n {
                        reached.insert(name.clone());
                    }
                });
            }
        }
        if reached.len() == before {
            break;
        }
    }
    file.components.retain(|c| reached.contains(&c.name));
    let routes = file.routes.take();
    file.tests.clear();
    file.launch.clear();
    file.sounds.clear();
    file.keyframes.clear();
    file.timelines.clear();
    // Styles and functions may be shared with the app; only declarations the
    // entry reaches are checked, so an unrelated app animation is harmless.
    let (styles, mut errors) = referenced_styles(&file);
    if let Err(refused) = check(&file, &styles) {
        errors.extend(refused);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    file.styles.retain(|s| styles.contains(&s.name));
    // No handler is admitted. Its unreferenced body must not retain helpers.
    for c in &mut file.components {
        c.actions.clear();
    }
    let functions = std::mem::take(&mut file.fns);
    loop {
        let calls = contract_syntax::scope::calls(&mut file);
        let before = file.fns.len();
        for f in &functions {
            if calls.contains(&f.name) && !file.fns.iter().any(|used| used.name == f.name) {
                file.fns.push(f.clone());
            }
        }
        if file.fns.len() == before {
            break;
        }
    }
    let shapes = std::mem::take(&mut file.shapes);
    loop {
        let mut used = contract_syntax::scope::type_names(&mut file);
        used.extend(contract_syntax::scope::calls(&mut file));
        used.insert(widget.props.clone());
        let before = file.shapes.len();
        for s in &shapes {
            if used.contains(&s.name) && !file.shapes.iter().any(|kept| kept.name == s.name) {
                file.shapes.push(s.clone());
            }
        }
        if file.shapes.len() == before {
            break;
        }
    }
    if contract_syntax::scope::calls(&mut file).contains("path") {
        if routes.as_ref().is_some_and(|r| r.slot == super::PROPS_SLOT) {
            return Err(vec![refusal("snapshot-props-name", "rename the `snapshot_props` router; the snapshot reader reserves that binding for entry props", routes.as_ref().unwrap().span)]);
        }
        // A URL shares the app's checked patterns, never its page data sources.
        file.routes = routes.map(|mut routes| {
            for row in &mut routes.rows {
                row.fields.clear();
            }
            routes
        });
    }
    let mut fonts = HashSet::new();
    let mut font_attrs = |attrs: &[Attr]| {
        for a in attrs.iter().filter(|a| a.name == "font-family") {
            literal_fonts(&a.value, &mut fonts);
        }
    };
    for c in &file.components {
        visit_nodes(&c.view, &mut |n| {
            if let Node::Element { attrs, .. } = n {
                font_attrs(attrs);
            }
        });
    }
    for s in &file.styles {
        font_attrs(&s.attrs);
    }
    file.fonts.retain(|f| fonts.contains(&f.name));
    let mut wrapper = contract_syntax::parse("component Snapshot\n  view\n    view\n")
        .expect("wrapper")
        .components
        .remove(0);
    wrapper.name = format!("__snapshot_{}", widget.kind);
    wrapper.states.push(Binding {
        name: super::PROPS_SLOT.into(),
        expr: preview,
        span,
    });
    wrapper.view = vec![Node::Use {
        name: root.name,
        args: root
            .props
            .iter()
            .map(|p| Attr {
                name: p.name.clone(),
                value: Expr::Member(
                    Box::new(Expr::Ident(super::PROPS_SLOT.into(), span)),
                    p.name.clone(),
                    span,
                ),
                span,
                written: true,
            })
            .collect(),
        children: vec![],
        span,
    }];
    // A component that injects `snapshot_viewport` reads the family's content
    // size: the wrapper declares the fact and provides it (LLP 1126 D3). The
    // shape is the injecting components' own, and must be one shape.
    let mut viewport: Option<(TypeExpr, Span)> = None;
    for c in &file.components {
        for inject in c.injects.iter().filter(|p| p.name == super::VIEWPORT_SLOT) {
            let Some(ty) = &inject.ty else {
                return Err(vec![refusal(
                    "snapshot-viewport",
                    format!(
                        "`{}` needs its shape: `inject` with `{}: <shape>` of `width` and `height`",
                        super::VIEWPORT_SLOT,
                        super::VIEWPORT_SLOT
                    ),
                    inject.span,
                )]);
            };
            match &viewport {
                Some((first, _)) if !same_type(first, ty) => {
                    return Err(vec![refusal("snapshot-viewport", format!("`{}` is declared with another shape in `{}`; every component reads the family's size through one shape", super::VIEWPORT_SLOT, c.name), inject.span)]);
                }
                Some(_) => {}
                None => viewport = Some((ty.clone(), inject.span)),
            }
        }
    }
    if let Some((shape, at)) = viewport {
        viewport_shape(&shape, &file, at)?;
        wrapper.resources.push(contract_syntax::ResourceDecl {
            name: super::VIEWPORT_SLOT.into(),
            source: exact_runner::viewport::SOURCE.into(),
            args: vec![],
            identity: None,
            shape,
            placeholder: None,
            span: at,
        });
        wrapper.provides.push(Binding {
            name: super::VIEWPORT_SLOT.into(),
            expr: Expr::Ident(super::VIEWPORT_SLOT.into(), at),
            span: at,
        });
    }
    file.components.insert(0, wrapper);
    super::paragraphs::check(&file, &widget.families)?;
    let strings = match app {
        Some(app) => crate::strings::load(app, &app.join("app.contract"))?,
        None => None,
    };
    let (plan, _) =
        crate::compile_file_output(&file, app, strings, false, contract_lower::Profile::Web)?;
    let props = plan
        .slots
        .iter()
        .find(|slot| plan.str(slot.name) == super::PROPS_SLOT)
        .expect("props slot")
        .ty;
    let mut builder = exact_plan::builder::PlanBuilder::from_plan(plan);
    let number = builder.primitive(exact_plan::TypeKind::Number);
    let entry = builder.record(
        "SnapshotTimelineEntry",
        &[("date", number), ("props", props)],
    );
    let timeline = builder.list(entry);
    let refresh = builder.source(&widget.refresh, &[number], timeline);
    let mut plan = builder
        .finish()
        .map_err(|e| vec![fail(format!("invalid refresh signature: {e:?}"))])?;
    plan.snapshot_entry.clone_from(&widget.kind);
    plan.snapshot_refresh = Some(refresh);
    // The snapshot reader exercises the same validating decoder as its host.
    let plan = Plan::decode_snapshot(&plan.encode(), &widget.kind)
        .map_err(|e| vec![fail(format!("invalid snapshot plan: {e:?}"))])?;
    super::inline::check(&plan, &widget.families, span)?;
    Ok(plan)
}

fn same_type(a: &TypeExpr, b: &TypeExpr) -> bool {
    match (a, b) {
        (TypeExpr::Named(a, _), TypeExpr::Named(b, _)) => a == b,
        (TypeExpr::Enum(a, _), TypeExpr::Enum(b, _)) => a == b,
        (TypeExpr::List(a, _), TypeExpr::List(b, _))
        | (TypeExpr::Option(a, _), TypeExpr::Option(b, _)) => same_type(a, b),
        _ => false,
    }
}

fn preview(v: &Value, ty: &TypeExpr, file: &File, depth: usize) -> Result<Expr, String> {
    if depth > 64 {
        return Err("props nest more than 64 levels".into());
    }
    let span = ty.span();
    let bad = || format!("{v} does not match {ty:?}; supply preview props of the declared shape");
    Ok(match ty {
        TypeExpr::Named(name, _) => match name.as_str() {
            "number" => Expr::Number(v.as_f64().ok_or_else(bad)?, span),
            "string" => Expr::Str(v.as_str().ok_or_else(bad)?.into(), span),
            "bool" => Expr::Bool(v.as_bool().ok_or_else(bad)?, span),
            _ => {
                let shape = file
                    .shapes
                    .iter()
                    .find(|s| s.name == *name)
                    .ok_or_else(bad)?;
                let object = v
                    .as_object()
                    .filter(|o| o.len() == shape.fields.len())
                    .ok_or_else(bad)?;
                let args = shape
                    .fields
                    .iter()
                    .map(|f| {
                        let value = object
                            .get(&f.name)
                            .ok_or_else(|| format!("missing `{}`; add it to preview", f.name))?;
                        Ok(Expr::NamedArg(
                            f.name.clone(),
                            Box::new(preview(value, &f.ty, file, depth + 1)?),
                            span,
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                Expr::Call(name.clone(), args, span)
            }
        },
        TypeExpr::Enum(values, _) => Expr::Str(
            v.as_str()
                .filter(|s| values.iter().any(|v| v == s))
                .ok_or_else(bad)?
                .into(),
            span,
        ),
        TypeExpr::List(inner, _) => Expr::Typed(
            Box::new(Expr::List(
                v.as_array()
                    .ok_or_else(bad)?
                    .iter()
                    .map(|v| preview(v, inner, file, depth + 1))
                    .collect::<Result<_, _>>()?,
                span,
            )),
            ty.clone(),
            span,
        ),
        TypeExpr::Option(inner, _) => Expr::Typed(
            Box::new(if v.is_null() {
                Expr::None(span)
            } else {
                Expr::Some(Box::new(preview(v, inner, file, depth + 1)?), span)
            }),
            ty.clone(),
            span,
        ),
    })
}

fn literal_fonts(e: &Expr, fonts: &mut HashSet<String>) {
    match e {
        Expr::Str(stack, _) => fonts.extend(
            stack
                .split(',')
                .map(|n| n.trim().trim_matches(['"', '\'']).to_string()),
        ),
        Expr::Ternary(_, yes, no, _) => {
            literal_fonts(yes, fonts);
            literal_fonts(no, fonts);
        }
        Expr::Match { some, none, .. } => {
            literal_fonts(some, fonts);
            literal_fonts(none, fonts);
        }
        Expr::Let { body, .. } => literal_fonts(body, fonts),
        _ => {}
    }
}
