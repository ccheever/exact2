//! The checker (LLP 1121 D3–D5, D7): names, kinds, ranges, cycles, the
//! mode key, the `under scheme=dark` rules and completeness, folding every
//! token for every full app tuple as it goes.

#[cfg(test)]
mod influence_tests;
mod pairs;

use crate::bound::Walk;
use crate::fold::{self, FoldError, Folded, Half, Named, Names};
use crate::measure::{self, Kind, Length, Value};
use crate::syntax::{role_spelling, At, Design, DialKind, Token};
use crate::{css, Diagnostic, Vocab};
use exact_color::Rgba8;
use std::collections::{BTreeSet, HashMap, HashSet};

/// A design's tuples are capped (LLP 1121 D5).
pub const MAX_TUPLES: usize = 64;

/// A checked token: its values, one per full app tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    /// The token's name as the design writes it, or the role it overrides
    /// as a row writes it (`-exact-secondary-label`, `Canvas`).
    pub name: String,
    /// The role it overrides, by the schema's name, if it does.
    pub role: Option<String>,
    /// Its kind (D3): written, or inferred from its value.
    pub kind: Kind,
    /// Its value for each full app tuple, in tuple order.
    pub values: Vec<Value>,
    /// The dials it depends on, in declaration order.
    pub dials: Vec<String>,
    /// The dials its own values name, in declaration order: what it reads
    /// itself, where `dials` is every dial its value moves with, through
    /// the tokens it names too.
    pub reads: Vec<String>,
    /// The tokens its values name, the default's and every `when` line's,
    /// by their names here, in declaration order.
    pub names: Vec<String>,
    /// The base names its own values read unset, bare (a length, a type's
    /// size), the default's and every `when` line's, a value no tuple
    /// selects included, first time each (stage 11 D's `bases`).
    pub bases: Vec<String>,
    /// What its value names in each full app tuple, as it folds there
    /// (stage 13 D4, `explain`'s `bases_by_tuple`), in a [`crate::traced`] check;
    /// empty when no value names anything, and in any other check.
    pub by_tuple: crate::ByTuple,
    /// Marked `scheme=fixed`: one colour in every scheme by design, which the
    /// one-scheme warning leaves alone.
    pub fixed: bool,
    /// What it is drawn on (`pair=`, D3): colour tokens by their names here,
    /// or roles by their rows, in the order written.
    pub pairs: Vec<String>,
}

/// A dial's folded default.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DialValue {
    Color(Folded),
    Number(f64),
    Length(f64),
}

struct Checker<'a> {
    design: &'a Design,
    errors: Vec<Diagnostic>,
    /// Token index by every name it answers to.
    names: HashMap<String, usize>,
    /// Role by every spelling a value may use, to the row's spelling.
    roles: HashMap<String, (String, String)>,
    dials: HashMap<&'a str, (usize, DialValue)>,
    /// Axis index and value index by name.
    axes: HashMap<&'a str, (usize, HashMap<&'a str, usize>)>,
    radix: Vec<usize>,
    /// Each tuple's [`Checker::digits`], made once: axes are many where each
    /// has one value.
    table: Vec<std::rc::Rc<[usize]>>,
    /// Folded (token, tuple), or `None` when it refused.
    memo: HashMap<(usize, usize), Option<Value>>,
    /// Per token, its kind: written, or inferred from its value.
    kinds: Vec<Kind>,
    /// Whether a word is a named length the kernel reads (a text style,
    /// `-exact-hairline`).
    named_length: &'a dyn Fn(&str) -> bool,
    /// The vocabulary's base names (`Vocab::bases`).
    bases: &'a [String],
    used: Vec<BTreeSet<usize>>,
    /// Per token, the tokens its values named as they folded.
    refs: Vec<BTreeSet<usize>>,
    /// Per token, the unset base names its values read, in fold order.
    based: Vec<Vec<String>>,
    /// Each token's dials its own values name, by index.
    reads: Vec<BTreeSet<usize>>,
    stack: Vec<usize>,
    /// Whether it records [`Checked::by_tuple`] ([`crate::traced`]).
    traced: bool,
    /// The tuple each frame of `stack` folds a token's value at, `None`
    /// where it checks a value no tuple selects (`unselected`), or where
    /// it records none.
    at: Vec<Option<usize>>,
    /// Per token and tuple, the tokens and unset base names its value
    /// there named as it folded ([`Checked::by_tuple`]).
    tuple_refs: HashMap<(usize, usize), (Vec<u32>, Vec<String>)>,
    /// The icons' names (D10), each a row's `symbol:`, no value's.
    icons: HashSet<&'a str>,
    /// Tokens refused as declared, which are not folded.
    refused: Vec<bool>,
    /// Names of refused tokens and dials: a use says nothing more.
    refused_names: HashSet<String>,
    /// Per token, the values (`None` the default, else a `when`) some
    /// tuple selected.
    selected: Vec<BTreeSet<Option<usize>>>,
}

/// The checked tokens, the tuple count and the dials' values, or the diagnostics.
type CheckResult = Result<(Vec<Checked>, usize, Vec<DialValue>), (Vec<Diagnostic>, Vec<Kind>)>;

