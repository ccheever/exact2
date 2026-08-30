//! Component inlining: uses become the used component's view, props become
//! the use's argument expressions, and names the child binds are renamed
//! apart so an argument expression from the parent can never be captured.
//!
//! Purely syntactic, so both type inference (which needs to see a handler's
//! real call site through a prop) and lowering run on the same expansion.
//!
//! LLP 1017 P4a/P4b live here too: an `inject` is filled from the nearest
//! enclosing `provide` on the way down (the compiler's context — no runtime
//! lookup, a missing provider a refusal), and a `slot` component's
//! `children` node is replaced by the nodes indented under its use, inlined
//! in the *use site's* scope.

use crate::ast::{Attr, Component, Expr, File, Node, TemplatePart};
use crate::parser::SyntaxError;
use std::collections::BTreeMap;

fn err<T>(
    id: &'static str,
    message: impl Into<String>,
    span: crate::Span,
) -> Result<T, SyntaxError> {
    Err(SyntaxError {
        id,
        message: message.into(),
        span,
    })
}

/// The root's view with every component use inlined.
pub fn inline(file: &File) -> Result<Vec<Node>, SyntaxError> {
    let mut counter = 0u32;
    let mut ctx = Ctx {
        file,
        counter: &mut counter,
        depth: 0,
        provides: Vec::new(),
        fill: None,
    };
    inline_nodes(&file.components[0].view, &BTreeMap::new(), &mut ctx)
}

/// What inlining carries down the tree besides the substitution.
struct Ctx<'a> {
    file: &'a File,
    counter: &'a mut u32,
    depth: u32,
    /// `provide`s in force, outermost first, each already substituted into
    /// the scope it was written in.
    provides: Vec<(String, Expr)>,
    /// The nodes that fill `children` here: `Some` inside a `slot`
    /// component's view (possibly empty), `None` elsewhere.
    fill: Option<Vec<Node>>,
}

fn inline_nodes(
    nodes: &[Node],
    subst: &BTreeMap<String, Expr>,
    ctx: &mut Ctx<'_>,
) -> Result<Vec<Node>, SyntaxError> {
    let mut out = Vec::with_capacity(nodes.len());
    for n in nodes {
        match n {
            Node::Use {
                name,
                args,
                children,
                span,
            } => {
                if ctx.depth > 32 {
                    return err(
                        "syntax-inline-depth",
                        format!("component `{name}` nests too deeply (a cycle?)"),
                        *span,
                    );
                }
                let Some(c) = ctx.file.components.iter().find(|c| &c.name == name) else {
                    return err(
                        "syntax-unknown-component",
                        format!("unknown component `{name}`"),
                        *span,
                    );
                };
                let mut child_subst: BTreeMap<String, Expr> = BTreeMap::new();
                for p in &c.props {
                    let Some(a) = args.iter().find(|a| a.name == p.name) else {
                        return err(
                            "syntax-missing-prop",
                            format!("`{name}` needs `{}`", p.name),
                            *span,
                        );
                    };
                    // The argument is an expression in the parent's scope: substitute the parent's own substitutions first.
                    child_subst.insert(p.name.clone(), subst_expr(&a.value, subst));
                }
                for p in &c.injects {
                    let Some((_, e)) = ctx.provides.iter().rev().find(|(n, _)| n == &p.name) else {
                        return err(
                            "syntax-missing-provide",
                            format!(
                                "`{name}` injects `{}`, and nothing above this use provides it: wrap the use in `provide {} = …`",
                                p.name, p.name
                            ),
                            *span,
                        );
                    };
                    child_subst.insert(p.name.clone(), e.clone());
                }
                if !children.is_empty() && !c.slot {
                    return err(
                        "syntax-no-slot",
                        format!("`{name}` declares no `slot`, so nothing can be indented under it"),
                        children[0].span(),
                    );
                }
                // The fill is the use site's: inlined here, in this scope.
                let fill = if c.slot {
                    Some(inline_nodes(children, subst, ctx)?)
                } else {
                    None
                };
                *ctx.counter += 1;
                let renamed = rename_component(c, *ctx.counter);
                let outer_fill = std::mem::replace(&mut ctx.fill, fill);
                ctx.depth += 1;
                let body = inline_nodes(&renamed.view, &child_subst, ctx);
                ctx.depth -= 1;
                ctx.fill = outer_fill;
                out.extend(body?);
            }
            Node::Provide {
                name, expr, body, ..
            } => {
                ctx.provides.push((name.clone(), subst_expr(expr, subst)));
                let inner = inline_nodes(body, subst, ctx);
                ctx.provides.pop();
                out.extend(inner?);
            }
            Node::Children { span } => match &ctx.fill {
                Some(fill) => out.extend(fill.iter().cloned()),
                None => {
                    return err(
                        "syntax-children-without-slot",
                        "`children` belongs in a component that declares `slot`",
                        *span,
                    )
                }
            },
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
            } => out.push(Node::Element {
                tag: tag.clone(),
                positional: positional.iter().map(|e| subst_expr(e, subst)).collect(),
                attrs: attrs
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: subst_expr(&a.value, subst),
                        span: a.span,
                    })
                    .collect(),
                children: inline_nodes(children, subst, ctx)?,
                span: *span,
            }),
            Node::When {
                cond,
                then,
                otherwise,
                span,
            } => out.push(Node::When {
                cond: subst_expr(cond, subst),
                then: inline_nodes(then, subst, ctx)?,
                otherwise: inline_nodes(otherwise, subst, ctx)?,
                span: *span,
            }),
            Node::Each {
                var,
                list,
                key,
                body,
                span,
            } => out.push(Node::Each {
                var: var.clone(),
                list: subst_expr(list, subst),
                key: subst_expr(key, subst),
                body: inline_nodes(body, subst, ctx)?,
                span: *span,
            }),
            Node::Match {
                subject,
                some,
                none,
                span,
            } => out.push(Node::Match {
                subject: subst_expr(subject, subst),
                some: (some.0.clone(), inline_nodes(&some.1, subst, ctx)?),
                none: inline_nodes(none, subst, ctx)?,
                span: *span,
            }),
        }
    }
    Ok(out)
}

