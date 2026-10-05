//! Action bodies (LLP 1017 P2): statements through every branch, and the
//! block-scoped, immutable `let` locals of LLP 1035.005.000 D2.

use super::{calls, checks, err, infer, ComponentTypes, Ref, Scope, Shapes, Sink, Ty, TypeError};
use contract_syntax::{Component, Expr, Span, Stmt, TemplatePart};

/// The `let`s a block sees: those in force (declared above, in it or a
/// block around it) and those declared below, which it may not read yet.
#[derive(Clone, Default)]
struct Lets<'a> {
    bound: Vec<(&'a str, Span)>,
    later: Vec<(&'a str, Span)>,
}

/// Check an action's body, recording each statement's refusal and moving
/// on. `lifted` is a child's action expanded into the root: its names were
/// checked in the child's own scope, where a `let` shadows nothing, and it
/// may spell a root name the child never saw.
pub(super) fn check_body(
    stmts: &[Stmt],
    scope: &Scope,
    lifted: bool,
    c: &Component,
    ct: &mut ComponentTypes,
    shapes: &Shapes,
    sink: &mut Sink,
) {
    let mut cx = Cx {
        c,
        ct,
        shapes,
        sink,
        lifted,
    };
    cx.block(stmts, scope, &Lets::default());
}

struct Cx<'c, 's> {
    c: &'c Component,
    ct: &'c mut ComponentTypes,
    shapes: &'c Shapes,
    sink: &'s mut Sink,
    lifted: bool,
}

impl Cx<'_, '_> {
    fn block<'a>(&mut self, stmts: &'a [Stmt], scope: &Scope, outer: &Lets<'a>) {
        let mut scope = scope.clone();
        let mut lets = outer.clone();
        let mut here: Vec<(&str, Span)> = Vec::new();
        for (i, stmt) in stmts.iter().enumerate() {
            lets.later = outer.later.clone();
            lets.later
                .extend(stmts[i + 1..].iter().filter_map(|s| match s {
                    Stmt::Let { name, span, .. } => Some((name.as_str(), *span)),
                    _ => None,
                }));
            if let Some(e) = read_too_early(stmt, &scope, &lets) {
                self.sink.push(e);
                continue;
            }
            if let Some(e) = calls::called_for_value(stmt, &scope, self.shapes) {
                self.sink.push(e);
                continue;
            }
            if let Stmt::Let { name, expr, span } = stmt {
                let declared = self.declare(name, *span, &here, &scope);
                self.sink.keep_unit(declared);
                let ty = self.sink.keep(infer(expr, &scope, self.shapes));
                // A refused `let` still binds, so what reads it is not
                // refused again as an unknown name.
                scope.push(vec![(name.clone(), Ref::Local(0), ty)]);
                here.push((name, *span));
                lets.bound.push((name, *span));
                continue;
            }
            let checked = self.stmt(stmt, &scope, &lets);
            self.sink.keep_unit(checked);
        }
    }

    /// A `let`'s name: new in its block, and naming nothing already in scope.
    fn declare(
        &self,
        name: &str,
        span: Span,
        here: &[(&str, Span)],
        scope: &Scope,
    ) -> Result<(), TypeError> {
        if let Some((_, first)) = here.iter().find(|(n, _)| *n == name) {
            return err(
                "type-let-duplicate",
                format!(
                    "`{name}` is already declared by the `let` on line {}: a block declares a local once; give this value its own name",
                    first.line
                ),
                span,
            );
        }
        if self.lifted {
            return Ok(());
        }
        let Some((r, _)) = scope.lookup(name) else {
            return Ok(());
        };
        let what = match r {
            Ref::Slot(_) => "a state",
            Ref::Derive(_) => "a derive",
            Ref::Resource(_) => "a resource",
            Ref::Mutation(_) => "a mutation",
            Ref::Action(_) => "an action",
            Ref::Prop(_) => "a prop",
            Ref::Param(_) => "a parameter",
            Ref::Item(_) | Ref::Index(_) | Ref::Bound(_) | Ref::Local(_) => "a local",
        };
        err(
            "type-let-shadow",
            format!(
                "`{name}` is already {what} here: a `let` never shadows a name in scope; give the local its own name"
            ),
            span,
        )
    }

    fn stmt<'a>(
        &mut self,
        stmt: &'a Stmt,
        scope: &Scope,
        lets: &Lets<'a>,
    ) -> Result<(), TypeError> {
        let (c, shapes) = (self.c, self.shapes);
        match stmt {
            Stmt::Let { .. } => unreachable!("bound by the block"),
            Stmt::Assign { target, expr, span } => {
                if let Some((_, at)) = lets.bound.iter().rev().find(|(n, _)| n == target) {
                    return err(
                        "type-let-reassign",
                        format!(
                            "`{target}` is the `let` on line {}, and a local is never reassigned: give the new value its own `let`, or make `{target}` a `state`",
                            at.line
                        ),
                        *span,
                    );
                }
                let Some(si) = c.states.iter().position(|s| &s.name == target) else {
                    // A mutation's slot may be assigned (`session = none`);
                    // its type is `option<T>` and is never inferred from here.
                    if let Some(mi) = c.mutations.iter().position(|m| &m.name == target) {
                        let t = infer(expr, scope, shapes)?;
                        let mt = Ty::Option(Box::new(self.ct.mutations[mi].clone()));
                        if !checks::can_unify(&mt, &t) {
                            return err(
                                "type-assign",
                                format!("`{target}` is `{mt}`, cannot assign `{t}`"),
                                *span,
                            );
                        }
                        return Ok(());
                    }
                    return err(
                        "type-assign-not-state",
                        format!("`{target}` is not a state or a mutation"),
                        *span,
                    );
                };
                let t = infer(expr, scope, shapes)?;
                match self.ct.slots[si].unify(&t) {
                    Some(u) => self.ct.slots[si] = u,
                    None => {
                        return err(
                            "type-assign",
                            format!("`{target}` is `{}`, cannot assign `{t}`", self.ct.slots[si]),
                            *span,
                        )
                    }
                }
            }
            Stmt::Command { name, args, span } => {
                // An action prop or injected action, called anywhere (LLP
                // 1089 D1, D7): its arguments here, against its declared
                // type; the action it names is run, and checked, after
                // lifting.
                if !contract_syntax::HOST_COMMANDS.contains(&name.as_str()) {
                    if let Some((Ref::Prop(_), Ty::Action(params))) = scope.lookup(name) {
                        return calls::prop_args(name, params, args, scope, shapes, *span);
                    }
                }
                // Refused as ambiguous (LLP 1089 D1): not a host command's too.
                if shapes.ambiguous.contains(span) {
                    return Ok(());
                }
                return checks::check_command(name, args, scope, shapes, &c.name, *span);
            }
            Stmt::Call { .. } => return calls::check(stmt, self.lifted, c, scope, shapes),
            Stmt::Send {
                target,
                source,
                args,
                span,
                ..
            } => {
                let Some(mi) = c.mutations.iter().position(|m| &m.name == target) else {
                    return err(
                        "type-send-not-mutation",
                        format!(
                            "`{target}` is not a mutation: declare `mutation {target} as shape T`"
                        ),
                        *span,
                    );
                };
                let mut params = Vec::with_capacity(args.len());
                for arg in args {
                    params.push(crate::source_argument(arg, source, scope, shapes)?);
                }
                let result = self.ct.mutations[mi].clone();
                crate::record_source(self.ct, source, params, result, *span)?;
            }
            Stmt::Refresh { target, span } => {
                if !c.resources.iter().any(|r| &r.name == target) {
                    return err(
                        "type-refresh-not-resource",
                        format!("`{target}` is not a resource"),
                        *span,
                    );
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                match infer(cond, scope, shapes) {
                    Ok(Ty::Bool) => {}
                    Ok(_) => self.sink.push(TypeError {
                        id: "type-condition",
                        message: "`if` needs a bool".into(),
                        span: cond.span(),
                    }),
                    Err(e) => self.sink.push(e),
                }
                self.block(then, scope, lets);
                self.block(otherwise, scope, lets);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                let ts = infer(subject, scope, shapes)?;
                let Ty::Option(inner) = ts else {
                    return err(
                        "type-match-subject",
                        format!("`match` needs an option, given `{ts}`"),
                        subject.span(),
                    );
                };
                let mut inner_scope = scope.clone();
                inner_scope.push(vec![(some.0.clone(), Ref::Local(0), (*inner).clone())]);
                self.block(&some.1, &inner_scope, lets);
                self.block(none, scope, lets);
            }
        }
        Ok(())
    }
}

