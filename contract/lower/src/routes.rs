//! @ref LLP 1038 D2/D3 — the table and launch slot; paths use ordinary templates.

use super::*;

impl Lowerer<'_> {
    pub(super) fn declare_routes(&mut self, file: &File) {
        let Some(routes) = &file.routes else { return };
        for row in &routes.rows {
            self.b.route(
                &row.name,
                &row.pattern,
                row.parent.map(|p| exact_plan::RoutesId(p as u32)),
                row.tab,
                row.notfound,
            );
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
