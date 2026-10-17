//! Component params (LLP 1121 D15): a param is a token its component owns.
//! Its full name is the component's identity and the param
//! (`post.contract#PostButton.radius`); below the IR (the reference table,
//! the fold, the live table, `uses`, the JS target's properties) it is a
//! dotted token name derived from that identity,
//! `param.f<file hash>.<Component>.<param>` (theme2's ruling, 2026-10-17).
//! The driver rewrites `param.<name>` where a component's rows take their
//! value, its uses' arguments and its params' defaults to that name before
//! anything reads them, so a param is read, folded, retained and bound as
//! any token is ([`rewrite`]'s module says where).

mod rewrite;

pub use rewrite::{reached, rewrite, unused};

use crate::LowerError;
use contract_syntax::params::{ComponentParam, ParamAttr, ParamValue, ATTRIBUTES};
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

/// Every param of `file`'s components, as tokens, in declaration order:
/// its Contract components', then its design's ([`design_tokens`]).
pub fn tokens(file: &File) -> Vec<ParamToken> {
    let designed = file.design.as_ref().map(|d| design_tokens(d).0);
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
        .chain(designed.into_iter().flatten())
        .collect()
}

/// The params of the design's components that aren't Contract (D15), as
/// tokens: each named by the design file and its component, its
/// `in low..high` a `range` and its metadata a Contract param's attributes;
/// with what is refused in them (an attribute no param admits, a `param.`
/// name its component does not declare). Its declaration makes each a root.
/// None when the design does not parse: its check says why.
pub fn design_tokens(design: &contract_syntax::DesignSource) -> (Vec<ParamToken>, Vec<LowerError>) {
    let Ok(parsed) = exact_tamber::parse(&design.text) else {
        return Default::default();
    };
    let (mut out, mut errors) = (Vec::new(), Vec::new());
    for c in &parsed.components {
        let (component, identity) = (&c.name.value, format!("{}#{}", design.home, c.name.value));
        let declared: Vec<&str> = c.params.iter().map(|p| p.name.value.as_str()).collect();
        let named = |n: &str| declared.contains(&n).then(|| token_name(&identity, n));
        for p in &c.params {
            let span = |at: &exact_tamber::At<String>| Span {
                line: at.line,
                col: at.col,
                end_col: at.col + at.value.len() as u32,
                source_id: design.source_id,
            };
            let mut bad = Vec::new();
            let default = rewrite::plain(&p.value.value, &named, &mut bad);
            for name in bad {
                errors.push(LowerError {
                    id: "tamber-param-unknown",
                    message: format!(
                        "`param.{name}`: `{component}` declares no param `{name}` (its params: {})",
                        declared.join(", ")
                    ),
                    span: span(&p.value),
                });
            }
            let range = p
                .range
                .as_ref()
                .map(|(low, high)| ("range".to_string(), format!("{low}..{high}")));
            let mut attrs = Vec::new();
            // `in low..high` is its range; `range=` is no metadata here.
            for (i, (name, value)) in range.iter().chain(&p.meta).enumerate() {
                let written_in = i == 0 && range.is_some();
                if !ATTRIBUTES.contains(&name.as_str()) || (name == "range" && !written_in) {
                    errors.push(LowerError {
                        id: "syntax-param-attribute",
                        message: format!("param `{}`: `{name}=` is no param attribute here ({}; a range is `in low..high`)", p.name.value, ATTRIBUTES[1..].join(", ")),
                        span: span(&p.name),
                    });
                    continue;
                }
                attrs.push(ParamAttr {
                    name: name.clone(),
                    value: ParamValue::Str(value.clone()),
                    span: span(&p.name),
                });
            }
            out.push(ParamToken {
                token: token_name(&identity, &p.name.value),
                full: full_name(&identity, &p.name.value),
                identity: identity.clone(),
                param: ComponentParam {
                    name: p.name.value.clone(),
                    kind: p.kind.clone(),
                    default,
                    default_span: span(&p.value),
                    attrs,
                    span: span(&p.name),
                },
            });
        }
    }
    (out, errors)
}