/// Check `design` against the schema's vocabulary.
pub(crate) fn check(design: &Design, vocab: &Vocab) -> CheckResult {
    // A base name the design leaves unset, bare, is a named length too:
    // kept as written, each host's (stage 11 B). One the design sets is its
    // token, folded as any is, so its value wins everywhere and its edits
    // re-fold what reads it.
    let unset_bases: Vec<&str> = vocab
        .bases
        .iter()
        .map(String::as_str)
        .filter(|b| !design.tokens.iter().any(|t| t.name.value == *b))
        .collect();
    let unset = |w: &str| unset_bases.contains(&w);
    let named_length = |w: &str| {
        vocab
            .named_lengths
            .iter()
            .any(|n| n.eq_ignore_ascii_case(w))
            || unset(w)
    };
    let mut c = Checker {
        traced: crate::trace::on(),
        design,
        errors: Vec::new(),
        names: HashMap::new(),
        roles: HashMap::new(),
        dials: HashMap::new(),
        axes: HashMap::new(),
        radix: Vec::new(),
        table: Vec::new(),
        memo: HashMap::new(),
        kinds: vec![Kind::Color; design.tokens.len()],
        named_length: &named_length,
        bases: &vocab.bases,
        used: vec![BTreeSet::new(); design.tokens.len()],
        refs: vec![BTreeSet::new(); design.tokens.len()],
        based: vec![Vec::new(); design.tokens.len()],
        reads: vec![BTreeSet::new(); design.tokens.len()],
        stack: Vec::new(),
        at: Vec::new(),
        tuple_refs: HashMap::new(),
        icons: design.icons.iter().map(|i| i.name.value.as_str()).collect(),
        refused: vec![false; design.tokens.len()],
        refused_names: HashSet::new(),
        selected: vec![BTreeSet::new(); design.tokens.len()],
    };
    for r in &vocab.roles {
        let row = if r.system {
            r.name.clone()
        } else {
            format!("-exact-{}", r.name)
        };
        c.roles
            .insert(row.to_ascii_lowercase(), (r.name.clone(), row));
    }
    let tuples = c.axes();
    c.dials();
    c.declare();
    // A chain of names past the bound, met inferring kinds: refused there,
    // and nothing folded (each fold would follow it again).
    if let Some(i) = c.kinds() {
        let (id, why) = crate::bound::too_deep();
        c.at(&design.tokens[i].name, id, why);
        let kinds = c.kinds.clone();
        return Err((c.finish(), kinds));
    }
    let Some(tuples) = tuples else {
        let kinds = c.kinds.clone();
        return Err((c.finish(), kinds));
    };
    let mut out = Vec::new();
    for (i, t) in design.tokens.iter().enumerate() {
        c.whens(t);
        if c.refused[i] {
            continue;
        }
        let mut values = Vec::new();
        for tuple in 0..tuples {
            if let Some(v) = c.value(i, tuple) {
                values.push(v);
            }
        }
        c.unselected(i);
        let fixed = c.scheme(t, &values);
        let pairs = c.pairs(i, t);
        let role = c.role_key(&t.name.value).map(|(name, _)| name);
        out.push((
            i,
            Checked {
                name: c
                    .role_key(&t.name.value)
                    .map_or(t.name.value.clone(), |(_, row)| row),
                role,
                kind: c.kinds[i],
                values,
                dials: Vec::new(),
                reads: Vec::new(),
                names: Vec::new(),
                bases: Vec::new(),
                by_tuple: crate::ByTuple::default(),
                fixed,
                pairs,
            },
        ));
    }
    // Influence needs every token folded; with refusals there is no
    // completeness to judge, so tamber-dial-inert waits for them.
    let judged = c.errors.is_empty();
    if judged {
        c.influence(tuples);
    }
    let out: Vec<Checked> = out
        .into_iter()
        .map(|(i, mut token)| {
            token.dials = c.used[i]
                .iter()
                .map(|&d| design.dials[d].name.value.clone())
                .collect();
            token.reads = c.reads[i]
                .iter()
                .map(|&d| design.dials[d].name.value.clone())
                .collect();
            token.bases = c.based[i].clone();
            // (none for a token whose values name nothing, most of a large
            // import; each distinct reading once, a tuple its index)
            if c.traced && (!c.refs[i].is_empty() || !c.based[i].is_empty()) {
                let none = (Vec::new(), Vec::new());
                for t in 0..tuples {
                    let read = c.tuple_refs.get(&(i, t)).unwrap_or(&none);
                    let at = match token.by_tuple.reads.iter().position(|r| r == read) {
                        Some(at) => at,
                        None => {
                            token.by_tuple.reads.push(read.clone());
                            token.by_tuple.reads.len() - 1
                        }
                    };
                    token.by_tuple.at.push(at as u8);
                }
            }
            token.names = c.refs[i]
                .iter()
                .map(|&j| {
                    let name = &design.tokens[j].name.value;
                    c.role_key(name).map_or(name.clone(), |(_, row)| row)
                })
                .collect();
            token
        })
        .collect();
    if judged {
        c.completeness();
    }
    c.exports();
    let dials = design
        .dials
        .iter()
        .map(|d| c.dials.get(d.name.value.as_str()).map(|(_, v)| v.clone()))
        .collect::<Option<Vec<_>>>();
    // A refusal carries each token's kind, written or inferred, so the
    // caller can say what else is wrong (`tamber-base-kind`).
    let kinds = c.kinds.clone();
    let errors = c.finish();
    match dials {
        Some(dials) if errors.is_empty() => Ok((out, tuples, dials)),
        _ => Err((errors, kinds)),
    }
}

impl<'a> Checker<'a> {
    /// Note unset base name `name` on the token folding at its tuple
    /// ([`Checked::by_tuple`]), where it folds at one.
    fn note_base(&mut self, name: &str) {
        if let (Some(&i), Some(&Some(t))) = (self.stack.last(), self.at.last()) {
            let bases = &mut self.tuple_refs.entry((i, t)).or_default().1;
            if !bases.iter().any(|b| b == name) {
                bases.push(name.to_string());
            }
        }
    }

    /// Whether `t` is marked `scheme=fixed`, refusing the mark on a token
    /// whose value differs between the schemes in some tuple (a pair with two
    /// sides, a role or a platform colour, which follow the scheme) and any
    /// other `scheme=` word.
    fn scheme(&mut self, t: &Token, values: &[Value]) -> bool {
        let words: Vec<&String> = t
            .meta
            .iter()
            .filter(|(k, _)| k == "scheme")
            .map(|(_, v)| v)
            .collect();
        let Some(&word) = words.first() else {
            return false;
        };
        let (line, col) = (t.name.line, t.name.col);
        if words.len() > 1 {
            self.errors.push(Diagnostic::new(
                "tamber-scheme-fixed",
                format!(
                    "`{}` says `scheme=` more than once: write `scheme=fixed` once",
                    t.name.value
                ),
                line,
                col,
            ));
            return false;
        }
        if word != "fixed" {
            self.errors.push(Diagnostic::new(
                "tamber-scheme-fixed",
                format!("`scheme={word}`: the one mark is `scheme=fixed`, a token one colour in every scheme"),
                line,
                col,
            ));
            return false;
        }
        if values.iter().any(|v| v.color().is_none()) {
            self.errors.push(Diagnostic::new(
                "tamber-scheme-fixed",
                format!(
                    "`{}` is a {}: `scheme=fixed` marks a colour",
                    t.name.value,
                    values[0].kind().name()
                ),
                line,
                col,
            ));
            return false;
        }
        let differs = values.iter().filter_map(Value::color).any(|v| match v {
            Folded::One(_) => false,
            Folded::Pair(light, dark) => !same(*light, *dark),
            Folded::Host(_) => true,
        });
        if differs {
            self.errors.push(Diagnostic::new(
                "tamber-scheme-fixed",
                format!("`{}` is marked `scheme=fixed` but its value differs between light and dark: drop the mark, or give it one colour", t.name.value),
                line,
                col,
            ));
            return false;
        }
        true
    }

