//! Component params (LLP 1121 D15): a param is a token its component owns.
//! Its full name is the component's identity and the param
//! (`post.contract#PostButton.radius`); below the IR (the reference table,
//! the fold, the live table, `uses`, the JS target's properties) it is a
//! dotted token name derived from that identity,
//! `param.f<file hash>.<Component>.<param>` (theme2's ruling, 2026-10-17).
//! The driver rewrites `param.<name>` in a component's rows, its uses'
//! arguments and its params' defaults to that name before anything reads
//! them, so a param is read, folded, retained and bound as any token is.

mod rewrite;

pub use rewrite::{rewrite, unused};

use crate::LowerError;
use contract_syntax::params::{ComponentParam, ParamValue};
use contract_syntax::{File, Span};

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
                message: format!("`{file}` and `{other}` would give their params one token name below the IR: rename either file"),
                span: p.param.span,
            }),
            None => seen.push((hash, file, p.param.span)),
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
    // An icon param whose default names another icon (a param's, or the
    // design's) takes that icon's sources: the icons block folds literals.
    for _ in 0..params.len() {
        let mut changed = false;
        for i in 0..design.icons.len() {
            let name = design.icons[i].value.value.trim().to_string();
            if name.contains('(') {
                continue;
            }
            if let Some(src) = design.icons.iter().find(|t| t.name.value == name) {
                let value = src.value.value.clone();
                design.icons[i].value.value = value;
                changed = true;
            }
        }
        if !changed {
            break;
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
    // A short name is a component's: two components of that name (two
    // identities) make it ambiguous, whichever of them has the param.
    let component = |p: &ParamToken| {
        p.identity
            .rsplit_once('#')
            .map_or(p.identity.clone(), |(_, c)| c.to_string())
    };
    let found: Vec<&ParamToken> = params
        .iter()
        .filter(|p| format!("{}.{}", component(p), p.param.name) == name)
        .collect();
    if let Some(p) = found.first() {
        let mut owners: Vec<&str> = params
            .iter()
            .filter(|q| component(q) == component(p))
            .map(|q| q.identity.as_str())
            .collect();
        owners.dedup();
        owners.sort_unstable();
        owners.dedup();
        if owners.len() > 1 {
            return Err(refuse(
                "tamber-param-ambiguous",
                format!(
                    "`{name}`: more than one component is called `{}` ({}): name the param in full",
                    component(p),
                    owners
                        .iter()
                        .map(|o| format!("`{o}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
    }
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
    fn a_param_is_named_by_its_identity() {
        let t = token_name("post.contract#PostButton", "radius");
        assert!(
            t.starts_with("param.f") && t.ends_with(".PostButton.radius"),
            "{t}"
        );
        assert_ne!(t, token_name("screens/post.contract#PostButton", "radius"));
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
