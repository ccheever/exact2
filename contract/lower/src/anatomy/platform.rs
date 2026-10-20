//! `lower-library-platform` (LLP 1139 D6): nothing in a library package
//! reads the host it runs on. A resource of `exactDelivery` or
//! `exactSurface`; and `exactViewport`'s `displayMode`, read from the
//! viewport wherever the package carries it: an alias (a derive, a state
//! an action assigns, a `let` in its own scope), a choice, an action's or a
//! `fn`'s parameter (a `fn` that returns its argument, or passes it on, as
//! well), and an inject whose nearest provider in the package provides it.
//! A prop's `displayMode` is the app's data, never the host's; an inject
//! the package doesn't provide is the deferred limit (QUEUE).

use crate::LowerError;
use contract_syntax::{Component, Expr, File, Node, Span, Stmt, TemplatePart};

/// The host sources a library may not read (D6).
const HOST: [&str; 2] = ["exactDelivery", "exactSurface"];

/// Each host read in the components of `file` that are a library
/// package's, each with its package (`lower-library-platform`).
pub(crate) fn check(file: &File, components: &[(&Component, &str)]) -> Vec<LowerError> {
    let fns = summaries(file);
    let scan = Scan { fns: &fns };
    // Which injects hold the viewport, to a fixed point: a provide may
    // pass on an injected one.
    let mut injected: Vec<(usize, String)> = Vec::new();
    let helds: Vec<Held> = loop {
        let helds: Vec<Held> = components
            .iter()
            .enumerate()
            .map(|(i, (c, _))| held(&scan, c, &injected, i))
            .collect();
        let mut next = Vec::new();
        for (i, (c, _)) in components.iter().enumerate() {
            for p in &c.injects {
                if provided(&scan, components, &helds, i, &p.name, &mut Vec::new()) {
                    next.push((i, p.name.clone()));
                }
            }
        }
        if next == injected {
            break helds;
        }
        injected = next;
    };
    let mut out = Vec::new();
    for (i, (c, _)) in components.iter().enumerate() {
        let comp = &c.name;
        let refuse = |what: String, span: Span| {
            LowerError {
            id: "lower-library-platform",
            message: format!("{what} in `{comp}` reads the host a library runs on: a library never forks per OS; read dimensions, media preferences, page facts or capabilities instead (LLP 1139 D6)"),
            span,
        }
        };
        for r in c
            .resources
            .iter()
            .filter(|r| HOST.contains(&r.source.as_str()))
        {
            out.push(refuse(
                format!("the resource `{}` of `{}`", r.name, r.source),
                r.span,
            ));
        }
        let mut found = Found::default();
        visit(&scan, c, &helds[i].0, &helds[i].1, &mut found);
        found.reads.sort_by_key(|s| (s.source_id, s.line, s.col));
        found.reads.dedup();
        out.extend(
            found
                .reads
                .into_iter()
                .map(|s| refuse("`displayMode` of `exactViewport`".into(), s)),
        );
    }
    out
}

/// What a walk of a component finds: each read, each state an action
/// assigns the viewport, each action parameter a call passes it.
#[derive(Default)]
struct Found {
    reads: Vec<Span>,
    states: Vec<String>,
    params: Vec<(String, usize)>,
}

/// Walk all of `c`'s expressions with `held` the names holding the
/// viewport, each action's tainted parameters `params` added to its own.
fn visit(scan: &Scan, c: &Component, held: &[String], params: &[(String, usize)], f: &mut Found) {
    let actions: Vec<&str> = c.actions.iter().map(|a| a.name.as_str()).collect();
    let mut cx = Cx {
        scan,
        actions: &actions,
        found: f,
    };
    cx.nodes(&c.view, held);
    for e in c
        .states
        .iter()
        .chain(&c.derives)
        .chain(&c.provides)
        .map(|b| &b.expr)
        .chain(c.resources.iter().flat_map(|r| &r.args))
    {
        cx.expr(e, held);
    }
    for q in &c.sequences {
        let options = q.options.iter().map(|(_, e, _)| e);
        for e in q
            .gate
            .iter()
            .chain(&q.key)
            .chain([&q.tracks])
            .chain(options)
        {
            cx.expr(e, held);
        }
    }
    for t in &c.tasks {
        for e in t
            .gate
            .iter()
            .chain(&t.key)
            .chain(std::iter::once(&t.timer.0))
        {
            cx.expr(e, held);
        }
    }
    for a in &c.actions {
        let mut own = held.to_vec();
        for (i, p) in a.params.iter().enumerate() {
            if params.contains(&(a.name.clone(), i)) {
                own.push(p.name.clone());
            } else {
                own.retain(|h| *h != p.name);
            }
        }
        cx.stmts(&a.body, own);
    }
}

