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
                Ok((render, activate)) => self.b.set_route_policy(id, render, activate),
                Err(e) => self.errors.push(e),
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
/// request`, undeclared `client`; `activate=idle|never`, inferred when
/// undeclared. Each value is a word, read by its spelling.
fn route_policy(
    row: &contract_syntax::RouteDecl,
) -> Result<(exact_plan::RenderPolicy, exact_plan::ActivatePolicy), LowerError> {
    use exact_plan::{ActivatePolicy, RenderPolicy};
    let mut render = RenderPolicy::Client;
    let mut activate = ActivatePolicy::Inferred;
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
            ("render", _) => "`render` is a word: client, build, cached or request".to_owned(),
            ("activate", _) => "`activate` is a word: idle or never".to_owned(),
            (other, _) => {
                format!("a route has no field `{other}`: it takes `render=` and `activate=`")
            }
        };
        return err(
            "lower-route-field",
            format!("{refusal} (route `{}`)", row.name),
            field.span,
        );
    }
    // A parameterized route's pages are listed at build by `pages=`, which
    // comes with enumeration (LLP 1048.000 D2).
    if render == RenderPolicy::Build && row.pattern.split('/').any(|s| s.starts_with(':')) {
        return err(
            "lower-route-field",
            format!(
                "route `{}` has parameters, so it can't render at build until its pages are listed (`pages=`, with enumeration)",
                row.name
            ),
            row.span,
        );
    }
    Ok((render, activate))
}
