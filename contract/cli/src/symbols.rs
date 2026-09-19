//! `contract symbols <file>`: every definition and every reference in a
//! file and the files it uses, as JSON (LLP 1035.005 D2) — the query an
//! editor needs before an LSP is justified. Definitions are components,
//! props, injects, states, derives, actions, resources, mutations, data
//! sources (first occurrence), styles, fns, shapes, and HTML `id`s;
//! references resolve to a definition's index. Locals (`each` items,
//! `match` bindings, parameters) and roster names are neither.

use crate::CompileError;
use contract_syntax::{
    Action, Attr, Component, Expr, File, Node, Span, Stmt, TemplatePart, TypeExpr,
};
use std::path::{Path, PathBuf};

/// One definition.
pub struct Definition {
    /// `component`, `prop`, `inject`, `state`, `derive`, `action`,
    /// `resource`, `mutation`, `source`, `style`, `fn`, `shape`, or `id`.
    pub kind: &'static str,
    /// The name.
    pub name: String,
    /// The file, as the path was given or joined from a `use`.
    pub file: String,
    /// Where the name is.
    pub span: Span,
    /// The enclosing component, for a component-scoped kind.
    pub component: Option<String>,
}

/// One reference, resolved.
pub struct Reference {
    /// The definition's kind.
    pub kind: &'static str,
    /// The name as written.
    pub name: String,
    /// The file.
    pub file: String,
    /// Where.
    pub span: Span,
    /// The index into the definitions.
    pub to: usize,
}

/// The attributes whose string value names an `id` (LLP 1001 §1).
const ID_REFS: [&str; 3] = ["navigationBack", "contextTarget", "popovertarget"];

/// The symbols of `path` and, transitively, of the files it uses.
pub fn symbols(path: &Path) -> Result<(Vec<Definition>, Vec<Reference>), CompileError> {
    let mut files: Vec<(String, File)> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    load(path, &mut files, &mut seen)?;
    let mut defs = Vec::new();
    for (file, f) in &files {
        definitions(file, f, &mut defs);
    }
    let mut refs = Vec::new();
    for (file, f) in &files {
        let mut r = Resolver {
            file,
            defs: &defs,
            out: &mut refs,
            locals: Vec::new(),
            component: None,
        };
        r.file(f);
    }
    Ok((defs, refs))
}

/// The symbols as JSON: `{ "definitions": [ {kind, name, file, line, col,
/// end_col, component?} … ], "references": [ {kind, name, file, line, col,
/// end_col, to} … ] }`.
pub fn symbols_json(path: &Path) -> Result<String, CompileError> {
    let (defs, refs) = symbols(path)?;
    let definitions: Vec<serde_json::Value> = defs
        .iter()
        .map(|d| {
            let mut v = serde_json::json!({
                "kind": d.kind,
                "name": d.name,
                "file": d.file,
                "line": d.span.line,
                "col": d.span.col,
                "end_col": d.span.end_col,
            });
            if let Some(c) = &d.component {
                v["component"] = serde_json::Value::String(c.clone());
            }
            v
        })
        .collect();
    let references: Vec<serde_json::Value> = refs
        .iter()
        .map(|r| {
            serde_json::json!({
                "kind": r.kind,
                "name": r.name,
                "file": r.file,
                "line": r.span.line,
                "col": r.span.col,
                "end_col": r.span.end_col,
                "to": r.to,
            })
        })
        .collect();
    Ok(serde_json::json!({ "definitions": definitions, "references": references }).to_string())
}

fn load(
    path: &Path,
    files: &mut Vec<(String, File)>,
    seen: &mut Vec<PathBuf>,
) -> Result<(), CompileError> {
    let key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if seen.contains(&key) {
        return Ok(());
    }
    seen.push(key);
    let display = path.display().to_string();
    let src = std::fs::read_to_string(path).map_err(|e| CompileError {
        pass: "use",
        id: "contract-use-unreadable".into(),
        message: format!("{display}: {e}"),
        file: String::new(),
        span: Span::default(),
        related: Box::default(),
    })?;
    let file = contract_syntax::parse(&src).map_err(|e| {
        let mut e = CompileError::from(e);
        e.file = display.clone();
        e
    })?;
    let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let uses: Vec<PathBuf> = file
        .uses
        .iter()
        .map(|u| dir.join(u.path.trim_start_matches("./")))
        .collect();
    files.push((display, file));
    for used in uses {
        load(&used, files, seen)?;
    }
    Ok(())
}

