//! The IR's `components` block (LLP 1121 D15, stage 7 A): each component of
//! the app that declares params, by its identity, and each param by its full
//! name, with its short name where it is unambiguous, its token name below
//! the IR, its kind, its default as written, its value per tuple, and the
//! editor's metadata. `describe` and `contract tamber ir` return it beside
//! the design's IR. Read from the app's Contract, loaded as a build loads it
//! (the same identities and token names).

use contract_lower::params::{self, ParamToken};
use contract_syntax::params::ParamValue;
use serde_json::{json, Value};
use std::path::Path;

/// The app's params, and the identity of every component it has, with
/// params or none (a short name is ambiguous when two share a name).
#[derive(Debug, Clone, Default)]
pub struct AppParams {
    /// Every param, as a token.
    pub tokens: Vec<ParamToken>,
    /// Each component that names parts, by identity and name, with what the
    /// Tamber IR says of its anatomy (LLP 1139 D5; the IR file only).
    pub library: Vec<(String, String, contract_lower::anatomy::Described)>,
    /// Every component's identity.
    pub identities: Vec<String>,
    /// Each source's file, by source id, as a diagnostic names it: relative
    /// to the app's directory where it is inside it.
    pub files: Vec<String>,
    /// Each source's path on this machine, by source id: what is read (an
    /// authored default's text), never what an answer says.
    pub paths: Vec<std::path::PathBuf>,
    /// The app's Contract did not load (the build says why): its params are
    /// unknown, not none.
    pub failed: bool,
    /// The designs of the packages it links (LLP 1121 D11), which a fold of
    /// the app's design joins as the build does.
    pub packages: Vec<contract_syntax::DesignSource>,
    /// Its tokens are an inline `theme` in its Contract, no design file.
    pub inline: bool,
    /// Each package root its load resolved (LLP 1121 D11), real: direct,
    /// hoisted or vendored, what the server never writes into.
    pub package_roots: Vec<std::path::PathBuf>,
    /// `tokens`' names in full, indexed on first use (`shown`).
    pub shown_index: ShownIndex,
}

/// [`AppParams::shown`]'s index, made from `tokens` on first use: what it
/// derives, never what it is (its `Debug` says nothing, so a kept load
/// prints as a cold one, the walk's check of the params kept).
#[derive(Clone, Default)]
pub struct ShownIndex(std::sync::OnceLock<params::Shown>);

impl std::fmt::Debug for ShownIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ShownIndex")
    }
}

impl AppParams {
    /// `message` with each param's token name written as its full name
    /// ([`params::shown`]), the index built once for the app's params.
    pub fn shown(&self, message: &str) -> String {
        self.shown_index
            .0
            .get_or_init(|| params::Shown::new(&self.tokens))
            .text(message)
    }
}

/// The app's params, loaded from `app_contract`; none when it has no
/// Contract, or it does not load (the build says why).
pub fn params_of(app_contract: &Path) -> AppParams {
    // None when it has no Contract; one there that can't be read is a load that failed (its
    // packages unknown: the server writes no package-looking file, LLP 1121 D11).
    let src = match std::fs::read_to_string(app_contract) {
        Ok(src) => src,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return AppParams::default(),
        Err(_) => {
            return AppParams {
                failed: true,
                ..AppParams::default()
            }
        }
    };
    let Ok(root) = crate::app_root(app_contract) else {
        return AppParams {
            failed: true,
            ..AppParams::default()
        };
    };
    let dir = app_contract.parent().unwrap_or(Path::new("."));
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    match crate::sources::load(app_contract, &src, &root) {
        Ok((file, sources)) => AppParams {
            tokens: params::tokens(&file),
            identities: params::identities(&file),
            library: file
                .components
                .iter()
                .filter(|c| {
                    !c.anatomy.parts.is_empty()
                        || c.params
                            .identity
                            .as_ref()
                            .is_some_and(|i| file.library.contains(i))
                })
                .map(|c| {
                    (
                        c.params.identity.clone().unwrap_or_else(|| c.name.clone()),
                        c.name.clone(),
                        contract_lower::anatomy::described(c, &file),
                    )
                })
                .collect(),
            packages: file.package_designs.clone(),
            inline: file.theme.is_some(),
            package_roots: crate::sources::packages::packages(&sources)
                .0
                .into_iter()
                .filter_map(|p| p.root.canonicalize().ok())
                .collect(),
            files: sources
                .paths()
                .iter()
                // As every answer names a file (`tamber_uses::shown`): never
                // with this machine's directories.
                .map(|p| crate::tamber_uses::shown(p, &dir))
                .collect(),
            paths: sources.paths().to_vec(),
            failed: false,
            shown_index: Default::default(),
        },
        // Its packages' roots as far as it loaded (a source past them can't be read): still the server's
        // never to write into (LLP 1121 D11).
        Err(_) => AppParams {
            failed: true,
            package_roots: crate::source_graph(app_contract)
                .packages
                .into_iter()
                .filter_map(|p| p.root.canonicalize().ok())
                .collect(),
            ..AppParams::default()
        },
    }
}

