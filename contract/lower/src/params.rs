//! Component params (LLP 1121 D15): a param is a token its component owns.
//! Its full name is the component's identity and the param
//! (`post.contract#PostButton.radius`); below the IR (the reference table,
//! the fold, the live table, `uses`, the JS target's properties) it is a
//! dotted token name derived from that identity,
//! `param.f<file hash>.<Component>.<param>` (theme2's ruling, 2026-10-17).
//! The driver rewrites `param.<name>` in a component's rows, its uses'
//! arguments and its params' defaults to that name before anything reads
//! them, so a param is read, folded, retained and bound as any token is.

use crate::LowerError;
use contract_syntax::params::{ComponentParam, ParamValue};
use contract_syntax::{Attr, Expr, File, Node, Span, TemplatePart};

/// The prefix every param's token name has, which no design token may.
pub const PREFIX: &str = "param.";

/// A file's part of a token name: FNV-1a of its root-relative path, as eight
/// hex digits after an `f` (a name's segment starts with a letter).
fn file_hash(file: &str) -> String {
    let h = file.bytes().fold(0x811c_9dc5u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    format!("f{h:08x}")
}

/// The token name of param `param` of the component `identity`
/// (`post.contract#PostButton`).
pub fn token_name(identity: &str, param: &str) -> String {
    let (file, component) = identity.rsplit_once('#').unwrap_or(("", identity));
    format!("{PREFIX}{}.{component}.{param}", file_hash(file))
}

/// The full name of param `param` of `identity`.
pub fn full_name(identity: &str, param: &str) -> String {
    format!("{identity}.{param}")
}

/// One param as a token: its token name, full name, kind, default (its
/// `param.` names rewritten) and where it is written.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamToken {
    /// `param.f3f9a01c2.PostButton.radius`.
    pub token: String,
    /// `post.contract#PostButton.radius`.
    pub full: String,
    /// The component's identity.
    pub identity: String,
    /// The param as declared, its default rewritten.
    pub param: ComponentParam,
}

/// Every param of `file`'s components, as tokens, in declaration order.
pub fn tokens(file: &File) -> Vec<ParamToken> {
    file.components
        .iter()
        .flat_map(|c| {
            let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
            c.params.list.iter().map(move |p| ParamToken {
                token: token_name(&identity, &p.name),
                full: full_name(&identity, &p.name),
                identity: identity.clone(),
                param: p.clone(),
            })
        })
        .collect()
}

/// `text` with each `param.<name>` a word of it written as `name`'s token
/// name by `named`; a name `named` does not know is refused, by `bad`.
fn words(text: &str, named: &dyn Fn(&str) -> Option<String>, bad: &mut Vec<String>) -> String {
    let ident = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find(PREFIX) {
        let before = rest[..at].chars().next_back();
        let tail = &rest[at + PREFIX.len()..];
        let len = tail.find(|c: char| !ident(c)).unwrap_or(tail.len());
        let name = &tail[..len];
        // A whole word: not inside another name (`my.param.x`), nor a longer
        // dotted name (`param.x.y`, a design token's).
        let whole = before.is_none_or(|c| !ident(c) && c != '.')
            && !name.is_empty()
            && !tail[len..].starts_with('.');
        out.push_str(&rest[..at]);
        match (whole, whole.then(|| named(name)).flatten()) {
            (true, Some(token)) => out.push_str(&token),
            (true, None) => {
                bad.push(name.to_string());
                out.push_str(&rest[at..at + PREFIX.len() + len]);
            }
            (false, _) => out.push_str(&rest[at..at + PREFIX.len() + len]),
        }
        rest = &tail[len..];
    }
    out.push_str(rest);
    out
}

