//! Module scope (LLP 1091 D1/D4/D5): each file's names are its own
//! declarations and what its `use` lines name. The loader gives every
//! declaration a program-unique name and rewrites each file through its own
//! scope before the files are merged, so every later pass sees one flat
//! namespace in which a name means one declaration.

use crate::ast::*;
use crate::parser::SyntaxError;
use crate::Span;
use std::cell::RefCell;
use std::collections::HashMap;

/// The namespaces a top-level name lives in. Shapes and `fn`s share one:
/// both are called as `Name(…)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// `component`.
    Component,
    /// `shape` and `fn`.
    Call,
    /// `style`.
    Style,
    /// `keyframes`.
    Keyframes,
    /// `timeline`.
    Timeline,
}

impl Kind {
    /// The words a refusal uses.
    pub fn what(self) -> &'static str {
        match self {
            Kind::Component => "component",
            Kind::Call => "shape or function",
            Kind::Style => "style",
            Kind::Keyframes => "keyframes",
            Kind::Timeline => "timeline",
        }
    }
}

/// A name another loaded file declares, which a file does not see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Elsewhere {
    /// The declaring file, as a refusal shows it.
    pub file: String,
    /// The loader's index of that file.
    pub unit: usize,
    /// The declaration's own name, which a `use` names: a generated one
    /// (`Card__ui`) is not.
    pub declared: String,
    /// Every file that declares a name `declared` in this namespace, by
    /// the loader's index: more than one, and which is meant is the author's.
    pub declaring: Vec<usize>,
}

/// A reference to a name only another file declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// Its namespace.
    pub kind: Kind,
    /// The name as written.
    pub name: String,
    /// Where.
    pub span: Span,
}

/// One file's view of the program's names.
#[derive(Debug, Default)]
pub struct Scope {
    /// Each name this file may write, by namespace, to the program-unique
    /// name of the declaration it means: its own declarations and its uses.
    pub names: HashMap<(Kind, String), String>,
    /// Names other loaded files declare that this file does not see, to the
    /// file that declares them, so a reference to one is refused with the
    /// `use` that would bring it rather than resolved behind the author's back.
    pub elsewhere: HashMap<(Kind, String), Elsewhere>,
    /// Every reference [`rescope`] met to a name in `elsewhere`, in order:
    /// the loader refuses them together, naming each `use` line the file
    /// lacks, so one compile shows them all (rules: every failure in one run).
    pub missing: RefCell<Vec<Missing>>,
    /// Refusals that are no `use` line's to fix, reported with the missing
    /// names rather than ending the pass before them.
    pub refused: RefCell<Vec<SyntaxError>>,
    /// Whether a call names a roster function, which no file declares and
    /// every file sees: never another file's name, so never refused.
    pub roster: Option<fn(&str) -> bool>,
    /// The types the compiler declares (`PointerEvent`, `Geometry`, …):
    /// every file sees them, so another file's `fn` of the name is never
    /// what a type means.
    pub builtin_types: std::collections::HashSet<String>,
    /// The program-unique names that are shapes, not `fn`s: a call of a
    /// shape named `path` is the router's `path()`, as the checker reads it.
    pub shapes: std::collections::HashSet<String>,
    /// Shapes other files declare that this file does not see as shapes:
    /// a written type of one of these names is refused even when this file
    /// has a `fn` of the name, which is no type.
    pub foreign_shapes: std::collections::HashMap<String, String>,
}

impl Scope {
    fn get(&self, kind: Kind, name: &str) -> Option<&str> {
        self.names.get(&(kind, name.to_owned())).map(String::as_str)
    }

    /// The program-unique name `name` means here; `Ok(None)` for a name no
    /// loaded file declares (a roster function, a primitive, a binding), left
    /// for later passes to resolve or refuse — and for one another file
    /// declares, recorded in `missing` for the loader to refuse.
    fn resolve(&self, kind: Kind, name: &str, span: Span) -> Result<Option<&str>, SyntaxError> {
        if let Some(to) = self.get(kind, name) {
            return Ok(Some(to));
        }
        if self.elsewhere.contains_key(&(kind, name.to_owned())) {
            self.missing.borrow_mut().push(Missing {
                kind,
                name: name.to_owned(),
                span,
            });
        }
        Ok(None)
    }

