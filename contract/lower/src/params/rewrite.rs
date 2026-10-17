//! Where a component names its params (LLP 1121 D15): `param.<name>` in its
//! rows (a style, shorthand or motion row, an image's source, named or
//! positional), anywhere in the row's expression (a call's argument, a
//! branch, a template), and in its params' defaults and `select` options,
//! is the param's token name. A quoted word, one after `#` (`url(#…)`), and
//! one touching a template's `${…}` are left as written, as a motion token's
//! are. Text that is no row (a `text`'s content, a `testId`) is left alone.
//! A param named where no component owns it (a `style` or `keyframes`, which
//! any component may apply; an argument to another component; a state's or
//! a derive's value) is refused, by its own id.

use super::{full_name, token_name, PREFIX};
use crate::LowerError;
use contract_syntax::{Component, Expr, File, Node, Span, TemplatePart};

/// Whether attribute `name` of an element tagged `tag` is a row a param may
/// stand in.
fn row(tag: &str, name: &str) -> bool {
    use crate::tags::AttrTarget;
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

/// Each `param.<name>` whole word of `text` outside quotes, not after `#`,
/// and (`glued`) not touching a template hole at its start or end, with its
/// byte range and name.
fn found(text: &str, glued: (bool, bool)) -> Vec<(usize, usize, String)> {
    let ident = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
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
    out
}

/// `text` with each `param.<name>` [`found`] in it written by `named`; a name
/// `named` does not know is pushed to `bad`.
fn written(
    text: &str,
    glued: (bool, bool),
    named: &dyn Fn(&str) -> Option<String>,
    bad: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    let mut at = 0;
    for (start, end, name) in found(text, glued) {
        out.push_str(&text[at..start]);
        match named(&name) {
            Some(token) => out.push_str(&token),
            None => {
                bad.push(name);
                out.push_str(&text[start..end]);
            }
        }
        at = end;
    }
    out.push_str(&text[at..]);
    out
}

/// Visit each string literal and template text in `e`, everywhere in it,
/// with whether a template hole touches it at its start or end.
/// What [`texts`] calls with each text: it, its hole sides, its span.
type Visit<'a> = dyn FnMut(&mut String, (bool, bool), Span) + 'a;

fn texts(e: &mut Expr, f: &mut Visit<'_>) {
    match e {
        Expr::Str(s, span) => f(s, (false, false), *span),
        Expr::Template(parts, span) => {
            let n = parts.len();
            let holes: Vec<bool> = parts
                .iter()
                .map(|p| matches!(p, TemplatePart::Expr(_)))
                .collect();
            for (i, p) in parts.iter_mut().enumerate() {
                match p {
                    TemplatePart::Text(s) => {
                        let glued = (i > 0 && holes[i - 1], i + 1 < n && holes[i + 1]);
                        f(s, glued, *span);
                    }
                    TemplatePart::Expr(x) => texts(x, f),
                }
            }
        }
        Expr::List(items, _) => items.iter_mut().for_each(|a| texts(a, f)),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => texts(x, f),
        Expr::Call(_, args, _) => args.iter_mut().for_each(|a| texts(a, f)),
        Expr::Binary(_, a, b, _) => {
            texts(a, f);
            texts(b, f);
        }
        Expr::Ternary(a, b, c, _) => {
            texts(a, f);
            texts(b, f);
            texts(c, f);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            texts(subject, f);
            texts(some, f);
            texts(none, f);
        }
        Expr::Let { value, body, .. } => {
            texts(value, f);
            texts(body, f);
        }
        _ => {}
    }
}

/// The places a param may be named in a component's view: each row's value
/// and each image's positional source, by `visit`; and each other place a
/// view writes a string (another component's argument), by `elsewhere`.
fn view(
    list: &mut [Node],
    visit: &mut dyn FnMut(&mut Expr),
    elsewhere: &mut dyn FnMut(&mut Expr, &str),
) {
    for node in list {
        match node {
            Node::Element {
                tag,
                attrs,
                positional,
                children,
                ..
            } => {
                for a in attrs.iter_mut().filter(|a| row(tag, &a.name)) {
                    visit(&mut a.value);
                }
                if sourced(tag) {
                    if let Some(p) = positional.first_mut() {
                        visit(p);
                    }
                }
                view(children, visit, elsewhere);
            }
            Node::When {
                then, otherwise, ..
            } => {
                view(then, visit, elsewhere);
                view(otherwise, visit, elsewhere);
            }
            Node::Each { body, .. } => view(body, visit, elsewhere),
            Node::Match { some, none, .. } => {
                view(&mut some.1, visit, elsewhere);
                view(none, visit, elsewhere);
            }
            Node::Use {
                name,
                args,
                children,
                ..
            } => {
                for a in args.iter_mut() {
                    elsewhere(&mut a.value, name);
                }
                view(children, visit, elsewhere);
            }
            Node::Children { .. } => {}
        }
    }
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
    texts(e, &mut |s, glued, span| {
        out.extend(found(s, glued).into_iter().map(|(_, _, n)| (n, span)));
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
    view(
        &mut c.view,
        &mut |e| {
            texts(e, &mut |s, glued, span| {
                let mut found = Vec::new();
                *s = written(s, glued, &named, &mut found);
                bad.extend(found.into_iter().map(|n| (n, span)));
            })
        },
        &mut |e, to| {
            for (n, span) in named_in(e) {
                scope.push(scoped(
                    span,
                    &n,
                    format!("an argument to `{to}`"),
                    &format!("pass a token, or give `{to}` a param of its own"),
                ));
            }
        },
    );
    // A state's, a derive's or an action's value is the runtime's.
    for b in c.states.iter_mut().chain(c.derives.iter_mut()) {
        for (n, span) in named_in(&mut b.expr) {
            scope.push(scoped(span, &n, format!("the value of `{}`", b.name), "a state's or derive's value is data, read at run time: name the param in the row that draws it"));
        }
    }
    for p in &mut c.params.list {
        let mut found = Vec::new();
        p.default = written(&p.default, (false, false), &named, &mut found);
        bad.extend(found.into_iter().map(|n| (n, p.default_span)));
        for a in p.attrs.iter_mut().filter(|a| a.name == "options") {
            if let contract_syntax::params::ParamValue::Str(s) = &mut a.value {
                let mut found = Vec::new();
                *s = written(s, (false, false), &named, &mut found);
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
    view(
        &mut c.view,
        &mut |e| {
            texts(e, &mut |s, _, _| {
                out.extend(
                    s.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                        .filter(|w| w.starts_with(PREFIX))
                        .map(str::to_string),
                )
            })
        },
        &mut |_, _| {},
    );
    out
}

/// Each param of `file` no row of its component reaches, directly or
/// through another of its params (`tamber-param-unused`). A state part's
/// rows (D8) land with stage 8, so a param only one would read is refused
/// here too.
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
                        "param `{}` is read by no row of `{}`, directly or through another of its params: read it in a row (`param.{}`), or remove it; a param read only by a state part (D8) lands with stage 8",
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
            found(t, glued)
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
        let named = |n: &str| (n == "fill").then(|| "T".to_string());
        let mut bad = Vec::new();
        assert_eq!(
            written(
                "a param.fill b param.nope",
                (false, false),
                &named,
                &mut bad
            ),
            "a T b param.nope"
        );
        assert_eq!(bad, ["nope"]);
    }
}
