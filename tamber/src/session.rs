//! A transaction's pure part (LLP 1121 D13, stage 3): operations over a
//! design's files, checked as a whole, giving each file's new text, the
//! inverse per file, the tokens whose values change, the diff, and whether
//! running builds take it in the live table or rebuild. The server
//! (`contract tamber serve`) owns the digests, the lock, the journal and
//! every write; this owns none.
//!
//! `live` is judged by the edit's closure (D13, stage 4): the plan's
//! retained shape unchanged (axes, dials, and each token the plan keeps by
//! D7's retention, with its kind, its role and whether it varies by an app
//! axis: an override gained or lost, a token kept added or gone, one
//! starting to vary), and the app's synthetic tokens (Contract rows'
//! expressions over tokens, folded with the design) re-folded with it,
//! their new values in the answer. The compiler keeps tokens by the same
//! [`Retention`], so this is the question the dev loops' plan structures
//! answer. A role override's value is live (stage 5). Kinds substituted at
//! build (durations, easings) wait for stage 6.

use crate::edit::{self, Op};
use crate::{Diagnostic, Folding, Vocab};

/// What a commit asks of running development builds (D13): a value change
/// in the token table only, or anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Only values: the live table takes it, with no reload.
    Live,
    /// The structure changed (a token, an axis, a dial added, removed or
    /// renamed, a kind or a role): the ordinary rebuild and reload.
    Rebuild,
}

impl Class {
    /// As the wire spells it.
    pub fn name(self) -> &'static str {
        match self {
            Class::Live => "live",
            Class::Rebuild => "rebuild",
        }
    }
}

/// A transaction planned: every edited file's new text and inverse. A file
/// its operations leave byte for byte as it was is not among them.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// Each edited file: its path, its new text, its inverse.
    pub files: Vec<(String, String, Vec<Op>)>,
    /// The tokens whose values change, added or removed: the file and the
    /// name, sorted.
    pub changed: Vec<(String, String)>,
    /// What running builds do with it.
    pub class: Class,
    /// A diff per edited file, the lines removed (`-`) and added (`+`).
    pub diff: String,
    /// The app's synthetic tokens whose values change, each its expression
    /// and its new value per full app tuple.
    pub synthetic: Vec<(String, Vec<crate::Folded>)>,
    /// Each token of the design file whose folded value changes, named as
    /// the plan's theme row names it (the declared spelling; a synthetic
    /// token by its expression), with its CSS text per full app tuple: a
    /// scrub's frame patches the running design's rows from these.
    pub values: Vec<(String, Vec<String>)>,
}

/// The app's synthetic tokens (D3): Contract rows' expressions over tokens,
/// folded with the design file `file`.
#[derive(Debug, Clone, Copy)]
pub struct Synthetic<'a> {
    /// The design file they fold with.
    pub file: &'a str,
    /// Each one's expression, and whether a colour row writes it: then it
    /// must fold (the build refuses it otherwise), where another that does
    /// not is dropped.
    pub exprs: &'a [(String, bool)],
    /// Which of its tokens the app's plan keeps (D7), as the compiler last
    /// found; none: every token, as an app with bindings keeps.
    pub retention: Option<&'a Retention>,
    /// The app's components' params (D15), which fold with the design file:
    /// each a token, its `params` line in place of its default.
    pub params: &'a crate::params::Params,
}

impl Default for Synthetic<'_> {
    fn default() -> Self {
        Synthetic {
            file: "",
            exprs: &[],
            retention: None,
            params: &crate::params::NONE,
        }
    }
}

/// D7's retention: which of a design's tokens an app's plan keeps. The
/// compiler keeps a token by [`Retention::keeps`] (and a synthetic token its
/// rows write), and a transaction's class asks the same, so `propose` and
/// the dev loops, which compare the plans' structures, agree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Retention {
    /// The app can name a token outside its views (D12's bindings: a data
    /// module, a native module): every token, bounded by `exports`.
    pub bindable: bool,
    /// A colour row's value is computed somewhere: any colour token may be
    /// named.
    pub computed: bool,
    /// A length or number row's value is computed and may be a string
    /// somewhere: any length or number token may be named.
    pub computed_measure: bool,
    /// Every word the views' literals write, any of which may name a token.
    pub words: std::collections::BTreeSet<String>,
    /// Every word a colour keyframe writes: a token (or a role's override)
    /// one names is folded into the plan's `@keyframes`, so its value is
    /// structure, and an edit of it rebuilds.
    pub folded: std::collections::BTreeSet<String>,
}