    fn rename(&self, kind: Kind, name: &mut String, span: Span) -> Result<(), SyntaxError> {
        if let Some(to) = self.resolve(kind, name, span)? {
            if to != name {
                *name = to.to_owned();
            }
        }
        Ok(())
    }
}

/// Rewrite `file` through `scope`: its declarations take their
/// program-unique names, and every reference to a top-level name — a
/// component use, a call, a type, `class=`, the keyframes an `animation`
/// literal names, a `clock(Name)` literal — takes the name of the
/// declaration it means. `animation-timeline=Name` is left to
/// [`resolve_clock_timelines_in`](crate::clock::resolve_clock_timelines_in),
/// which knows the bindings that shadow it.
pub fn rescope(file: &mut File, scope: &Scope) -> Result<(), SyntaxError> {
    walk(file, scope, None)
}

/// Every name `file` binds locally: component members, parameters, and
/// `each`, `match`, arrow and `let` binders. The loader renames another
/// file's `fn` or shape of one of these names, so the type checker, which
/// calls a `fn` before a binding, never reaches past the binding to it.
pub fn bindings(file: &mut File) -> std::collections::HashSet<String> {
    let mut seen = Seen::default();
    walk(file, &Scope::default(), Some(&mut seen)).expect("an empty scope refuses nothing");
    seen.bound
}

/// Every name `file` calls as `name(…)`.
pub fn calls(file: &mut File) -> std::collections::HashSet<String> {
    let mut seen = Seen::default();
    walk(file, &Scope::default(), Some(&mut seen)).expect("an empty scope refuses nothing");
    seen.called
}

/// What a collecting walk records.
#[derive(Default)]
struct Seen {
    bound: std::collections::HashSet<String>,
    called: std::collections::HashSet<String>,
}

fn walk(file: &mut File, scope: &Scope, seen: Option<&mut Seen>) -> Result<(), SyntaxError> {
    let mut r = Rewriter {
        scope,
        locals: Vec::new(),
        callable: Vec::new(),
        seen,
    };
    let File {
        names: _,
        routes,
        uses: _,
        fonts: _,
        shapes,
        styles,
        keyframes,
        timelines,
        // A `@color-profile`'s dashed name is global, as in CSS.
        color_profiles: _,
        fns,
        tests,
        launch: _,
        components,
    } = file;
    for shape in shapes {
        r.scope.rename(Kind::Call, &mut shape.name, shape.span)?;
        for field in &mut shape.fields {
            r.ty(&mut field.ty)?;
        }
    }
    for style in styles {
        r.scope.rename(Kind::Style, &mut style.name, style.span)?;
        r.attrs(&mut style.attrs)?;
    }
    for k in keyframes {
        r.scope.rename(Kind::Keyframes, &mut k.name, k.span)?;
        for frame in &mut k.frames {
            r.attrs(&mut frame.attrs)?;
        }
    }
    for t in timelines {
        r.scope.rename(Kind::Timeline, &mut t.name, t.span)?;
    }
    for f in fns {
        r.scope.rename(Kind::Call, &mut f.name, f.span)?;
        r.params(&mut f.params)?;
        r.ty(&mut f.ret)?;
        let mark = r.bind(f.params.iter().map(|p| p.name.clone()));
        r.expr(&mut f.body)?;
        r.unbind(mark);
    }
    if let Some(routes) = routes {
        for row in &mut routes.rows {
            for field in &mut row.fields {
                // `pages=source(args)` names a data source, as a resource
                // does; only its arguments are Contract.
                match (&*field.name, &mut field.value) {
                    ("pages", Expr::Call(_, args, _)) => r.exprs(args)?,
                    _ => r.expr(&mut field.value)?,
                }
            }
        }
    }
    for test in tests {
        for step in &mut test.steps {
            if let Step::ExpectState { value, .. } = step {
                r.expr(value)?;
            }
        }
    }
    for c in components {
        r.component(c)?;
    }
    Ok(())
}

struct Rewriter<'a> {
    scope: &'a Scope,
    /// The bindings in scope where the rewrite stands — a component's
    /// members, an action's or `fn`'s parameters, `each`, `match`, arrow and
    /// `let` binders. A call of one is the binding's, never a top-level name.
    locals: Vec<String>,
    /// For each of `locals`, whether it is an action, prop or inject: the
    /// innermost binding of `t` being one stops `t(…)` being the strings
    /// intrinsic, as the type checker reads it.
    callable: Vec<bool>,
    /// Every binding seen, when collecting them.
    seen: Option<&'a mut Seen>,
}

