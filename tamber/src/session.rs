//! A transaction's pure part (LLP 1121 D13, stage 3): operations over a
//! design's files, checked as a whole, giving each file's new text, the
//! inverse per file, the tokens whose values change, the diff, and whether
//! running builds take it in the live table or rebuild. The server
//! (`contract tamber serve`) owns the digests, the lock, the journal and
//! every write; this owns none.

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

/// A transaction planned: every edited file's new text and inverse.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// Each edited file: its path, its new text, its inverse.
    pub files: Vec<(String, String, Vec<Op>)>,
    /// The tokens whose values change, added or removed, by name.
    pub changed: Vec<String>,
    /// What running builds do with it.
    pub class: Class,
    /// A diff per edited file, the lines removed (`-`) and added (`+`).
    pub diff: String,
}

/// A refusal: the file it is in, and the checker's or the edit's
/// diagnostics.
pub type Refused = Vec<(String, Diagnostic)>;

/// Plan `ops` (each with the file it edits) over `files` (each path with
/// its text). Every file's operations are applied in order and the file
/// checked whole; a refusal changes nothing. `undo` marks an inverse the
/// server issued: its restores, and nothing else.
pub fn plan(
    files: &[(String, String)],
    ops: &[(String, Op)],
    vocab: &Vocab,
    undo: bool,
) -> Result<Planned, Refused> {
    let mut out = Vec::new();
    let mut changed = Vec::new();
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
        let (before, after) = (
            crate::check(text, vocab).ok(),
            crate::check(&edited.text, vocab).ok(),
        );
        changed.extend(changed_tokens(before.as_ref(), after.as_ref()));
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
    })
}

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

/// One hunk: the lines `a` and `b` differ in, `-` and `+`, under the path.
fn file_diff(path: &str, a: &str, b: &str) -> String {
    let (x, y): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let start = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
    let end = x[start..]
        .iter()
        .rev()
        .zip(y[start..].iter().rev())
        .take_while(|(p, q)| p == q)
        .count();
    let mut out = format!(
        "--- {path}\n+++ {path}\n@@ -{} +{} @@\n",
        start + 1,
        start + 1
    );
    for l in &x[start..x.len() - end] {
        out.push_str(&format!("-{l}\n"));
    }
    for l in &y[start..y.len() - end] {
        out.push_str(&format!("+{l}\n"));
    }
    out
}