impl Retention {
    /// Whether the plan keeps `design`'s token `name` of `kind`, which
    /// overrides `role` if it does: every role override; what the views may
    /// name (any of a kind a computed row of theirs reads); and in an app
    /// that can name a token outside its views, every token, or those
    /// `exports` lists.
    pub fn keeps(
        &self,
        design: &crate::Design,
        name: &str,
        role: Option<&str>,
        kind: crate::Kind,
    ) -> bool {
        let computed = match kind {
            crate::Kind::Color => self.computed,
            _ => self.computed_measure,
        };
        let exported = || {
            design.exports.as_ref().is_none_or(|exports| {
                exports
                    .iter()
                    .any(|x| crate::exports_names(&x.value, name, role))
            })
        };
        role.is_some() || computed || self.words.contains(name) || self.bindable && exported()
    }
}

/// A refusal: the file it is in, and the checker's or the edit's
/// diagnostics.
pub type Refused = Vec<(String, Diagnostic)>;

/// Plan `ops` (each with the file it edits) over `files` (each path with
/// its text). Every file's operations are applied in order and the file
/// checked whole; a refusal changes nothing. `undo` marks an inverse the
/// server issued: its restores, and nothing else. `synthetic` is the app's
/// synthetic tokens, re-folded with their file.
pub fn plan(
    files: &[(String, String)],
    ops: &[(String, Op)],
    vocab: &Vocab,
    undo: bool,
    synthetic: Synthetic,
) -> Result<Planned, Refused> {
    let mut out = Vec::new();
    let mut changed = Vec::new();
    let mut moved = Vec::new();
    let mut values = Vec::new();
    let mut structure = false;
    let mut diff = String::new();
    for (path, text) in files {
        let mine: Vec<Op> = ops
            .iter()
            .filter(|(p, _)| p == path)
            .map(|(_, op)| op.clone())
            .collect();
        if mine.is_empty() {
            continue;
        }
        let edited = match undo {
            true => edit::undo(text, &mine, vocab),
            // The design file the app builds is checked with its params.
            false if *path == synthetic.file => {
                edit::apply_all_with(text, &mine, vocab, synthetic.params)
            }
            false => edit::apply_all(text, &mine, vocab),
        }
        .map_err(|all| {
            all.into_iter()
                .map(|d| (path.clone(), d))
                .collect::<Refused>()
        })?;
        if edited.text == *text {
            continue;
        }
        let (exprs, params): (&[(String, bool)], &crate::params::Params) =
            if *path == synthetic.file {
                (synthetic.exprs, synthetic.params)
            } else {
                (&[], &crate::params::NONE)
            };
        let (before, after, moved_set) = folded(text, &edited.text, (exprs, params), vocab);
        structure |= moved_set;
        // A profile's colour (`color(--…)`) the compiler resolves itself, and
        // folds the rest around it: an edit to a design with one either side
        // is the build's (live editing of those waits).
        structure |= [text.as_str(), edited.text.as_str()].iter().any(|t| {
            crate::parse(t).map_or(true, |d| {
                d.tokens.iter().any(|t| {
                    std::iter::once(&t.value.value)
                        .chain(t.whens.iter().map(|w| &w.value.value))
                        .any(|v| v.to_ascii_lowercase().contains("color(--"))
                })
            })
        });
        // A role override's value is live (LLP 1121 stage 5): each build
        // sets its session's overrides from the batch and re-presents them.
        // Gaining or losing one moves the shape.
        let tokens = changed_tokens(before.as_ref(), after.as_ref());
        // A motion token is written into its rows at build (D3, D13): a
        // change to one, or to what it folds from, rebuilds.
        let motion = |f: &Option<Folding>, name: &str| {
            f.as_ref().is_some_and(|f| {
                f.tokens.iter().any(|c| {
                    c.name == name && matches!(c.kind, crate::Kind::Duration | crate::Kind::Easing)
                })
            })
        };
        structure |= tokens
            .iter()
            .any(|t| motion(&before, t) || motion(&after, t));
        if *path == synthetic.file {
            if let Some(f) = &after {
                for (c, declared) in f.tokens.iter().zip(&f.design.tokens) {
                    if !tokens.contains(&c.name) {
                        continue;
                    }
                    let name = match c
                        .name
                        .strip_prefix('~')
                        .and_then(|i| i.parse::<usize>().ok())
                    {
                        Some(i) => exprs[i].0.clone(),
                        None => declared.name.value.clone(),
                    };
                    values.push((name, c.values.iter().map(|v| v.css()).collect()));
                }
            }
        }
        for t in tokens {
            match t.strip_prefix('~').and_then(|i| i.parse::<usize>().ok()) {
                Some(i) => {
                    let values = after
                        .as_ref()
                        .and_then(|f| f.tokens.iter().find(|c| c.name == t))
                        .map(|c| c.values.iter().filter_map(|v| v.color().cloned()).collect())
                        .unwrap_or_default();
                    moved.push((exprs[i].0.clone(), values));
                }
                None => changed.push((path.clone(), t)),
            }
        }
        structure |= match (&before, &after) {
            (Some(b), Some(a)) => {
                let retention = if *path == synthetic.file {
                    synthetic.retention
                } else {
                    None
                };
                shape(b, retention, vocab) != shape(a, retention, vocab)
            }
            _ => true,
        };
        diff.push_str(&file_diff(path, text, &edited.text));
        out.push((path.clone(), edited.text, edited.inverse));
    }
    for path in ops.iter().map(|(p, _)| p) {
        if !files.iter().any(|(f, _)| f == path) {
            return Err(vec![(
                path.clone(),
                Diagnostic::new(
                    "tamber-edit",
                    format!("`{path}` is no file of this design"),
                    1,
                    1,
                ),
            )]);
        }
    }
    changed.sort();
    changed.dedup();
    Ok(Planned {
        files: out,
        changed,
        class: if structure {
            Class::Rebuild
        } else {
            Class::Live
        },
        diff,
        synthetic: moved,
        values,
    })
}

