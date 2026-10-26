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

pub use rewrite::{escaped, owned, reached, rewrite, unused, Owned};

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
    /// The package whose file declares it; none for the app's.
    pub package: Option<String>,
    /// That package's directory, whose `assets/` a literal `icon(svg …)`
    /// default reads (LLP 1121 stage 13); none for the app's.
    pub root: Option<String>,
}

/// Every param of `file`'s components, as tokens, in declaration order:
/// its Contract components', then its design's and its packages' designs'
/// ([`design_tokens`]; a package's named `<package>/<file>#Name`, stage 12
/// B1).
pub fn tokens(file: &File) -> Vec<ParamToken> {
    let designed: Vec<ParamToken> = file
        .design
        .iter()
        .chain(&file.package_designs)
        .flat_map(|d| design_tokens(d).0)
        .collect();
    file.components
        .iter()
        .flat_map(|c| {
            let identity = c.params.identity.clone().unwrap_or_else(|| c.name.clone());
            c.params.list.iter().map(move |p| ParamToken {
                token: token_name(&identity, &p.name),
                full: full_name(&identity, &p.name),
                identity: identity.clone(),
                param: p.clone(),
                package: c.params.package.clone(),
                root: c.params.root.clone(),
            })
        })
        .chain(designed)
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
                package: design.package.clone(),
                root: design.root.clone(),
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
        joined: Vec::new(),
        joined_icons: Vec::new(),
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
    // (a message naming no param builds no index: a literal is most)
    if !message.contains(PREFIX) {
        return message.to_string();
    }
    Shown::new(params).text(message)
}

/// [`shown`]'s index of the params' token names, built once for many
/// messages (a component list's, or a reply's diagnostics: rebuilt per
/// message it was the params x the messages, Astra's round 3).
#[derive(Debug, Clone, Default)]
pub struct Shown {
    full: std::collections::HashMap<String, String>,
    /// Each token name's length, longest first.
    lengths: Vec<usize>,
}

impl Shown {
    /// The index of `params`' token names.
    pub fn new(params: &[ParamToken]) -> Shown {
        let mut full = std::collections::HashMap::new();
        for p in params.iter().rev() {
            full.insert(p.token.clone(), p.full.clone());
        }
        let mut lengths: Vec<usize> = full.keys().map(String::len).collect();
        lengths.sort_unstable_by(|a, b| b.cmp(a));
        lengths.dedup();
        Shown { full, lengths }
    }

    /// `message` with each param's token name written as its full name.
    pub fn text(&self, message: &str) -> String {
        // Only where a token can start (each begins with `PREFIX`), the
        // longest token there, so a name that prefixes another does not
        // split it: the tokens looked up by name, not a replace over the
        // text per param (the checker's work in param counts, Astra's round
        // 2).
        if !message.contains(PREFIX) {
            return message.to_string();
        }
        let mut out = String::with_capacity(message.len());
        let mut at = 0;
        while let Some(found) = message[at..].find(PREFIX) {
            let start = at + found;
            out.push_str(&message[at..start]);
            let hit = self.lengths.iter().find_map(|&n| {
                let end = start + n;
                let word = message.get(start..end)?;
                self.full.get(word).map(|f| (end, f.as_str()))
            });
            match hit {
                Some((end, f)) => {
                    out.push_str(f);
                    at = end;
                }
                None => {
                    out.push_str(PREFIX);
                    at = start + PREFIX.len();
                }
            }
        }
        out.push_str(&message[at..]);
        out
    }
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
/// each token with its line's span) has its range checked as each value is
/// written ([`written_ranges`], which passes that value's span as
/// `written`), and the rest refused at its line.
pub fn folded(
    params: &[ParamToken],
    folding: &exact_tamber::Folding,
    overridden: &[(String, Span)],
    written: Option<Span>,
) -> Vec<LowerError> {
    folded_in(params, &Folded::new(folding), overridden, written)
}

/// A folding's tokens by name, each the first, for [`folded_in`]: made once
/// per folding (the checker's work in param counts: a scan of the tokens per
/// param was params x tokens).
pub struct Folded<'f> {
    tokens: std::collections::HashMap<&'f str, &'f exact_tamber::Checked>,
    icons: std::collections::HashSet<&'f str>,
}

