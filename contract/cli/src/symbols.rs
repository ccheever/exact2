//! Compiler-owned navigation over the same imports, syntax and types as a build.
//! @ref LLP 1035.005 D2 — JSON queries before an editor protocol.

use crate::{
    sources::{self, Sources},
    CompileError,
};
use contract_syntax::*;
use contract_types::{Ref, Scope, Ty, Types};
use std::{collections::BTreeMap, io::Write, path::Path};

struct Definition {
    kind: &'static str,
    name: String,
    span: Span,
    component: Option<String>,
    owner: Option<String>,
}
struct Reference {
    span: Span,
    to: usize,
}
#[derive(Default)]
struct Graph {
    definitions: Vec<Definition>,
    references: Vec<Reference>,
    // Locals resolve by lexical stack; named declarations by namespace/owner.
    index: BTreeMap<(&'static str, String, String, String), usize>,
    ids: BTreeMap<String, Vec<usize>>,
}
impl Graph {
    fn define(
        &mut self,
        kind: &'static str,
        name: &str,
        span: Span,
        component: Option<&str>,
        owner: Option<&str>,
    ) -> usize {
        let index = self.definitions.len();
        self.index.insert(
            (
                kind,
                component.unwrap_or("").into(),
                owner.unwrap_or("").into(),
                name.into(),
            ),
            index,
        );
        self.definitions.push(Definition {
            kind,
            name: name.into(),
            span,
            component: component.map(str::to_owned),
            owner: owner.map(str::to_owned),
        });
        if kind == "id" {
            self.ids.entry(name.into()).or_default().push(index);
        }
        index
    }
    fn find(
        &self,
        kind: &'static str,
        name: &str,
        component: Option<&str>,
        owner: Option<&str>,
    ) -> Option<usize> {
        self.index
            .get(&(
                kind,
                component.unwrap_or("").into(),
                owner.unwrap_or("").into(),
                name.into(),
            ))
            .copied()
    }
    fn refer(&mut self, span: Span, to: usize) {
        if span != self.definitions[to].span {
            self.references.push(Reference { span, to });
        }
    }
    fn source(&mut self, name: &str, span: Span) {
        let to = self
            .find("source", name, None, None)
            .unwrap_or_else(|| self.define("source", name, span, None, None));
        self.refer(span, to);
    }
    fn id(&mut self, name: &str, span: Span) {
        // An authored ID can appear in more than one component. Report all
        // matching declarations rather than inventing a unique runtime target.
        if let Some(ids) = self.ids.get(name).cloned() {
            for to in ids {
                self.refer(span, to);
            }
        }
    }
    fn json(&self, sources: &Sources) -> String {
        // Emit the existing graph directly, without cloning every name and
        // filename into a second tree of JSON objects.
        let mut out = Vec::with_capacity((self.definitions.len() + self.references.len()) * 128);
        out.extend_from_slice(b"{\"definitions\":[");
        for (i, definition) in self.definitions.iter().enumerate() {
            if i != 0 {
                out.push(b',');
            }
            write_symbol(&mut out, sources, definition, definition.span, None);
        }
        out.extend_from_slice(b"],\"references\":[");
        for (i, reference) in self.references.iter().enumerate() {
            if i != 0 {
                out.push(b',');
            }
            write_symbol(
                &mut out,
                sources,
                &self.definitions[reference.to],
                reference.span,
                Some(reference.to),
            );
        }
        out.extend_from_slice(b"]}");
        String::from_utf8(out).expect("JSON serialization is UTF-8")
    }
}

fn write_symbol(
    out: &mut Vec<u8>,
    sources: &Sources,
    definition: &Definition,
    span: Span,
    to: Option<usize>,
) {
    // Keep the previous sorted property order as well as its optional fields.
    write!(out, "{{\"col\":{}", span.col).unwrap();
    if to.is_none() {
        if let Some(component) = &definition.component {
            out.extend_from_slice(b",\"component\":");
            quote(out, component);
        }
    }
    write!(out, ",\"end_col\":{},\"file\":", span.end_col).unwrap();
    quote(out, &sources.path(span).to_string_lossy());
    out.extend_from_slice(b",\"kind\":");
    quote(out, definition.kind);
    write!(out, ",\"line\":{},\"name\":", span.line).unwrap();
    quote(out, &definition.name);
    if let Some(to) = to {
        write!(out, ",\"to\":{to}").unwrap();
    } else if let Some(owner) = &definition.owner {
        out.extend_from_slice(b",\"owner\":");
        quote(out, owner);
    }
    out.push(b'}');
}

fn quote(out: &mut Vec<u8>, value: &str) {
    serde_json::to_writer(out, value).expect("writing JSON into a Vec cannot fail");
}

/// Definitions and references as JSON, with exact original byte ranges.
/// Uses the build's import policy and type checker. No plan or bake is produced.
/// Local bindings, parameters and shape fields are included; built-in names have
/// no authored definition. Repeated ID declarations produce multiple edges.
pub fn symbols_json(path: &Path) -> Result<String, CompileError> {
    let src = std::fs::read_to_string(path).map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: e.to_string(),
        span: Span::default(),
        file: Some(path.into()),
        related: Box::new([]),
    })?;
    let root = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(|e| CompileError {
            pass: "use",
            id: "contract-use-unreadable".into(),
            message: e.to_string(),
            span: Span::default(),
            file: Some(path.into()),
            related: Box::new([]),
        })?;
    let (file, sources) = sources::load(path, &src, &root)?;
    let (types, expanded) = if file.components.is_empty() {
        (
            Types {
                shapes: contract_types::check_declarations(&file)
                    .map_err(|e| sources.resolve(e.into()))?,
                components: Vec::new(),
            },
            None,
        )
    } else {
        let checked = contract_types::check(&file).map_err(|e| sources.resolve(e.into()))?;
        (checked.types, Some(checked.expanded))
    };
    let mut r = Resolver {
        file: &file,
        types: &types,
        graph: Graph::default(),
        component: None,
        scope: Scope::default(),
        locals: Vec::new(),
        owner: None,
    };
    r.declarations();
    for import in &sources.imports {
        for kind in ["component", "shape", "style", "fn"] {
            if let Some(to) = r.graph.find(kind, &import.name, None, None) {
                r.graph.refer(file.names.name(import.span), to);
            }
        }
    }
    r.file(expanded.as_ref().map(|e| &e.root));
    Ok(r.graph.json(&sources))
}