/// A file's fold before and after, each with the synthetic tokens that
/// fold appended as `~<index>` (as the compiler folds them, dropping those
/// that do not). `moved` says the two sides fold different ones: the plan's
/// table would gain or lose an entry, which only a build makes.
fn folded(
    before: &str,
    after: &str,
    (exprs, params): (&[(String, bool)], &crate::params::Params),
    vocab: &Vocab,
) -> (Option<Folding>, Option<Folding>, bool) {
    let (b, kept_b) = foldable(before, exprs, params, vocab);
    let (a, kept_a) = foldable(after, exprs, params, vocab);
    let moved = b.is_some() && a.is_some() && kept_b != kept_a;
    (b, a, moved)
}

/// `text` folded with each synthetic token that folds with it, and which;
/// none when the file does not fold, or a strict one does not (the build
/// would refuse it).
fn foldable(
    text: &str,
    exprs: &[(String, bool)],
    params: &crate::params::Params,
    vocab: &Vocab,
) -> (Option<Folding>, Vec<usize>) {
    let Ok(design) = crate::parse(text) else {
        return (None, Vec::new());
    };
    // The params fold with the design, each in its `params` line's place.
    let design = crate::params::laid(&design, params);
    let mut kept: Vec<usize> = (0..exprs.len()).collect();
    loop {
        let mut checked = design.clone();
        for &i in &kept {
            let at = |value: String| crate::At {
                value,
                line: SYNTHETIC + i as u32,
                col: 1,
            };
            checked.tokens.push(crate::Token {
                name: at(format!("~{i}")),
                kind: None,
                value: at(exprs[i].0.clone()),
                whens: Vec::new(),
                meta: Vec::new(),
            });
        }
        match crate::check_design(checked, vocab) {
            Ok(f) => return (Some(f), kept),
            Err(all) => {
                let bad: Vec<usize> = all
                    .iter()
                    .filter(|d| d.line >= SYNTHETIC)
                    .map(|d| (d.line - SYNTHETIC) as usize)
                    .collect();
                if bad.is_empty() || bad.len() < all.len() || bad.iter().any(|&i| exprs[i].1) {
                    // The file itself does not fold.
                    return (None, Vec::new());
                }
                kept.retain(|i| !bad.contains(i));
            }
        }
    }
}

/// A synthetic token's line, past any file's (as the compiler numbers it).
const SYNTHETIC: u32 = 1 << 30;

/// The tokens whose values differ between two folds, or that one has and
/// the other lacks.
fn changed_tokens(before: Option<&Folding>, after: Option<&Folding>) -> Vec<String> {
    let values = |f: Option<&Folding>| -> Vec<(String, Vec<crate::Value>)> {
        f.map(|f| {
            f.tokens
                .iter()
                .map(|t| (t.name.clone(), t.values.clone()))
                .collect()
        })
        .unwrap_or_default()
    };
    let (a, b) = (values(before), values(after));
    let mut out = Vec::new();
    for (name, v) in &a {
        if b.iter()
            .find(|(n, _)| n == name)
            .is_none_or(|(_, w)| w != v)
        {
            out.push(name.clone());
        }
    }
    for (name, _) in &b {
        if !a.iter().any(|(n, _)| n == name) {
            out.push(name.clone());
        }
    }
    out
}