/// Diagnostic `d` of a design folded with the app's params where an author
/// finds it: one of a param's (on [`params::LINE`] and after) at its
/// default, in its declaring file (its path, by the default's source),
/// which is returned; every name in full.
pub fn relocate(d: &mut exact_tamber::Diagnostic, app: &AppParams) -> Option<String> {
    d.message = contract_lower::package_unband(&app.shown(&d.message));
    // A package token's (D11): at its line in its package's design.
    if let Some((band, line)) = contract_lower::package_line(d.line) {
        let source = app.packages.get(band)?;
        d.line = line;
        return Some(
            app.files
                .get(source.source_id as usize)
                .cloned()
                .unwrap_or_else(|| source.home.clone()),
        );
    }
    let p = app.tokens.get(d.line.checked_sub(params::LINE)? as usize)?;
    d.line = p.param.default_span.line;
    d.col = p.param.default_span.col;
    let id = p.param.default_span.source_id as usize;
    Some(app.files.get(id).cloned().unwrap_or_else(|| {
        p.identity
            .rsplit_once('#')
            .map_or(p.identity.clone(), |(f, _)| f.to_string())
    }))
}

/// [`relocate`]d, as a line a terminal prints: `file:line:col: id: message`,
/// the design's file `design` unless a param's.
pub fn located(d: &exact_tamber::Diagnostic, app: &AppParams, design: &Path) -> String {
    let mut d = d.clone();
    match relocate(&mut d, app) {
        Some(file) => format!("{file}:{d}"),
        None => format!("{}:{d}", design.display()),
    }
}

/// `design` laid out with the app's params as the build lays it (D15): each
/// a token, each of its `params` lines in its param's place; and its
/// packages' tokens joined, as the build joins them (LLP 1121 D11).
pub fn laid(design: &exact_tamber::Design, app: &AppParams) -> exact_tamber::Design {
    let mut out = contract_lower::with_package_designs(Some(design.clone()), &app.packages)
        .unwrap_or_else(|| design.clone());
    params::add_to(&mut out, &app.tokens, &app.identities);
    out
}

