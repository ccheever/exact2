//! Where a component names its params (LLP 1121 D15): `param.<name>` where
//! one of its rows (a style, shorthand or motion row, an image's `symbol:`
//! source, named or positional) takes its value, and in its params' defaults
//! and `select` options, is the param's token name. A row takes its value
//! from a string or a template, through a `?:` branch, a `match` arm, a
//! `let` and a `+`, as a motion token's row does; a param named in the
//! row's computation (a function's argument, a condition, a comparison, a
//! callback) is refused, since what it computes is data the param's value
//! never reaches. A quoted word (across a template's holes too), one after
//! `#` (`url(#…)`), and one touching a template's `${…}` are left as
//! written, as a motion token's are. Text that is no row (a `text`'s
//! content, a `testId`, an image's URL) is left alone. An argument to
//! another component passes the param to it, and [`reached`] refuses it
//! where the child reads it otherwise than as a row's value. A param named
//! where no component owns it (a `style` or `keyframes`, which any component
//! may apply; a state's or a derive's value) is refused, by its own id.

use super::{full_name, token_name, ParamToken, PREFIX};
use crate::LowerError;
use contract_syntax::{BinOp, Component, Expr, File, Node, Span, TemplatePart};

/// Whether attribute `name` of an element tagged `tag` is a row a param may
/// stand in.
fn row(tag: &str, name: &str) -> bool {
    use crate::tags::AttrTarget;
    // A state part's row is a row (LLP 1121 D8, stage 8 F: a param read only
    // by `:active` is reached).
    let name = contract_syntax::state::state_of(name).map_or(name, |(_, row)| row);
    matches!(
        crate::tags::attr(name),
        Some(AttrTarget::Styles(_) | AttrTarget::Shorthand | AttrTarget::Flex)
    ) || crate::motion::is_row(name)
        || (name == "src" && tag == "image")
}

/// A tag whose first positional is a source a param may name (an icon).
fn sourced(tag: &str) -> bool {
    tag == "image"
}

/// Whether an image's source `e` is a symbol's (`symbol:…`), the one source
/// a token names; a URL or an asset's path is no row.
fn symbolic(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s.starts_with("symbol:"),
        Expr::Template(parts, _) => {
            matches!(parts.first(), Some(TemplatePart::Text(s)) if s.starts_with("symbol:"))
        }
        _ => false,
    }
}

/// What a part of an image's source is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    /// A symbol's (`symbol:…`), a row's value.
    Symbol,
    /// A URL or an asset's path: no row, left as written.
    Url,
    /// A condition, a `match` subject or a `let`'s value: a computation.
    Data,
}

/// Each part of an image's source `e`: a `?:`'s branches and a `match`'s
/// arms apart, so a URL branch is left as written beside a symbol's.
fn source_parts(e: &mut Expr, f: &mut dyn FnMut(&mut Expr, Part)) {
    match e {
        Expr::Ternary(c, a, b, _) => {
            f(c, Part::Data);
            source_parts(a, f);
            source_parts(b, f);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            f(subject, Part::Data);
            source_parts(some, f);
            source_parts(none, f);
        }
        // An expansion's shared derive: its value read by name.
        Expr::Let { value, body, .. } => {
            f(value, Part::Data);
            source_parts(body, f);
        }
        // `"symbol:" + …`: a symbol's, as its first string says.
        Expr::Binary(BinOp::Add, a, _, _) if leftmost(a).is_some_and(symbolic) => {
            f(e, Part::Symbol)
        }
        _ => {
            let part = match symbolic(e) {
                true => Part::Symbol,
                false => Part::Url,
            };
            f(e, part)
        }
    }
}

/// The first operand of a `+` chain.
fn leftmost(e: &Expr) -> Option<&Expr> {
    match e {
        Expr::Binary(BinOp::Add, a, _, _) => leftmost(a),
        other => Some(other),
    }
}

/// Whether attribute `name` of `tag` is an image's source (named).
fn source(tag: &str, name: &str) -> bool {
    sourced(tag) && name == "src"
}