/// A read of a `let` declared below it, in this block or one around it:
/// statements run in order, and a local is read only after its line.
fn read_too_early(stmt: &Stmt, scope: &Scope, lets: &Lets<'_>) -> Option<TypeError> {
    if lets.later.is_empty() {
        return None;
    }
    let mut reads = Vec::new();
    match stmt {
        Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => {
            names(expr, &mut Vec::new(), &mut reads)
        }
        Stmt::Command { args, .. } | Stmt::Send { args, .. } | Stmt::Call { args, .. } => {
            for a in args {
                names(a, &mut Vec::new(), &mut reads);
            }
        }
        Stmt::If { cond: e, .. } | Stmt::Match { subject: e, .. } => {
            names(e, &mut Vec::new(), &mut reads)
        }
        Stmt::Refresh { .. } => {}
    }
    reads.into_iter().find_map(|(name, span)| {
        let (_, at) = lets.later.iter().find(|(n, _)| *n == name)?;
        scope.lookup(name).is_none().then(|| TypeError {
            id: "type-let-before-declaration",
            message: format!(
                "`{name}` is read before the `let` on line {} declares it: a local is visible only to the statements after it",
                at.line
            ),
            span,
        })
    })
}

/// The names `e` reads, outside the binders written inside it.
fn names<'e>(e: &'e Expr, bound: &mut Vec<&'e str>, out: &mut Vec<(&'e str, Span)>) {
    match e {
        Expr::Ident(n, span) => {
            if !bound.contains(&n.as_str()) {
                out.push((n, *span));
            }
        }
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::EmptyList(_) => {}
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    names(x, bound, out);
                }
            }
        }
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => names(x, bound, out),
        Expr::Call(_, args, _) => args.iter().for_each(|a| names(a, bound, out)),
        Expr::Binary(_, a, b, _) => {
            names(a, bound, out);
            names(b, bound, out);
        }
        Expr::Ternary(a, b, x, _) => {
            names(a, bound, out);
            names(b, bound, out);
            names(x, bound, out);
        }
        Expr::Match {
            subject,
            var,
            some,
            none,
            ..
        } => {
            names(subject, bound, out);
            names(none, bound, out);
            bound.push(var);
            names(some, bound, out);
            bound.pop();
        }
        Expr::Let {
            name, value, body, ..
        } => {
            names(value, bound, out);
            bound.push(name);
            names(body, bound, out);
            bound.pop();
        }
        Expr::Arrow { params, body, .. } => {
            bound.extend(params.iter().map(String::as_str));
            names(body, bound, out);
            bound.truncate(bound.len() - params.len());
        }
    }
}