/// Substitute prop names by argument expressions. A curried handler
/// `prop(args)` where the prop's argument is an action `f` or `f(a…)`
/// becomes `f(a…, args)`.
fn subst_expr(e: &Expr, subst: &BTreeMap<String, Expr>) -> Expr {
    match e {
        Expr::Ident(n, _) => match subst.get(n) {
            Some(r) => r.clone(),
            None => e.clone(),
        },
        Expr::Call(n, args, span) => {
            let args: Vec<Expr> = args.iter().map(|a| subst_expr(a, subst)).collect();
            match subst.get(n) {
                Some(Expr::Ident(f, _)) => Expr::Call(f.clone(), args, *span),
                Some(Expr::Call(f, first, _)) => {
                    let mut all = first.clone();
                    all.extend(args);
                    Expr::Call(f.clone(), all, *span)
                }
                _ => Expr::Call(n.clone(), args, *span),
            }
        }
        Expr::Member(o, f, span) => Expr::Member(Box::new(subst_expr(o, subst)), f.clone(), *span),
        Expr::Some(x, span) => Expr::Some(Box::new(subst_expr(x, subst)), *span),
        Expr::Unary(op, x, span) => Expr::Unary(*op, Box::new(subst_expr(x, subst)), *span),
        Expr::Binary(op, a, b, span) => Expr::Binary(
            *op,
            Box::new(subst_expr(a, subst)),
            Box::new(subst_expr(b, subst)),
            *span,
        ),
        Expr::Ternary(a, b, c, span) => Expr::Ternary(
            Box::new(subst_expr(a, subst)),
            Box::new(subst_expr(b, subst)),
            Box::new(subst_expr(c, subst)),
            *span,
        ),
        Expr::Match {
            subject,
            var,
            some,
            none,
            span,
        } => {
            let mut inner = subst.clone();
            inner.remove(var);
            Expr::Match {
                subject: Box::new(subst_expr(subject, subst)),
                var: var.clone(),
                some: Box::new(subst_expr(some, &inner)),
                none: Box::new(subst_expr(none, subst)),
                span: *span,
            }
        }
        Expr::Template(parts, span) => Expr::Template(
            parts
                .iter()
                .map(|p| match p {
                    TemplatePart::Text(t) => TemplatePart::Text(t.clone()),
                    TemplatePart::Expr(x) => TemplatePart::Expr(subst_expr(x, subst)),
                })
                .collect(),
            *span,
        ),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => e.clone(),
    }
}

/// Rename every name a component's view binds (`each` vars, `match` vars)
/// with a unique suffix, so inlined bodies never capture parent names.
fn rename_component(c: &Component, n: u32) -> Component {
    let mut renamed = c.clone();
    renamed.view = rename_nodes(&c.view, &BTreeMap::new(), n);
    renamed
}

fn rename_nodes(nodes: &[Node], map: &BTreeMap<String, String>, n: u32) -> Vec<Node> {
    nodes
        .iter()
        .map(|node| match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
            } => Node::Element {
                tag: tag.clone(),
                positional: positional.iter().map(|e| rename_expr(e, map)).collect(),
                attrs: attrs
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: rename_expr(&a.value, map),
                        span: a.span,
                    })
                    .collect(),
                children: rename_nodes(children, map, n),
                span: *span,
            },
            Node::Use {
                name,
                args,
                children,
                span,
            } => Node::Use {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|a| Attr {
                        name: a.name.clone(),
                        value: rename_expr(&a.value, map),
                        span: a.span,
                    })
                    .collect(),
                children: rename_nodes(children, map, n),
                span: *span,
            },
            Node::Provide {
                name,
                expr,
                body,
                span,
            } => Node::Provide {
                name: name.clone(),
                expr: rename_expr(expr, map),
                body: rename_nodes(body, map, n),
                span: *span,
            },
            Node::Children { span } => Node::Children { span: *span },
            Node::When {
                cond,
                then,
                otherwise,
                span,
            } => Node::When {
                cond: rename_expr(cond, map),
                then: rename_nodes(then, map, n),
                otherwise: rename_nodes(otherwise, map, n),
                span: *span,
            },
            Node::Each {
                var,
                list,
                key,
                body,
                span,
            } => {
                let mut inner = map.clone();
                let fresh = format!("{var}__{n}");
                inner.insert(var.clone(), fresh.clone());
                Node::Each {
                    var: fresh,
                    list: rename_expr(list, map),
                    key: rename_expr(key, &inner),
                    body: rename_nodes(body, &inner, n),
                    span: *span,
                }
            }
            Node::Match {
                subject,
                some,
                none,
                span,
            } => {
                let mut inner = map.clone();
                let fresh = format!("{}__{n}", some.0);
                inner.insert(some.0.clone(), fresh.clone());
                Node::Match {
                    subject: rename_expr(subject, map),
                    some: (fresh, rename_nodes(&some.1, &inner, n)),
                    none: rename_nodes(none, map, n),
                    span: *span,
                }
            }
        })
        .collect()
}

fn rename_expr(e: &Expr, map: &BTreeMap<String, String>) -> Expr {
    let subst: BTreeMap<String, Expr> = map
        .iter()
        .map(|(k, v)| (k.clone(), Expr::Ident(v.clone(), e.span())))
        .collect();
    subst_expr(e, &subst)
}