/// Each `param.<name>` whole word of `text` outside quotes, not after `#`,
/// and (`glued`) not touching a template hole at its start or end, with its
/// byte range and name; and the quote `text` leaves open, starting in
/// `quote` (a template's earlier text's).
fn found(
    text: &str,
    glued: (bool, bool),
    mut quote: Option<char>,
) -> (Vec<(usize, usize, String)>, Option<char>) {
    let ident = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < bytes.len() {
        let (i, c) = bytes[k];
        if let Some(q) = quote {
            if c == '\\' {
                k += 2;
                continue;
            }
            if c == q {
                quote = None;
            }
            k += 1;
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            k += 1;
            continue;
        }
        if text[i..].starts_with(PREFIX) {
            let before = text[..i].chars().next_back();
            let tail = &text[i + PREFIX.len()..];
            let len = tail.find(|c: char| !ident(c)).unwrap_or(tail.len());
            let end = i + PREFIX.len() + len;
            let whole = before.is_none_or(|c| !ident(c) && c != '.' && c != '#')
                && len > 0
                && !tail[len..].starts_with('.')
                && !(glued.0 && i == 0)
                && !(glued.1 && end == text.len());
            if whole {
                out.push((i, end, tail[..len].to_string()));
            }
            while k < bytes.len() && bytes[k].0 < end {
                k += 1;
            }
            continue;
        }
        k += 1;
    }
    (out, quote)
}

/// Where a text is: its hole sides, the quote it starts in, its literal's
/// span, and whether it supplies a row's value (else it is data a
/// computation reads).
#[derive(Clone, Copy)]
struct At {
    glued: (bool, bool),
    quote: Option<char>,
    span: Span,
    value: bool,
    /// Inside a template's hole inside quotes: part of a quoted name.
    quoted: bool,
}

/// A text on its own: a param's default, a `select`'s options.
const PLAIN: At = At {
    glued: (false, false),
    quote: None,
    span: Span {
        line: 0,
        col: 0,
        end_col: 0,
        source_id: 0,
    },
    value: true,
    quoted: false,
};

/// [`written`] for a text on its own: a design component's param default.
pub(super) fn plain(
    text: &str,
    named: &dyn Fn(&str) -> Option<String>,
    bad: &mut Vec<String>,
) -> String {
    written(text, PLAIN, named, bad)
}

/// `text` with each `param.<name>` [`found`] in it written by `named`; a name
/// `named` does not know is pushed to `bad`.
fn written(
    text: &str,
    at: At,
    named: &dyn Fn(&str) -> Option<String>,
    bad: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    let mut from = 0;
    for (start, end, name) in found(text, at.glued, at.quote).0 {
        out.push_str(&text[from..start]);
        match named(&name) {
            Some(token) => out.push_str(&token),
            None => {
                bad.push(name);
                out.push_str(&text[start..end]);
            }
        }
        from = end;
    }
    out.push_str(&text[from..]);
    out
}

/// What [`texts`] calls with each text and where it is.
type Visit<'a> = dyn FnMut(&mut String, At) + 'a;

/// Visit each string literal and template text in `e`, everywhere in it;
/// `value`: whether `e` supplies a row's value, which its strings and
/// templates do through `?:` branches, `match` arms, a `let`'s body and `+`
/// (as [`crate::motion`] writes a motion token in), and nothing else does:
/// a `let` (an expansion's shared derive or call) is read by name, a
/// comparison too, so its value is data.
fn texts(e: &mut Expr, value: bool, f: &mut Visit<'_>) {
    walk(e, value, false, f)
}

/// [`texts`]; `quoted`: inside a template's hole inside quotes.
fn walk(e: &mut Expr, value: bool, quoted: bool, f: &mut Visit<'_>) {
    let at = |span: Span, glued, quote| At {
        glued,
        quote,
        span,
        value,
        quoted,
    };
    match e {
        Expr::Str(s, span) => f(s, at(*span, (false, false), None)),
        Expr::Template(parts, span) => {
            let n = parts.len();
            let holes: Vec<bool> = parts
                .iter()
                .map(|p| matches!(p, TemplatePart::Expr(_)))
                .collect();
            let mut quote = None;
            for (i, p) in parts.iter_mut().enumerate() {
                match p {
                    TemplatePart::Text(s) => {
                        let glued = (i > 0 && holes[i - 1], i + 1 < n && holes[i + 1]);
                        let start = quote;
                        quote = found(s, glued, start).1;
                        f(s, at(*span, glued, start));
                    }
                    // A hole inside quotes is part of a quoted name.
                    TemplatePart::Expr(x) => walk(x, value, quoted || quote.is_some(), f),
                }
            }
        }
        Expr::Ternary(c, a, b, _) => {
            walk(c, false, quoted, f);
            walk(a, value, quoted, f);
            walk(b, value, quoted, f);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            walk(subject, false, quoted, f);
            walk(some, value, quoted, f);
            walk(none, value, quoted, f);
        }
        // Compiler-only: an expansion's shared derive or call, read by
        // name wherever its body says, a comparison too: data.
        Expr::Let { value: v, body, .. } => {
            walk(v, false, quoted, f);
            walk(body, value, quoted, f);
        }
        Expr::Binary(op, a, b, _) => {
            let joined = value && matches!(op, BinOp::Add);
            walk(a, joined, quoted, f);
            walk(b, joined, quoted, f);
        }
        Expr::List(items, _) => items.iter_mut().for_each(|a| walk(a, false, quoted, f)),
        Expr::Call(_, args, _) => args.iter_mut().for_each(|a| walk(a, false, quoted, f)),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => walk(x, false, quoted, f),
        Expr::Arrow { body, .. } => walk(body, false, quoted, f),
        Expr::Number(..) | Expr::Bool(..) | Expr::None(_) | Expr::Ident(..) => {}
    }
}

