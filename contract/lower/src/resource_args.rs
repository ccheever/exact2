//! A resource's arguments and `with` context, and, over a list (LLP 1017.000
//! P4c, amended 2026-11-04), its list and key: the arguments and the key read
//! the element as their one parameter, which the runner passes each
//! element's request.

use super::{LowerError, Lowerer};
use contract_syntax::{Component, Expr};
use contract_types::{Ref, Scope, Ty};

impl Lowerer<'_> {
    pub(crate) fn resource_args(
        &mut self,
        root: &Component,
        scope: &Scope,
    ) -> Result<(), LowerError> {
        for (i, r) in root.resources.iter().enumerate() {
            let mut inner = scope.clone();
            if let Some(each) = &r.each {
                if exact_plan::runner_owned_source(&r.source) {
                    return Err(LowerError {
                        id: "lower-resource-for-source",
                        message: format!(
                            "`{}` is the runner's, one answer for the app: a resource over a list asks a data source for each element",
                            r.source
                        ),
                        span: r.span,
                    });
                }
                let list = self.expr_code(&each.list, scope, 0)?;
                let item = match contract_types::infer(&each.list, scope, &self.types.shapes) {
                    Ok(Ty::List(t)) => *t,
                    _ => Ty::Unknown,
                };
                inner.push(vec![(each.item.clone(), Ref::Param(0), item)]);
                let element = Expr::Ident(each.item.clone(), each.span);
                let key = self.expr_code(each.key.as_ref().unwrap_or(&element), &inner, 0)?;
                self.b.set_resource_each(self.resources[i], list, key);
            }
            let mut args = Vec::new();
            for a in &r.args {
                args.push(self.expr_code(a, &inner, 0)?);
            }
            let range = self.b.args(&args);
            self.b.set_resource_args(self.resources[i], range);
            // @ref LLP 1027.005 D6 — declare first, install the full argument
            // list, then count its context tail. No `with` keeps the default 0.
            if let Some(identity) = r.identity {
                let context = u16::try_from(r.args.len() - identity).map_err(|_| LowerError {
                    id: "lower-resource-context",
                    message: "a resource supports at most 65535 request context values".into(),
                    span: r.span,
                })?;
                self.b.set_resource_context(self.resources[i], context);
            }
        }
        Ok(())
    }
}