/// The names in component `i` (`c`) that hold the viewport: its
/// `exactViewport` resources, its injects in `injected`, and, to a fixed
/// point, a derive or state whose value is one, a state an action assigns
/// one, an action's parameter a call passes one (kept per action).
fn held(scan: &Scan, c: &Component, injected: &[(usize, String)], i: usize) -> Held {
    let base: Vec<String> = c
        .resources
        .iter()
        .filter(|r| r.source == "exactViewport")
        .map(|r| r.name.clone())
        .chain(
            injected
                .iter()
                .filter(|(j, _)| *j == i)
                .map(|(_, n)| n.clone()),
        )
        .collect();
    let mut held = base;
    let mut params: Vec<(String, usize)> = Vec::new();
    loop {
        let before = (held.len(), params.len());
        for b in c.states.iter().chain(&c.derives) {
            if !held.contains(&b.name) && scan.viewport(&b.expr, &held) {
                held.push(b.name.clone());
            }
        }
        let mut found = Found::default();
        visit(scan, c, &held, &params, &mut found);
        for s in found.states {
            if !held.contains(&s) {
                held.push(s);
            }
        }
        for p in found.params {
            if !params.contains(&p) {
                params.push(p);
            }
        }
        if (held.len(), params.len()) == before {
            break;
        }
    }
    (held, params)
}

/// The names a component holds the viewport in, and each action
/// parameter a call passes it (`(action, index)`), the action's own.
type Held = (Vec<String>, Vec<(String, usize)>);

/// Whether inject `name` of component `i` holds the viewport: its nearest
/// provider in its package, on any path a use of it takes, provides one.
/// A slot's fill keeps its writer's providers, so the path is the
/// components whose views write the use (`seen` ends a cycle).
fn provided(
    scan: &Scan,
    components: &[(&Component, &str)],
    helds: &[Held],
    i: usize,
    name: &str,
    seen: &mut Vec<usize>,
) -> bool {
    if seen.contains(&i) {
        return false;
    }
    seen.push(i);
    let (me, package) = components[i];
    for (j, (p, theirs)) in components.iter().enumerate() {
        if *theirs != package || !uses(&p.view, &me.name) {
            continue;
        }
        let tainted = match p.provides.iter().find(|b| b.name == name) {
            Some(b) => scan.viewport(&b.expr, &helds[j].0),
            None => provided(scan, components, helds, j, name, seen),
        };
        if tainted {
            return true;
        }
    }
    false
}

/// Whether `nodes` use component `name`, a fill's too.
fn uses(nodes: &[Node], name: &str) -> bool {
    nodes.iter().any(|n| match n {
        Node::Use {
            name: used,
            children,
            ..
        } => used == name || uses(children, name),
        Node::Element { children, .. } | Node::Each { body: children, .. } => uses(children, name),
        Node::When {
            then, otherwise, ..
        } => uses(then, name) || uses(otherwise, name),
        Node::Match { some, none, .. } => uses(&some.1, name) || uses(none, name),
        Node::Children { .. } => false,
    })
}

/// A `fn`: which parameters' `displayMode` it reads, itself or through a
/// `fn` it passes them to, and which it may return.
struct Summary {
    name: String,
    reads: Vec<usize>,
    returns: Vec<usize>,
}