/// The places a param may be named in a component's view: each row's value,
/// each image's `symbol:` source, and each argument to another component
/// (the param passes to it: [`reached`] refuses one the child reads
/// otherwise than as a row's value).
fn view(list: &mut [Node], visit: &mut dyn FnMut(&mut Expr, bool)) {
    for node in list {
        match node {
            Node::Element {
                tag,
                attrs,
                positional,
                children,
                ..
            } => {
                // An image's source: a symbol's part is a row's value, a
                // condition a computation, a URL left as written.
                let mut part = |e: &mut Expr, p: Part| match p {
                    Part::Symbol => visit(e, true),
                    Part::Data => visit(e, false),
                    Part::Url => {}
                };
                for a in attrs.iter_mut().filter(|a| row(tag, &a.name)) {
                    match source(tag, &a.name) {
                        true => source_parts(&mut a.value, &mut part),
                        false => part(&mut a.value, Part::Symbol),
                    }
                }
                if sourced(tag) {
                    if let Some(p) = positional.first_mut() {
                        source_parts(p, &mut part);
                    }
                }
                view(children, visit);
            }
            Node::When {
                then, otherwise, ..
            } => {
                view(then, visit);
                view(otherwise, visit);
            }
            Node::Each { body, .. } => view(body, visit),
            Node::Match { some, none, .. } => {
                view(&mut some.1, visit);
                view(none, visit);
            }
            Node::Use { args, children, .. } => {
                for a in args.iter_mut() {
                    visit(&mut a.value, true);
                }
                view(children, visit);
            }
            Node::Children { .. } => {}
        }
    }
}

/// Every expression of the expanded view `list`, with whether it is a
/// row's value ([`view`]'s places) or anything else the view reads (a
/// text's content, an attribute that is no row, a condition, a list).
fn everything(list: &mut [Node], visit: &mut dyn FnMut(&mut Expr, bool)) {
    for node in list {
        match node {
            Node::Element {
                tag,
                attrs,
                positional,
                children,
                ..
            } => {
                let mut part = |e: &mut Expr, p: Part| visit(e, p == Part::Symbol);
                for a in attrs.iter_mut() {
                    match (row(tag, &a.name), source(tag, &a.name)) {
                        (true, true) => source_parts(&mut a.value, &mut part),
                        (row, _) => part(&mut a.value, if row { Part::Symbol } else { Part::Data }),
                    }
                }
                for (i, p) in positional.iter_mut().enumerate() {
                    match sourced(tag) && i == 0 {
                        true => source_parts(p, &mut part),
                        false => part(p, Part::Data),
                    }
                }
                everything(children, visit);
            }
            Node::When {
                cond,
                then,
                otherwise,
                ..
            } => {
                visit(cond, false);
                everything(then, visit);
                everything(otherwise, visit);
            }
            Node::Each {
                list, key, body, ..
            } => {
                visit(list, false);
                visit(key, false);
                everything(body, visit);
            }
            Node::Match {
                subject,
                some,
                none,
                ..
            } => {
                visit(subject, false);
                everything(&mut some.1, visit);
                everything(none, visit);
            }
            Node::Use { args, children, .. } => {
                for a in args.iter_mut() {
                    visit(&mut a.value, true);
                }
                everything(children, visit);
            }
            Node::Children { .. } => {}
        }
    }
}

