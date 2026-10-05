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
    /// Whether a call names a roster function, which no file declares and
    /// every file sees: never another file's name, so never refused.
    pub roster: Option<fn(&str) -> bool>,
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
    let mut r = Rewriter {
        scope,
        locals: Vec::new(),
    };
    let File {
        names: _,
        routes,
        uses: _,
        fonts: _,
        sounds: _,
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
        let mark = r.bind(f.params.iter().map(|p| p.name.clone()));
        r.expr(&mut f.body)?;
        r.locals.truncate(mark);
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
    /// The bindings in scope where the rewrite stands — a component's
    /// members, an action's or `fn`'s parameters, `each`, `match`, arrow and
    /// `let` binders. A call of one is the binding's, never a top-level name.
    locals: Vec<String>,
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
        let members: Vec<String> = (props.iter().chain(injects.iter()).map(|p| p.name.clone()))
            .chain(
                provides
                    .iter()
                    .chain(&*states)
                    .chain(&*derives)
                    .map(|b| b.name.clone()),
            )
            .chain(resources.iter().map(|r| r.name.clone()))
            .chain(mutations.iter().map(|m| m.name.clone()))
            .chain(actions.iter().map(|a| a.name.clone()))
            .chain(tasks.iter().map(|t| t.name.clone()))
            .collect();
        let mark = self.bind(members);
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
            let inner = self.bind(a.params.iter().map(|p| p.name.clone()));
            self.stmts(&mut a.body)?;
            self.locals.truncate(inner);
        }
        for t in tasks {
            self.expr(&mut t.timer.0)?;
        }
        self.nodes(view)?;
        self.locals.truncate(mark);
        Ok(())
    }

    /// Bring `names` into scope; returns the mark to truncate back to.
    fn bind(&mut self, names: impl IntoIterator<Item = String>) -> usize {
        let mark = self.locals.len();
        self.locals.extend(names);
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
            TypeExpr::Named(name, span) => self.scope.rename(Kind::Call, name, *span),
            TypeExpr::Option(inner, _) | TypeExpr::List(inner, _) => self.ty(inner),
        }
    }

    fn stmts(&mut self, stmts: &mut [Stmt]) -> R {
        let mark = self.locals.len();
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, expr, .. } => {
                    self.expr(expr)?;
                    self.locals.push(name.clone());
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
                    self.locals.truncate(inner);
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
        self.locals.truncate(mark);
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
                    self.locals.truncate(mark);
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
                    self.locals.truncate(mark);
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
                "animation" | "exit-animation" | "exitAnimation" => {
                    self.animation(&mut a.value, true)?
                }
                "animation-name" | "animationName" => self.animation(&mut a.value, false)?,
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

    /// Resolve and rename the keyframes names in a literal's text parts. A
    /// computed part is a word no file wrote: when it stands where the name
    /// would, the animation's name is not literal and is left alone.
    fn keyframes_in(&self, parts: &mut [Part<'_>], shorthand: bool, span: Span) -> R {
        // Whether the current animation's name has been seen (or is computed),
        // and whether the next text begins glued to a computed part.
        let (mut named, mut glued) = (false, false);
        for part in parts.iter_mut() {
            let Part::Text(text) = part else {
                glued = true;
                continue;
            };
            // A computed value glued to a unit (`${d}ms`) is a time; one that
            // stands alone may be the name, which is then not literal.
            if glued
                && !text.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                named = true;
            }
            let mut out = String::with_capacity(text.len());
            let mut depth = 0usize;
            let mut changed = false;
            let runs: Vec<(&str, bool)> = words(text).collect();
            for (i, &(run, is_word)) in runs.iter().enumerate() {
                let first = i == 0 && glued;
                if !is_word {
                    for c in run.chars() {
                        match c {
                            '(' => depth += 1,
                            ')' => depth = depth.saturating_sub(1),
                            ',' if depth == 0 => named = false,
                            _ => {}
                        }
                    }
                    out.push_str(run);
                    continue;
                }
                let function = runs
                    .get(i + 1)
                    .is_some_and(|(next, word)| !word && next.starts_with('('));
                let candidate = !named
                    && !first
                    && depth == 0
                    && !function
                    && !(shorthand && is_animation_keyword(run))
                    && !run.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                    && !(run.starts_with('-')
                        && run[1..].starts_with(|c: char| c.is_ascii_digit() || c == '.'))
                    && run != "none";
                if candidate {
                    named = true;
                    if let Some(to) = self.scope.resolve(Kind::Keyframes, run, span)? {
                        if to != run {
                            out.push_str(to);
                            changed = true;
                            continue;
                        }
                    }
                } else if !shorthand && depth == 0 && !first {
                    named = true;
                }
                out.push_str(run);
            }
            if changed {
                **text = out;
            }
            glued = false;
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
            Expr::Number(..)
            | Expr::Str(..)
            | Expr::Bool(..)
            | Expr::None(_)
            | Expr::EmptyList(_)
            | Expr::Ident(..) => Ok(()),
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
                let roster = self.scope.roster.is_some_and(|f| f(name))
                    && !self.scope.names.contains_key(&(Kind::Call, name.clone()));
                if !self.local(name) && !roster {
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
                self.locals.truncate(mark);
                self.expr(none)
            }
            Expr::Arrow { params, body, .. } => {
                let mark = self.bind(params.iter().cloned());
                self.expr(body)?;
                self.locals.truncate(mark);
                Ok(())
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.expr(value)?;
                let mark = self.bind([name.clone()]);
                self.expr(body)?;
                self.locals.truncate(mark);
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

/// The words CSS's `animation` shorthand reads as something other than a
/// name: timing functions, iteration count, direction, fill mode, play
/// state, and the CSS-wide keywords.
fn is_animation_keyword(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "linear"
            | "ease"
            | "ease-in"
            | "ease-out"
            | "ease-in-out"
            | "step-start"
            | "step-end"
            | "infinite"
            | "normal"
            | "reverse"
            | "alternate"
            | "alternate-reverse"
            | "none"
            | "forwards"
            | "backwards"
            | "both"
            | "running"
            | "paused"
            | "initial"
            | "inherit"
            | "unset"
            | "revert"
            | "revert-layer"
    )
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
