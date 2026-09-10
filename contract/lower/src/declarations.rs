//! Declaration bodies are emitted only after their owning lexical scopes exist.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Expanded, Expr, Node, Span};
use contract_types::{Ref, Scope};
use exact_plan::asm::Asm;

pub(super) fn one_visual_root(nodes: &[Node]) -> bool {
    match nodes {
        [Node::Element { .. }] => true,
        [Node::Scope { body, .. }] => one_visual_root(body),
        _ => false,
    }
}

impl Lowerer<'_> {
    fn declaration_scope(
        &self,
        owner: Option<u32>,
        root: &Scope,
        span: Span,
    ) -> Result<Scope, LowerError> {
        match owner {
            None => Ok(root.clone()),
            Some(tag) => self
                .owner_scopes
                .get(&tag)
                .cloned()
                .ok_or_else(|| LowerError {
                    id: "lower-owner-scope",
                    message: format!("declaration's scope {tag} was not lowered"),
                    span,
                }),
        }
    }

    pub(super) fn declaration_bodies(
        &mut self,
        ex: &Expanded,
        scope: &Scope,
    ) -> Result<(), LowerError> {
        let root = self.root;
        for (i, state) in root.states.iter().enumerate() {
            let inner = self.declaration_scope(ex.owners[i], scope, state.span)?;
            let code = self.expr_code(&state.expr, &inner, 0)?;
            self.b.set_slot_init(self.slots[i], code);
            if let Some(tag) = ex.owners[i] {
                self.b
                    .set_slot_owner(self.slots[i], self.owner_regions[&tag]);
            }
        }
        for (i, derive) in root.derives.iter().enumerate() {
            let code = self.expr_code(&derive.expr, scope, 0)?;
            self.b.set_derive_body(self.derives[i], code);
        }
        for (i, resource) in root.resources.iter().enumerate() {
            let inner = self.declaration_scope(ex.resource_owners[i], scope, resource.span)?;
            let mut args = Vec::new();
            for arg in &resource.args {
                args.push(self.expr_code(arg, &inner, 0)?);
            }
            let range = self.b.args(&args);
            self.b.set_resource_args(self.resources[i], range);
            if let Some(fallback) = &resource.fallback {
                let code = self.expr_code(fallback, &inner, 0)?;
                self.b.set_resource_fallback(self.resources[i], code);
            }
            if let Some(tag) = ex.resource_owners[i] {
                self.b
                    .set_resource_owner(self.resources[i], self.owner_regions[&tag]);
            }
        }
        for (i, owner) in ex.mutation_owners.iter().enumerate() {
            if let Some(tag) = owner {
                self.b
                    .set_slot_owner(self.mutation_slots[i], self.owner_regions[tag]);
            }
        }
        for (i, action) in root.actions.iter().enumerate() {
            let mut inner = self.declaration_scope(ex.action_owners[i], scope, action.span)?;
            inner.push(
                action
                    .params
                    .iter()
                    .enumerate()
                    .map(|(pi, p)| {
                        (
                            p.name.clone(),
                            Ref::Param(pi as u32),
                            self.types.components[0].actions[i][pi].clone(),
                        )
                    })
                    .collect(),
            );
            let mut asm = Asm::new();
            let mut locals = 0;
            for stmt in &action.body {
                self.stmt(&mut asm, stmt, &inner, &mut locals)?;
            }
            let code = self.b.code(asm);
            self.b.set_action_body(self.actions[i], code);
        }
        for (i, task) in root.tasks.iter().enumerate() {
            let Expr::Number(ms, _) = task.every.0 else {
                return err(
                    "lower-timer-literal",
                    "`every` needs a literal number of milliseconds",
                    task.every.2,
                );
            };
            if !(ms.is_finite() && ms.fract() == 0.0 && ms >= 1.0 && ms <= u32::MAX as f64) {
                return err(
                    "lower-timer-interval",
                    format!("`every` needs a whole number of milliseconds, at least 1; given {ms}"),
                    task.every.2,
                );
            }
            let Some(ai) = root.actions.iter().position(|a| a.name == task.every.1) else {
                return err(
                    "analyze-unknown-action",
                    format!("`{}` is not an action", task.every.1),
                    task.every.2,
                );
            };
            let timer = self.b.timer(ms as u32, self.actions[ai]);
            let inner = self.declaration_scope(ex.task_owners[i], scope, task.span)?;
            let mut args = Vec::new();
            for arg in &ex.task_args[i] {
                args.push(self.expr_code(arg, &inner, 0)?);
            }
            let range = self.b.args(&args);
            self.b.set_timer_args(timer, range);
            if let Some(tag) = ex.task_owners[i] {
                self.b.set_timer_owner(timer, self.owner_regions[&tag]);
            }
        }
        Ok(())
    }
}