/// Every `fn`'s [`Summary`], to a fixed point.
fn summaries(file: &File) -> Vec<Summary> {
    let mut all: Vec<Summary> = file
        .fns
        .iter()
        .map(|f| Summary {
            name: f.name.clone(),
            reads: Vec::new(),
            returns: Vec::new(),
        })
        .collect();
    loop {
        let next: Vec<(Vec<usize>, Vec<usize>)> = file
            .fns
            .iter()
            .map(|f| {
                let scan = Scan { fns: &all };
                let (mut reads, mut returns) = (Vec::new(), Vec::new());
                for (i, p) in f.params.iter().enumerate() {
                    let held = std::slice::from_ref(&p.name);
                    let mut found = Found::default();
                    let mut cx = Cx {
                        scan: &scan,
                        actions: &[],
                        found: &mut found,
                    };
                    cx.expr(&f.body, held);
                    if !found.reads.is_empty() {
                        reads.push(i);
                    }
                    if scan.viewport(&f.body, held) {
                        returns.push(i);
                    }
                }
                (reads, returns)
            })
            .collect();
        let changed = all
            .iter()
            .zip(&next)
            .any(|(s, (r, t))| s.reads != *r || s.returns != *t);
        for (s, (r, t)) in all.iter_mut().zip(next) {
            s.reads = r;
            s.returns = t;
        }
        if !changed {
            break;
        }
    }
    all
}

struct Scan<'a> {
    fns: &'a [Summary],
}

impl Scan<'_> {
    /// Whether `e`'s value may be the viewport a name of `held` holds.
    fn viewport(&self, e: &Expr, held: &[String]) -> bool {
        match e {
            Expr::Ident(n, _) => held.iter().any(|h| h == n),
            Expr::Ternary(_, a, b, _) => self.viewport(a, held) || self.viewport(b, held),
            Expr::Some(x, _) => self.viewport(x, held),
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => {
                let inner = bound(held, var, self.viewport(subject, held));
                self.viewport(some, &inner) || self.viewport(none, held)
            }
            Expr::Let {
                name, value, body, ..
            } => self.viewport(body, &bound(held, name, self.viewport(value, held))),
            Expr::Typed(body, _, _) => self.viewport(body, held),
            Expr::Call(f, args, _) => self.fns.iter().any(|s| {
                s.name == *f
                    && s.returns
                        .iter()
                        .any(|&i| args.get(i).is_some_and(|a| self.viewport(a, held)))
            }),
            _ => false,
        }
    }
}

/// `held` with `name` holding the viewport (`yes`) or shadowed.
fn bound(held: &[String], name: &str, yes: bool) -> Vec<String> {
    let mut out = without(held, name);
    if yes {
        out.push(name.to_string());
    }
    out
}

fn without(held: &[String], name: &str) -> Vec<String> {
    held.iter().filter(|h| *h != name).cloned().collect()
}

/// One walk's context.
struct Cx<'a, 'f> {
    scan: &'a Scan<'a>,
    actions: &'a [&'a str],
    found: &'f mut Found,
}

