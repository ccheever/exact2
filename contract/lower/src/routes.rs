//! @ref LLP 1038 D2/D3 — the table and launch slot; paths use ordinary templates.

use super::*;

impl Lowerer<'_> {
    pub(super) fn declare_routes(&mut self, file: &File) {
        let Some(routes) = &file.routes else { return };
        for row in &routes.rows {
            let id = self.b.route(
                &row.name,
                &row.pattern,
                row.parent.map(|p| exact_plan::RoutesId(p as u32)),
                row.tab,
                row.notfound,
            );
            match route_policy(row) {
                Ok((render, activate, paint)) => {
                    self.b.set_route_policy(id, render, activate, paint)
                }
                Err(e) => self.errors.push(e),
            }
            // @ref LLP 1048.000 D2 — the source listing its pages, and its
            // arguments (values, as types checked).
            let pages = row.fields.iter().find(|f| f.name == "pages");
            if let Some(Expr::Call(source, args, _)) = pages.map(|f| &f.value) {
                let args: Vec<Value> = args
                    .iter()
                    .map(|arg| match arg {
                        Expr::Number(n, _) => Value::Number(*n),
                        Expr::Str(s, _) => Value::str(s),
                        Expr::Bool(b, _) => Value::Bool(*b),
                        _ => Value::Unit,
                    })
                    .collect();
                self.b.set_route_pages(id, source, &args);
            }
        }
        // expand inserted this root slot before all authored/lifted states.
        self.b.set_router(self.slots[0]);
    }

    pub(crate) fn path_expr(
        &self,
        args: &[Expr],
        span: Span,
        scope: &Scope,
    ) -> Result<Expr, LowerError> {
        contract_types::routes::expand_path(args, span, scope, &self.types.shapes).map_err(|e| {
            LowerError {
                id: e.id,
                message: e.message,
                span: e.span,
            }
        })
    }
}

/// A route's policy fields (LLP 1048.003 D5): `render=client|build|cached|
/// request`, undeclared `client`; `activate=idle|never|interaction`, inferred when
/// undeclared; `paint=settled|boot` (LLP 1048.005), undeclared `settled`. Each
/// value is a word, read by its spelling.
fn route_policy(
    row: &contract_syntax::RouteDecl,
) -> Result<
    (
        exact_plan::RenderPolicy,
        exact_plan::ActivatePolicy,
        exact_plan::PaintPolicy,
    ),
    LowerError,
> {
    use exact_plan::{ActivatePolicy, PaintPolicy, RenderPolicy};
    let mut render = RenderPolicy::Client;
    let mut activate = ActivatePolicy::Inferred;
    let mut paint = PaintPolicy::Settled;
    for field in &row.fields {
        let word = match &field.value {
            Expr::Ident(word, _) => word.as_str(),
            _ => "",
        };
        let refusal = match (field.name.as_str(), word) {
            ("render", "client") => {
                render = RenderPolicy::Client;
                continue;
            }
            ("render", "build") => {
                render = RenderPolicy::Build;
                continue;
            }
            ("render", "cached") => {
                render = RenderPolicy::Cached;
                continue;
            }
            ("render", "request") => {
                render = RenderPolicy::Request;
                continue;
            }
            ("activate", "idle") => {
                activate = ActivatePolicy::Idle;
                continue;
            }
            ("activate", "never") => {
                activate = ActivatePolicy::Never;
                continue;
            }
            ("activate", "interaction") => {
                activate = ActivatePolicy::Interaction;
                continue;
            }
            ("paint", "settled") => {
                paint = PaintPolicy::Settled;
                continue;
            }
            ("paint", "boot") => {
                paint = PaintPolicy::Boot;
                continue;
            }
            // Its source call is types' and `declare_routes`'.
            ("pages", _) => continue,
            ("render", _) => "`render` is a word: client, build, cached or request".to_owned(),
            ("activate", _) => "`activate` is a word: idle, never or interaction".to_owned(),
            ("paint", _) => "`paint` is a word: settled or boot".to_owned(),
            (other, _) => format!(
                "a route has no field `{other}`: it takes `render=`, `activate=`, `paint=` and `pages=`"
            ),
        };
        return err(
            "lower-route-field",
            format!("{refusal} (route `{}`)", row.name),
            field.span,
        );
    }
    // A parameterized route renders at build only the pages `pages=` lists
    // (LLP 1048.000 D2); `pages=` names pages a build or a server renders.
    let listed = row.fields.iter().any(|f| f.name == "pages");
    if render == RenderPolicy::Build
        && !listed
        && row.pattern.split('/').any(|s| s.starts_with(':'))
    {
        return err(
            "lower-route-field",
            format!(
                "route `{}` has parameters, so it renders at build only the pages `pages=source()` lists",
                row.name
            ),
            row.span,
        );
    }
    if listed && render == RenderPolicy::Client {
        return err(
            "lower-route-field",
            format!(
                "route `{}` renders on the client, so it has no pages to list: `pages=` goes with `render=build`, `cached` or `request`",
                row.name
            ),
            row.span,
        );
    }
    // A boot document is sent while a served page waits on its answers; a
    // page made at build, or on the client, waits on nothing a reader sees.
    if paint == PaintPolicy::Boot && !matches!(render, RenderPolicy::Cached | RenderPolicy::Request)
    {
        return err(
            "lower-route-field",
            format!(
                "route `{}` is not rendered per request, so it has no boot document to paint first: `paint=boot` goes with `render=cached` or `request`",
                row.name
            ),
            row.span,
        );
    }
    Ok((render, activate, paint))
}
