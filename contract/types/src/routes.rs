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
    let mut table = exact_route::Table {
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
    // @ref LLP 1116.009 D2 — the `notfound` the compiler writes, beside the
    // table the author wrote and checked: the check above is of that table,
    // so a missing `/` is still refused.
    if contract_syntax::implicit_notfound(file) {
        table.routes.push(exact_route::Route {
            name: "notfound".into(),
            pattern: String::new(),
            parent: None,
            tab: false,
            notfound: true,
        });
    }
    for s in &file.shapes {
        if matches!(
            s.name.as_str(),
            "Router" | "Tab" | "Entry" | "Params" | "SharedFiles"
        ) {
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
        // @ref LLP 1133.024 D3 — what `sharedFiles(entry, name)` answers.
        (
            "SharedFiles",
            vec![
                ("files", list(record("Picked"))),
                ("dropped", Ty::Number),
                ("pending", Ty::Number),
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
/// The router's `push` and `replace` are also the web's list and text
/// spellings (`xs.push(x)`, `s.replace(a, b)`): a call that cannot be the
/// router's says what to write instead (@ref LLP 1088 D7.4).
pub(super) fn not_the_router(
    f: Stdlib,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
    span: Span,
) -> Result<(), TypeError> {
    let list = || {
        args.first()
            .is_some_and(|a| matches!(infer(a, scope, shapes), Ok(Ty::List(_))))
    };
    let why = match f {
        Stdlib::Push if list() => {
            contract_syntax::idioms::refusal("push").expect("refused")
        }
        Stdlib::Replace if args.len() == 3 => "`replace(nav, location)` is the router's; for text, write `s.replaceAll(find, with)`, the web's `String.prototype.replaceAll` (LLP 1088 D2)".into(),
        _ => return Ok(()),
    };
    err("type-refused-idiom", why, span)
}

/// Whether a standard function exists only in an app with a `routes`
/// declaration: the router functions, and `encodeRouteSegment`.
pub fn needs_routes(f: Stdlib) -> bool {
    f.params()
        .iter()
        .copied()
        .chain(std::iter::once(f.returns()))
        .any(|ty| matches!(ty, "Router" | "Entry" | "list<Entry>"))
        || f == Stdlib::EncodeRouteSegment
}

pub(super) fn require_table(f: Stdlib, shapes: &Shapes, span: Span) -> Result<(), TypeError> {
    if needs_routes(f) && shapes.routes.is_none() {
        return err(
            "type-unknown-function",
            format!("declare `routes` to use `{}`", f.name()),
            span,
        );
    }
    Ok(())
}

pub(super) fn location(
    f: Stdlib,
    args: &[Expr],
    scope: &Scope,
    shapes: &Shapes,
) -> Result<(), TypeError> {
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
        Expr::Template(parts, span) => {
            let call = path_call(parts, table, scope, shapes)
                .map(|c| match c.contains('`') {
                    true => format!(": write `` {c} ``"),
                    false => format!(": write `{c}`"),
                })
                .unwrap_or_default();
            err("route-template", format!("a computed location is `path(\"route\", …)`, which encodes each parameter (a `/` or `%` in a template's would make another location){call}"), *span)
        }
        Expr::Str(url, span) if table.matches_pattern(url).is_none() => {
            err("route-no-match", format!("no route matches `{url}`"), *span)
        }
        _ => Ok(()),
    }
}

/// The `path()` a template location spells, when each interpolation is a
/// whole segment of one route's pattern and simple enough to write back:
/// `` `/day/${d.key}` `` is `path("day", d.key)`. A query after it is
/// concatenated, `path("open") + `?u=${u}``.
fn path_call(
    parts: &[TemplatePart],
    table: &exact_route::Table,
    scope: &Scope,
    shapes: &Shapes,
) -> Option<String> {
    // Text that reads the same written back in a string or a template: no
    // escape, quote, backtick or `$` (the generic hint covers the rest).
    fn plain(s: &str) -> bool {
        !s.chars()
            .any(|c| matches!(c, '\\' | '`' | '"' | '$') || c.is_control())
    }
    fn written(e: &Expr) -> Option<String> {
        Some(match e {
            Expr::Ident(n, _) => n.clone(),
            Expr::Member(base, field, _) => format!("{}.{field}", written(base)?),
            Expr::Str(s, _) if plain(s) => format!("\"{s}\""),
            Expr::Number(n, _) => n.to_string(),
            Expr::Call(f, args, _) => {
                let args = args.iter().map(written).collect::<Option<Vec<_>>>()?;
                // A method, and `length`, as the author writes them.
                match (f.as_str(), args.split_first()) {
                    ("length", Some((recv, []))) => format!("{recv}.length"),
                    (m, Some((recv, rest))) if contract_syntax::idioms::is_method(m) => {
                        format!("{recv}.{m}({})", rest.join(", "))
                    }
                    _ => format!("{f}({})", args.join(", ")),
                }
            }
            _ => return None,
        })
    }
    if parts
        .iter()
        .any(|p| matches!(p, TemplatePart::Text(t) if !plain(t)))
    {
        return None;
    }
    // The path's segments, each its pieces, and the query's own text.
    let mut segments: Vec<Vec<Result<String, &Expr>>> = vec![Vec::new()];
    let mut query: Option<String> = None;
    for part in parts {
        match (part, query.as_mut()) {
            (TemplatePart::Text(t), Some(q)) => q.push_str(t),
            (TemplatePart::Expr(e), Some(q)) => q.push_str(&format!("${{{}}}", written(e)?)),
            (TemplatePart::Expr(e), None) => segments.last_mut()?.push(Err(e)),
            (TemplatePart::Text(t), None) => {
                for (i, c) in t.char_indices() {
                    match c {
                        '/' => segments.push(Vec::new()),
                        '?' | '#' => {
                            query = Some(t[i..].to_string());
                            break;
                        }
                        _ => match segments.last_mut()?.last_mut() {
                            Some(Ok(s)) => s.push(c),
                            _ => segments.last_mut()?.push(Ok(String::from(c))),
                        },
                    }
                }
            }
        }
    }
    let route = table.routes.iter().find(|r| {
        let pattern: Vec<&str> = r.pattern.split('/').collect();
        !r.notfound
            && pattern.len() == segments.len()
            && pattern
                .iter()
                .zip(&segments)
                .all(|(p, s)| match s.as_slice() {
                    [Err(_)] => p.starts_with(':'),
                    [Ok(text)] => p.starts_with(':') || *p == text.as_str(),
                    [] => p.is_empty(),
                    _ => false,
                })
    })?;
    let mut args = vec![format!("\"{}\"", route.name)];
    for (p, s) in route.pattern.split('/').zip(&segments) {
        match (p.starts_with(':'), s.as_slice()) {
            (true, [Err(e)]) => {
                let value = written(e)?;
                let ty = infer(e, scope, shapes).ok()?;
                args.push(if ty.is_string() || ty == Ty::Number {
                    value
                } else {
                    format!("`${{{value}}}`")
                });
            }
            (true, [Ok(text)]) => args.push(format!("\"{text}\"")),
            _ => {}
        }
    }
    let query = query.map(|q| format!(" + `{q}`")).unwrap_or_default();
    Some(format!("path({}){query}", args.join(", ")))
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
                ty if ty.is_string() => arg.clone(),
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

/// A route's `pages=` (LLP 1048.000 D2, LLP 1048.003 D5): a source call over
/// values that lists the values of the route's one parameter, `list<string>`.
/// The build asks it once, and the server for its sitemap. Its signature
/// joins the seam's.
pub(super) fn pages_source(
    row: &contract_syntax::RouteDecl,
    field: &contract_syntax::Attr,
    ct: &mut ComponentTypes,
) -> Result<(), TypeError> {
    let params = row
        .pattern
        .split('/')
        .filter(|s| s.starts_with(':'))
        .count();
    if params != 1 {
        return err(
            "type-route-pages",
            format!(
                "`pages=` lists the values of one parameter; route `{}` has {params}",
                row.name
            ),
            field.span,
        );
    }
    let Expr::Call(source, args, _) = &field.value else {
        return err(
            "type-route-pages",
            format!(
                "`pages=` is a source call, `pages=source(…)` (route `{}`)",
                row.name
            ),
            field.span,
        );
    };
    let mut params = Vec::with_capacity(args.len());
    for arg in args {
        params.push(match arg {
            Expr::Number(..) => Ty::Number,
            Expr::Str(..) => Ty::String,
            Expr::Bool(..) => Ty::Bool,
            other => {
                return err(
                    "type-route-pages",
                    format!(
                        "`pages=`'s arguments are values, asked once (route `{}`)",
                        row.name
                    ),
                    other.span(),
                )
            }
        });
    }
    let list = Ty::List(Box::new(Ty::String));
    record_source(ct, source, params, list, field.span)
}
