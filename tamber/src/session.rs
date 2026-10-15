//! A transaction's pure part (LLP 1121 D13, stage 3): operations over a
//! design's files, checked as a whole, giving each file's new text, the
//! inverse per file, the tokens whose values change, the diff, and whether
//! running builds take it in the live table or rebuild. The server
//! (`contract tamber serve`) owns the digests, the lock, the journal and
//! every write; this owns none.
//!
//! `live` is judged by the edit's closure (D13, stage 4): the design's shape
//! (axes, dials, the tokens with their kinds and roles) unchanged, no role
//! override's value changed (the hosts take their role tables at boot), and
//! the app's synthetic tokens (Contract rows' expressions over tokens,
//! folded with the design) re-folded with it, their new values in the
//! answer. Kinds substituted at build (durations, easings) wait for stage 6.

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
}

/// The app's synthetic tokens (D3): Contract rows' expressions over tokens,
/// folded with the design file `file`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Synthetic<'a> {
    /// The design file they fold with.
    pub file: &'a str,
    /// Each one's expression.
    pub exprs: &'a [String],
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
        let exprs: &[String] = if *path == synthetic.file {
            synthetic.exprs
        } else {
            &[]
        };
        let (before, after, moved_set) = folded(text, &edited.text, exprs, vocab);
        structure |= moved_set;
        let tokens = changed_tokens(before.as_ref(), after.as_ref());
        // A role override's value is the hosts' at boot: a change reaches
        // it, directly or through what it names, only by a rebuild.
        structure |= tokens.iter().any(|t| {
            [&before, &after].iter().any(|f| {
                f.as_ref()
                    .and_then(|f| f.tokens.iter().find(|c| c.name == *t))
                    .is_some_and(|c| c.role.is_some())
            })
        });
        for t in tokens {
            match t.strip_prefix('~').and_then(|i| i.parse::<usize>().ok()) {
                Some(i) => {
                    let values = after
                        .as_ref()
                        .and_then(|f| f.tokens.iter().find(|c| c.name == t))
                        .map(|c| c.values.clone())
                        .unwrap_or_default();
                    moved.push((exprs[i].clone(), values));
                }
                None => changed.push((path.clone(), t)),
            }
        }
        structure |= match (&before, &after) {
            (Some(b), Some(a)) => shape(b) != shape(a),
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
    })
}

/// A file's fold before and after, each with the synthetic tokens that
/// fold appended as `~<index>` (as the compiler folds them, dropping those
/// that do not). `moved` says the two sides fold different ones: the plan's
/// table would gain or lose an entry, which only a build makes.
fn folded(
    before: &str,
    after: &str,
    exprs: &[String],
    vocab: &Vocab,
) -> (Option<Folding>, Option<Folding>, bool) {
    let (b, kept_b) = foldable(before, exprs, vocab);
    let (a, kept_a) = foldable(after, exprs, vocab);
    let moved = b.is_some() && a.is_some() && kept_b != kept_a;
    (b, a, moved)
}

/// `text` folded with each synthetic token that folds with it, and which.
fn foldable(text: &str, exprs: &[String], vocab: &Vocab) -> (Option<Folding>, Vec<usize>) {
    let Ok(design) = crate::parse(text) else {
        return (None, Vec::new());
    };
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
                value: at(exprs[i].clone()),
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
                if bad.is_empty() || bad.len() < all.len() {
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
    let values = |f: Option<&Folding>| -> Vec<(String, Vec<crate::Folded>)> {
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

/// What decides live or rebuild: the axes with their values, the dials
/// with their kinds, the tokens with their kinds and roles; not a value.
fn shape(f: &Folding) -> String {
    let mut s = String::new();
    for a in &f.design.axes {
        s.push_str(&format!(
            "axis {} {:?} {}\n",
            a.name.value, a.values, a.under_dark
        ));
    }
    for d in &f.design.dials {
        s.push_str(&format!("dial {} {}\n", d.name.value, d.kind.name()));
    }
    for (t, declared) in f.tokens.iter().zip(&f.design.tokens) {
        s.push_str(&format!(
            "token {} {:?} {:?} {}\n",
            t.name, declared.kind, t.role, t.fixed
        ));
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