/// `e`'s strings with their `param.` names rewritten.
fn expr(e: &mut Expr, named: &dyn Fn(&str) -> Option<String>, bad: &mut Vec<(String, Span)>) {
    let mut found = Vec::new();
    match e {
        Expr::Str(s, span) => {
            *s = words(s, named, &mut found);
            bad.extend(found.into_iter().map(|n| (n, *span)));
        }
        Expr::Template(parts, span) => {
            for part in parts.iter_mut() {
                match part {
                    TemplatePart::Text(s) => *s = words(s, named, &mut found),
                    TemplatePart::Expr(e) => expr(e, named, bad),
                }
            }
            bad.extend(found.into_iter().map(|n| (n, *span)));
        }
        Expr::Ternary(_, yes, no, _) => {
            expr(yes, named, bad);
            expr(no, named, bad);
        }
        Expr::Match { some, none, .. } => {
            expr(some, named, bad);
            expr(none, named, bad);
        }
        Expr::Let { value, body, .. } => {
            expr(value, named, bad);
            expr(body, named, bad);
        }
        Expr::Binary(_, left, right, _) => {
            expr(left, named, bad);
            expr(right, named, bad);
        }
        _ => {}
    }
}

fn attrs(list: &mut [Attr], named: &dyn Fn(&str) -> Option<String>, bad: &mut Vec<(String, Span)>) {
    for a in list {
        expr(&mut a.value, named, bad);
    }
}

fn nodes(list: &mut [Node], named: &dyn Fn(&str) -> Option<String>, bad: &mut Vec<(String, Span)>) {
    for node in list {
        match node {
            // A text's content is no row: it is left as written.
            Node::Element {
                attrs: a, children, ..
            } => {
                attrs(a, named, bad);
                nodes(children, named, bad);
            }
            Node::When {
                then, otherwise, ..
            } => {
                nodes(then, named, bad);
                nodes(otherwise, named, bad);
            }
            Node::Each { body, .. } => nodes(body, named, bad),
            Node::Match { some, none, .. } => {
                nodes(&mut some.1, named, bad);
                nodes(none, named, bad);
            }
            Node::Use { args, children, .. } => {
                attrs(args, named, bad);
                nodes(children, named, bad);
            }
            Node::Children { .. } => {}
        }
    }
}