/// `text`, a design, folded with the app's params, its `params` lines in
/// their places, and the design as parsed, without them; a refusal as the
/// fold gives it ([`relocate`] it), and one for a `params` line the build
/// refuses.
pub fn fold_with(
    text: &str,
    app: &AppParams,
    vocab: &exact_tamber::Vocab,
) -> Result<(exact_tamber::Folding, exact_tamber::Design), Vec<exact_tamber::Diagnostic>> {
    let design = exact_tamber::parse(text)?;
    // What the build refuses of the packages' designs, as it refuses it.
    let refused = contract_lower::join_package_designs(Some(design.clone()), &app.packages).1;
    if !refused.is_empty() {
        return Err(refused.into_iter().map(|e| packaged(e, app)).collect());
    }
    // (an unchanged design answered from the last few checked: a client
    // asking again can't make the server fold it, dial influence and all,
    // each time; theme2's condition on the counts)
    let folding = exact_tamber::check_design_cached(laid(&design, app), vocab)?;
    let wrong =
        contract_lower::package_references(&folding, &app.packages, &app.tokens, &app.identities);
    if !wrong.is_empty() {
        return Err(wrong.into_iter().map(|e| packaged(e, app)).collect());
    }
    // A `params` line the build refuses (ambiguous, or a param's second).
    let refused: Vec<exact_tamber::Diagnostic> =
        params::entry_errors(&design, &app.tokens, &app.identities)
            .into_iter()
            .map(|(line, col, e)| exact_tamber::Diagnostic {
                id: e.id,
                message: e.message,
                line,
                col,
            })
            .collect();
    if !refused.is_empty() {
        return Err(refused);
    }
    Ok((folding, design))
}

/// A refusal of a package's design as a diagnostic of the joined design:
/// its line in its package's band, which [`relocate`] says at its file.
fn packaged(e: contract_lower::LowerError, app: &AppParams) -> exact_tamber::Diagnostic {
    let band = app
        .packages
        .iter()
        .position(|p| p.source_id == e.span.source_id);
    // Banded only where the band is one (a design past the bands' limit is
    // refused at its own line, named in the message).
    let encoded = band
        .map(|b| (b, contract_lower::package_band_line(b, e.span.line)))
        .filter(|(b, line)| contract_lower::package_line(*line).map(|(k, _)| k) == Some(*b));
    let message = match (band, encoded) {
        (Some(b), None) => format!("{}: {}", app.packages[b].home, e.message),
        _ => e.message,
    };
    exact_tamber::Diagnostic {
        id: e.id,
        message,
        line: encoded.map_or(e.span.line, |(_, line)| line),
        col: e.span.col,
    }
}

/// The package tokens a fold of the app's design joins (LLP 1121 D11), as
/// the build numbers them: what the design server's plans fold beside it;
/// and the package icons beside them (stage 13), which an app icon may name.
pub fn joined_all(app: &AppParams) -> (Vec<exact_tamber::Token>, Vec<exact_tamber::Token>) {
    contract_lower::with_package_designs(None, &app.packages)
        .map_or_else(Default::default, |d| (d.tokens, d.icons))
}

/// The words of `text` a token name may be.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
        .filter(|w| !w.is_empty())
}

/// The names `text` reads that are tokens or icons of `folding`, and every
/// one those read in turn: a token's names, and an icon's alias as the
/// design writes it (`design`, before aliases are resolved), a param's by
/// its value (the design's `params` line, else its default). The lookups
/// made once ([`Reach`]; the checker's work in param counts).
fn reads(text: &str, reach: &Reach) -> Vec<String> {
    let Some(f) = reach.folding else {
        return Vec::new();
    };
    let known = |w: &str| reach.tokens.contains_key(w) || reach.icons.contains(w);
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<String> = words(text)
        .filter(|w| known(w) && seen.insert(w.to_string()))
        .map(str::to_string)
        .collect();
    let _ = f;
    let mut i = 0;
    while i < out.len() {
        let name = out[i].clone();
        let mut next: Vec<String> = reach
            .tokens
            .get(name.as_str())
            .map_or(Vec::new(), |names| names.to_vec());
        if let Some(value) = reach.aliases.get(name.as_str()) {
            next.extend(words(value).map(str::to_string));
        }
        if let Some(value) = reach.params.get(name.as_str()) {
            next.extend(words(value).map(str::to_string));
        }
        for n in next {
            if known(&n) && seen.insert(n.clone()) {
                out.push(n);
            }
        }
        i += 1;
    }
    out
}