// ---- definitions ----------------------------------------------------------

fn definitions(file: &str, f: &File, out: &mut Vec<Definition>) {
    let def = |kind: &'static str, name: &str, span: Span, component: Option<&str>| Definition {
        kind,
        name: name.to_string(),
        file: file.to_string(),
        span,
        component: component.map(str::to_string),
    };
    for s in &f.shapes {
        out.push(def("shape", &s.name, s.span, None));
    }
    for s in &f.styles {
        out.push(def("style", &s.name, s.span, None));
    }
    for d in &f.fns {
        out.push(def("fn", &d.name, d.span, None));
    }
    for c in &f.components {
        let cn = Some(c.name.as_str());
        out.push(def("component", &c.name, c.span, None));
        for p in &c.props {
            out.push(def("prop", &p.name, p.span, cn));
        }
        for p in &c.injects {
            out.push(def("inject", &p.name, p.span, cn));
        }
        for b in &c.states {
            out.push(def("state", &b.name, b.span, cn));
        }
        for b in &c.derives {
            out.push(def("derive", &b.name, b.span, cn));
        }
        for r in &c.resources {
            out.push(def("resource", &r.name, r.span, cn));
            if !out.iter().any(|d| d.kind == "source" && d.name == r.source) {
                out.push(def("source", &r.source, r.source_span, None));
            }
        }
        for m in &c.mutations {
            out.push(def("mutation", &m.name, m.span, cn));
        }
        for a in &c.actions {
            out.push(def("action", &a.name, a.span, cn));
            let mut sources = Vec::new();
            sends(&a.body, &mut sources);
            for (name, span) in sources {
                if !out.iter().any(|d| d.kind == "source" && d.name == *name) {
                    out.push(def("source", name, span, None));
                }
            }
        }
        let mut ids = Vec::new();
        id_attrs(&c.view, &mut ids);
        for (name, span) in ids {
            out.push(def("id", name, span, cn));
        }
    }
}

/// Every `send … = source(…)` in a body, in order.
fn sends<'a>(stmts: &'a [Stmt], out: &mut Vec<(&'a String, Span)>) {
    for s in stmts {
        match s {
            Stmt::Send {
                source,
                source_span,
                ..
            } => out.push((source, *source_span)),
            Stmt::If {
                then, otherwise, ..
            } => {
                sends(then, out);
                sends(otherwise, out);
            }
            Stmt::Match { some, none, .. } => {
                sends(&some.1, out);
                sends(none, out);
            }
            Stmt::Assign { .. } | Stmt::Command { .. } | Stmt::Refresh { .. } => {}
        }
    }
}

/// Every literal `id="…"` in a view, in order.
fn id_attrs<'a>(nodes: &'a [Node], out: &mut Vec<(&'a String, Span)>) {
    for n in nodes {
        match n {
            Node::Element {
                attrs, children, ..
            } => {
                for a in attrs {
                    if let (true, Expr::Str(id, span)) = (a.name == "id", &a.value) {
                        out.push((id, *span));
                    }
                }
                id_attrs(children, out);
            }
            Node::Use { children, .. } => id_attrs(children, out),
            Node::Provide { body, .. } => id_attrs(body, out),
            Node::Children { .. } => {}
            Node::When {
                then, otherwise, ..
            } => {
                id_attrs(then, out);
                id_attrs(otherwise, out);
            }
            Node::Each { body, .. } => id_attrs(body, out),
            Node::Match { some, none, .. } => {
                id_attrs(&some.1, out);
                id_attrs(none, out);
            }
        }
    }
}

// ---- references -----------------------------------------------------------

struct Resolver<'a> {
    file: &'a str,
    defs: &'a [Definition],
    out: &'a mut Vec<Reference>,
    /// Names bound by `each`, `match`, and parameters, innermost last.
    locals: Vec<String>,
    /// The component whose declarations names resolve against.
    component: Option<&'a Component>,
}