    fn finish(mut self) -> Vec<Diagnostic> {
        self.errors
            .sort_by(|a, b| (a.line, a.col, a.id).cmp(&(b.line, b.col, b.id)));
        self.errors.dedup();
        self.errors
    }

    fn at<T>(&mut self, at: &At<T>, id: &'static str, message: impl Into<String>) {
        self.errors
            .push(Diagnostic::new(id, message, at.line, at.col));
    }

    /// The role a token's key overrides: its schema name and its row
    /// spelling. `-exact-secondary-label`, LLP 1095 D4's bare
    /// `secondary-label`, or a CSS system colour's name.
    fn role_key(&self, key: &str) -> Option<(String, String)> {
        let lower = key.to_ascii_lowercase();
        if let Some(r) = self.roles.get(&lower) {
            return Some(r.clone());
        }
        if role_spelling(key).is_some() {
            return None;
        }
        self.roles.get(&format!("-exact-{lower}")).cloned()
    }

    /// Whether a token or dial may take `name`: not a CSS colour keyword,
    /// a CSS-wide keyword, or a word CSS's colour functions read.
    fn reserved(name: &str) -> bool {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "none"
                | "from"
                | "in"
                | "alpha"
                | "transparent"
                | "currentcolor"
                | "inherit"
                | "initial"
                | "unset"
                | "revert"
                | "revert-layer"
        ) || exact_color::parse(name).is_some()
    }

    fn axes(&mut self) -> Option<usize> {
        let design = self.design;
        let mut tuples = 1usize;
        for (i, axis) in design.axes.iter().enumerate() {
            if matches!(
                axis.name.value.as_str(),
                "scheme" | "contrast" | "color-scheme"
            ) {
                self.at(&axis.name, "tamber-axis-host", format!("`{}` is the host's trait, not an app axis: a token varies by it through `light-dark()` or a role (D5)", axis.name.value));
            }
            let mut values = HashMap::new();
            for (j, v) in axis.values.iter().enumerate() {
                if values.insert(v.as_str(), j).is_some() {
                    self.at(
                        &axis.name,
                        "tamber-duplicate",
                        format!("`{}` lists `{v}` twice", axis.name.value),
                    );
                }
            }
            if self
                .axes
                .insert(axis.name.value.as_str(), (i, values))
                .is_some()
            {
                self.at(
                    &axis.name,
                    "tamber-duplicate",
                    format!("a second axis `{}`", axis.name.value),
                );
            }
            tuples = tuples.saturating_mul(axis.values.len());
            self.radix.push(axis.values.len());
        }
        if tuples > MAX_TUPLES {
            let named: Vec<String> = design
                .axes
                .iter()
                .map(|a| format!("{} ({})", a.name.value, a.values.len()))
                .collect();
            let first = &design.axes[0].name;
            self.at(
                first,
                "tamber-tuples",
                format!(
                    "the axes {} make {tuples} tuples; a design has at most {MAX_TUPLES}",
                    named.join(" × ")
                ),
            );
            return None;
        }
        self.table = (0..tuples).map(|n| self.digits_of(n).into()).collect();
        Some(tuples)
    }

    fn dials(&mut self) {
        let design = self.design;
        for (i, d) in design.dials.iter().enumerate() {
            self.refused_names.insert(d.name.value.clone());
            if Self::reserved(&d.name.value) {
                self.at(
                    &d.name,
                    "tamber-name-reserved",
                    format!("`{}` is a CSS word; name the dial otherwise", d.name.value),
                );
                continue;
            }
            let value = match d.kind {
                DialKind::Color => {
                    let folded = css::parse(&d.value.value)
                        .map_err(|m| FoldError {
                            id: css::refusal_id(&d.value.value, &m),
                            message: m,
                        })
                        .and_then(|items| fold::color(&items, &mut Literal { roles: &self.roles }));
                    match folded {
                        Ok(Folded::Host(h)) => {
                            let what = Folded::Host(h).css();
                            self.at(
                                &d.value,
                                "tamber-dial",
                                format!(
                                    "`{what}` is the host's; a dial is a colour the design sets"
                                ),
                            );
                            continue;
                        }
                        Ok(f) => DialValue::Color(f),
                        Err(e) => {
                            self.at(&d.value, e.id, e.message);
                            continue;
                        }
                    }
                }
                kind => match number(&d.value.value, kind) {
                    Some(n) if kind == DialKind::Number => DialValue::Number(n),
                    Some(n) => DialValue::Length(n),
                    None => {
                        let shape = if kind == DialKind::Number {
                            "a number"
                        } else {
                            "a length in px"
                        };
                        self.at(
                            &d.value,
                            "tamber-dial",
                            format!(
                                "`{}`: a {} dial's value is {shape}",
                                d.value.value,
                                kind.name()
                            ),
                        );
                        continue;
                    }
                },
            };
            match (&d.range, &value) {
                (Some(_), DialValue::Color(_)) => {
                    self.at(&d.name, "tamber-dial-range", "a colour dial has no range");
                    continue;
                }
                (None, DialValue::Number(_) | DialValue::Length(_)) => {
                    self.at(
                        &d.name,
                        "tamber-dial-range",
                        format!(
                            "a {} dial has a closed range: `in low..high`",
                            d.kind.name()
                        ),
                    );
                    continue;
                }
                (Some((lo, hi)), DialValue::Number(v) | DialValue::Length(v)) => {
                    match (number(lo, d.kind), number(hi, d.kind)) {
                        (Some(lo), Some(hi)) if lo <= hi => {
                            if !(lo..=hi).contains(v) {
                                self.at(&d.value, "tamber-dial-range", format!("`{}` is outside `{}`'s range; a value out of range is refused, not clamped", d.value.value, d.name.value));
                                continue;
                            }
                        }
                        _ => {
                            self.at(
                                &d.name,
                                "tamber-dial-range",
                                format!(
                                    "`in {lo}..{hi}`: a range is two {}s, low first",
                                    if d.kind == DialKind::Number {
                                        "number"
                                    } else {
                                        "px length"
                                    }
                                ),
                            );
                            continue;
                        }
                    }
                }
                _ => {}
            }
            self.refused_names.remove(&d.name.value);
            if self
                .dials
                .insert(d.name.value.as_str(), (i, value))
                .is_some()
            {
                self.at(
                    &d.name,
                    "tamber-duplicate",
                    format!("a second dial `{}`", d.name.value),
                );
            }
        }
    }

    /// Every token's names; duplicates and reserved names refused.
    fn declare(&mut self) {
        let design = self.design;
        for (i, t) in design.tokens.iter().enumerate() {
            self.refused[i] = true;
            let key = &t.name.value;
            let role = self.role_key(key);
            let spellings = match &role {
                // A role answers to its row spelling and to the key's.
                Some((name, row)) => vec![row.to_ascii_lowercase(), name.to_ascii_lowercase()],
                None => vec![key.clone()],
            };
            self.refused_names.extend(spellings.iter().cloned());
            if role.is_none() && role_spelling(key).is_some() {
                self.at(
                    &t.name,
                    "tamber-unknown-role",
                    format!("`{key}` names no role"),
                );
                continue;
            }
            if role.is_none() && Self::reserved(key) {
                self.at(
                    &t.name,
                    "tamber-name-reserved",
                    format!("`{key}` is a CSS word; name the token otherwise"),
                );
                continue;
            }
            if self.dials.contains_key(key.as_str()) {
                self.at(
                    &t.name,
                    "tamber-duplicate",
                    format!("`{key}` is a dial's name"),
                );
                continue;
            }
            match t.kind.as_deref() {
                None | Some("color") => {}
                Some("length" | "number" | "box-shadow" | "text-shadow") if role.is_some() => {
                    self.at(
                        &t.name,
                        "tamber-kind",
                        format!("`{key}` overrides a colour role, so it is a colour"),
                    );
                    continue;
                }
                // A motion token is substituted at build: one value (D3).
                Some("duration" | "easing") if !t.whens.is_empty() => {
                    self.at(&t.name, "tamber-motion-axis", "a duration or an easing is written into its `transition` or `animation` at build, so it has one value: no `when` (vary the row instead)");
                    continue;
                }
                Some(
                    "length" | "number" | "type" | "family" | "box-shadow" | "text-shadow"
                    | "duration" | "easing",
                ) if t
                    .whens
                    .iter()
                    .any(|w| w.conds.iter().any(|(a, _)| self.under(a))) =>
                {
                    self.at(&t.name, "tamber-under-kind", "only a colour may vary by an axis `under scheme=dark`: the kernel picks a length's slice without knowing the scheme");
                    continue;
                }
                Some(
                    "length" | "number" | "box-shadow" | "text-shadow" | "duration" | "easing"
                    | "type" | "family",
                ) => {}
                // An icon under `theme` is the icons check's to refuse.
                Some("icon") => continue,
                Some(other) => {
                    self.at(&t.name, "tamber-syntax", format!("`{other}` is not a kind"));
                    continue;
                }
            }
            self.refused[i] = false;
            for s in &spellings {
                self.refused_names.remove(s);
            }
            for s in spellings {
                if let Some(&j) = self.names.get(&s) {
                    if j != i {
                        self.refused[i] = true;
                        self.at(
                            &t.name,
                            "tamber-duplicate",
                            format!(
                                "`{key}` is declared twice (line {})",
                                design.tokens[j].name.line
                            ),
                        );
                    }
                    continue;
                }
                self.names.insert(s, i);
            }
        }
    }

    /// Each token's kind: written, else inferred from its default value
    /// (§3), a lone name taking the kind of what it names; else a colour.
    fn kinds(&mut self) -> Option<usize> {
        let design = self.design;
        let mut walk = Walk {
            on: vec![false; design.tokens.len()],
            known: vec![None; design.tokens.len()],
            depth: 0,
            deep: None,
        };
        for i in 0..design.tokens.len() {
            self.kinds[i] = self.kind_of(i, &mut walk);
            if walk.deep.is_some() {
                return walk.deep;
            }
            let t = &design.tokens[i];
            if !self.refused[i]
                && matches!(self.kinds[i], Kind::Duration | Kind::Easing)
                && !t.whens.is_empty()
            {
                self.refused[i] = true;
                self.at(&t.name, "tamber-motion-axis", "a duration or an easing is written into its `transition` or `animation` at build, so it has one value: no `when` (vary the row instead)");
                continue;
            }
            // A row reads a dotted word as the token wherever it stands: no
            // CSS property, keyframes name or easing keyword has a dot.
            if !self.refused[i]
                && matches!(self.kinds[i], Kind::Duration | Kind::Easing)
                && !t.name.value.contains('.')
            {
                self.refused[i] = true;
                let name = &t.name.value;
                let group = if self.kinds[i] == Kind::Duration {
                    "duration"
                } else {
                    "easing"
                };
                self.at(&t.name, "tamber-motion-name", format!("`{name}`: a duration's or an easing's name is dotted (`{group}.{name}`), so a `transition` or `animation` reads it as the token wherever it stands, never as a property, a keyframes name or a keyword"));
                continue;
            }
            // A shadow's kind is written: two rows read alike values.
            if !self.refused[i]
                && self.kinds[i] == Kind::Color
                && t.kind.is_none()
                && crate::shadow::looks(
                    &t.value.value,
                    &mut |name| self.named_kind(name, i, &mut walk) == Kind::Length,
                    self.named_length,
                )
            {
                self.refused[i] = true;
                self.at(&t.name, "tamber-kind", format!("`{}` reads as a shadow: write its kind, `{}: box-shadow` or `{}: text-shadow`", t.name.value, t.name.value, t.name.value));
                continue;
            }
            // A role override is a colour, written or inferred.
            if !self.refused[i]
                && self.kinds[i] != Kind::Color
                && self.role_key(&t.name.value).is_some()
            {
                self.refused[i] = true;
                self.at(
                    &t.name,
                    "tamber-kind",
                    format!(
                        "`{}` overrides a colour role, so it is a colour",
                        t.name.value
                    ),
                );
            }
        }
        walk.deep
    }

    /// Token `i`'s kind; `walk` holds the names followed to it, so a cycle
    /// (refused when it folds) ends as a colour, and each kind found, so a
    /// chain is followed once however many tokens read it, a cycle's too (its
    /// design is refused, and the walk's order is the build's and the
    /// check's alike: no 2^n over a cycle read twice a level). A chain past
    /// [`crate::bound::MAX_REFERENCE_DEPTH`] stops where it passes (`Walk::deep`).
    fn kind_of(&self, i: usize, walk: &mut Walk) -> Kind {
        let t = &self.design.tokens[i];
        if let Some(k) = t.kind.as_deref().and_then(Kind::parse) {
            return k;
        }
        if let Some(k) = walk.known[i] {
            return k;
        }
        if walk.on[i] {
            return Kind::Color;
        }
        if !walk.enter(i) {
            return Kind::Color;
        }
        walk.on[i] = true;
        let kind = self.inferred_kind(i, walk);
        walk.depth -= 1;
        walk.on[i] = false;
        walk.known[i] = Some(kind);
        kind
    }

    /// [`Checker::kind_of`] for a token whose kind is not written.
    fn inferred_kind(&self, i: usize, walk: &mut Walk) -> Kind {
        let t = &self.design.tokens[i];
        if let Some(k) = crate::typeface::inferred(&t.value.value, self.named_length) {
            return k;
        }
        let Ok(items) = css::parse(&t.value.value) else {
            return Kind::Color;
        };
        let kind = match items.as_slice() {
            // A function over numbers is a number, over a length a length:
            // a dimension, or a name of a length token or dial.
            [css::Item::Func(f, args, _)]
                if matches!(f.as_str(), "calc" | "min" | "max" | "clamp") =>
            {
                Some(match self.measures(args.iter().flatten(), i, walk) {
                    true => Kind::Length,
                    false => Kind::Number,
                })
            }
            // A declared token's or dial's name wins over a keyword's
            // spelling (an easing keyword, a named length).
            [css::Item::Ident(name)]
                if self.names.get(name.as_str()).is_some_and(|&j| j != i)
                    || self.dials.contains_key(name.as_str())
                    || !(self.named_length)(name)
                        && !crate::motion::KEYWORDS.contains(&name.as_str()) =>
            {
                Some(self.named_kind(name, i, walk))
            }
            _ => measure::inferred(&items, self.named_length)
                .or_else(|| crate::motion::inferred(&items)),
        };
        kind.unwrap_or(Kind::Color)
    }

    /// The kind of what `name` names: a token's (followed), a dial's; a
    /// colour otherwise.
    fn named_kind(&self, name: &str, i: usize, walk: &mut Walk) -> Kind {
        match self.names.get(name) {
            Some(&j) if j != i => self.kind_of(j, walk),
            _ => match self.dials.get(name) {
                Some((_, DialValue::Number(_))) => Kind::Number,
                Some((_, DialValue::Length(_))) => Kind::Length,
                _ => Kind::Color,
            },
        }
    }

    /// Whether a function's items hold a length: a dimension, or a name of
    /// a length token or dial, at any depth (each function a level of
    /// [`crate::bound::MAX_REFERENCE_DEPTH`], as each name followed is).
    fn measures<'i>(
        &self,
        items: impl Iterator<Item = &'i css::Item>,
        i: usize,
        walk: &mut Walk,
    ) -> bool {
        for item in items {
            let length = match item {
                css::Item::Dim(..) => true,
                css::Item::Ident(name) => {
                    (self.named_length)(name) || self.named_kind(name, i, walk) == Kind::Length
                }
                css::Item::Func(_, args, _) => {
                    if !walk.enter(i) {
                        return false;
                    }
                    let length = self.measures(args.iter().flatten(), i, walk);
                    walk.depth -= 1;
                    length
                }
                _ => false,
            };
            if length {
                return true;
            }
        }
        false
    }

    /// A `when` names declared axes and values, each axis once.
    fn whens(&mut self, t: &'a Token) {
        for w in &t.whens {
            let mut seen = BTreeSet::new();
            for (axis, value) in &w.conds {
                let at = At {
                    value: (),
                    line: w.line,
                    col: w.col,
                };
                match self.axes.get(axis.as_str()) {
                    None => self.at(
                        &at,
                        "tamber-unknown-axis",
                        format!("`{axis}` is no declared axis"),
                    ),
                    Some((_, values)) if !values.contains_key(value.as_str()) => self.at(
                        &at,
                        "tamber-unknown-axis",
                        format!("`{value}` is not a value of `{axis}`"),
                    ),
                    Some(_) if !seen.insert(axis) => self.at(
                        &at,
                        "tamber-syntax",
                        format!("`{axis}` twice in one `when`"),
                    ),
                    Some(_) => {}
                }
            }
        }
    }

    /// The value index of each axis in `tuple` (first axis slowest).
    fn digits(&self, tuple: usize) -> std::rc::Rc<[usize]> {
        let made = self.table.get(tuple).cloned();
        made.unwrap_or_else(|| self.digits_of(tuple).into())
    }

    fn digits_of(&self, mut tuple: usize) -> Vec<usize> {
        let mut out = vec![0; self.radix.len()];
        for (i, r) in self.radix.iter().enumerate().rev() {
            out[i] = tuple % r;
            tuple /= r;
        }
        out
    }

    fn holds(&self, cond: &(String, String), digits: &[usize]) -> bool {
        self.axes
            .get(cond.0.as_str())
            .is_some_and(|(axis, values)| values.get(cond.1.as_str()) == Some(&digits[*axis]))
    }

    fn under(&self, axis: &str) -> bool {
        self.axes
            .get(axis)
            .is_some_and(|(i, _)| self.design.axes[*i].under_dark)
    }

    /// The `when` a tuple selects (the one naming the most axes among
    /// those that hold), skipping those that name an axis `under` when
    /// `base`. Two that hold and name as many are refused.
    fn select(&mut self, t: &'a Token, digits: &[usize], base: bool) -> Option<usize> {
        let holding: Vec<usize> = (0..t.whens.len())
            .filter(|&i| {
                let w = &t.whens[i];
                !(base && w.conds.iter().any(|(a, _)| self.under(a)))
                    && w.conds.iter().all(|c| self.holds(c, digits))
            })
            .collect();
        let most = holding.iter().map(|&i| t.whens[i].conds.len()).max()?;
        let mut best = holding
            .into_iter()
            .filter(|&i| t.whens[i].conds.len() == most);
        let first = best.next()?;
        for other in best {
            let w = &t.whens[other];
            let at = At {
                value: (),
                line: w.line,
                col: w.col,
            };
            self.at(
                &at,
                "tamber-when-ambiguous",
                format!(
                    "this `when` and the one on line {} both hold and name as many axes",
                    t.whens[first].line
                ),
            );
        }
        Some(first)
    }

    /// Token `i`'s value for `tuple`, folded once; refusals reported.
    fn value(&mut self, i: usize, tuple: usize) -> Option<Value> {
        if let Some(v) = self.memo.get(&(i, tuple)) {
            return v.clone();
        }
        let design = self.design;
        let t = &design.tokens[i];
        if self.stack.contains(&i) {
            let mut chain: Vec<&str> = self.stack
                [self.stack.iter().position(|&s| s == i).unwrap_or(0)..]
                .iter()
                .map(|&s| design.tokens[s].name.value.as_str())
                .collect();
            chain.push(&t.name.value);
            // (a long one said by its ends: its words stay short)
            if chain.len() > 12 {
                chain.splice(6..chain.len() - 6, ["…"]);
            }
            self.at(
                &t.name,
                "tamber-cycle",
                format!("a cycle: {}", chain.join(" → ")),
            );
            return None;
        }
        // Each name followed recurses: a chain past the bound is refused
        // where it passes, before the stack would run out.
        let Some(_level) = crate::bound::deeper() else {
            let (id, why) = crate::bound::too_deep();
            self.at(&t.name, id, why);
            self.memo.insert((i, tuple), None);
            return None;
        };
        self.stack.push(i);
        self.at.push(self.traced.then_some(tuple));
        let digits = self.digits(tuple);
        let chosen = self.select(t, &digits, false);
        self.selected[i].insert(chosen);
        let folded = match chosen {
            Some(w) if t.whens[w].conds.iter().any(|(a, _)| self.under(a)) => {
                let base = self.select(t, &digits, true);
                self.selected[i].insert(base);
                self.under_value(i, tuple, w, base).map(Value::Color)
            }
            Some(w) => self.fold_kind(tuple, &t.whens[w].value, self.kinds[i]),
            None => self.fold_kind(tuple, &t.value, self.kinds[i]),
        };
        self.stack.pop();
        self.at.pop();
        self.memo.insert((i, tuple), folded.clone());
        folded
    }

    /// A value no tuple selects is still checked (folded at the first).
    fn unselected(&mut self, i: usize) {
        let t = &self.design.tokens[i];
        let tuples: usize = self.radix.iter().product();
        for which in std::iter::once(None).chain((0..t.whens.len()).map(Some)) {
            if self.selected[i].contains(&which) {
                continue;
            }
            // In a tuple where its conditions hold, so a hidden `when` is
            // checked against the value it would keep there.
            let conds = which.map_or(&[][..], |w| &t.whens[w].conds[..]);
            let tuple = (0..tuples)
                .find(|&n| {
                    let digits = self.digits(n);
                    conds.iter().all(|c| self.holds(c, &digits))
                })
                .unwrap_or(0);
            let digits = self.digits(tuple);
            self.stack.push(i);
            self.at.push(None);
            match which {
                // A hidden `when` on an axis `under scheme=dark` is checked
                // as one: its light side against the value it would keep.
                Some(w) if t.whens[w].conds.iter().any(|(a, _)| self.under(a)) => {
                    let base = self.select(t, &digits, true);
                    let _ = self.under_value(i, tuple, w, base);
                }
                Some(w) => {
                    let _ = self.fold_kind(tuple, &t.whens[w].value, self.kinds[i]);
                }
                None => {
                    let _ = self.fold_kind(tuple, &t.value, self.kinds[i]);
                }
            }
            self.stack.pop();
            self.at.pop();
        }
    }

    /// Which dials move which tokens (D4's completeness, the IR's per-token
    /// list): each dial set to other values, every token refolded, and a
    /// token that changes is moved by it. A dial read only where nothing
    /// shows (a discarded `light-dark()` side) moves nothing.
    fn influence(&mut self, tuples: usize) {
        let design = self.design;
        let baseline = self.memo.clone();
        let errors = self.errors.len();
        // Only a token that reads a dial, or names one that does, can move
        // with it (the names each fold read): refolded alone, each probe
        // (the checker's work in dial counts: every token at every dial
        // was dials x tokens).
        let n = design.tokens.len();
        let mut named_by = vec![Vec::new(); n];
        let mut readers = vec![Vec::new(); design.dials.len()];
        for i in 0..n {
            self.refs[i].iter().for_each(|&j| named_by[j].push(i));
            self.reads[i].iter().for_each(|&d| readers[d].push(i));
        }
        // (each dial's visit marked by its number, so nothing is cleared per
        // dial: round 1 of the counts' review)
        let mut moving = vec![usize::MAX; n];
        for (d, dial) in design.dials.iter().enumerate() {
            #[cfg(test)]
            let readers = &influence_tests::every_token(&readers, n);
            let mut todo = readers[d].clone();
            let mut reach = Vec::new();
            while let Some(i) = todo.pop() {
                if std::mem::replace(&mut moving[i], d) != d {
                    reach.push(i);
                    todo.extend(&named_by[i]);
                }
            }
            // Refolded in the design's order, as every token was: a token's
            // names folded before it (Astra's round 1: a chain read from its
            // far end passed the depth bound).
            reach.sort_unstable();
            let Some((_, value)) = self.dials.get(dial.name.value.as_str()).cloned() else {
                continue;
            };
            // Probes: opaque and translucent colours and a modern one, and
            // numbers across the range, so a dial seen only through its
            // alpha or between its ends still shows.
            let colour =
                |r, g, b, a| DialValue::Color(Folded::One(Half::legacy(Rgba8 { r, g, b, a })));
            let range = dial
                .range
                .as_ref()
                .and_then(|(lo, hi)| Some((number(lo, dial.kind)?, number(hi, dial.kind)?)));
            let across = |lo: f64, hi: f64| {
                [0.0, 0.13, 0.29, 0.5, 0.71, 0.87, 1.0].map(|f| lo + (hi - lo) * f)
            };
            let others: Vec<DialValue> = match (&value, range) {
                (DialValue::Color(_), _) => vec![
                    colour(0, 0, 0, 1.0),
                    colour(255, 255, 255, 1.0),
                    colour(51, 102, 153, 0.5),
                    colour(200, 30, 90, 0.0),
                    DialValue::Color(Folded::One(Half::Wide(exact_color::Wide {
                        space: exact_color::Space::Oklch,
                        c: [0.7, 0.15, 140.0],
                        alpha: 0.8,
                    }))),
                ],
                (DialValue::Number(_), Some((lo, hi))) => {
                    across(lo, hi).map(DialValue::Number).to_vec()
                }
                (DialValue::Length(_), Some((lo, hi))) => {
                    across(lo, hi).map(DialValue::Length).to_vec()
                }
                _ => Vec::new(),
            };
            for other in others {
                if other == value {
                    continue;
                }
                self.dials.insert(dial.name.value.as_str(), (d, other));
                for &i in &reach {
                    for tuple in 0..tuples {
                        self.memo.remove(&(i, tuple));
                    }
                }
                for &i in &reach {
                    if self.refused[i] {
                        continue;
                    }
                    for tuple in 0..tuples {
                        // A probe the fold refuses is no evidence either way.
                        let base = baseline.get(&(i, tuple)).cloned().flatten();
                        if self.value(i, tuple).is_some_and(|v| Some(v) != base) {
                            self.used[i].insert(d);
                        }
                    }
                }
            }
            self.dials.insert(dial.name.value.as_str(), (d, value));
            for &i in &reach {
                for tuple in 0..tuples {
                    match baseline.get(&(i, tuple)) {
                        Some(v) => drop(self.memo.insert((i, tuple), v.clone())),
                        None => drop(self.memo.remove(&(i, tuple))),
                    }
                }
            }
        }
        self.memo = baseline;
        self.errors.truncate(errors);
    }

    /// A token's value at `tuple`, folded as its kind.
    fn fold_kind(&mut self, tuple: usize, value: &At<String>, kind: Kind) -> Option<Value> {
        if kind == Kind::Color {
            return self.fold_at(tuple, value).map(Value::Color);
        }
        // A shadow list's commas are its own, and a family's: their folds
        // split the text.
        let shadow = matches!(
            kind,
            Kind::BoxShadow | Kind::TextShadow | Kind::Type | Kind::Family
        );
        let items = match css::parse(&value.value) {
            Ok(items) => items,
            Err(_) if shadow => Vec::new(),
            Err(m) => {
                self.at(value, css::refusal_id(&value.value, &m), m);
                return None;
            }
        };
        let named_length = self.named_length;
        let mut scope = Scope {
            checker: self,
            tuple,
        };
        let folded = match kind {
            Kind::Length => measure::length(&items, &mut scope, named_length).map(Value::Length),
            Kind::BoxShadow | Kind::TextShadow => {
                crate::shadow::fold(&value.value, kind == Kind::TextShadow, &mut scope)
                    .map(Value::Shadow)
            }
            Kind::Duration => crate::motion::duration(&items, &mut scope).map(Value::Duration),
            Kind::Easing => crate::motion::easing(&items, &mut scope).map(Value::Easing),
            Kind::Type => {
                crate::typeface::fold(&value.value, &mut scope, named_length).map(Value::Type)
            }
            Kind::Family => crate::typeface::family(&value.value, &mut scope).map(Value::Family),
            _ => measure::number(&items, &mut scope).map(Value::Number),
        };
        match folded {
            Ok(v) => {
                // An unset base name it reads bare, noted on the token folding.
                // And each an expression over base names reads (stage 12 B2),
                // and a shadow's lengths (B3).
                let named: Vec<&str> = match &v {
                    Value::Length(Length::Named(n)) => vec![n],
                    Value::Length(Length::Base(r)) => r.bases(),
                    Value::Shadow(s) => s.bases(),
                    Value::Type(t) => match &t.size {
                        Length::Named(n) => vec![n],
                        _ => Vec::new(),
                    },
                    _ => Vec::new(),
                };
                if let Some(&i) = self.stack.last() {
                    for n in named {
                        if self.bases.iter().any(|b| b == n) {
                            self.note_base(n);
                            if !self.based[i].iter().any(|b| b == n) {
                                self.based[i].push(n.to_string());
                            }
                        }
                    }
                }
                Some(v)
            }
            Err(e) if e.id.is_empty() => None,
            Err(e) => {
                self.at(value, e.id, e.message);
                None
            }
        }
    }

    fn fold_at(&mut self, tuple: usize, value: &At<String>) -> Option<Folded> {
        let items = match css::parse(&value.value) {
            Ok(items) => items,
            Err(m) => {
                self.at(value, css::refusal_id(&value.value, &m), m);
                return None;
            }
        };
        let mut scope = Scope {
            checker: self,
            tuple,
        };
        match fold::color(&items, &mut scope) {
            Ok(f) => Some(f),
            Err(e) if e.id.is_empty() => None,
            Err(e) => {
                self.at(value, e.id, e.message);
                None
            }
        }
    }

    /// A value under a `when` naming an axis `under scheme=dark` (D5): the
    /// light side is the value's without such axes, the dark side the
    /// `when`'s.
    fn under_value(
        &mut self,
        i: usize,
        tuple: usize,
        w: usize,
        base: Option<usize>,
    ) -> Option<Folded> {
        let t = &self.design.tokens[i];
        let when = &t.whens[w];
        let base_value = base.map_or(&t.value, |b| &t.whens[b].value);
        // The kernel picks a slice without knowing the scheme, so only a
        // colour may vary by such an axis.
        let measure = |items: Vec<css::Item>| match items.as_slice() {
            [css::Item::Func(f, ..)] => {
                matches!(f.as_str(), "calc" | "min" | "max" | "clamp" | "round")
            }
            [one] => matches!(
                one,
                css::Item::Number(_)
                    | css::Item::Percent(_)
                    | css::Item::Dim(..)
                    | css::Item::Str(_)
            ),
            _ => true,
        };
        if [base_value, &when.value]
            .iter()
            .any(|v| css::parse(&v.value).is_ok_and(measure))
        {
            let at = At {
                value: (),
                line: when.line,
                col: when.col,
            };
            self.at(&at, "tamber-under-kind", "only a colour may vary by an axis `under scheme=dark`: the kernel picks a length's slice without knowing the scheme");
            return None;
        }
        let light = self.fold_at(tuple, base_value)?;
        let dark = self.fold_at(tuple, &when.value)?;
        let at = At {
            value: (),
            line: when.line,
            col: when.col,
        };
        let role = |f: &Folded| matches!(f, Folded::Host(_)).then(|| f.css());
        if let Some(host) = role(&light).or_else(|| role(&dark)) {
            self.at(&at, "tamber-under-role", format!("`{host}` has no light side to keep under an axis `under scheme=dark`, and a `light-dark()` with a role as a side is refused until LLP 1095 D10: write the light and dark colours"));
            return None;
        }
        let keep = light.side(false).ok()?;
        let dark = match dark {
            Folded::One(h) => h,
            Folded::Pair(l, d) if same(l, keep) => d,
            Folded::Pair(l, _) => {
                self.at(&at, "tamber-under-light", format!("the light side `{}` is not the default's `{}`: an axis `under scheme=dark` varies the dark side only", l.css(), keep.css()));
                return None;
            }
            Folded::Host(_) => return None,
        };
        Some(Folded::Pair(keep, dark))
    }

    fn completeness(&mut self) {
        let design = self.design;
        let moving: BTreeSet<usize> = self.used.iter().flatten().copied().collect();
        for d in &design.dials {
            let Some(&(i, _)) = self.dials.get(d.name.value.as_str()) else {
                continue;
            };
            if !moving.contains(&i) {
                self.at(
                    &d.name,
                    "tamber-dial-inert",
                    format!(
                        "`{}` moves no token; every dial moves at least one (D4)",
                        d.name.value
                    ),
                );
            }
        }
    }

    fn exports(&mut self) {
        let Some(exports) = &self.design.exports else {
            return;
        };
        for e in exports {
            let key = self
                .role_key(&e.value)
                .map_or(e.value.clone(), |(_, row)| row.to_ascii_lowercase());
            // An icon may be exported too: generated bindings name it (D12).
            let icon = self.icons.contains(e.value.as_str());
            if !self.names.contains_key(&key) && !icon {
                self.at(
                    e,
                    "tamber-unknown-token",
                    format!("`{}` is no token this design declares", e.value),
                );
            }
        }
    }
}

