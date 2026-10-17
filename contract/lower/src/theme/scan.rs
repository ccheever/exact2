//! What an app's view says about its design's tokens (LLP 1121 D3, D7):
//! the names its literals write, the expressions over tokens they write
//! (each a synthetic token), and whether a colour row is computed, when a
//! string from data may name any token.

use super::role_named;
use contract_syntax::{Attr, Component, Expr, File, Node, Span, Stmt, TemplatePart};
use exact_kernel::{StyleCodec, StyleId};
use exact_tamber::Design;
use std::collections::HashSet;

/// CSS's colour functions: an expression over a token is one of them.
const COLOUR_FNS: &[&str] = &[
    "light-dark",
    "color-mix",
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "contrast-color",
    "-exact-platform-color",
];

/// A row whose value is or holds a colour.
pub(crate) fn colour_codec(row: StyleId) -> bool {
    matches!(
        row.codec(),
        StyleCodec::ColorValue
            | StyleCodec::KeywordColor
            | StyleCodec::Rgba8
            | StyleCodec::Color2
            | StyleCodec::SymbolPalette
            | StyleCodec::Paint
            | StyleCodec::BackgroundImage
            | StyleCodec::MaskImage
            | StyleCodec::BoxShadow
            | StyleCodec::TextShadow
            | StyleCodec::Filter
    )
}

/// An attribute that may hold a colour: a colour row, or a shorthand
/// (`border`, `outline`, `background`) that sets one.
fn colour_attr(name: &str) -> bool {
    match crate::tags::attr(name) {
        Some(crate::tags::AttrTarget::Styles(rows)) => rows.iter().any(|r| colour_codec(*r)),
        Some(crate::tags::AttrTarget::Shorthand) => true,
        _ => false,
    }
}

/// The words of a value, as a name is written in one.
pub(crate) fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
        .filter(|w| !w.is_empty())
}

/// An expression over tokens a literal writes.
pub(crate) struct Synthetic {
    /// Its whole text, which names it in the plan.
    pub expr: String,
    /// Where it is first written.
    pub span: Span,
    /// Written in a colour row's value, so it must fold; one only a `fn`, a
    /// state or another attribute writes is dropped when it does not.
    pub strict: bool,
}

#[derive(Default)]
pub(crate) struct Said {
    /// Each expression over a token a literal writes, first where written.
    pub synthetic: Vec<Synthetic>,
    /// A colour row's value is computed somewhere: any token may be named.
    pub computed: bool,
    /// A length or number row's value is computed and may be a string
    /// somewhere: any length or number token may be named (LLP 1121 D7).
    pub computed_measure: bool,
    /// Every word the literals write that is a token's name.
    words: HashSet<String>,
}

impl Said {
    /// What `file`'s styles, `root`'s view and the `fn`s, states, derives and
    /// actions they read say of `design`'s tokens.
    pub(crate) fn of(file: &File, root: &Component, design: &Design) -> Said {
        let mut literals: Vec<(String, Span, bool)> = Vec::new();
        let mut computed = false;
        let mut computed_measure = false;
        let mut attrs = |attrs: &[Attr]| {
            for a in attrs {
                let colour = colour_attr(&a.name);
                computed |= colour && !literal(&a.value);
                computed_measure |=
                    !colour && measure_attr(&a.name) && !literal(&a.value) && !number(&a.value);
                // A colour row's own value must fold; a literal elsewhere in
                // it (a condition's, a call's argument) may not be one.
                if colour {
                    values(&a.value, &mut literals);
                }
                exprs(&a.value, &mut |e| strings(e, false, &mut literals));
            }
        };
        for s in &file.styles {
            attrs(&s.attrs);
        }
        let mut stack: Vec<&Node> = root.view.iter().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Element {
                    attrs: a, children, ..
                } => {
                    attrs(a);
                    stack.extend(children);
                }
                Node::When {
                    then, otherwise, ..
                } => stack.extend(then.iter().chain(otherwise)),
                Node::Each { body, .. } => stack.extend(body),
                Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none)),
                Node::Use { children, .. } => stack.extend(children),
                Node::Children { .. } => {}
            }
        }
        // A computed row may read a `fn`, a state, a derive or what an
        // action assigns: their literals too.
        let mut loose = |e: &Expr| exprs(e, &mut |e| strings(e, false, &mut literals));
        let bindings = root.derives.iter().chain(&root.states);
        for e in file
            .fns
            .iter()
            .map(|f| &f.body)
            .chain(bindings.map(|b| &b.expr))
        {
            loose(e);
        }
        for a in &root.actions {
            stmts(&a.body, &mut loose);
        }
        let declared: Vec<&str> = design
            .tokens
            .iter()
            .map(|t| t.name.value.as_str())
            .collect();
        let roles: Vec<u8> = declared
            .iter()
            .filter_map(|n| role_named(n).map(|(id, _)| id))
            .collect();
        let is_token = |w: &str| {
            declared.contains(&w) || role_named(w).is_some_and(|(id, _)| roles.contains(&id))
        };
        let mut said = Said {
            computed,
            computed_measure,
            ..Default::default()
        };
        for (text, span, strict) in literals {
            said.words
                .extend(words(&text).filter(|w| is_token(w)).map(str::to_string));
            for expr in expressions(&text, &is_token) {
                match said.synthetic.iter_mut().find(|s| s.expr == expr) {
                    Some(s) if strict && !s.strict => {
                        s.strict = true;
                        s.span = span;
                    }
                    Some(_) => {}
                    None => said.synthetic.push(Synthetic {
                        expr: expr.to_string(),
                        span,
                        strict,
                    }),
                }
            }
        }
        said
    }

    /// Whether the view names `name`: a token a literal writes, or a
    /// synthetic token's expression.
    pub(crate) fn names(&self, name: &str) -> bool {
        self.words.contains(name) || self.synthetic.iter().any(|s| s.expr == name)
    }
}