impl<'f> Folded<'f> {
    /// The tokens of `folding`.
    pub fn new(folding: &'f exact_tamber::Folding) -> Self {
        let mut tokens = std::collections::HashMap::new();
        for c in folding.tokens.iter().rev() {
            tokens.insert(c.name.as_str(), c);
        }
        let icons = folding.icons.iter().map(|i| i.name.as_str()).collect();
        Folded { tokens, icons }
    }
}

/// [`folded`] over a folding's tokens found by name.
pub fn folded_in(
    params: &[ParamToken],
    folding: &Folded,
    overridden: &[(String, Span)],
    written: Option<Span>,
) -> Vec<LowerError> {
    use exact_tamber::{Length, Value};
    let mut errors = Vec::new();
    let mut set_by: std::collections::HashMap<&str, &Span> = std::collections::HashMap::new();
    for (token, span) in overridden.iter().rev() {
        set_by.insert(token.as_str(), span);
    }
    for t in params {
        let p = &t.param;
        let set = set_by.get(t.token.as_str()).map(|s| ((), *s));
        let span = written.or(set.map(|(_, s)| *s)).unwrap_or(p.default_span);
        let refuse = |id: &'static str, message: String| LowerError { id, message, span };
        // A value the design sets is checked as written, each at its line
        // ([`written_ranges`]).
        let ranged = written.is_some() || set.is_none();
        if let (true, Some(Ok((low, high)))) = (ranged, range(p)) {
            let Some(token) = folding.tokens.get(t.token.as_str()).copied() else {
                continue;
            };
            for v in &token.values {
                // Over base names the design leaves unset, or one bare (stage
                // 12 B2): each column's px, the first outside said.
                let based = match (p.kind.as_str(), v) {
                    ("length", Value::Length(l @ (Length::Base(_) | Length::Named(_)))) => {
                        crate::scale::ranged(&l.css())
                    }
                    _ => None,
                };
                // One a column doesn't fold: said here too, since the design
                // server checks params without lowering the theme (round 3,
                // Astra), and as what it is, not as a unit.
                if let ("length", Value::Length(l)) = (p.kind.as_str(), v) {
                    if let Some(column) = crate::scale::unfolded(&l.css()) {
                        errors.push(refuse(
                            "tamber-param-range",
                            format!(
                                "param `{}` is `{}`, which doesn't fold to a length on {column}: {}",
                                t.full,
                                v.css(),
                                crate::scale::UNFOLDED
                            ),
                        ));
                        break;
                    }
                }
                if let Some(columns) = based {
                    let out = columns
                        .into_iter()
                        // In the kernel's precision: `0.1 × 16` is 1.6 there.
                        .find(|(_, n)| *n < low as f32 || *n > high as f32);
                    if let Some((column, n)) = out {
                        errors.push(refuse(
                            "tamber-param-range",
                            format!(
                                "param `{}` is `{}`, {} on {column}, outside its range {}..{}",
                                t.full,
                                v.css(),
                                crate::scale::px(n),
                                low,
                                high
                            ),
                        ));
                        break;
                    }
                    continue;
                }
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
                    "icon" => folding.icons.contains(name).then_some("icon"),
                    _ => folding.tokens.get(name).map(|c| c.kind.name()),
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

/// Each value a line of the design's `params` block writes (its own and
/// each `when` line's) that folds outside its param's range in some tuple,
/// or to a length that is no px (`tamber-range-unit`), at that value's line
/// and column: each is folded on its own in its param's place, so a `when`
/// value no tuple picks is checked too, as written, a token's or a `calc()`
/// as well as a literal. A value that does not fold is the fold's to say.
pub fn written_ranges(
    design: &exact_tamber::Design,
    params: &[ParamToken],
    identities: &[String],
    vocab: &exact_tamber::Vocab,
) -> Vec<(u32, u32, LowerError)> {
    let resolver = Resolver::new(params, identities);
    // Each ranged line: its index, its param, and its values.
    let ranged: Vec<(usize, &ParamToken, Vec<&exact_tamber::At<String>>)> = design
        .params
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let t = resolver.resolve(&entry.name.value).ok()?;
            let _ = range(&t.param)?;
            let values = std::iter::once(&entry.value).chain(entry.whens.iter().map(|w| &w.value));
            Some((index, t, values.collect()))
        })
        .collect();
    // The dial readers' stem: the first `exact.probe{n}` that starts no
    // name of the design (a token's, an icon's, a param line's, a dial's or
    // an axis's).
    let taken = |stem: &str| {
        let below = format!("{stem}.");
        design
            .tokens
            .iter()
            .chain(&design.icons)
            .chain(&design.params)
            .map(|t| &t.name.value)
            .chain(design.dials.iter().map(|d| &d.name.value))
            .chain(design.axes.iter().map(|a| &a.name.value))
            .any(|n| n == stem || n.starts_with(&below))
    };
    let stem = (0..)
        .map(|n| format!("exact.probe{n}"))
        .find(|stem| !taken(stem))
        .expect("some stem is free");
    // One value of a line folded in its param's place, the rest of the
    // design as written: the range errors at it.
    let probe = |at: &[(usize, &exact_tamber::At<String>)]| {
        let mut probe = design.clone();
        for (index, v) in at {
            probe.params[*index].value = (*v).clone();
            probe.params[*index].whens.clear();
        }
        // Each dial read once beside them, so a probe that clears the `when`
        // lines reading one doesn't leave it moving no token (Astra's round
        // 2: `tamber-dial-inert` failed the batch, and each value folded
        // alone again).
        if at.len() > 1 {
            for (k, d) in design.dials.iter().enumerate() {
                // (under a stem no name of the design starts with: a reader
                // never takes an authored token's place, Astra's round 3)
                let at = |value: String| exact_tamber::At {
                    value,
                    line: u32::MAX - k as u32,
                    col: 1,
                };
                probe.tokens.push(exact_tamber::Token {
                    name: at(format!("{stem}.dial{k}")),
                    kind: None,
                    value: at(d.name.value.clone()),
                    whens: Vec::new(),
                    meta: Vec::new(),
                });
            }
        }
        add_to(&mut probe, params, identities);
        crate::theme::stand_in_profiles(&mut probe);
        exact_tamber::check_design(probe, vocab).ok()
    };
    let errors = |t: &ParamToken, v: &exact_tamber::At<String>, folding: &Folded| {
        let span = Span {
            line: v.line,
            col: v.col,
            end_col: v.col,
            source_id: 0,
        };
        folded_in(std::slice::from_ref(t), folding, &[], Some(span))
            .into_iter()
            .filter(|e| matches!(e.id, "tamber-param-range" | "tamber-range-unit"))
            .map(|e| (v.line, v.col, e))
            .collect::<Vec<_>>()
    };
    let one = |index: usize, t: &ParamToken, v: &exact_tamber::At<String>| {
        probe(&[(index, v)]).map_or(Vec::new(), |f| errors(t, v, &Folded::new(&f)))
    };
    // A line whose value names a param, or a token that reads one, folds
    // with what that holds, so each of its values is folded alone; the
    // others (literals, dials and tokens reading no param) fold together,
    // each line's k-th value in one probe, since none reads another line
    // (the checker's work in param counts: a fold of the design per value
    // was values x the design).
    // (a design token can read a param in turn, Grok's round 2: the tokens
    // that do, directly or through another, found once from the params'
    // names out, Astra's and Grok's round 3; a dial reads none)
    fn words(text: &str) -> impl Iterator<Item = &str> {
        let word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '#');
        text.split(move |c: char| !word(c))
            .filter(|w| !w.is_empty())
    }
    let param_lines: std::collections::HashSet<&str> = design
        .params
        .iter()
        .map(|e| e.name.value.as_str())
        .collect();
    let param_word = |w: &str| resolver.names_a_param(w) || param_lines.contains(w);
    // Each design token by each word its value and `when` values hold.
    let mut readers: std::collections::HashMap<&str, Vec<usize>> = std::collections::HashMap::new();
    let mut reading: Vec<usize> = Vec::new();
    let mut reads = vec![false; design.tokens.len()];
    for (i, t) in design.tokens.iter().enumerate() {
        let texts = std::iter::once(&t.value.value).chain(t.whens.iter().map(|w| &w.value.value));
        for w in texts.flat_map(|text| words(text)) {
            if param_word(w) {
                if !reads[i] {
                    reads[i] = true;
                    reading.push(i);
                }
            } else {
                readers.entry(w).or_default().push(i);
            }
        }
    }
    while let Some(i) = reading.pop() {
        for &j in readers
            .get(design.tokens[i].name.value.as_str())
            .into_iter()
            .flatten()
        {
            if !reads[j] {
                reads[j] = true;
                reading.push(j);
            }
        }
    }
    let reading_tokens: std::collections::HashSet<&str> = design
        .tokens
        .iter()
        .zip(&reads)
        .filter(|(_, r)| **r)
        .map(|(t, _)| t.name.value.as_str())
        .collect();
    let reads_a_param = |v: &exact_tamber::At<String>| {
        words(&v.value).any(|w| param_word(w) || reading_tokens.contains(w))
    };
    let (alone, together): (Vec<_>, Vec<_>) = ranged
        .iter()
        .partition(|(_, _, values)| values.iter().any(|v| reads_a_param(v)));
    let mut out = Vec::new();
    for (index, t, values) in &alone {
        for v in values {
            out.extend(one(*index, t, v));
        }
    }
    let most = together
        .iter()
        .map(|(_, _, values)| values.len())
        .max()
        .unwrap_or(0);
    for k in 0..most {
        let at: Vec<(usize, &ParamToken, &exact_tamber::At<String>)> = together
            .iter()
            .filter_map(|(index, t, values)| Some((*index, *t, *values.get(k)?)))
            .collect();
        let placed: Vec<(usize, &exact_tamber::At<String>)> =
            at.iter().map(|(i, _, v)| (*i, *v)).collect();
        match probe(&placed) {
            Some(folding) => {
                let folding = Folded::new(&folding);
                at.iter()
                    .for_each(|(_, t, v)| out.extend(errors(t, v, &folding)))
            }
            // One of them does not fold: each alone, as written (a value
            // that does not fold is the fold's to say).
            None => at
                .iter()
                .for_each(|(index, t, v)| out.extend(one(*index, t, v))),
        }
    }
    // In the lines' order, each line's values in order, as before.
    out.sort_by_key(|(line, col, _)| (*line, *col));
    out
}

/// Each line of the design's `params` block the build refuses, at its
/// name: a short name two components share (`tamber-param-ambiguous`),
/// and a second line for one param (`tamber-param-twice`). The compiler and
/// the design server both refuse a design with one.
pub fn entry_errors(
    design: &exact_tamber::Design,
    params: &[ParamToken],
    identities: &[String],
) -> Vec<(u32, u32, LowerError)> {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut out = Vec::new();
    let resolver = Resolver::new(params, identities);
    for entry in &design.params {
        let at = (entry.name.line, entry.name.col);
        match resolver.resolve(&entry.name.value) {
            Ok(p) if seen.contains(p.token.as_str()) => out.push((
                at.0,
                at.1,
                LowerError {
                    id: "tamber-param-twice",
                    message: format!(
                        "`{}`: the design's `params` block already gives `{}` a value; one line a param (its `when` lines under it)",
                        entry.name.value, p.full
                    ),
                    span: Span::default(),
                },
            )),
            Ok(p) => drop(seen.insert(&p.token)),
            Err(e) if e.id == "tamber-param-ambiguous" => out.push((at.0, at.1, e)),
            Err(_) => {}
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
    let resolver = Resolver::new(params, identities);
    design
        .params
        .iter()
        .filter_map(|entry| match resolver.resolve(&entry.name.value) {
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
    Resolver::new(params, identities).resolve(name)
}

/// [`resolve`] for many names over one app's params: the folds, the
/// names' index and the tokens by name made once (the checker's work in
/// param counts: a fold of every param per name was params x names).
pub struct Resolver<'a> {
    folds: exact_tamber::params::Params,
    index: exact_tamber::params::Index,
    by_token: std::collections::HashMap<&'a str, &'a ParamToken>,
}

impl<'a> Resolver<'a> {
    /// The resolver of `params` among the app's components (`identities`).
    pub fn new(params: &'a [ParamToken], identities: &[String]) -> Self {
        let mut by_token = std::collections::HashMap::new();
        for q in params {
            by_token.entry(q.token.as_str()).or_insert(q);
        }
        let folds = folds(params, identities);
        Resolver {
            index: exact_tamber::params::Index::new(&folds),
            folds,
            by_token,
        }
    }

    /// Each param's first line in the design's `params` block, by its token
    /// (one pass, where a scan of the lines per param was params x lines).
    pub fn lines<'d>(
        &self,
        design: &'d exact_tamber::Design,
    ) -> std::collections::HashMap<&'a str, &'d exact_tamber::Token> {
        let mut out = std::collections::HashMap::new();
        for e in &design.params {
            if let Ok(p) = self.resolve(&e.name.value) {
                out.entry(p.token.as_str()).or_insert(e);
            }
        }
        out
    }

    /// Whether `name` names a param, by its full name or a short one
    /// (ambiguous or not): [`Resolver::resolve`]'s question, no error made.
    pub fn names_a_param(&self, name: &str) -> bool {
        use exact_tamber::params::Unnamed;
        self.by_token.contains_key(name)
            || !matches!(self.index.named(&self.folds, name), Err(Unnamed::Unknown))
    }

    /// As [`resolve`] answers.
    pub fn resolve(&self, name: &str) -> Result<&'a ParamToken, LowerError> {
        use exact_tamber::params::{called, Unnamed};
        let refuse = |id: &'static str, message: String| LowerError {
            id,
            message,
            span: Span::default(),
        };
        let folds = &self.folds;
        match self.index.named(folds, name) {
            Ok(p) => Ok(*self
                .by_token
                .get(p.token.as_str())
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
                    called(folds, component)
                        .iter()
                        .map(|o| format!("`{o}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
            }
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
            package: None,
            root: None,
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

    /// `shown` by lookup at each token's start gives what a replace per param,
    /// longest first, gave: tokens that prefix others, repeats, a token twice
    /// in the list, text around and between (Astra's round 2 on the counts).
    #[test]
    fn shown_by_lookup_is_the_replace_per_param() {
        let old = |message: &str, params: &[ParamToken]| {
            let mut out = message.to_string();
            let mut by_len: Vec<&ParamToken> = params.iter().collect();
            by_len.sort_by_key(|p| std::cmp::Reverse(p.token.len()));
            for p in by_len {
                out = out.replace(&p.token, &p.full);
            }
            out
        };
        let token = |identity: &str, name: &str| ParamToken {
            token: token_name(identity, name),
            full: full_name(identity, name),
            identity: identity.into(),
            param: ComponentParam {
                name: name.into(),
                kind: "length".into(),
                default: "1px".into(),
                default_span: Span::default(),
                attrs: Vec::new(),
                span: Span::default(),
            },
            package: None,
            root: None,
        };
        let params = vec![
            token("a.contract#Card", "gap"),
            token("a.contract#Card", "gap2"),
            token("a.contract#Card", "g"),
            token("b.contract#Row", "gap"),
            token("a.contract#Card", "gap"),
        ];
        let t = |i: usize| params[i].token.clone();
        for message in [
            String::from("no params here"),
            format!("`{}` and `{}`", t(0), t(1)),
            format!("{}{}{} param. {}", t(2), t(1), t(0), t(3)),
            format!("calc({} + {}) param.x {}", t(0), t(0), t(2)),
            format!("param.{}", t(3)),
        ] {
            assert_eq!(
                shown(&message, &params),
                old(&message, &params),
                "{message}"
            );
        }
    }
}