/// Rewrite `param.<name>` in each component of `file` (its view's rows, its
/// uses' arguments, its params' defaults) to the token name of
/// the component's param `name`. A `param.` name the component does not
/// declare is refused, as is one written in a component with no params.
pub fn rewrite(file: &mut File) -> Vec<LowerError> {
    let mut errors = Vec::new();
    for c in &mut file.components {
        let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
        let declared: Vec<String> = c.params.list.iter().map(|p| p.name.clone()).collect();
        let named = |n: &str| {
            declared
                .iter()
                .any(|d| d == n)
                .then(|| token_name(&identity, n))
        };
        let mut bad = Vec::new();
        nodes(&mut c.view, &named, &mut bad);
        for p in &mut c.params.list {
            let mut found = Vec::new();
            p.default = words(&p.default, &named, &mut found);
            bad.extend(found.into_iter().map(|n| (n, p.default_span)));
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
    }
    errors
}

/// Two declaring files whose paths hash alike: their params' token names
/// would be one, so the build is refused, naming both.
pub fn collisions(params: &[ParamToken]) -> Vec<LowerError> {
    let mut seen: Vec<(String, &str, Span)> = Vec::new();
    let mut errors = Vec::new();
    for p in params {
        let file = p.identity.rsplit_once('#').map_or("", |(f, _)| f);
        let hash = file_hash(file);
        match seen.iter().find(|(h, f, _)| *h == hash && *f != file) {
            Some((_, other, _)) => errors.push(LowerError {
                id: "tamber-param-collision",
                message: format!("`{file}` and `{other}` give their params one token name (`{PREFIX}{hash}…`): rename either file"),
                span: p.param.span,
            }),
            None => seen.push((hash, file, p.param.span)),
        }
    }
    errors
}

/// The words a component's view reads: its rows' and its uses' arguments'
/// literals.
fn read(list: &[Node], out: &mut Vec<String>) {
    fn text(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Str(s, _) => out.push(s.clone()),
            Expr::Template(parts, _) => {
                for p in parts {
                    match p {
                        TemplatePart::Text(s) => out.push(s.clone()),
                        TemplatePart::Expr(e) => text(e, out),
                    }
                }
            }
            Expr::Ternary(_, a, b, _)
            | Expr::Match {
                some: a, none: b, ..
            }
            | Expr::Let {
                value: a, body: b, ..
            }
            | Expr::Binary(_, a, b, _) => {
                text(a, out);
                text(b, out);
            }
            _ => {}
        }
    }
    for node in list {
        match node {
            Node::Element {
                attrs, children, ..
            } => {
                attrs.iter().for_each(|a| text(&a.value, out));
                read(children, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                read(then, out);
                read(otherwise, out);
            }
            Node::Each { body, .. } => read(body, out),
            Node::Match { some, none, .. } => {
                read(&some.1, out);
                read(none, out);
            }
            Node::Use { args, children, .. } => {
                args.iter().for_each(|a| text(&a.value, out));
                read(children, out);
            }
            Node::Children { .. } => {}
        }
    }
}

/// Each param of `file` no row of its component reaches, through any chain
/// of its params (`tamber-param-unused`). A state part's rows (D8) land
/// with stage 8, so a param only one would read is refused here too.
pub fn unused(file: &File) -> Vec<LowerError> {
    let names = |text: &str| -> Vec<String> {
        text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
            .filter(|w| w.starts_with(PREFIX))
            .map(str::to_string)
            .collect()
    };
    let mut errors = Vec::new();
    for c in &file.components {
        if c.params.list.is_empty() {
            continue;
        }
        let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
        let mut texts = Vec::new();
        read(&c.view, &mut texts);
        let mut reached: Vec<String> = texts.iter().flat_map(|t| names(t)).collect();
        // Through the defaults of the params reached.
        loop {
            let more: Vec<String> = c
                .params
                .list
                .iter()
                .filter(|p| reached.contains(&token_name(&identity, &p.name)))
                .flat_map(|p| names(&p.default))
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

/// Where a param's token is said to be written in a design it is added to:
/// line [`LINE`] + its index, past any design's own and below the
/// compiler's synthetic tokens'.
pub const LINE: u32 = 1 << 29;

/// `design` with each of `params` added as a token of its kind (an icon
/// param among its icons), on line [`LINE`] + its index.
pub fn add_to(design: &mut exact_tamber::Design, params: &[ParamToken]) {
    for (i, p) in params.iter().enumerate() {
        let at = |value: &str| exact_tamber::At {
            value: value.to_string(),
            line: LINE + i as u32,
            col: 1,
        };
        let icon = p.param.kind == "icon";
        let token = exact_tamber::Token {
            name: at(&p.token),
            kind: (!icon).then(|| p.param.kind.clone()),
            value: at(&p.param.default),
            whens: Vec::new(),
            meta: Vec::new(),
        };
        match icon {
            true => design.icons.push(token),
            false => design.tokens.push(token),
        }
    }
}

/// `message` with each param's token name written as its full name: what
/// an author reads never names the hash.
pub fn shown(message: &str, params: &[ParamToken]) -> String {
    let mut out = message.to_string();
    // Longest first, so a name that prefixes another does not split it.
    let mut by_len: Vec<&ParamToken> = params.iter().collect();
    by_len.sort_by_key(|p| std::cmp::Reverse(p.token.len()));
    for p in by_len {
        out = out.replace(&p.token, &p.full);
    }
    out
}

/// The kinds a param may be (D3's, and an icon).
const KINDS: [&str; 10] = [
    "color",
    "length",
    "number",
    "type",
    "family",
    "box-shadow",
    "text-shadow",
    "duration",
    "easing",
    "icon",
];

/// The controls a param of `kind` takes (D15's table).
fn controls(kind: &str) -> &'static [&'static str] {
    match kind {
        "color" => &["color", "token", "select", "none"],
        "length" => &["slider", "stepper", "token", "select", "none"],
        "number" => &["slider", "stepper", "select", "none"],
        "duration" | "easing" => &["slider", "token", "select", "none"],
        _ => &["token", "select", "none"],
    }
}

/// Each param's declaration checked as written: its kind, its control for
/// that kind, `select` with its `options`, a `range` on a kind that has one,
/// a positive `step`, a `priority` of 1, 2 or 3. Those whose kind is no
/// kind are left out of the design (their index in `params`).
pub fn declared(params: &[ParamToken]) -> (Vec<LowerError>, Vec<usize>) {
    let mut errors = Vec::new();
    let mut unkinded = Vec::new();
    for (i, t) in params.iter().enumerate() {
        let p = &t.param;
        let refuse = |id: &'static str, message: String| LowerError {
            id,
            message,
            span: p.span,
        };
        if !KINDS.contains(&p.kind.as_str()) {
            errors.push(refuse(
                "tamber-param-kind",
                format!(
                    "param `{}`: `{}` is no kind ({})",
                    t.full,
                    p.kind,
                    KINDS.join(", ")
                ),
            ));
            unkinded.push(i);
            continue;
        }
        let control = p.attr("control").map(ParamValue::text);
        if let Some(c) = &control {
            if !controls(&p.kind).contains(&c.as_str()) {
                errors.push(refuse(
                    "tamber-param-control",
                    format!(
                        "param `{}`: a {} takes `control=` {}, not `{c}` (D15)",
                        t.full,
                        p.kind,
                        controls(&p.kind).join(", ")
                    ),
                ));
            }
        }
        match (control.as_deref() == Some("select"), p.attr("options").is_some()) {
            (true, false) => errors.push(refuse("tamber-param-control", format!("param `{}`: `control=\"select\"` takes its choices as `options=\"a|b\"`, tokens of its kind", t.full))),
            (false, true) => errors.push(refuse("tamber-param-control", format!("param `{}`: `options` are a `select`'s choices: say `control=\"select\"`", t.full))),
            _ => {}
        }
        if let Some(r) = range(p) {
            if !matches!(p.kind.as_str(), "length" | "number" | "duration") {
                errors.push(refuse("tamber-param-range", format!("param `{}`: a {} has no range; a length (px), a number or a duration (ms) does", t.full, p.kind)));
            } else if let Err(why) = r {
                errors.push(refuse(
                    "tamber-param-range",
                    format!("param `{}`: {why}", t.full),
                ));
            }
        }
        if let Some(step) = p.attr("step") {
            if number(step).is_none_or(|n| n <= 0.0 || !n.is_finite()) {
                errors.push(refuse(
                    "tamber-param-step",
                    format!("param `{}`: `step` is a positive number", t.full),
                ));
            }
        }
        if let Some(priority) = p.attr("priority") {
            if !number(priority).is_some_and(|n| matches!(n, 1.0 | 2.0 | 3.0)) {
                errors.push(refuse(
                    "tamber-param-priority",
                    format!(
                        "param `{}`: `priority` is 1 (surfaced first), 2 or 3 (behind \"more\")",
                        t.full
                    ),
                ));
            }
        }
    }
    (errors, unkinded)
}

/// Each param checked as it folds, for every tuple: a ranged length is px
/// (`tamber-range-unit`) and every value is inside its range; a `select`'s
/// options name tokens of its kind.
pub fn folded(params: &[ParamToken], folding: &exact_tamber::Folding) -> Vec<LowerError> {
    use exact_tamber::{Length, Value};
    let mut errors = Vec::new();
    for t in params {
        let p = &t.param;
        let refuse = |id: &'static str, message: String| LowerError {
            id,
            message,
            span: p.default_span,
        };
        if let Some(Ok((low, high))) = range(p) {
            let Some(token) = folding.tokens.iter().find(|c| c.name == t.token) else {
                continue;
            };
            for v in &token.values {
                let n = match (p.kind.as_str(), v) {
                    ("length", Value::Length(Length::Px(n))) => *n,
                    ("length", other) => {
                        errors.push(refuse("tamber-range-unit", format!("param `{}` is `{}`: a ranged length is px, so its range is checked at build; a `rem`, `em` or text style is not (D15)", t.full, other.css())));
                        break;
                    }
                    (_, Value::Number(n)) | (_, Value::Duration(n)) => *n,
                    _ => continue,
                };
                if n < low || n > high {
                    errors.push(refuse(
                        "tamber-param-range",
                        format!(
                            "param `{}` is {} in a tuple, outside its range {}..{}",
                            t.full,
                            v.css(),
                            low,
                            high
                        ),
                    ));
                    break;
                }
            }
        }
        if let Some(options) = p.attr("options") {
            for name in options.text().split('|').map(str::trim) {
                let kind = match p.kind.as_str() {
                    "icon" => folding
                        .icons
                        .iter()
                        .any(|i| i.name == name)
                        .then_some("icon"),
                    _ => folding
                        .tokens
                        .iter()
                        .find(|c| c.name == name)
                        .map(|c| c.kind.name()),
                };
                match kind {
                    Some(k) if k == p.kind => {}
                    Some(k) => errors.push(refuse(
                        "tamber-param-options",
                        format!(
                            "param `{}`: option `{name}` is a {k} token, not a {}",
                            t.full, p.kind
                        ),
                    )),
                    None => errors.push(refuse(
                        "tamber-param-options",
                        format!(
                            "param `{}`: option `{name}` is no token of the design",
                            t.full
                        ),
                    )),
                }
            }
        }
    }
    errors
}

/// The param `name` names among `params`: its full name
/// (`post.contract#PostButton.radius`), or its short one (`PostButton.radius`)
/// where one component of that name declares it; where two do, the short
/// name is refused (`tamber-param-ambiguous`), naming both.
pub fn resolve<'a>(params: &'a [ParamToken], name: &str) -> Result<&'a ParamToken, LowerError> {
    let refuse = |id: &'static str, message: String| LowerError {
        id,
        message,
        span: Span::default(),
    };
    if let Some(p) = params.iter().find(|p| p.full == name) {
        return Ok(p);
    }
    let found: Vec<&ParamToken> = params
        .iter()
        .filter(|p| {
            let component = p
                .identity
                .rsplit_once('#')
                .map_or(p.identity.as_str(), |(_, c)| c);
            format!("{component}.{}", p.param.name) == name
        })
        .collect();
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(refuse(
            "tamber-param-unknown",
            format!("`{name}` is no component's param"),
        )),
        many => Err(refuse(
            "tamber-param-ambiguous",
            format!(
                "`{name}` is a param of more than one component ({}): name it in full",
                many.iter()
                    .map(|p| format!("`{}`", p.full))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )),
    }
}