/// What decides live or rebuild, the plan's retained shape: the axes with
/// their values, the dials with their kinds, and each token the plan keeps
/// (every one with no `retention`) with its kind, its role and whether it
/// varies by an app axis (the plan's slices), in which tuples it is a role
/// the web draws in the accent, and any value the structure carries whole
/// (one naming a custom property); not another value.
fn shape(f: &Folding, retention: Option<&Retention>, vocab: &Vocab) -> String {
    let mut s = String::new();
    // An icon's sources are the plan's table and the asset set (D10).
    for i in &f.icons {
        s.push_str(&format!(
            "icon {} {:?} {:?} {:?}\n",
            i.name, i.sf, i.material, i.svg
        ));
    }
    for a in &f.design.axes {
        s.push_str(&format!(
            "axis {} {:?} {}\n",
            a.name.value, a.values, a.under_dark
        ));
    }
    for d in &f.design.dials {
        s.push_str(&format!("dial {} {}\n", d.name.value, d.kind.name()));
    }
    // A token's kind as folded: written or inferred (a value that turns a
    // length into a colour moves which rows may name it).
    for (t, declared) in f.tokens.iter().zip(&f.design.tokens) {
        // The name as written, which the plan's row carries (a role
        // override's spelling too: `canvastext` for `CanvasText`).
        let name = &declared.name.value;
        // A synthetic token (`~<index>`) the fold kept is the plan's.
        let kept = name.starts_with('~')
            || retention.is_none_or(|r| r.keeps(&f.design, name, t.role.as_deref(), t.kind));
        if !kept {
            continue;
        }
        let varies = t.values.windows(2).any(|w| w[0].css() != w[1].css());
        // Each tuple's: whether its value is a role the web draws in the
        // accent (the JS target writes those tuples apart, `--exact-accent`),
        // and a value the plan's structure carries whole (one naming a custom
        // property, `(--`, as a platform colour's `var(--…)` does).
        let accent: Vec<bool> = t
            .values
            .iter()
            .map(|v| match v {
                crate::Value::Color(crate::Folded::Host(crate::Host::Role(row))) => {
                    vocab.roles.iter().any(|r| {
                        let spelled = if r.system {
                            r.name.clone()
                        } else {
                            format!("-exact-{}", r.name)
                        };
                        r.accent && spelled.eq_ignore_ascii_case(row)
                    })
                }
                _ => false,
            })
            .collect();
        // And a type token's longhands and family, a family token's stack
        // (stage 6 C): a font a plan has not declared is a build's, and the
        // rows a type token writes are lowered (the lead's ruling, D6).
        let whole: Vec<String> = t
            .values
            .iter()
            .filter_map(|v| match v {
                crate::Value::Type(t) => Some(format!("{:?} {:?}", t.longhands(), t.family)),
                crate::Value::Family(f) => Some(f.clone()),
                v => Some(v.css()).filter(|c| c.contains("(--")),
            })
            .collect();
        s.push_str(&format!(
            "token {name} {} {:?} {} {varies} {accent:?} {whole:?}\n",
            t.kind.name(),
            t.role,
            t.fixed
        ));
        // A token a colour keyframe names, its value folded into the plan.
        if retention.is_some_and(|r| {
            r.folded.contains(name) || t.role.as_ref().is_some_and(|role| r.folded.contains(role))
        }) {
            let values: Vec<String> = t.values.iter().map(crate::Value::css).collect();
            s.push_str(&format!("folded {name} {values:?}\n"));
        }
    }
    s
}

/// One unified hunk: the lines `a` and `b` differ in, `-` and `+`, under
/// the path, with each side's start and count; a last line with no newline
/// is marked so.
fn file_diff(path: &str, a: &str, b: &str) -> String {
    let (x, y): (Vec<&str>, Vec<&str>) = (
        a.split_inclusive('\n').collect(),
        b.split_inclusive('\n').collect(),
    );
    let start = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
    let end = x[start..]
        .iter()
        .rev()
        .zip(y[start..].iter().rev())
        .take_while(|(p, q)| p == q)
        .count();
    let (old, new) = (&x[start..x.len() - end], &y[start..y.len() - end]);
    // An empty side starts at the line before it (unified diff's rule).
    let at = |n: usize| if n == 0 { start } else { start + 1 };
    let mut out = format!(
        "--- {path}\n+++ {path}\n@@ -{},{} +{},{} @@\n",
        at(old.len()),
        old.len(),
        at(new.len()),
        new.len()
    );
    for (sign, lines) in [('-', old), ('+', new)] {
        for l in lines {
            match l.strip_suffix('\n') {
                Some(l) => out.push_str(&format!("{sign}{l}\n")),
                None => out.push_str(&format!("{sign}{l}\n\\ No newline at end of file\n")),
            }
        }
    }
    out
}
