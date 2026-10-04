//! Module scope (LLP 1091 D1/D4/D5): each file's names are its own
//! declarations and what its `use` lines name. The loader gives every
//! declaration a program-unique name and rewrites each file through its own
//! scope before the files are merged, so every later pass sees one flat
//! namespace in which a name means one declaration.

use crate::ast::*;
use crate::parser::SyntaxError;
use crate::Span;
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

/// One file's view of the program's names.
#[derive(Debug, Default)]
pub struct Scope {
    /// Each name this file may write, by namespace, to the program-unique
    /// name of the declaration it means: its own declarations and its uses.
    pub names: HashMap<(Kind, String), String>,
    /// Names other loaded files declare that this file does not see, to the
    /// file that declares them, so a reference to one is refused with the
    /// `use` that would bring it rather than resolved behind the author's back.
    pub elsewhere: HashMap<(Kind, String), String>,
}

impl Scope {
    fn get(&self, kind: Kind, name: &str) -> Option<&str> {
        self.names.get(&(kind, name.to_owned())).map(String::as_str)
    }

    /// The program-unique name `name` means here; `Ok(None)` for a name no
    /// loaded file declares (a roster function, a primitive, a binding), left
    /// for later passes to resolve or refuse.
    fn resolve(&self, kind: Kind, name: &str, span: Span) -> Result<Option<&str>, SyntaxError> {
        if let Some(to) = self.get(kind, name) {
            return Ok(Some(to));
        }
        match self.elsewhere.get(&(kind, name.to_owned())) {
            Some(file) => Err(SyntaxError {
                id: "contract-use-missing",
                message: format!(
                    "`{name}` is a {} declared in `{file}`, which this file does not name: add `use {name} from \"…\"` (LLP 1091 D1)",
                    kind.what()
                ),
                span,
            }),
            None => Ok(None),
        }
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
    let mut r = Rewriter { scope };
    let File {
        names: _,
        routes,
        uses: _,
        fonts: _,
        shapes,
        styles,
        keyframes,
        timelines,
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
        r.expr(&mut f.body)?;
    }
    if let Some(routes) = routes {
        for row in &mut routes.rows {
            r.attrs(&mut row.fields)?;
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
}

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
        self.params(props)?;
        self.params(injects)?;
        for b in provides.iter_mut().chain(states).chain(derives) {
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
            self.stmts(&mut a.body)?;
        }
        for t in tasks {
            self.expr(&mut t.timer.0)?;
        }
        self.nodes(view)
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
            TypeExpr::Named(name, span) => self.scope.rename(Kind::Call, name, *span),
            TypeExpr::Option(inner, _) | TypeExpr::List(inner, _) => self.ty(inner),
        }
    }

    fn stmts(&mut self, stmts: &mut [Stmt]) -> R {
        for stmt in stmts {
            match stmt {
                Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => self.expr(expr)?,
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
                    self.stmts(&mut some.1)?;
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
        Ok(())
    }

    fn nodes(&mut self, nodes: &mut [Node]) -> R {
        for node in nodes {
            match node {
                Node::Element {
                    positional,
                    attrs,
                    children,
                    ..
                } => {
                    self.exprs(positional)?;
                    self.attrs(attrs)?;
                    self.nodes(children)?;
                }
                Node::Use {
                    name,
                    args,
                    children,
                    span,
                } => {
                    self.scope.rename(Kind::Component, name, *span)?;
                    self.attrs(args)?;
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
                    list, key, body, ..
                } => {
                    self.expr(list)?;
                    self.expr(key)?;
                    self.nodes(body)?;
                }
                Node::Match {
                    subject,
                    some,
                    none,
                    ..
                } => {
                    self.expr(subject)?;
                    self.nodes(&mut some.1)?;
                    self.nodes(none)?;
                }
            }
        }
        Ok(())
    }

    fn attrs(&mut self, attrs: &mut [Attr]) -> R {
        for a in attrs {
            match a.name.as_str() {
                "class" => self.class(&mut a.value)?,
                "animation" | "animation-name" | "animationName" | "exit-animation"
                | "exitAnimation" => self.animation(&mut a.value)?,
                "animation-timeline" | "animationTimeline" => self.timeline(&mut a.value)?,
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
    /// A name computed as the app runs is matched against the table as it is
    /// today: no file can be said to have written it.
    fn animation(&mut self, value: &mut Expr) -> R {
        match value {
            Expr::Str(text, span) => {
                if let Some(to) = self.keyframes_text(text, *span)? {
                    *text = to;
                }
                Ok(())
            }
            Expr::Template(parts, span) => {
                for part in parts {
                    if let TemplatePart::Text(text) = part {
                        if let Some(to) = self.keyframes_text(text, *span)? {
                            *text = to;
                        }
                    }
                }
                Ok(())
            }
            Expr::Ternary(_, yes, no, _) => {
                self.animation(yes)?;
                self.animation(no)
            }
            Expr::Match { some, none, .. } => {
                self.animation(some)?;
                self.animation(none)
            }
            _ => Ok(()),
        }
    }

    fn keyframes_text(&self, text: &str, span: Span) -> Result<Option<String>, SyntaxError> {
        let mut out = String::with_capacity(text.len());
        let mut changed = false;
        for (word, is_word) in words(text) {
            match is_word
                .then(|| self.scope.resolve(Kind::Keyframes, word, span))
                .transpose()?
                .flatten()
            {
                Some(to) if to != word => {
                    out.push_str(to);
                    changed = true;
                }
                _ => out.push_str(word),
            }
        }
        Ok(changed.then_some(out))
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
                self.scope.rename(Kind::Call, name, *span)?;
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
                some,
                none,
                ..
            } => {
                self.expr(subject)?;
                self.expr(some)?;
                self.expr(none)
            }
            Expr::Arrow { body, .. } => self.expr(body),
            Expr::Let { value, body, .. } => {
                self.expr(value)?;
                self.expr(body)
            }
            Expr::Typed(inner, ty, _) => {
                self.expr(inner)?;
                self.ty(ty)
            }
        }
    }
}

/// `text` split into runs of name characters (a keyframes name's: letters,
/// digits, `_`, `-`) and the runs between them, in order.
fn words(text: &str) -> impl Iterator<Item = (&str, bool)> {
    let name = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let mut rest = text;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let is_word = name(first);
        let end = rest
            .find(|c: char| name(c) != is_word)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some((run, is_word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split_names_from_what_is_between_them() {
        let runs: Vec<_> = words("pulse 1s, fade-in 2s").collect();
        assert_eq!(
            runs,
            [
                ("pulse", true),
                (" ", false),
                ("1s", true),
                (", ", false),
                ("fade-in", true),
                (" ", false),
                ("2s", true)
            ]
        );
    }
}