/// Each place in the expanded `root` (every component inlined, its
/// declarations lifted) where a param's token, passed through a prop,
/// reached something other than a row's value: a text's content, an
/// attribute that is no row, a computation in a row (a function's
/// argument, a condition), a state, a derive or an action. Refused
/// (`tamber-param-scope`), naming the param in full.
pub fn reached(root: &Component, params: &[ParamToken]) -> Vec<LowerError> {
    if params.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(String, Span)> = Vec::new();
    let mut list = root.view.clone();
    everything(&mut list, &mut |e, row| {
        texts(e, row, &mut |s, at| {
            if !at.value || at.quoted {
                found.push((s.clone(), at.span));
            }
        })
    });
    let mut out = Vec::new();
    for (text, span) in found {
        if let Some(p) = params.iter().find(|p| text.contains(&p.token)) {
            out.push(LowerError {
                id: "tamber-param-scope",
                message: format!("param `{}` reaches a place no row takes its value from (a text's content, an attribute that is no row, a function's argument or a condition), through an argument that passed it: pass it only to a prop the component writes a row's value with", p.full),
                span,
            });
        }
    }
    // What the expansion lifted out of the view (states, derives, actions).
    let mut rest = root.clone();
    rest.view.clear();
    rest.params = Default::default();
    let rest = format!("{rest:?}");
    for p in params.iter().filter(|p| rest.contains(&p.token)) {
        out.push(LowerError {
            id: "tamber-param-scope",
            message: format!("param `{}` reaches a state, a derive or an action, through an argument that passed it: pass it only to a prop the component writes a row's value with", p.full),
            span: p.param.span,
        });
    }
    out
}

/// A refusal of a param named where no component owns it.
fn scoped(span: Span, name: &str, place: String, fix: &str) -> LowerError {
    LowerError {
        id: "tamber-param-scope",
        message: format!(
            "`param.{name}` in {place}: a param is named in its own component's rows (D15); {fix}"
        ),
        span,
    }
}

/// The `param.` words a string in `e` writes, each with its literal's span.
fn named_in(e: &mut Expr) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    texts(e, false, &mut |s, at| {
        if at.quoted {
            return;
        }
        out.extend(
            found(s, at.glued, at.quote)
                .0
                .into_iter()
                .map(|(_, _, n)| (n, at.span)),
        );
    });
    out
}

/// Rewrite `param.<name>` where each component of `file` names its params
/// (module docs) to the param's token name; refuse a name the component does
/// not declare (`tamber-param-unknown`), and one where no component owns it
/// (`tamber-param-scope`).
pub fn rewrite(file: &mut File) -> Vec<LowerError> {
    let mut errors = Vec::new();
    // A `style` or `keyframes` is any applying component's: no one's params.
    for s in &mut file.styles {
        for a in &mut s.attrs {
            for (n, span) in named_in(&mut a.value) {
                errors.push(scoped(span, &n, format!("the style `{}`", s.name), "a style is shared by every component that applies it: write the row on the component's element"));
            }
        }
    }
    for k in &mut file.keyframes {
        for a in k.frames.iter_mut().flat_map(|f| f.attrs.iter_mut()) {
            for (n, span) in named_in(&mut a.value) {
                errors.push(scoped(span, &n, format!("the keyframes `{}`", k.name), "keyframes are shared by every animation that names them: write the value as a token's"));
            }
        }
    }
    for c in &mut file.components {
        rewrite_component(c, &mut errors);
    }
    errors
}

