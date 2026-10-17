//! Compile a widget's reachable component tree with a typed, replaceable
//! props slot. No request, app route, timer or handler enters this plan.

use super::{check, refusal, visit_nodes};
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
    file.routes = None;
    file.tests.clear();
    file.launch.clear();
    file.sounds.clear();
    file.keyframes.clear();
    file.timelines.clear();
    // Styles and functions may be shared with the app; only declarations the
    // entry reaches are checked, so an unrelated app animation is harmless.
    let mut styles = HashSet::new();
    for c in &file.components {
        visit_nodes(&c.view, &mut |n| {
            if let Node::Element { attrs, .. } = n {
                for a in attrs.iter().filter(|a| a.name == "class") {
                    match &a.value {
                        Expr::Ident(name, _) => {
                            styles.insert(name.clone());
                        }
                        Expr::Str(names, _) => {
                            styles.extend(names.split_whitespace().map(str::to_owned))
                        }
                        _ => {}
                    }
                }
            }
        });
    }
    file.styles.retain(|s| styles.contains(&s.name));
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
    let mut fonts = HashSet::new();
    let mut dynamic_font = false;
    let mut font_attrs = |attrs: &[Attr]| {
        for a in attrs.iter().filter(|a| a.name == "font-family") {
            match &a.value {
                Expr::Str(stack, _) => fonts.extend(
                    stack
                        .split(',')
                        .map(|n| n.trim().trim_matches(['\"', '\'']).to_string()),
                ),
                _ => dynamic_font = true,
            }
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
    if !dynamic_font {
        file.fonts.retain(|f| fonts.contains(&f.name));
    }
    check(&file)?;
    for c in &mut file.components {
        c.actions.clear();
    }
    let mut wrapper = contract_syntax::parse("component Snapshot\n  view\n    view\n")
        .expect("wrapper")
        .components
        .remove(0);
    wrapper.name = format!("__snapshot_{}", widget.kind);
    wrapper.states.push(Binding {
        name: "snapshot_props".into(),
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
                    Box::new(Expr::Ident("snapshot_props".into(), span)),
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
    file.components.insert(0, wrapper);
    let strings = match app {
        Some(app) => crate::strings::load(app, &app.join("app.contract"))?,
        None => None,
    };
    let (mut plan, _) =
        crate::compile_file_output(&file, app, strings, false, contract_lower::Profile::Web)?;
    plan.snapshot_entry.clone_from(&widget.kind);
    // The snapshot reader exercises the same validating decoder as its host.
    Plan::decode_snapshot(&plan.encode(), &widget.kind)
        .map_err(|e| vec![fail(format!("invalid snapshot plan: {e:?}"))])
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