/// The same colour, whatever its form: `#ffffff` is `color(srgb 1 1 1)`.
fn same(a: Half, b: Half) -> bool {
    let (a, b) = (a.wide(), b.wide());
    let (x, y) = (a.linear_srgb(), b.linear_srgb());
    (a.alpha - b.alpha).abs() < 1e-9 && x.iter().zip(&y).all(|(p, q)| (p - q).abs() < 1e-9)
}

/// `1.5` for a number dial, `12px` for a length one (`0` too), in px.
fn number(text: &str, kind: DialKind) -> Option<f64> {
    match (css::parse(text).ok()?.as_slice(), kind) {
        ([css::Item::Number(n)], DialKind::Number) => Some(*n),
        ([css::Item::Dim(n, unit)], DialKind::Length) if unit == "px" => Some(*n),
        ([css::Item::Number(n)], DialKind::Length) if *n == 0.0 => Some(0.0),
        _ => None,
    }
}

/// A dial's default: literals only.
struct Literal<'r> {
    roles: &'r HashMap<String, (String, String)>,
}

impl Names for Literal<'_> {
    fn named(&mut self, name: &str) -> Result<Named, FoldError> {
        if exact_color::parse(name).is_some() {
            return Ok(Named::Unknown);
        }
        if let Some((_, row)) = self.roles.get(&name.to_ascii_lowercase()) {
            return Ok(Named::Role(row.clone()));
        }
        Err(FoldError {
            id: "tamber-dial",
            message: format!("`{name}`: a dial's value is a literal"),
        })
    }
}