/// What [`reads`] looks up, by name, made once: the folding's tokens (their
/// names) and icons, the design's icons' values, and each param's value
/// (its `params` line's, else its default).
struct Reach<'a> {
    folding: Option<&'a exact_tamber::Folding>,
    tokens: std::collections::HashMap<&'a str, &'a [String]>,
    icons: std::collections::HashSet<&'a str>,
    aliases: std::collections::HashMap<&'a str, &'a str>,
    params: std::collections::HashMap<&'a str, &'a str>,
}

impl<'a> Reach<'a> {
    fn new(
        folding: Option<&'a exact_tamber::Folding>,
        design: &'a exact_tamber::Design,
        app: &'a AppParams,
        lines: &std::collections::HashMap<&'a str, &'a exact_tamber::Token>,
    ) -> Self {
        let mut tokens = std::collections::HashMap::new();
        let mut icons = std::collections::HashSet::new();
        for t in folding.iter().flat_map(|f| f.tokens.iter().rev()) {
            tokens.insert(t.name.as_str(), t.names.as_slice());
        }
        for i in folding.iter().flat_map(|f| f.icons.iter()) {
            icons.insert(i.name.as_str());
        }
        let mut aliases = std::collections::HashMap::new();
        for t in design.icons.iter().rev() {
            aliases.insert(t.name.value.as_str(), t.value.value.as_str());
        }
        let mut params = std::collections::HashMap::new();
        for p in app.tokens.iter().rev() {
            let value = lines
                .get(p.token.as_str())
                .map_or(p.param.default.as_str(), |e| e.value.value.as_str());
            params.insert(p.token.as_str(), value);
        }
        Reach {
            folding,
            tokens,
            icons,
            aliases,
            params,
        }
    }
}