/// A param's `range`, as `(low, high)`.
pub fn range(p: &ComponentParam) -> Option<Result<(f64, f64), String>> {
    let text = p.attr("range")?.text();
    Some(
        text.split_once("..")
            .and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)))
            .filter(|(a, b)| a <= b)
            .ok_or_else(|| format!("`range=\"{text}\"` is `low..high`, two numbers, low first")),
    )
}

/// A param attribute's number: written as one, or as a string of one.
pub fn number(v: &ParamValue) -> Option<f64> {
    match v {
        ParamValue::Number(n) => Some(*n),
        ParamValue::Str(s) => s.trim().parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_param_is_named_by_its_identity_and_rewritten_as_a_whole_word() {
        let t = token_name("post.contract#PostButton", "radius");
        assert!(
            t.starts_with("param.") && t.ends_with(".PostButton.radius"),
            "{t}"
        );
        assert_ne!(t, token_name("screens/post.contract#PostButton", "radius"));
        let named = |n: &str| (n == "fill").then(|| "T".to_string());
        let mut bad = Vec::new();
        assert_eq!(
            words(
                "color-mix(in oklab, param.fill 88%, x.param.fill) param.fill.y",
                &named,
                &mut bad
            ),
            "color-mix(in oklab, T 88%, x.param.fill) param.fill.y"
        );
        assert!(bad.is_empty());
        words("param.nope", &named, &mut bad);
        assert_eq!(bad, ["nope"]);
    }

    #[test]
    fn two_files_whose_names_hash_alike_are_refused_naming_both() {
        // Two paths with one hash, found by the birthday bound.
        let mut seen = std::collections::HashMap::new();
        let (a, b) = (0..1_000_000u32)
            .find_map(|i| {
                let file = format!("f{i}.contract");
                seen.insert(file_hash(&file), file.clone())
                    .filter(|other| *other != file)
                    .map(|other| (other, file))
            })
            .expect("a collision");
        let token = |file: &str| ParamToken {
            token: token_name(&format!("{file}#C"), "x"),
            full: full_name(&format!("{file}#C"), "x"),
            identity: format!("{file}#C"),
            param: ComponentParam {
                name: "x".into(),
                kind: "length".into(),
                default: "1px".into(),
                default_span: Span::default(),
                attrs: Vec::new(),
                span: Span::default(),
            },
        };
        let e = collisions(&[token(&a), token(&b)]);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].id, "tamber-param-collision");
        assert!(
            e[0].message.contains(&a) && e[0].message.contains(&b),
            "{}",
            e[0].message
        );
        assert!(
            collisions(&[token(&a), token(&a)]).is_empty(),
            "one file twice is one identity"
        );
    }
}
