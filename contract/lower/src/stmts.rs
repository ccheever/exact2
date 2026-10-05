//! Action bodies: assignments, commands, `send`, `refresh`, `if`/`else` and
//! `match` (LLP 1017 P2), and `let` (LLP 1035.005.000 D2). A branch is a
//! forward jump over the arm not taken, as the ternary and the inline
//! `match` are in `expr.rs`; a `match` binds a local for its `some` block,
//! and a `let` binds one for the rest of its block, dropped where the
//! block ends. A call is its callee's statements in a block of their own
//! (LLP 1089 D8). Still no loops or recursion; a body always terminates
//! (LLP 1005 §2).

use crate::{expr, LowerError, Lowerer};
use contract_syntax::Stmt;
use contract_types::{Ref, Scope, Ty};
use exact_plan::asm::Asm;
use exact_plan::Opcode;

impl Lowerer<'_> {
    /// The slot a state's or a mutation's name writes.
    pub(crate) fn slot_named(&self, name: &str) -> exact_plan::SlotsId {
        match self.root.states.iter().position(|s| s.name == name) {
            Some(si) => self.slots[si],
            None => {
                let mi = self.root.mutations.iter().position(|m| m.name == name);
                self.mutation_slots[mi.expect("the type pass resolved it")]
            }
        }
    }

    /// Lower a block of statements in `scope`. Each `let` is evaluated once
    /// where it stands and read from the locals stack by the statements
    /// after it; the block drops its locals before control leaves it.
    pub(crate) fn block(
        &mut self,
        asm: &mut Asm,
        stmts: &[Stmt],
        scope: &Scope,
        locals: &mut u16,
    ) -> Result<(), LowerError> {
        let mut scope = scope.clone();
        let mut bound = 0;
        for stmt in stmts {
            if let Stmt::Let { name, expr, .. } = stmt {
                let ty = expr::compile(self, asm, expr, &scope, locals)?;
                asm.bind_local();
                scope.push(vec![(name.clone(), Ref::Local(*locals as u32), ty)]);
                *locals += 1;
                bound += 1;
                continue;
            }
            self.stmt(asm, stmt, &scope, locals)?;
        }
        for _ in 0..bound {
            *locals -= 1;
            asm.drop_local();
        }
        Ok(())
    }

    fn stmt(
        &mut self,
        asm: &mut Asm,
        stmt: &Stmt,
        scope: &Scope,
        locals: &mut u16,
    ) -> Result<(), LowerError> {
        let root = self.root;
        match stmt {
            Stmt::Let { .. } => unreachable!("bound by the block"),
            Stmt::Assign { target, expr, .. } => {
                expr::compile(self, asm, expr, scope, locals)?;
                asm.store_slot(self.slot_named(target));
            }
            Stmt::Send {
                target,
                source,
                args,
                ..
            } => {
                for arg in args {
                    expr::compile(self, asm, arg, scope, locals)?;
                }
                let m = self.mutations[root
                    .mutations
                    .iter()
                    .position(|m| &m.name == target)
                    .unwrap()];
                let source = self.b.str(source);
                asm.send(m, source, args.len() as u16);
            }
            Stmt::Refresh { target, .. } => {
                let r = self.resources[root
                    .resources
                    .iter()
                    .position(|r| &r.name == target)
                    .unwrap()];
                asm.refresh(r);
            }
            // A call (LLP 1089 D8): its callee's statements, its parameters'
            // `let`s first, as a block of their own, whose locals drop before
            // the next statement. No call opcode: the plan never sees one.
            Stmt::Call { body, .. } => self.block(asm, body, scope, locals)?,
            Stmt::Command { name, args, .. } => {
                let args = expr::command_args(name, args);
                for arg in &args {
                    expr::compile_or_none(self, asm, *arg, scope, locals)?;
                }
                let name = self.b.str(name);
                asm.command(name, args.len() as u16);
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                expr::compile(self, asm, cond, scope, locals)?;
                let els = asm.label();
                let end = asm.label();
                asm.jump_if_false(els);
                self.block(asm, then, scope, locals)?;
                asm.jump(end);
                asm.place(els);
                self.block(asm, otherwise, scope, locals)?;
                asm.place(end);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let bound_ty = match expr::compile(self, asm, subject, scope, locals)? {
                    Ty::Option(t) => *t,
                    _ => Ty::Unknown,
                };
                let is_none = asm.label();
                let end = asm.label();
                asm.jump_if_none(is_none);
                asm.simple(Opcode::Unwrap);
                asm.bind_local();
                let index = *locals;
                *locals += 1;
                let mut inner = scope.clone();
                inner.push(vec![(some.0.clone(), Ref::Local(index as u32), bound_ty)]);
                self.block(asm, &some.1, &inner, locals)?;
                *locals -= 1;
                asm.drop_local();
                asm.jump(end);
                asm.place(is_none);
                asm.simple(Opcode::Pop);
                self.block(asm, none, scope, locals)?;
                asm.place(end);
            }
        }
        Ok(())
    }
}