/// Every component's identity in `file`, with params or none.
pub fn identities(file: &File) -> Vec<String> {
    file.components
        .iter()
        .map(|c| c.params.identity.clone().unwrap_or_else(|| c.name.clone()))
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

/// `params`, and the identity of every component of the app
/// (`components`), as the design's folds take them.
pub fn folds(params: &[ParamToken], components: &[String]) -> exact_tamber::params::Params {
    exact_tamber::params::Params {
        list: params
            .iter()
            .map(|p| exact_tamber::params::Param {
                token: p.token.clone(),
                full: p.full.clone(),
                kind: p.param.kind.clone(),
                default: p.param.default.clone(),
            })
            .collect(),
        components: components.to_vec(),
    }
}

/// `design` laid out with `params` ([`exact_tamber::params::laid`]): each a
/// token of its kind on line [`LINE`] + its index, the design's `params`
/// lines in their places, icon aliases resolved. `components`: every
/// component's identity, for what a short name names.
pub fn add_to(design: &mut exact_tamber::Design, params: &[ParamToken], components: &[String]) {
    *design = exact_tamber::params::laid(design, &folds(params, components));
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
/// options name tokens of its kind. A param the design sets (`overridden`,
/// each token with its line's span) is refused at that line.
pub fn folded(
    params: &[ParamToken],
    folding: &exact_tamber::Folding,
    overridden: &[(String, Span)],
) -> Vec<LowerError> {
    use exact_tamber::{Length, Value};
    let mut errors = Vec::new();
    for t in params {
        let p = &t.param;
        let span = overridden
            .iter()
            .find(|(token, _)| *token == t.token)
            .map_or(p.default_span, |(_, span)| *span);
        let refuse = |id: &'static str, message: String| LowerError { id, message, span };
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

/// Each literal value a line of the design's `params` block writes (its
/// own and each `when` line's) outside its param's range, at that value's
/// line and column: a `when` value no tuple picks is checked too, as the
/// author wrote it. A value naming tokens is checked as it folds
/// ([`folded`]).
pub fn written_ranges(
    design: &exact_tamber::Design,
    params: &[ParamToken],
    identities: &[String],
) -> Vec<(u32, u32, LowerError)> {
    let mut out = Vec::new();
    for entry in &design.params {
        let Ok(t) = resolve(params, identities, &entry.name.value) else {
            continue;
        };
        let Some(Ok((low, high))) = range(&t.param) else {
            continue;
        };
        let values = std::iter::once(&entry.value).chain(entry.whens.iter().map(|w| &w.value));
        for v in values {
            let text = v.value.trim();
            let n = match t.param.kind.as_str() {
                "length" => text
                    .strip_suffix("px")
                    .and_then(|n| n.trim().parse::<f64>().ok()),
                "duration" => text
                    .strip_suffix("ms")
                    .and_then(|n| n.trim().parse::<f64>().ok())
                    .or_else(|| {
                        text.strip_suffix('s')
                            .and_then(|n| n.trim().parse::<f64>().ok())
                            .map(|s| s * 1000.0)
                    }),
                _ => text.parse::<f64>().ok(),
            };
            if n.is_some_and(|n| n < low || n > high) {
                out.push((
                    v.line,
                    v.col,
                    LowerError {
                        id: "tamber-param-range",
                        message: format!(
                            "param `{}` is set to {text}, outside its range {low}..{high}",
                            t.full
                        ),
                        span: Span::default(),
                    },
                ));
            }
        }
    }
    out
}

/// Each line of the design's `params` block that names no param of the
/// app, as a warning (`tamber-param-unknown`, D15): its declaring file moved
/// or was renamed, or the param was removed, so the value no longer
/// applies. A short name two components share is the build's refusal.
pub fn unknown(
    design: &exact_tamber::Design,
    params: &[ParamToken],
    identities: &[String],
    source_id: u32,
) -> Vec<LowerError> {
    design
        .params
        .iter()
        .filter_map(|entry| match resolve(params, identities, &entry.name.value) {
            Err(e) if e.id == "tamber-param-unknown" => Some(LowerError {
                id: "tamber-param-unknown",
                message: format!(
                    "`{}` in the design's `params` block names no param of this app, so its value applies to nothing: was its declaring file moved or renamed (a new identity, D15)?",
                    entry.name.value
                ),
                span: Span {
                    line: entry.name.line,
                    col: entry.name.col,
                    end_col: entry.name.col + entry.name.value.len() as u32,
                    source_id,
                },
            }),
            _ => None,
        })
        .collect()
}

/// The param `name` names among `params`: its full name
/// (`post.contract#PostButton.radius`), or its short one (`PostButton.radius`)
/// where one component of the app (`identities`, every one's) has that
/// component name; where two do, whichever has the param or neither, the
/// short name is refused (`tamber-param-ambiguous`), naming both. The
/// decision is [`exact_tamber::params::named`]'s, the design server's too.
pub fn resolve<'a>(
    params: &'a [ParamToken],
    identities: &[String],
    name: &str,
) -> Result<&'a ParamToken, LowerError> {
    use exact_tamber::params::{called, named, Unnamed};
    let refuse = |id: &'static str, message: String| LowerError {
        id,
        message,
        span: Span::default(),
    };
    let folds = folds(params, identities);
    match named(&folds, name) {
        Ok(p) => Ok(params
            .iter()
            .find(|q| q.token == p.token)
            .expect("folds are params'")),
        Err(Unnamed::Unknown) => Err(refuse(
            "tamber-param-unknown",
            format!("`{name}` is no component's param"),
        )),
        Err(Unnamed::Ambiguous) => {
            let component = name.rsplit_once('.').map_or(name, |(c, _)| c);
            Err(refuse(
                "tamber-param-ambiguous",
                format!(
                    "`{name}`: more than one component is called `{component}` ({}): name the param in full",
                    called(&folds, component)
                        .iter()
                        .map(|o| format!("`{o}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
        }
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