/// The IR's tokens with every param's left out: a tool reads params by
/// full name, in `components`, never by their names below the IR.
pub fn hide(ir: &mut Value) {
    for list in ["tokens", "icons"] {
        if let Some(items) = ir.get_mut(list).and_then(Value::as_array_mut) {
            items.retain(|t| {
                !t["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with(params::PREFIX))
            });
        }
    }
}

/// A param's short name, `Component.param`, when no other component of
/// that name is in the app (`identities`, with params or none, and every
/// param's own: a design's and a package design's components, which
/// `resolve` reads too; stage 12 B1 round 1), else none.
fn short(p: &ParamToken, sharing: &std::collections::HashMap<String, usize>) -> Option<String> {
    let name = component(&p.identity);
    (sharing.get(&name) == Some(&1)).then(|| format!("{name}.{}", p.param.name))
}

/// A component's name in its identity.
fn component(identity: &str) -> String {
    identity
        .rsplit_once('#')
        .map_or(identity.to_string(), |(_, c)| c.to_string())
}

/// How many distinct identities (the app's components', and the params')
/// have each component name, counted once ([`short`]).
fn sharing(identities: &[String], all: &[ParamToken]) -> std::collections::HashMap<String, usize> {
    let distinct: std::collections::BTreeSet<&str> = identities
        .iter()
        .map(String::as_str)
        .chain(all.iter().map(|q| q.identity.as_str()))
        .collect();
    let mut out = std::collections::HashMap::new();
    for identity in distinct {
        *out.entry(component(identity)).or_insert(0) += 1;
    }
    out
}

/// The IR's `library` block (LLP 1139 D5): each component that names parts,
/// its parts with their groups and slot, and its variant props.
pub fn library(app: &AppParams, design: Option<&exact_tamber::Design>) -> Value {
    // The tokens each chain row's value reaches, folded with the app's
    // design and its packages' (what the server re-checks on a write).
    let folding =
        contract_lower::with_package_designs(design.cloned(), &app.packages).and_then(|d| {
            exact_tamber::check_design_cached(laid(&d, app), &contract_lower::tamber_vocab()).ok()
        });
    let reaches = |value: &str| -> Vec<String> {
        let value = value.trim();
        // A base name the design leaves unset, or an expression over base
        // names: the base names it reads.
        let bases = || {
            exact_kernel::style::scale::named_in(value)
                .into_iter()
                .map(str::to_string)
                .collect()
        };
        let Some(f) = &folding else { return bases() };
        let reached = |name: &str| {
            std::iter::once(name.to_string()).chain(contract_lower::anatomy::slot::reach(f, name))
        };
        match f.tokens.iter().any(|t| t.name == value) {
            // The token and all it reaches, transitively, base names too.
            true => reached(value).collect(),
            // An expression: each token it names and all they reach, and its
            // base names.
            false => {
                let mut all: Vec<String> = bases();
                for w in value
                    .split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
                    .filter(|w| f.tokens.iter().any(|t| t.name == *w))
                {
                    for r in reached(w) {
                        if !all.contains(&r) {
                            all.push(r);
                        }
                    }
                }
                all
            }
        }
    };
    Value::Array(
        app.library
            .iter()
            .map(|(identity, name, d)| {
                let chain: Vec<Value> = d
                    .chain
                    .iter()
                    .map(|(tag, rows)| {
                        let mut tokens: Vec<String> = Vec::new();
                        for (_, v) in rows {
                            for t in reaches(v) {
                                if !tokens.contains(&t) {
                                    tokens.push(t);
                                }
                            }
                        }
                        json!({"tag": tag, "rows": rows.iter().map(|(r, v)| json!([r, v])).collect::<Vec<_>>(), "tokens": tokens})
                    })
                    .collect();
                json!({
                    "identity": identity,
                    "name": name,
                    "parts": d.parts.iter().map(|(n, g, slot)| json!({"name": n, "groups": g, "slot": slot})).collect::<Vec<_>>(),
                    "variants": d.variants.iter().map(|(p, o)| json!({"prop": p, "options": o})).collect::<Vec<_>>(),
                    "library": d.library,
                    "chain": chain,
                })
            })
            .collect(),
    )
}

/// The `components` block for `all`, each param's value per tuple folded
/// with `design` (the app's, or none).
pub fn components(
    app: &AppParams,
    design: Option<&exact_tamber::Design>,
    vocab: &exact_tamber::Vocab,
) -> Value {
    let all = app.tokens.as_slice();
    let design = design.cloned().unwrap_or_default();
    let folding = exact_tamber::check_design_cached(laid(&design, app), vocab).ok();
    // What each param's default reaches (stage 12 B2-levels, *1.15*).
    let designs = crate::tamber_levels::Designs::parsed("", Some(design.clone()), app, vocab);
    let mut out: Vec<Value> = Vec::new();
    // Each component's params, in first-seen order; the params' lines, the
    // folded tokens and icons and what reads reach, each found by name once
    // (the checker's work in param counts).
    let mut groups: Vec<(&str, Vec<&ParamToken>)> = Vec::new();
    let mut group_of: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for q in all {
        let g = *group_of.entry(q.identity.as_str()).or_insert_with(|| {
            groups.push((q.identity.as_str(), Vec::new()));
            groups.len() - 1
        });
        groups[g].1.push(q);
    }
    let resolver = params::Resolver::new(all, &app.identities);
    let lines = resolver.lines(&design);
    let reach = Reach::new(folding.as_ref(), &design, app, &lines);
    let mut folded_tokens = std::collections::HashMap::new();
    let mut folded_icons = std::collections::HashMap::new();
    for t in folding.iter().flat_map(|f| f.tokens.iter().rev()) {
        folded_tokens.insert(t.name.as_str(), t);
    }
    for i in folding.iter().flat_map(|f| f.icons.iter().rev()) {
        folded_icons.insert(i.name.as_str(), i);
    }
    let sharing = sharing(&app.identities, all);
    for (identity, members) in &groups {
        let list: Vec<Value> = members
            .iter()
            .copied()
            .map(|q| {
                let attr = |name: &str| q.param.attr(name).map(ParamValue::text);
                // The app's value, its `params` line; a literal there in
                // place of a default that names tokens no longer follows them
                // (D15, marked as D14 marks a pinned dial override).
                let set = lines.get(q.token.as_str()).copied();
                // Each token or icon the default names that the set value
                // and its `when` values no longer read, directly or through
                // another token.
                // Each token or icon the default names that a value set
                // (the line's own, or a `when` line's) no longer reads,
                // directly or through another: D14's mark, per branch.
                let pinned: Vec<String> = match set {
                    Some(e) => {
                        let direct: Vec<String> = reads(&q.param.default, &reach)
                            .into_iter()
                            .filter(|w| words(&q.param.default).any(|d| d == w))
                            .collect();
                        let branches = std::iter::once(&e.value.value).chain(e.whens.iter().map(|w| &w.value.value));
                        let mut lost: Vec<String> = Vec::new();
                        for b in branches {
                            let kept = reads(b, &reach);
                            for w in direct.iter().filter(|w| !kept.contains(w)) {
                                if !lost.contains(w) {
                                    lost.push(w.clone());
                                }
                            }
                        }
                        lost.iter().map(|w| app.shown(w)).collect()
                    }
                    None => Vec::new(),
                };
                let num = |name: &str| q.param.attr(name).and_then(params::number);
                // An icon's sources, the same in every tuple.
                let values: Option<Value> = folding.as_ref().and_then(|f| match q.param.kind.as_str() {
                    "icon" => {
                        let i = folded_icons.get(q.token.as_str())?;
                        let one = json!({"sf": i.sf, "material": i.material, "svg": i.svg});
                        Some(json!(vec![one; f.tuples]))
                    }
                    _ => {
                        let t = folded_tokens.get(q.token.as_str())?;
                        Some(json!(t.values.iter().map(|v| v.css()).collect::<Vec<_>>()))
                    }
                });
                let mut row = json!({
                    "name": q.full,
                    "short": short(q, &sharing),
                    "param": q.param.name,
                    "token": q.token,
                    "kind": q.param.kind,
                    "default": app.shown(&q.param.default),
                    "values": values,
                    "set": set.map(|e| json!({"value": app.shown(&e.value.value), "line": e.name.line, "whens": e.whens.len()})),
                    "pinned": pinned,
                    "range": params::range(&q.param).and_then(Result::ok).map(|(lo, hi)| json!([lo, hi])),
                    "step": num("step"),
                    "control": attr("control").unwrap_or_else(|| "token".into()),
                    "options": attr("options").map(|o| app.shown(&o).split('|').map(|s| s.trim().to_string()).collect::<Vec<_>>()),
                    "label": attr("label"),
                    "help": attr("help"),
                    "group": attr("group"),
                    "priority": num("priority").unwrap_or(2.0),
                });
                if let Some(reaches) = designs.reaches(q) {
                    row["reaches"] = reaches;
                }
                row
            })
            .collect();
        let name = identity.rsplit_once('#').map_or(*identity, |(_, c)| c);
        out.push(json!({"identity": identity, "name": name, "params": list}));
    }
    // An exported library component's parts and variant props (LLP 1139 D5,
    // stage 12 D3, *1.15*), from C's IR, with params or without (one with
    // none listed with `params: []`).
    for (identity, _, d) in app.library.iter().filter(|(_, _, d)| d.library) {
        let at = match out.iter().position(|c| c["identity"] == identity.as_str()) {
            Some(i) => i,
            None => {
                // (named from its identity, as one with params is: never the compiler's renamed `Badge__index`)
                let name = identity
                    .rsplit_once('#')
                    .map_or(identity.as_str(), |(_, c)| c);
                out.push(json!({"identity": identity, "name": name, "params": []}));
                out.len() - 1
            }
        };
        out[at]["library"] = json!(true);
        out[at]["parts"] = json!(d
            .parts
            .iter()
            .map(|(n, g, slot)| json!({"name": n, "groups": g, "slot": slot}))
            .collect::<Vec<_>>());
        out[at]["variants"] = json!(d
            .variants
            .iter()
            .map(|(p, o)| json!({"prop": p, "options": o}))
            .collect::<Vec<_>>());
    }
    Value::Array(out)
}