/// The names a token's value sees, at one tuple.
struct Scope<'c, 'a> {
    checker: &'c mut Checker<'a>,
    tuple: usize,
}

impl Names for Scope<'_, '_> {
    // A read, recorded on the token folding: one a later term cancels
    // (`space.md - space.md + 3px`) is still read.
    fn base(&mut self, name: &str) -> bool {
        let c = &mut *self.checker;
        let base = c.bases.iter().any(|b| b == name) && !c.names.contains_key(name);
        if let (true, Some(&i)) = (base, c.stack.last()) {
            c.note_base(name);
            if !c.based[i].iter().any(|b| b == name) {
                c.based[i].push(name.to_string());
            }
        }
        base
    }

    fn named(&mut self, name: &str) -> Result<Named, FoldError> {
        // A base name the design leaves unset, read where no length keeps
        // it (a bare one is a named length, and a length's `calc()` and a
        // shadow's length ask [`Names::base`] first): its length is each
        // host's, so nothing folds it. One the design sets is its token.
        if self.base(name) {
            return Err(FoldError {
                id: "tamber-base-expression",
                message: format!("`{name}` is a base length, each platform's own: a length or a shadow's length names it (`card.pad = {name}`, `calc({name} * 2)`, `0 {name} 4px black`), but no other value can, unless the design sets it"),
            });
        }
        let c = &mut *self.checker;
        let lower = name.to_ascii_lowercase();
        // Silence: a refused token or dial said so where it is written.
        let quiet = || FoldError {
            id: "",
            message: String::new(),
        };
        let token = c.names.get(name).copied().or_else(|| {
            c.names
                .get(&c.role_key(name)?.1.to_ascii_lowercase())
                .copied()
        });
        if let Some(j) = token {
            if let Some(&i) = c.stack.last() {
                c.refs[i].insert(j);
                if let Some(&Some(t)) = c.at.last() {
                    let refs = &mut c.tuple_refs.entry((i, t)).or_default().0;
                    if !refs.contains(&(j as u32)) {
                        refs.push(j as u32);
                    }
                }
            }
            return c
                .value(j, self.tuple)
                .map(|v| match v {
                    Value::Color(f) => Named::Color(f),
                    Value::Length(l) => Named::Length(l),
                    Value::Number(n) => Named::Number(n),
                    // A shadow token: only a whole shadow value names one.
                    Value::Shadow(s) => Named::Shadow(s),
                    Value::Duration(ms) => Named::Duration(ms),
                    Value::Easing(e) => Named::Easing(e),
                    Value::Type(t) => Named::Type(t),
                    Value::Family(f) => Named::Family(f),
                })
                .ok_or_else(quiet);
        }
        if let Some((d, value)) = c.dials.get(name) {
            if let Some(&i) = c.stack.last() {
                c.reads[i].insert(*d);
            }
            return Ok(match value {
                DialValue::Color(f) => Named::Color(f.clone()),
                DialValue::Number(n) => Named::Number(*n),
                DialValue::Length(n) => Named::Length(Length::Px(*n)),
            });
        }
        if c.refused_names.contains(name) || c.refused_names.contains(&lower) {
            return Err(quiet());
        }
        // An icon (D10) is a row's `symbol:`, no value's.
        if c.icons.contains(name) {
            return Err(FoldError {
                id: "tamber-kind",
                message: format!("`{name}` is an icon: a `symbol:` source names it, not a value"),
            });
        }
        if let Some((_, row)) = c.roles.get(&lower) {
            return Ok(Named::Role(row.clone()));
        }
        Ok(Named::Unknown)
    }
}