impl Cx<'_, '_> {
    fn nodes(&mut self, nodes: &[Node], held: &[String]) {
        for n in nodes {
            match n {
                Node::Element {
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    for e in positional.iter().chain(attrs.iter().map(|a| &a.value)) {
                        self.expr(e, held);
                    }
                    self.nodes(children, held);
                }
                Node::Use { args, children, .. } => {
                    for a in args {
                        self.expr(&a.value, held);
                    }
                    self.nodes(children, held);
                }
                Node::Each {
                    var,
                    index,
                    list,
                    key,
                    body,
                    ..
                } => {
                    self.expr(list, held);
                    let mut inner = without(held, var);
                    if let Some(i) = index {
                        inner.retain(|h| h != i);
                    }
                    self.expr(key, &inner);
                    self.nodes(body, &inner);
                }
                Node::When {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond, held);
                    self.nodes(then, held);
                    self.nodes(otherwise, held);
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject, held);
                    let yes = self.scan.viewport(subject, held);
                    self.nodes(&some.1, &bound(held, &some.0, yes));
                    self.nodes(none, held);
                }
                Node::Children { .. } => {}
            }
        }
    }

    /// Each read in `e`, each name bound in its own scope.
    fn expr(&mut self, e: &Expr, held: &[String]) {
        match e {
            Expr::Number(..)
            | Expr::Str(..)
            | Expr::Bool(..)
            | Expr::None(..)
            | Expr::Ident(..) => {}
            Expr::Member(base, field, span) => {
                if field == "displayMode" && self.scan.viewport(base, held) {
                    self.found.reads.push(*span);
                }
                self.expr(base, held);
            }
            Expr::Template(parts, _) => {
                for p in parts {
                    if let TemplatePart::Expr(x) = p {
                        self.expr(x, held);
                    }
                }
            }
            Expr::List(items, _) => items.iter().for_each(|a| self.expr(a, held)),
            Expr::Some(x, _)
            | Expr::NamedArg(_, x, _)
            | Expr::Typed(x, _, _)
            | Expr::Unary(_, x, _) => self.expr(x, held),
            Expr::Call(name, args, span) => {
                for a in args {
                    self.expr(a, held);
                }
                let passed = |i: usize| args.get(i).is_some_and(|a| self.scan.viewport(a, held));
                if let Some(s) = self.scan.fns.iter().find(|s| s.name == *name) {
                    if s.reads.iter().any(|&i| passed(i)) {
                        self.found.reads.push(*span);
                    }
                }
                if self.actions.contains(&name.as_str()) {
                    let tainted: Vec<usize> = (0..args.len()).filter(|&i| passed(i)).collect();
                    self.found
                        .params
                        .extend(tainted.into_iter().map(|i| (name.clone(), i)));
                }
            }
            Expr::Binary(_, a, b, _) => {
                self.expr(a, held);
                self.expr(b, held);
            }
            Expr::Ternary(a, b, c, _) => {
                self.expr(a, held);
                self.expr(b, held);
                self.expr(c, held);
            }
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => {
                self.expr(subject, held);
                let yes = self.scan.viewport(subject, held);
                self.expr(some, &bound(held, var, yes));
                self.expr(none, held);
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.expr(value, held);
                let yes = self.scan.viewport(value, held);
                self.expr(body, &bound(held, name, yes));
            }
            Expr::Arrow { params, body, .. } => {
                let inner: Vec<String> = held
                    .iter()
                    .filter(|h| !params.contains(h))
                    .cloned()
                    .collect();
                self.expr(body, &inner);
            }
        }
    }

    /// Each read in an action's statements, a `let` held for the
    /// statements after it in its block; an inlined call's own body left
    /// out when it is a prop's or an inject's (its writer's).
    fn stmts(&mut self, body: &[Stmt], mut held: Vec<String>) {
        for s in body {
            match s {
                Stmt::Let { name, expr, .. } => {
                    self.expr(expr, &held);
                    let yes = self.scan.viewport(expr, &held);
                    held = bound(&held, name, yes);
                }
                Stmt::Assign { target, expr, .. } => {
                    self.expr(expr, &held);
                    if self.scan.viewport(expr, &held) {
                        self.found.states.push(target.clone());
                    }
                }
                Stmt::Command { name, args, span } => {
                    // A call of another action, unexpanded (the lint's).
                    let call = Expr::Call(name.clone(), args.clone(), *span);
                    self.expr(&call, &held);
                }
                Stmt::Send { args, .. } => args.iter().for_each(|a| self.expr(a, &held)),
                Stmt::Refresh { .. } => {}
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond, &held);
                    self.stmts(then, held.clone());
                    self.stmts(otherwise, held.clone());
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject, &held);
                    let yes = self.scan.viewport(subject, &held);
                    self.stmts(&some.1, bound(&held, &some.0, yes));
                    self.stmts(none, held.clone());
                }
                Stmt::Call {
                    args,
                    body,
                    binding,
                    ..
                } => {
                    args.iter().for_each(|a| self.expr(a, &held));
                    if binding.is_none() {
                        self.stmts(body, held.clone());
                    }
                }
            }
        }
    }
}