/// The written types no declaration can be (`types` reads them first).
const PRIMITIVES: [&str; 4] = ["number", "string", "bool", "unit"];

type R = Result<(), SyntaxError>;

impl Rewriter<'_> {
    fn component(&mut self, c: &mut Component) -> R {
        let Component {
            name,
            props,
            injects,
            provides,
            slot: _,
            states,
            derives,
            resources,
            mutations,
            actions,
            tasks,
            view,
            span,
        } = c;
        self.scope.rename(Kind::Component, name, *span)?;
        // In the order the type checker brings them into scope: props and
        // injects; each state, seen by those after it; then the rest.
        let mark = self.bind_callable(props.iter().chain(injects.iter()).map(|p| p.name.clone()));
        self.params(props)?;
        self.params(injects)?;
        for b in states.iter_mut() {
            self.expr(&mut b.expr)?;
            self.bind([b.name.clone()]);
        }
        self.bind(
            derives
                .iter()
                .map(|b| b.name.clone())
                .chain(resources.iter().map(|r| r.name.clone()))
                .chain(mutations.iter().map(|m| m.name.clone())),
        );
        self.bind_callable(actions.iter().map(|a| a.name.clone()));
        for b in provides.iter_mut().chain(derives) {
            self.expr(&mut b.expr)?;
        }
        for res in resources {
            self.exprs(&mut res.args)?;
            self.ty(&mut res.shape)?;
            if let Some(p) = &mut res.placeholder {
                self.exprs(&mut p.args)?;
            }
        }
        for m in mutations {
            self.ty(&mut m.shape)?;
        }
        for a in actions {
            self.params(&mut a.params)?;
            let inner = self.bind(a.params.iter().map(|p| p.name.clone()));
            self.stmts(&mut a.body)?;
            self.unbind(inner);
        }
        for t in tasks {
            self.expr(&mut t.timer.0)?;
            for e in t.gate.iter_mut().chain(t.key.iter_mut()) {
                self.expr(e)?;
            }
        }
        self.nodes(view)?;
        self.unbind(mark);
        Ok(())
    }

    /// Bring `names` into scope; returns the mark to unbind back to.
    fn bind(&mut self, names: impl IntoIterator<Item = String>) -> usize {
        self.bind_as(names, false)
    }

    /// Bring actions, props or injects into scope.
    fn bind_callable(&mut self, names: impl IntoIterator<Item = String>) -> usize {
        self.bind_as(names, true)
    }

    fn unbind(&mut self, mark: usize) {
        self.locals.truncate(mark);
        self.callable.truncate(mark);
    }

    /// Whether the innermost binding of `t` is an action, prop or inject.
    fn callable_t(&self) -> bool {
        self.locals
            .iter()
            .rposition(|l| l == "t")
            .is_some_and(|i| self.callable[i])
    }

    fn bind_as(&mut self, names: impl IntoIterator<Item = String>, callable: bool) -> usize {
        let mark = self.locals.len();
        self.locals.extend(names);
        self.callable.resize(self.locals.len(), callable);
        if let Some(seen) = self.seen.as_deref_mut() {
            seen.bound.extend(self.locals[mark..].iter().cloned());
        }
        mark
    }

    fn local(&self, name: &str) -> bool {
        self.locals.iter().any(|l| l == name)
    }

    fn params(&mut self, params: &mut [Param]) -> R {
        for p in params {
            if let Some(ty) = &mut p.ty {
                self.ty(ty)?;
            }
        }
        Ok(())
    }

    fn ty(&mut self, ty: &mut TypeExpr) -> R {
        match ty {
            TypeExpr::Named(name, _) if PRIMITIVES.contains(&name.as_str()) => Ok(()),
            // A type is a shape, never a `fn`: a shape this file sees, else
            // one the compiler declares (`PointerEvent`) or bare `action`,
            // else a name another file declares, refused.
            TypeExpr::Named(name, span) => match self.scope.get(Kind::Call, name) {
                Some(to) if self.scope.shapes.contains(to) => {
                    self.scope.rename(Kind::Call, name, *span)
                }
                _ if self.scope.builtin_types.contains(name.as_str()) || name == "action" => Ok(()),
                // Nothing of the name here: another file's is refused with the
                // rest of the file's missing names (§21).
                None => self.scope.rename(Kind::Call, name, *span),
                // A `fn` of the name here, which is no type: another file's
                // shape of the name is not reachable past it by a `use`.
                Some(_) => {
                    if let Some(file) = self.scope.foreign_shapes.get(name.as_str()) {
                        self.scope.refused.borrow_mut().push(SyntaxError {
                            id: "contract-use-missing",
                            message: format!(
                                "`{name}` is a shape declared in `{file}`, and this file's `{name}` is a `fn`; rename one, then name the shape in a `use` (LLP 1091 D1)"
                            ),
                            span: *span,
                        });
                    }
                    Ok(())
                }
            },
            TypeExpr::Option(inner, _) | TypeExpr::List(inner, _) => self.ty(inner),
        }
    }

    fn stmts(&mut self, stmts: &mut [Stmt]) -> R {
        let mark = self.locals.len();
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, expr, .. } => {
                    self.expr(expr)?;
                    self.bind([name.clone()]);
                }
                Stmt::Assign { expr, .. } => self.expr(expr)?,
                Stmt::Command { args, .. } | Stmt::Send { args, .. } => self.exprs(args)?,
                Stmt::Refresh { .. } => {}
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond)?;
                    self.stmts(then)?;
                    self.stmts(otherwise)?;
                }
                Stmt::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject)?;
                    let inner = self.bind([some.0.clone()]);
                    self.stmts(&mut some.1)?;
                    self.unbind(inner);
                    self.stmts(none)?;
                }
                // The parser never makes a call (LLP 1089); expansion does,
                // after every file is in one scope.
                Stmt::Call { args, body, .. } => {
                    self.exprs(args)?;
                    self.stmts(body)?;
                }
            }
        }
        self.unbind(mark);
        Ok(())
    }

    fn nodes(&mut self, nodes: &mut [Node]) -> R {
        for node in nodes {
            match node {
                Node::Element {
                    tag,
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    self.exprs(positional)?;
                    self.element_attrs(tag == "canvas", attrs)?;
                    self.nodes(children)?;
                }
                Node::Use {
                    name,
                    args,
                    children,
                    span,
                } => {
                    self.scope.rename(Kind::Component, name, *span)?;
                    // A component's argument is a value: `class`, `animation`
                    // and the like are the child's props, not its style rows.
                    for arg in args.iter_mut() {
                        self.expr(&mut arg.value)?;
                    }
                    self.nodes(children)?;
                }
                Node::Children { .. } => {}
                Node::When {
                    cond,
                    then,
                    otherwise,
                    ..
                } => {
                    self.expr(cond)?;
                    self.nodes(then)?;
                    self.nodes(otherwise)?;
                }
                Node::Each {
                    var,
                    index,
                    list,
                    key,
                    body,
                    ..
                } => {
                    self.expr(list)?;
                    let mark = self.bind(std::iter::once(var.clone()).chain(index.clone()));
                    self.expr(key)?;
                    self.nodes(body)?;
                    self.unbind(mark);
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject)?;
                    let mark = self.bind([some.0.clone()]);
                    self.nodes(&mut some.1)?;
                    self.unbind(mark);
                    self.nodes(none)?;
                }
            }
        }
        Ok(())
    }

    fn attrs(&mut self, attrs: &mut [Attr]) -> R {
        self.element_attrs(false, attrs)
    }

    /// `canvas` owns `surface` (lower/src/lib.rs): only there is a
    /// `surface=name(args)` head the drawing module's; on any other element
    /// or component argument it is a call like any other.
    fn element_attrs(&mut self, canvas: bool, attrs: &mut [Attr]) -> R {
        for a in attrs {
            match a.name.as_str() {
                "class" => self.class(&mut a.value)?,
                "animation" | "exit-animation" | "exitAnimation" => {
                    self.animation(&mut a.value, true)?
                }
                "animation-name" | "animationName" => self.animation(&mut a.value, false)?,
                "animation-timeline" | "animationTimeline" => self.timeline(&mut a.value)?,
                // `surface=name(args)`: the name is the drawing module's, not
                // a function (types/checks.rs); only its arguments are Contract.
                "surface" if canvas => {
                    if let Expr::Call(_, args, _) = &mut a.value {
                        self.exprs(args)?;
                        continue;
                    }
                }
                _ => {}
            }
            self.expr(&mut a.value)?;
        }
        Ok(())
    }

    /// `class=Name` and `class=(c ? A : B)` name styles (LLP 1017 P6).
    fn class(&mut self, value: &mut Expr) -> R {
        match value {
            Expr::Ident(name, span) => self.scope.rename(Kind::Style, name, *span),
            Expr::Ternary(_, yes, no, _) => {
                self.class(yes)?;
                self.class(no)
            }
            _ => Ok(()),
        }
    }

    /// The keyframes an `animation` literal names, where it is written (D5).
    /// In a shorthand that is each comma-separated animation's first word
    /// that is not one of CSS's animation keywords or a number, as CSS reads
    /// it; in `animation-name`, each item. A name computed as the app runs is
    /// matched against the table as it is today: no file can be said to have
    /// written it.
    fn animation(&mut self, value: &mut Expr, shorthand: bool) -> R {
        match value {
            Expr::Str(text, span) => {
                let span = *span;
                let mut parts = [Part::Text(text)];
                self.keyframes_in(&mut parts, shorthand, span)
            }
            Expr::Template(template, span) => {
                let span = *span;
                let mut parts: Vec<Part<'_>> = template
                    .iter_mut()
                    .map(|part| match part {
                        TemplatePart::Text(text) => Part::Text(text),
                        TemplatePart::Expr(_) => Part::Computed,
                    })
                    .collect();
                self.keyframes_in(&mut parts, shorthand, span)
            }
            Expr::Ternary(_, yes, no, _) => {
                self.animation(yes, shorthand)?;
                self.animation(no, shorthand)
            }
            Expr::Match { some, none, .. } => {
                self.animation(some, shorthand)?;
                self.animation(none, shorthand)
            }
            _ => Ok(()),
        }
    }

    /// Resolve and rename the keyframes names in a literal's text parts,
    /// read as `motion`'s grammar reads the shorthand (its `Animations::
    /// grammar`, mirrored here, which `contract` cannot depend on): split at
    /// top-level commas and spaces, each animation's parts taking, in turn,
    /// a time, an easing, a count, a direction, a fill mode, a play state,
    /// and then the name — so a keyword whose slot is full is the name. A
    /// computed part's role is unknown until it runs; it fills no slot.
    fn keyframes_in(&self, parts: &mut [Part<'_>], shorthand: bool, span: Span) -> R {
        const HOLE: char = '\u{1}';
        let combined: String = parts
            .iter()
            .map(|part| match part {
                Part::Text(text) => text.as_str(),
                Part::Computed => "\u{1}",
            })
            .collect();
        let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for (at, decl) in split_top(&combined, ',', 0) {
            // As `motion` reads it: each animation trimmed, then split at
            // spaces alone.
            let lead = decl.len() - decl.trim_start().len();
            let tokens: Vec<(usize, &str)> = split_top(decl.trim(), ' ', at + lead)
                .into_iter()
                .filter(|(_, t)| !t.is_empty())
                .collect();
            // A computed part's slot is known when its text says it
            // (`steps(${n}, …)`, `${d}ms`); otherwise it may be any. Every
            // reading `motion` would accept is tried: a literal is renamed
            // when it is the name in every one, left when it is the name in
            // none, and where it is the name in some but not others and also
            // renamed keyframes, no rename is right, so the compile is
            // refused (LLP 1091 §20).
            let names: Vec<(usize, &str)> = if shorthand {
                let roles = literal_roles(&tokens, HOLE);
                // A computed comma may make more than one animation here, each
                // with its name: every literal that is always a name.
                let mut names = Vec::new();
                for (i, &(at, t)) in tokens.iter().enumerate() {
                    if t.contains(HOLE) {
                        continue;
                    }
                    let (can_name, can_other) = roles[i];
                    let all = can_name && !can_other;
                    let some = can_name && can_other;
                    if all {
                        names.push((at, t));
                    } else if some {
                        let bare = t.trim_matches(|c| c == '"' || c == '\'');
                        if self
                            .scope
                            .get(Kind::Keyframes, bare)
                            .is_some_and(|to| to != bare)
                        {
                            return Err(SyntaxError {
                                id: "contract-animation-ambiguous",
                                message: format!(
                                    "`{t}` beside a computed value is the animation's name or a keyword depending on the value when it runs, and `{t}` here is keyframes another file's `{t}` renamed; quote the name (`'{bare}'`) or name the keyframes otherwise (LLP 1091 D5)"
                                ),
                                span,
                            });
                        }
                    }
                }
                names
            } else {
                tokens
                    .into_iter()
                    .next()
                    .filter(|(_, t)| !t.contains(HOLE))
                    .into_iter()
                    .collect()
            };
            for (start, token) in names {
                let quoted = token.len() >= 2
                    && ((token.starts_with('"') && token.ends_with('"'))
                        || (token.starts_with('\'') && token.ends_with('\'')));
                let (name, start) = if quoted {
                    (&token[1..token.len() - 1], start + 1)
                } else {
                    (token, start)
                };
                if name == "none" {
                    continue;
                }
                if let Some(to) = self.scope.resolve(Kind::Keyframes, name, span)? {
                    if to != name {
                        edits.push((start..start + name.len(), to.to_owned()));
                    }
                }
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        let mut out = combined;
        for (range, to) in edits.into_iter().rev() {
            out.replace_range(range, &to);
        }
        let mut segments = out.split(HOLE);
        let mut segment = segments.next();
        for part in parts.iter_mut() {
            match part {
                Part::Text(text) => {
                    if let Some(to) = segment.take() {
                        if text.as_str() != to {
                            **text = to.to_owned();
                        }
                    }
                }
                Part::Computed => segment = segments.next(),
            }
        }
        Ok(())
    }

    /// A `clock(Name)` literal names a timeline where it is written (D5); a
    /// bare name is the clock pass's.
    fn timeline(&mut self, value: &mut Expr) -> R {
        match value {
            Expr::Str(text, span) => {
                let Some(name) = text
                    .trim()
                    .strip_prefix("clock(")
                    .and_then(|rest| rest.strip_suffix(')'))
                    .map(str::trim)
                else {
                    return Ok(());
                };
                if let Some(to) = self.scope.resolve(Kind::Timeline, name, *span)? {
                    if to != name {
                        *text = format!("clock({to})");
                    }
                }
                Ok(())
            }
            Expr::Ternary(_, yes, no, _) => {
                self.timeline(yes)?;
                self.timeline(no)
            }
            Expr::Match { some, none, .. } => {
                self.timeline(some)?;
                self.timeline(none)
            }
            // A template with no interpolation is a literal too.
            Expr::Template(parts, span) => {
                if let [TemplatePart::Text(text)] = parts.as_slice() {
                    let mut literal = Expr::Str(text.clone(), *span);
                    self.timeline(&mut literal)?;
                    if let Expr::Str(to, _) = literal {
                        parts[0] = TemplatePart::Text(to);
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn exprs(&mut self, exprs: &mut [Expr]) -> R {
        for e in exprs {
            self.expr(e)?;
        }
        Ok(())
    }

    fn expr(&mut self, e: &mut Expr) -> R {
        match e {
            Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {
                Ok(())
            }
            Expr::List(items, _) => self.exprs(items),
            Expr::Template(parts, _) => {
                for part in parts {
                    if let TemplatePart::Expr(e) = part {
                        self.expr(e)?;
                    }
                }
                Ok(())
            }
            Expr::Some(inner, _)
            | Expr::Member(inner, _, _)
            | Expr::NamedArg(_, inner, _)
            | Expr::Unary(_, inner, _) => self.expr(inner),
            Expr::Call(name, args, span) => {
                if let Some(seen) = self.seen.as_deref_mut() {
                    seen.called.insert(name.clone());
                }
                // As the type checker reads a call: this file's `fn` or
                // shape first, then a binding, then the roster.
                let declared = self.scope.names.contains_key(&(Kind::Call, name.clone()));
                let roster = self.scope.roster.is_some_and(|f| f(name));
                // `pending` and `failed` are read before any `fn`, and `t`
                // before any but an action or prop of its name.
                let routing = name == "path"
                    && self
                        .scope
                        .get(Kind::Call, name)
                        .is_none_or(|to| self.scope.shapes.contains(to));
                let intrinsic = routing
                    || matches!(name.as_str(), "pending" | "failed")
                    || (name == "t" && !self.callable_t());
                if !intrinsic && (declared || !(self.local(name) || roster)) {
                    self.scope.rename(Kind::Call, name, *span)?;
                }
                self.exprs(args)
            }
            Expr::Binary(_, a, b, _) => {
                self.expr(a)?;
                self.expr(b)
            }
            Expr::Ternary(c, a, b, _) => {
                self.expr(c)?;
                self.expr(a)?;
                self.expr(b)
            }
            Expr::Match {
                subject,
                var,
                some,
                none,
                ..
            } => {
                self.expr(subject)?;
                let mark = self.bind([var.clone()]);
                self.expr(some)?;
                self.unbind(mark);
                self.expr(none)
            }
            Expr::Arrow { params, body, .. } => {
                let mark = self.bind(params.iter().cloned());
                self.expr(body)?;
                self.unbind(mark);
                Ok(())
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.expr(value)?;
                let mark = self.bind([name.clone()]);
                self.expr(body)?;
                self.unbind(mark);
                Ok(())
            }
            Expr::Typed(inner, ty, _) => {
                self.expr(inner)?;
                self.ty(ty)
            }
        }
    }
}

/// One part of an `animation` literal: text to read, or a computed value.
enum Part<'a> {
    Text(&'a mut String),
    Computed,
}

/// One animation's slots, filled in the order `motion` fills them.
#[derive(Default, Clone, PartialEq, Eq, Hash)]
struct Shorthand {
    named: bool,
    times: u32,
    eased: bool,
    counted: bool,
    directed: bool,
    filled: bool,
    stated: bool,
}

impl Shorthand {
    /// Fill a computed part's slot, as `motion` would fill it with a
    /// value of that kind; `None` where that value would be refused.
    fn fill(&mut self, role: Role) -> Option<bool> {
        let free = |slot: &mut bool| (!std::mem::replace(slot, true)).then_some(false);
        match role {
            Role::Time => {
                self.times = self.times.saturating_add(1);
                (self.times <= 2).then_some(false)
            }
            Role::Easing => free(&mut self.eased),
            Role::Count => free(&mut self.counted),
            Role::Direction => free(&mut self.directed),
            Role::Fill => free(&mut self.filled),
            Role::Play => free(&mut self.stated),
            Role::Name => free(&mut self.named).map(|_| true),
        }
    }

    /// Fill `part` in as `motion` does: `Some(true)` for the name,
    /// `Some(false)` for a slot, `None` where `motion` refuses the shorthand
    /// (a third time, a second name, a word that is nothing).
    fn take(&mut self, part: &str) -> Option<bool> {
        // `motion` parses numbers as Rust does: `inf` and `nan` are numbers.
        let number = |n: &str| n.parse::<f64>().ok();
        let time = part
            .strip_suffix("ms")
            .or_else(|| part.strip_suffix('s'))
            .and_then(number)
            .is_some();
        if time {
            self.times = self.times.saturating_add(1);
            return (self.times <= 2).then_some(false);
        }
        let easing = matches!(
            part,
            "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "step-start" | "step-end"
        ) || ["cubic-bezier(", "steps(", "linear(", "spring("]
            .iter()
            .any(|f| part.starts_with(f));
        let free = |slot: &mut bool| !std::mem::replace(slot, true);
        if easing && free(&mut self.eased) {
            return Some(false);
        }
        if (part == "infinite" || number(part).is_some_and(|n| n >= 0.0)) && free(&mut self.counted)
        {
            return Some(false);
        }
        if matches!(
            part,
            "normal" | "reverse" | "alternate" | "alternate-reverse"
        ) && free(&mut self.directed)
        {
            return Some(false);
        }
        if matches!(part, "none" | "forwards" | "backwards" | "both") && free(&mut self.filled) {
            return Some(false);
        }
        if matches!(part, "running" | "paused") && free(&mut self.stated) {
            return Some(false);
        }
        if is_name(part) && free(&mut self.named) {
            return Some(true);
        }
        None
    }
}

/// What a computed part of a shorthand may be.
#[derive(Clone, Copy)]
enum Role {
    Time,
    Easing,
    Count,
    Direction,
    Fill,
    Play,
    Name,
}

const ROLES: [Role; 7] = [
    Role::Time,
    Role::Easing,
    Role::Count,
    Role::Direction,
    Role::Fill,
    Role::Play,
    Role::Name,
];

/// For each literal part of one animation, whether some reading `motion`
/// would accept takes it as the name, and whether some takes it as anything
/// else. A computed part (holding `hole`) whose text says its kind is that
/// kind; any other may be any kind of value, or empty (no part at all). Read
/// exactly, at any length, by the slots each prefix can leave filled: those
/// reachable from the start, and those from which the rest still reads.
fn literal_roles(tokens: &[(usize, &str)], hole: char) -> Vec<(bool, bool)> {
    use std::collections::HashSet;
    // What a computed part's own text settles: a quoted one is a name, a
    // function's (`steps(${n}, …)`) an easing. Any other may be any kind of
    // value — its literal text says nothing for certain (`${x}ms` may read
    // `items`) — and one that is the value alone may also be empty.
    let known = |t: &str| {
        let quoted = t.len() >= 2
            && ((t.starts_with('"') && t.ends_with('"'))
                || (t.starts_with('\'') && t.ends_with('\'')));
        if quoted {
            Some(Role::Name)
        } else if ["cubic-bezier(", "steps(", "linear(", "spring("]
            .iter()
            .any(|f| t.starts_with(f))
        {
            Some(Role::Easing)
        } else {
            None
        }
    };
    // The slots after `token` from `state`, each with whether it was the
    // name: one for a literal, one per kind for an unknown computed part.
    let step = |state: &Shorthand, token: &str| -> Vec<(Shorthand, bool)> {
        let mut out = Vec::new();
        if !token.contains(hole) {
            let mut next = state.clone();
            if let Some(named) = next.take(token) {
                out.push((next, named));
            }
            return out;
        }
        match known(token) {
            Some(role) => {
                let mut next = state.clone();
                if let Some(named) = next.fill(role) {
                    out.push((next, named));
                }
            }
            None => {
                // Empty: the value adds no part (only a value alone can be).
                if token.chars().all(|c| c == hole) {
                    out.push((state.clone(), false));
                }
                // A comma in the value ends this animation and begins the
                // next (`spin 1s ${sep} pulse 1s`); what it adds on either
                // side is read as more of this kind of guess, so here it is
                // a fresh animation's slots.
                out.push((Shorthand::default(), false));
                for role in ROLES {
                    let mut next = state.clone();
                    if let Some(named) = next.fill(role) {
                        out.push((next, named));
                    }
                }
            }
        }
        out
    };
    let n = tokens.len();
    let mut forward: Vec<HashSet<Shorthand>> = vec![HashSet::new(); n + 1];
    forward[0].insert(Shorthand::default());
    for i in 0..n {
        let next: HashSet<Shorthand> = forward[i]
            .iter()
            .flat_map(|s| step(s, tokens[i].1).into_iter().map(|(t, _)| t))
            .collect();
        forward[i + 1] = next;
    }
    // Any slots at the end read: an animation with no name is dropped by
    // `motion`, not refused.
    let mut alive: Vec<HashSet<Shorthand>> = vec![HashSet::new(); n + 1];
    alive[n] = forward[n].clone();
    for i in (0..n).rev() {
        let keep: HashSet<Shorthand> = forward[i]
            .iter()
            .filter(|s| {
                step(s, tokens[i].1)
                    .iter()
                    .any(|(t, _)| alive[i + 1].contains(t))
            })
            .cloned()
            .collect();
        alive[i] = keep;
    }
    (0..n)
        .map(|i| {
            let mut roles = (false, false);
            for s in &alive[i] {
                for (t, named) in step(s, tokens[i].1) {
                    if alive[i + 1].contains(&t) {
                        if named {
                            roles.0 = true;
                        } else {
                            roles.1 = true;
                        }
                    }
                }
            }
            roles
        })
        .collect()
}

/// `motion`'s `is_name`: a CSS custom-ident or a quoted string, not a
/// CSS-wide keyword.
fn is_name(part: &str) -> bool {
    if part.len() >= 2
        && ((part.starts_with('"') && part.ends_with('"'))
            || (part.starts_with('\'') && part.ends_with('\'')))
    {
        return true;
    }
    let Some(first) = part.chars().next() else {
        return false;
    };
    let starts = first.is_ascii_alphabetic()
        || first == '_'
        || (first == '-' && part.len() > 1 && !part[1..].starts_with(|c: char| c.is_ascii_digit()));
    starts
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !matches!(
            part,
            "initial" | "inherit" | "unset" | "revert" | "revert-layer" | "default"
        )
}

/// `s` split at `sep` outside parentheses, each piece with its byte offset
/// (plus `base`), as `motion`'s `split_top_level` splits.
fn split_top(s: &str, sep: char, base: usize) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push((base + start, &s[start..i]));
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push((base + start, &s[start..]));
    out
}