fn rewrite_component(c: &mut Component, errors: &mut Vec<LowerError>) {
    let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
    let declared: Vec<String> = c.params.list.iter().map(|p| p.name.clone()).collect();
    let named = |n: &str| {
        declared
            .iter()
            .any(|d| d == n)
            .then(|| token_name(&identity, n))
    };
    let mut bad: Vec<(String, Span)> = Vec::new();
    let mut scope: Vec<LowerError> = Vec::new();
    view(&mut c.view, &mut |e, value| {
        texts(e, value, &mut |s, at| {
            if at.quoted {
                // Part of a quoted name: left as written.
            } else if at.value {
                let mut found = Vec::new();
                *s = written(s, at, &named, &mut found);
                bad.extend(found.into_iter().map(|n| (n, at.span)));
            } else {
                for (_, _, n) in found(s, at.glued, at.quote).0 {
                    scope.push(scoped(at.span, &n, format!("a computation in a row of `{}`", c.name), "a param is a row's value, written as a string, a template, a `?:` branch, a `match` arm or a `+`; what a function, a condition or a comparison computes from it is data its value never reaches"));
                }
            }
        })
    });
    // A state's, a derive's or an action's value is the runtime's.
    for b in c.states.iter_mut().chain(c.derives.iter_mut()) {
        for (n, span) in named_in(&mut b.expr) {
            scope.push(scoped(span, &n, format!("the value of `{}`", b.name), "a state's or derive's value is data, read at run time: name the param in the row that draws it"));
        }
    }
    for p in &mut c.params.list {
        let mut found = Vec::new();
        p.default = written(&p.default, PLAIN, &named, &mut found);
        bad.extend(found.into_iter().map(|n| (n, p.default_span)));
        for a in p.attrs.iter_mut().filter(|a| a.name == "options") {
            if let contract_syntax::params::ParamValue::Str(s) = &mut a.value {
                let mut found = Vec::new();
                *s = written(s, PLAIN, &named, &mut found);
                bad.extend(found.into_iter().map(|n| (n, a.span)));
            }
        }
    }
    for (name, span) in bad {
        errors.push(LowerError {
            id: "tamber-param-unknown",
            message: format!(
                "`param.{name}`: `{}` declares no param `{name}`{}",
                c.name,
                if declared.is_empty() {
                    String::new()
                } else {
                    format!(" (its params: {})", declared.join(", "))
                }
            ),
            span,
        });
    }
    errors.extend(scope);
}

/// The param token names a component's rows read.
fn read(c: &mut Component) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    view(&mut c.view, &mut |e, value| {
        texts(e, value, &mut |s, at| {
            if at.value && !at.quoted {
                out.extend(
                    s.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                        .filter(|w| w.starts_with(PREFIX))
                        .map(str::to_string),
                )
            }
        })
    });
    out
}

/// Each param of `file` no row of its component reaches, directly or
/// through another of its params (`tamber-param-unused`); a state part's
/// row is a row (D8, D15's `pressed`).
pub fn unused(file: &File) -> Vec<LowerError> {
    let mut errors = Vec::new();
    for c in &file.components {
        if c.params.list.is_empty() {
            continue;
        }
        let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
        let mut reached = read(&mut c.clone());
        let words = |text: &str| -> Vec<String> {
            text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                .filter(|w| w.starts_with(PREFIX))
                .map(str::to_string)
                .collect()
        };
        loop {
            let more: Vec<String> = c
                .params
                .list
                .iter()
                .filter(|p| reached.contains(&token_name(&identity, &p.name)))
                .flat_map(|p| words(&p.default))
                .filter(|n| !reached.contains(n))
                .collect();
            if more.is_empty() {
                break;
            }
            reached.extend(more);
        }
        for p in &c.params.list {
            if !reached.contains(&token_name(&identity, &p.name)) {
                errors.push(LowerError {
                    id: "tamber-param-unused",
                    message: format!(
                        "param `{}` is read by no row of `{}`, directly or through another of its params: read it in a row or a state part's row (`param.{}`), or remove it",
                        full_name(&identity, &p.name),
                        c.name,
                        p.name
                    ),
                    span: p.span,
                });
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_param_is_a_whole_word_outside_quotes_fragments_and_holes() {
        let names = |t: &str, glued| {
            found(t, glued, None)
                .0
                .into_iter()
                .map(|(_, _, n)| n)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(
                "color-mix(in oklab, param.fill 88%, x.param.fill) param.fill.y",
                (false, false)
            ),
            ["fill"]
        );
        assert!(names("\"param.fill\", system-ui", (false, false)).is_empty());
        assert!(names("url(#param.fill)", (false, false)).is_empty());
        assert!(names("param.fill", (true, false)).is_empty());
        assert!(names("x param.fill", (false, true)).is_empty());
        // A template's text after `"${face} ` is still inside the quote.
        assert!(found("param.fill\", x", (true, false), Some('"'))
            .0
            .is_empty());
        assert_eq!(found("\"${", (false, true), None).1, Some('"'));
        let named = |n: &str| (n == "fill").then(|| "T".to_string());
        let mut bad = Vec::new();
        assert_eq!(
            written("a param.fill b param.nope", PLAIN, &named, &mut bad),
            "a T b param.nope"
        );
        assert_eq!(bad, ["nope"]);
    }
}