/// An attribute that sets a row a length or number token may name.
fn measure_attr(name: &str) -> bool {
    match crate::tags::attr(name) {
        Some(crate::tags::AttrTarget::Styles(rows)) => rows.iter().any(|r| {
            !colour_codec(*r)
                && matches!(
                    r.codec(),
                    StyleCodec::Dimension
                        | StyleCodec::LineHeight
                        | StyleCodec::F32
                        | StyleCodec::U16
                )
        }),
        _ => false,
    }
}

/// Whether a value is a number however it is chosen: its leaves through
/// `?:`, `match` and `let` are number literals.
fn number(e: &Expr) -> bool {
    match e {
        Expr::Number(..) => true,
        Expr::Ternary(_, yes, no, _) => number(yes) && number(no),
        Expr::Match { some, none, .. } => number(some) && number(none),
        Expr::Let { body, .. } => number(body),
        _ => false,
    }
}

/// Whether a value is literal: its leaves through `?:`, `match` and `let`
/// are string literals.
fn literal(e: &Expr) -> bool {
    match e {
        Expr::Str(..) => true,
        Expr::Ternary(_, yes, no, _) => literal(yes) && literal(no),
        Expr::Match { some, none, .. } => literal(some) && literal(none),
        Expr::Let { body, .. } => literal(body),
        _ => false,
    }
}

/// The literals a value is, through `?:`, `match` and `let`: strict.
fn values(e: &Expr, out: &mut Vec<(String, Span, bool)>) {
    match e {
        Expr::Ternary(_, yes, no, _) => {
            values(yes, out);
            values(no, out);
        }
        Expr::Match { some, none, .. } => {
            values(some, out);
            values(none, out);
        }
        Expr::Let { body, .. } => values(body, out),
        other => strings(other, true, out),
    }
}

/// The string literals a value is, through `?:`, `match` and `let`.
pub(crate) fn literal_leaves(e: &Expr, out: &mut Vec<(String, Span)>) {
    match e {
        Expr::Str(text, span) => out.push((text.clone(), *span)),
        Expr::Ternary(_, yes, no, _) => {
            literal_leaves(yes, out);
            literal_leaves(no, out);
        }
        Expr::Match { some, none, .. } => {
            literal_leaves(some, out);
            literal_leaves(none, out);
        }
        Expr::Let { body, .. } => literal_leaves(body, out),
        _ => {}
    }
}

/// A string literal's text, or a template's, with its place.
fn strings(e: &Expr, strict: bool, out: &mut Vec<(String, Span, bool)>) {
    match e {
        Expr::Str(text, span) => out.push((text.clone(), *span, strict)),
        Expr::Template(parts, span) => {
            for p in parts {
                if let TemplatePart::Text(text) = p {
                    out.push((text.clone(), *span, strict));
                }
            }
        }
        _ => {}
    }
}

/// Visit `e` and every expression inside it.
fn exprs(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) | Expr::Ident(..) => {}
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    exprs(x, f);
                }
            }
        }
        Expr::List(items, _) => items.iter().for_each(|a| exprs(a, f)),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => exprs(x, f),
        Expr::Call(_, args, _) => args.iter().for_each(|a| exprs(a, f)),
        Expr::Binary(_, a, b, _) => {
            exprs(a, f);
            exprs(b, f);
        }
        Expr::Ternary(a, b, c, _) => {
            exprs(a, f);
            exprs(b, f);
            exprs(c, f);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            exprs(subject, f);
            exprs(some, f);
            exprs(none, f);
        }
        Expr::Let { value, body, .. } => {
            exprs(value, f);
            exprs(body, f);
        }
        Expr::Arrow { body, .. } => exprs(body, f),
    }
}

/// Every expression an action's statements hold.
fn stmts(body: &[Stmt], f: &mut dyn FnMut(&Expr)) {
    for s in body {
        match s {
            Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => f(expr),
            Stmt::Command { args, .. } | Stmt::Send { args, .. } => args.iter().for_each(&mut *f),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                f(cond);
                stmts(then, f);
                stmts(otherwise, f);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                f(subject);
                stmts(&some.1, f);
                stmts(none, f);
            }
            Stmt::Call { args, body, .. } => {
                args.iter().for_each(&mut *f);
                stmts(body, f);
            }
            Stmt::Refresh { .. } => {}
        }
    }
}

/// Each outermost colour function in `text` that names a token.
fn expressions<'t>(text: &'t str, is_token: &dyn Fn(&str) -> bool) -> Vec<&'t str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = text[from..].find('(').map(|k| from + k) {
        let start = text[..open]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .map_or(0, |k| k + 1);
        let mut depth = 0;
        let mut end = None;
        for (k, c) in text[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + k + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        let name = &text[start..open];
        if COLOUR_FNS.iter().any(|f| f.eq_ignore_ascii_case(name))
            && words(&text[open..end]).any(is_token)
        {
            out.push(&text[start..end]);
            from = end;
        } else {
            from = open + 1;
        }
    }
    out
}
