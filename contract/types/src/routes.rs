//! @ref LLP 1038 D2/D3 — one checked table, four positional shapes, encoded paths.

use super::*;

pub(super) fn declare(file: &File, shapes: &mut Shapes) -> Result<(), TypeError> {
    let Some(routes) = &file.routes else {
        return Ok(());
    };
    if file.components.is_empty() {
        return err(
            "analyze-routes-not-root",
            "`routes` belongs to the app's root file",
            routes.span,
        );
    }
    let table = exact_route::Table {
        routes: routes
            .rows
            .iter()
            .map(|r| exact_route::Route {
                name: r.name.clone(),
                pattern: r.pattern.clone(),
                parent: r.parent,
                tab: r.tab,
                notfound: r.notfound,
            })
            .collect(),
    };
    if let Err(e) = table.check() {
        let id = match e.code.as_str() {
            "route-duplicate" => "route-duplicate",
            "route-shadowed" => "route-shadowed",
            "route-parent-param" => "route-parent-param",
            "route-root" => "route-root",
            _ => "route-pattern",
        };
        return err(
            id,
            e.message,
            routes.rows.get(e.route).map_or(routes.span, |r| r.span),
        );
    }
    for s in &file.shapes {
        if matches!(s.name.as_str(), "Router" | "Tab" | "Entry" | "Params") {
            return err(
                "type-shape-reserved",
                format!("`{}` is declared by `routes`", s.name),
                s.span,
            );
        }
    }
    let record = |name: &str| Ty::Record(name.into());
    let list = |ty| Ty::List(Box::new(ty));
    for (name, fields) in [
        (
            "Router",
            vec![
                ("tab", Ty::String),
                ("tabs", list(record("Tab"))),
                ("next", Ty::Number),
            ],
        ),
        (
            "Tab",
            vec![("name", Ty::String), ("stack", list(record("Entry")))],
        ),
        (
            "Entry",
            vec![
                ("id", Ty::Number),
                ("name", Ty::String),
                ("url", Ty::String),
                ("tab", Ty::String),
                ("params", record("Params")),
            ],
        ),
        (
            "Params",
            table
                .param_names()
                .into_iter()
                .map(|n| (n, Ty::String))
                .collect(),
        ),
    ] {
        shapes.map.insert(
            name.into(),
            fields.into_iter().map(|(n, t)| (n.into(), t)).collect(),
        );
    }
    shapes.routes = Some(table);
    Ok(())
}

/// A router value selects the roster overload even when an action has the
/// same name. Ordinary curried action references retain their precedence.
pub(super) fn value_call(name: &str, args: &[Expr], scope: &Scope, shapes: &Shapes) -> bool {
    shapes.routes.is_some()
        && Stdlib::from_name(name).is_some_and(|f| f.params().first() == Some(&"Router"))
        && args
            .first()
            .is_some_and(|e| infer(e, scope, shapes).ok() == Some(Ty::Record("Router".into())))
}

/// Router roster entries require the runtime context declared by `routes`.
pub(super) fn require_table(f: Stdlib, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    let needs_table = f
        .params()
        .iter()
        .copied()
        .chain(std::iter::once(f.returns()))
        .any(|ty| matches!(ty, "Router" | "Entry" | "list<Entry>"));
    if (needs_table || f == Stdlib::EncodeRouteSegment) && shapes.routes.is_none() {
        return err(
            "type-unknown-function",
            format!("declare `routes` to use `{}`", f.name()),
            span,
        );
    }
    Ok(())
}

pub(super) fn location(f: Stdlib, args: &[Expr], shapes: &Shapes) -> Result<(), TypeError> {
    if !matches!(
        f,
        Stdlib::Open | Stdlib::Push | Stdlib::Replace | Stdlib::Go
    ) {
        return Ok(());
    }
    let Some(table) = &shapes.routes else {
        return Ok(());
    };
    match &args[1] {
        Expr::Template(_, span) => err("route-template", "use `path()`", *span),
        Expr::Str(url, span) if table.matches_pattern(url).is_none() => {
            err("route-no-match", format!("no route matches `{url}`"), *span)
        }
        _ => Ok(()),
    }
}

/// Expand the compiler's `path()` call to the ordinary template/roster AST.
/// Numbers use `toString` before the shared segment validator/encoder.
pub fn expand_path(
    args: &[Expr],
    span: Span,
    scope: &Scope,
    shapes: &Shapes,
) -> Result<Expr, TypeError> {
    let unknown = || TypeError {
        id: "route-unknown",
        message: "`path()` needs a known route name and its exact parameter count".into(),
        span,
    };
    let Some(table) = &shapes.routes else {
        return err("route-unknown", "declare `routes` to use `path()`", span);
    };
    let Some(Expr::Str(name, _)) = args.first() else {
        return err(
            "route-unknown",
            "`path()` needs a string-literal route name as its first argument",
            span,
        );
    };
    // Table::path is the arity/name authority, including refusal of notfound.
    table
        .path(
            name,
            &args
                .iter()
                .skip(1)
                .map(|arg| match arg {
                    Expr::Str(value, _) => value.as_str(),
                    _ => "parameter",
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|e| TypeError {
            id: "route-unknown",
            message: e.message,
            span,
        })?;
    let route = table
        .routes
        .iter()
        .find(|r| &r.name == name)
        .ok_or_else(unknown)?;
    let mut values = args.iter().skip(1);
    let mut parts = Vec::new();
    for (i, segment) in route.pattern.split('/').enumerate() {
        if i > 0 {
            parts.push(TemplatePart::Text("/".into()));
        }
        if segment.starts_with(':') {
            let arg = values.next().expect("arity checked");
            let value = match infer(arg, scope, shapes)? {
                Ty::String => arg.clone(),
                Ty::Number => Expr::Call("toString".into(), vec![arg.clone()], arg.span()),
                ty => {
                    return err(
                        "type-argument",
                        format!("`path()` expects a string or number, given `{ty}`"),
                        arg.span(),
                    )
                }
            };
            parts.push(TemplatePart::Expr(Expr::Call(
                "encodeRouteSegment".into(),
                vec![value],
                arg.span(),
            )));
        } else if !segment.is_empty() {
            parts.push(TemplatePart::Text(segment.into()));
        }
    }
    Ok(Expr::Template(parts, span))
}
