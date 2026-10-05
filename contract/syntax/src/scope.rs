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
            let tokens: Vec<(usize, &str)> = split_top(decl, ' ', at)
                .into_iter()
                .map(|(i, t)| (i, t.trim_matches(|c: char| c.is_whitespace())))
                .filter(|(_, t)| !t.is_empty())
                .collect();
            let name = if shorthand {
                let mut slots = Shorthand::default();
                tokens
                    .into_iter()
                    .find(|(_, t)| !t.contains(HOLE) && slots.name(t))
            } else {
                tokens.into_iter().next().filter(|(_, t)| !t.contains(HOLE))
            };
            let Some((start, token)) = name else {
                continue;
            };
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
                if let Some(seen) = self.seen.as_deref_mut() {
                    seen.called.insert(name.clone());
                }
                // As the type checker reads a call: this file's `fn` or
                // shape first, then a binding, then the roster.
                let declared = self.scope.names.contains_key(&(Kind::Call, name.clone()));
                let roster = self.scope.roster.is_some_and(|f| f(name));
                // `pending` and `failed` are read before any `fn`, and `t`
                // before any but an action or prop of its name.
                let intrinsic = matches!(name.as_str(), "pending" | "failed")
                    || (name == "t" && !self.local(name));
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

/// One animation's slots, filled in the order `motion` fills them.
#[derive(Default)]
struct Shorthand {
    times: u8,
    eased: bool,
    counted: bool,
    directed: bool,
    filled: bool,
    stated: bool,
}

impl Shorthand {
    /// Whether `part` is the animation's name: it fills no free slot before
    /// the name's, as `motion`'s grammar takes it.
    fn name(&mut self, part: &str) -> bool {
        let number = |n: &str| {
            n.starts_with(|c: char| c.is_ascii_digit() || matches!(c, '.' | '-' | '+'))
                && n.parse::<f64>().is_ok_and(f64::is_finite)
        };
        let time = part
            .strip_suffix("ms")
            .or_else(|| part.strip_suffix('s'))
            .is_some_and(number);
        if time && self.times < 2 {
            self.times += 1;
            return false;
        }
        let easing = matches!(
            part,
            "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out" | "step-start" | "step-end"
        ) || ["cubic-bezier(", "steps(", "linear(", "spring("]
            .iter()
            .any(|f| part.starts_with(f));
        let slot = if easing {
            &mut self.eased
        } else if part == "infinite" || (number(part) && !part.starts_with('-')) {
            &mut self.counted
        } else if matches!(
            part,
            "normal" | "reverse" | "alternate" | "alternate-reverse"
        ) {
            &mut self.directed
        } else if matches!(part, "none" | "forwards" | "backwards" | "both") {
            &mut self.filled
        } else if matches!(part, "running" | "paused") {
            &mut self.stated
        } else {
            return is_name(part);
        };
        if *slot {
            return is_name(part);
        }
        *slot = true;
        false
    }
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