impl<'a> Resolver<'a> {
    fn find(&self, kinds: &[&str], name: &str, component: Option<&str>) -> Option<usize> {
        self.defs.iter().position(|d| {
            kinds.contains(&d.kind) && d.name == name && d.component.as_deref() == component
        })
    }

    /// An `id` definition, whichever component declares it: ids are one
    /// namespace across the app.
    fn id(&self, id: &str) -> Option<usize> {
        self.defs
            .iter()
            .position(|d| d.kind == "id" && d.name == id)
    }

    fn refer(&mut self, to: usize, name: &str, span: Span) {
        // A definition is not a reference to itself.
        let d = &self.defs[to];
        if d.file == self.file && d.span == span {
            return;
        }
        self.out.push(Reference {
            kind: d.kind,
            name: name.to_string(),
            file: self.file.to_string(),
            span,
            to,
        });
    }

    /// A name in expression position, in the current component.
    fn name(&mut self, name: &str, span: Span, call: bool) {
        if self.locals.iter().any(|l| l == name) {
            return;
        }
        let component = self.component.map(|c| c.name.as_str());
        let scoped = [
            "prop", "inject", "state", "derive", "resource", "mutation", "action",
        ];
        if let Some(i) = component.and_then(|c| self.find(&scoped, name, Some(c))) {
            self.refer(i, name, span);
        } else if let (true, Some(i)) = (call, self.find(&["fn"], name, None)) {
            self.refer(i, name, span);
        }
    }

    fn file(&mut self, f: &'a File) {
        for u in &f.uses {
            if let Some(i) = self.find(&["component", "shape", "style", "fn"], &u.name, None) {
                self.refer(i, &u.name, u.span);
            }
        }
        for s in &f.shapes {
            for field in &s.fields {
                self.type_expr(&field.ty);
            }
        }
        for s in &f.styles {
            for a in &s.attrs {
                self.expr(&a.value);
            }
        }
        for d in &f.fns {
            for p in &d.params {
                if let Some(t) = &p.ty {
                    self.type_expr(t);
                }
                self.locals.push(p.name.clone());
            }
            self.type_expr(&d.ret);
            self.expr(&d.body);
            self.locals.clear();
        }
        for c in &f.components {
            self.component = Some(c);
            self.component_body(c);
            self.component = None;
        }
    }

    fn component_body(&mut self, c: &'a Component) {
        let cn = Some(c.name.as_str());
        for p in c.props.iter().chain(&c.injects) {
            if let Some(t) = &p.ty {
                self.type_expr(t);
            }
        }
        for b in c.states.iter().chain(&c.derives) {
            self.expr(&b.expr);
        }
        for r in &c.resources {
            if let Some(i) = self.find(&["source"], &r.source, None) {
                self.refer(i, &r.source, r.source_span);
            }
            for a in &r.args {
                self.expr(a);
            }
            self.type_expr(&r.shape);
        }
        for m in &c.mutations {
            self.type_expr(&m.shape);
        }
        for a in &c.actions {
            self.action(a, cn);
        }
        for t in &c.tasks {
            self.expr(&t.every.0);
            if let Some(i) = self.find(&["action"], &t.every.1, cn) {
                self.refer(i, &t.every.1, t.every.2);
            }
        }
        self.nodes(&c.view);
    }

    fn action(&mut self, a: &'a Action, cn: Option<&str>) {
        for (w, span) in &a.writes {
            if let Some(i) = self.find(&["state", "mutation"], w, cn) {
                self.refer(i, w, *span);
            }
        }
        for p in &a.params {
            if let Some(t) = &p.ty {
                self.type_expr(t);
            }
            self.locals.push(p.name.clone());
        }
        self.stmts(&a.body, cn);
        self.locals.clear();
    }