struct Resolver<'a> {
    file: &'a File,
    types: &'a Types,
    graph: Graph,
    component: Option<&'a Component>,
    scope: Scope,
    locals: Vec<(String, usize)>,
    owner: Option<String>,
}
impl<'a> Resolver<'a> {
    fn declarations(&mut self) {
        let names = &self.file.names;
        for shape in &self.file.shapes {
            self.graph
                .define("shape", &shape.name, names.name(shape.span), None, None);
            for field in &shape.fields {
                self.graph
                    .define("field", &field.name, field.span, None, Some(&shape.name));
            }
        }
        for style in &self.file.styles {
            self.graph
                .define("style", &style.name, names.name(style.span), None, None);
        }
        for f in &self.file.fns {
            self.graph
                .define("fn", &f.name, names.name(f.span), None, None);
        }
        for c in &self.file.components {
            self.graph
                .define("component", &c.name, names.name(c.span), None, None);
            let cn = Some(c.name.as_str());
            for (kind, params) in [("prop", &c.props), ("inject", &c.injects)] {
                for p in params {
                    self.graph.define(kind, &p.name, p.span, cn, None);
                }
            }
            for (kind, bindings) in [("state", &c.states), ("derive", &c.derives)] {
                for b in bindings {
                    self.graph
                        .define(kind, &b.name, names.name(b.span), cn, None);
                }
            }
            for resource in &c.resources {
                self.graph.define(
                    "resource",
                    &resource.name,
                    names.name(resource.span),
                    cn,
                    None,
                );
            }
            for mutation in &c.mutations {
                self.graph.define(
                    "mutation",
                    &mutation.name,
                    names.name(mutation.span),
                    cn,
                    None,
                );
            }
            for action in &c.actions {
                self.graph
                    .define("action", &action.name, names.name(action.span), cn, None);
            }
            for task in &c.tasks {
                self.graph
                    .define("task", &task.name, names.name(task.span), cn, None);
            }
            self.ids(&c.view, &c.name);
        }
        if let Some(routes) = &self.file.routes {
            self.graph.define(
                "state",
                &routes.slot,
                names.name(routes.span),
                Some(&self.file.components[0].name),
                None,
            );
            for route in &routes.rows {
                self.graph
                    .define("route", &route.name, names.name(route.span), None, None);
            }
        }
    }
    fn ids(&mut self, nodes: &[Node], component: &str) {
        for node in nodes {
            match node {
                Node::Element {
                    attrs, children, ..
                } => {
                    for a in attrs {
                        if let ("id" | "testId", Expr::Str(id, span)) = (a.name.as_str(), &a.value)
                        {
                            self.graph.define(
                                if a.name == "id" { "id" } else { "testId" },
                                id,
                                *span,
                                Some(component),
                                None,
                            );
                        }
                    }
                    self.ids(children, component);
                }
                Node::Use { children, .. } => self.ids(children, component),
                Node::Provide { body, .. } | Node::Each { body, .. } => self.ids(body, component),
                Node::When {
                    then, otherwise, ..
                } => {
                    self.ids(then, component);
                    self.ids(otherwise, component);
                }
                Node::Match { some, none, .. } => {
                    self.ids(&some.1, component);
                    self.ids(none, component);
                }
                Node::Children { .. } => {}
            }
        }
    }
    fn refer(
        &mut self,
        kind: &'static str,
        name: &str,
        span: Span,
        component: Option<&str>,
        owner: Option<&str>,
    ) {
        if let Some(to) = self.graph.find(kind, name, component, owner) {
            self.graph.refer(span, to);
        }
    }
    fn name(&mut self, name: &str, span: Span) {
        if let Some((_, to)) = self.locals.iter().rev().find(|(n, _)| n == name) {
            self.graph.refer(span, *to);
            return;
        }
        if let Some(c) = self.component {
            for kind in [
                "prop", "inject", "state", "derive", "resource", "mutation", "action",
            ] {
                if let Some(to) = self.graph.find(kind, name, Some(&c.name), None) {
                    self.graph.refer(span, to);
                    return;
                }
            }
        }
    }
    fn target(&mut self, kinds: &[&'static str], name: &str, span: Span) {
        if let Some(c) = self.component {
            for kind in kinds {
                if let Some(to) = self.graph.find(kind, name, Some(&c.name), None) {
                    self.graph.refer(span, to);
                    return;
                }
            }
        }
    }
    fn local(&mut self, kind: &'static str, name: &str, span: Span, ty: Ty) {
        let to = self.graph.define(
            kind,
            name,
            span,
            self.component.map(|c| c.name.as_str()),
            self.owner.as_deref(),
        );
        self.locals.push((name.into(), to));
        self.scope.push(vec![(name.into(), Ref::Local(0), ty)]);
    }
    fn pop_local(&mut self) {
        self.locals.pop();
        self.scope.pop();
    }
    fn ty(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Named(name, span) => {
                if matches!(self.types.shapes.resolve(ty), Ok(Ty::Record(ref shape)) if shape == name)
                {
                    self.refer("shape", name, *span, None, None);
                }
            }
            TypeExpr::List(inner, _) | TypeExpr::Option(inner, _) => self.ty(inner),
        }
    }
    fn infer(&self, expr: &Expr) -> Ty {
        contract_types::infer(expr, &self.scope, &self.types.shapes).unwrap_or(Ty::Unknown)
    }
    fn file(&mut self, expanded_root: Option<&Component>) {
        for shape in &self.file.shapes {
            for field in &shape.fields {
                self.ty(&field.ty);
            }
        }
        for f in &self.file.fns {
            self.owner = Some(f.name.clone());
            for p in &f.params {
                let ty = p.ty.as_ref().expect("checked function parameter");
                self.ty(ty);
                self.local(
                    "parameter",
                    &p.name,
                    p.span,
                    self.types.shapes.resolve(ty).expect("checked type"),
                );
            }
            self.ty(&f.ret);
            self.expr(&f.body);
            for _ in &f.params {
                self.pop_local();
            }
            self.owner = None;
        }
        for (ci, c) in self.file.components.iter().enumerate() {
            self.component = Some(c);
            self.scope = self.types.component_scope(
                if ci == 0 {
                    expanded_root.expect("component present")
                } else {
                    c
                },
                &self.types.components[ci],
            );
            for p in c.props.iter().chain(&c.injects) {
                if let Some(ty) = &p.ty {
                    self.ty(ty);
                }
            }
            for b in c.states.iter().chain(&c.derives) {
                self.expr(&b.expr);
            }
            for r in &c.resources {
                self.graph
                    .source(&r.source, self.file.names.sources[&r.span]);
                for arg in &r.args {
                    self.expr(arg);
                }
                self.ty(&r.shape);
            }
            for m in &c.mutations {
                self.ty(&m.shape);
            }
            for (ai, a) in c.actions.iter().enumerate() {
                self.owner = Some(a.name.clone());
                for (name, span) in &a.writes {
                    self.name(name, *span);
                }
                for (pi, p) in a.params.iter().enumerate() {
                    if let Some(ty) = &p.ty {
                        self.ty(ty);
                    }
                    self.local(
                        "parameter",
                        &p.name,
                        p.span,
                        self.types.components[ci].actions[ai][pi].clone(),
                    );
                }
                self.stmts(&a.body);
                for _ in &a.params {
                    self.pop_local();
                }
                self.owner = None;
            }
            for t in &c.tasks {
                self.expr(&t.every.0);
                self.name(&t.every.1, self.file.names.name(t.every.2));
            }
            self.nodes(&c.view);
        }
        self.component = None;
    }
    fn stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Assign { target, expr, span } => {
                    self.target(&["state", "mutation"], target, *span);
                    self.expr(expr);
                }
                Stmt::Command { name, args, .. } => {
                    if let ("focus", [Expr::Str(id, span)]) = (name.as_str(), args.as_slice()) {
                        self.graph.id(id, *span);
                    }
                    for arg in args {
                        self.expr(arg);
                    }
                }
                Stmt::Send {
                    target,
                    source,
                    args,
                    span,
                } => {
                    self.target(&["mutation"], target, self.file.names.name(*span));
                    self.graph.source(source, self.file.names.sources[span]);
                    for arg in args {
                        self.expr(arg);
                    }
                }
                Stmt::Refresh { target, span } => {
                    self.target(&["resource"], target, self.file.names.name(*span))
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond);
                    self.stmts(then);
                    self.stmts(otherwise);
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    span,
                } => {
                    self.expr(subject);
                    let ty = match self.infer(subject) {
                        Ty::Option(t) => *t,
                        _ => Ty::Unknown,
                    };
                    self.local("local", &some.0, self.file.names.name(*span), ty);
                    self.stmts(&some.1);
                    self.pop_local();
                    self.stmts(none);
                }
            }
        }
    }
    fn nodes(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element {
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    for p in positional {
                        self.expr(p);
                    }
                    for a in attrs {
                        self.attr(a);
                    }
                    self.nodes(children);
                }
                Node::Use {
                    name,
                    args,
                    children,
                    span,
                } => {
                    self.refer("component", name, *span, None, None);
                    for a in args {
                        self.refer("prop", &a.name, a.span, Some(name), None);
                        self.expr(&a.value);
                    }
                    self.nodes(children);
                }
                Node::Provide {
                    name,
                    expr,
                    body,
                    span,
                } => {
                    self.graph.define(
                        "provide",
                        name,
                        self.file.names.name(*span),
                        self.component.map(|c| c.name.as_str()),
                        None,
                    );
                    self.expr(expr);
                    self.nodes(body);
                }
                Node::Children { .. } => {}
                Node::When {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond);
                    self.nodes(then);
                    self.nodes(otherwise);
                }
                Node::Each {
                    var,
                    list,
                    key,
                    body,
                    span,
                    ..
                } => {
                    self.expr(list);
                    let ty = match self.infer(list) {
                        Ty::List(t) => *t,
                        _ => Ty::Unknown,
                    };
                    self.local("local", var, self.file.names.name(*span), ty);
                    self.expr(key);
                    self.nodes(body);
                    self.pop_local();
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    span,
                } => {
                    self.expr(subject);
                    let ty = match self.infer(subject) {
                        Ty::Option(t) => *t,
                        _ => Ty::Unknown,
                    };
                    self.local("local", &some.0, self.file.names.name(*span), ty);
                    self.nodes(&some.1);
                    self.pop_local();
                    self.nodes(none);
                }
            }
        }
    }
    fn attr(&mut self, a: &Attr) {
        match (a.name.as_str(), &a.value) {
            ("id" | "testId", Expr::Str(..)) => {}
            ("class", Expr::Ident(name, span)) => self.refer("style", name, *span, None, None),
            ("navigationBack" | "contextTarget" | "popovertarget", Expr::Str(id, span)) => {
                self.graph.id(id, *span)
            }
            (name, Expr::Call(action, args, span))
                if contract_analyze::HANDLERS.contains(&name) =>
            {
                self.name(action, *span);
                for arg in args {
                    self.expr(arg);
                }
            }
            _ => self.expr(&a.value),
        }
    }
    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
            Expr::Template(parts, _) => {
                for part in parts {
                    if let TemplatePart::Expr(e) = part {
                        self.expr(e);
                    }
                }
            }
            Expr::Some(inner, _) | Expr::Unary(_, inner, _) => self.expr(inner),
            Expr::Ident(name, span) => self.name(name, *span),
            Expr::Member(base, field, span) => {
                self.expr(base);
                if let Ty::Record(shape) = self.infer(base) {
                    self.refer("field", field, *span, None, Some(&shape));
                }
            }
            Expr::Call(name, args, span) => {
                if self.types.shapes.fns.contains_key(name) {
                    self.refer("fn", name, *span, None, None);
                } else if name == "path" {
                    if let Some(Expr::Str(route, span)) = args.first() {
                        self.refer("route", route, *span, None, None);
                    }
                } else if matches!(self.infer(expr), Ty::Action(_)) {
                    self.name(name, *span);
                }
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Binary(_, lhs, rhs, _) => {
                self.expr(lhs);
                self.expr(rhs);
            }
            Expr::Ternary(cond, then, otherwise, _) => {
                self.expr(cond);
                self.expr(then);
                self.expr(otherwise);
            }
            Expr::Match {
                subject,
                var,
                some,
                none,
                span,
            } => {
                self.expr(subject);
                let ty = match self.infer(subject) {
                    Ty::Option(t) => *t,
                    _ => Ty::Unknown,
                };
                self.local("local", var, self.file.names.name(*span), ty);
                self.expr(some);
                self.pop_local();
                self.expr(none);
            }
        }
    }
}