    fn stmts(&mut self, stmts: &'a [Stmt], cn: Option<&str>) {
        for s in stmts {
            match s {
                Stmt::Assign { target, expr, span } => {
                    if let Some(i) = self.find(&["state", "mutation"], target, cn) {
                        self.refer(i, target, *span);
                    }
                    self.expr(expr);
                }
                Stmt::Command { name, args, .. } => {
                    if let ("focus", [Expr::Str(id, span)]) = (name.as_str(), args.as_slice()) {
                        if let Some(i) = self.id(id) {
                            self.refer(i, id, *span);
                        }
                        continue;
                    }
                    for a in args {
                        self.expr(a);
                    }
                }
                Stmt::Send {
                    target,
                    source,
                    source_span,
                    args,
                    span,
                } => {
                    if let Some(i) = self.find(&["mutation"], target, cn) {
                        self.refer(i, target, *span);
                    }
                    if let Some(i) = self.find(&["source"], source, None) {
                        self.refer(i, source, *source_span);
                    }
                    for a in args {
                        self.expr(a);
                    }
                }
                Stmt::Refresh { target, span } => {
                    if let Some(i) = self.find(&["resource"], target, cn) {
                        self.refer(i, target, *span);
                    }
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond);
                    self.stmts(then, cn);
                    self.stmts(otherwise, cn);
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject);
                    self.locals.push(some.0.clone());
                    self.stmts(&some.1, cn);
                    self.locals.pop();
                    self.stmts(none, cn);
                }
            }
        }
    }

    fn nodes(&mut self, nodes: &'a [Node]) {
        for n in nodes {
            match n {
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
                    if let Some(i) = self.find(&["component"], name, None) {
                        self.refer(i, name, *span);
                    }
                    for a in args {
                        if let Some(i) = self.find(&["prop"], &a.name, Some(name)) {
                            self.refer(i, &a.name, a.span);
                        }
                        self.expr(&a.value);
                    }
                    self.nodes(children);
                }
                Node::Provide { expr, body, .. } => {
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
                    ..
                } => {
                    self.expr(list);
                    self.locals.push(var.clone());
                    self.expr(key);
                    self.nodes(body);
                    self.locals.pop();
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject);
                    self.locals.push(some.0.clone());
                    self.nodes(&some.1);
                    self.locals.pop();
                    self.nodes(none);
                }
            }
        }
    }

    fn attr(&mut self, a: &'a Attr) {
        match (a.name.as_str(), &a.value) {
            ("id", Expr::Str(..)) => {}
            ("class", Expr::Ident(name, span)) => {
                if let Some(i) = self.find(&["style"], name, None) {
                    self.refer(i, name, *span);
                }
            }
            (attr, Expr::Str(id, span)) if ID_REFS.contains(&attr) => {
                if let Some(i) = self.id(id) {
                    self.refer(i, id, *span);
                }
            }
            _ => self.expr(&a.value),
        }
    }

    fn type_expr(&mut self, t: &TypeExpr) {
        match t {
            TypeExpr::Named(name, span) => {
                if let Some(i) = self.find(&["shape"], name, None) {
                    self.refer(i, name, *span);
                }
            }
            TypeExpr::Option(inner, _) | TypeExpr::List(inner, _) => self.type_expr(inner),
        }
    }

    fn expr(&mut self, e: &'a Expr) {
        match e {
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) => {}
            Expr::Template(parts, _) => {
                for p in parts {
                    if let TemplatePart::Expr(e) = p {
                        self.expr(e);
                    }
                }
            }
            Expr::Some(inner, _) | Expr::NamedArg(_, inner, _) => self.expr(inner),
            Expr::Ident(name, span) => self.name(name, *span, false),
            Expr::Member(base, _, _) => self.expr(base),
            Expr::Call(name, args, span) => {
                self.name(name, *span, true);
                for a in args {
                    self.expr(a);
                }
            }
            Expr::Unary(_, inner, _) => self.expr(inner),
            Expr::Binary(_, l, r, _) => {
                self.expr(l);
                self.expr(r);
            }
            Expr::Ternary(c, a, b, _) => {
                self.expr(c);
                self.expr(a);
                self.expr(b);
            }
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => {
                self.expr(subject);
                self.locals.push(var.clone());
                self.expr(some);
                self.locals.pop();
                self.expr(none);
            }
        }
    }
}
