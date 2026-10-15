//! Importers (LLP 1121 D14, stage 3): another system's theme read as a
//! transaction of `add-token`, `set-token` and `add-axis` operations, the role
//! overrides it means offered apart, and an unmapped report. Nothing is
//! dropped silently: the source's values number exactly the mapped ones plus
//! the reported ones, checked before the import is returned.
//!
//! An import is planned against the design as it is: a token it would write
//! with the value the design has already is no operation, so re-importing an
//! unchanged source proposes nothing.

use crate::edit::{Op, WhenLine};
use crate::{Design, Diagnostic};

pub mod shadcn;

/// An import, ready to propose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// The transaction: the tokens (and any axis) the source maps to.
    pub ops: Vec<Op>,
    /// Role overrides and other choices the source means, each offered on
    /// its own so a person can decline it (D1: a design stays the
    /// platform's unless chosen otherwise).
    pub offers: Vec<Offer>,
    /// Every source value not mapped, and why.
    pub unmapped: Vec<Unmapped>,
    /// The count the import was checked against.
    pub counted: Counted,
}

/// One offered choice: what it means and its operations (none where it is
/// not the design's to make, such as the root's `accent-color`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// What it overrides: a role's row spelling, or `accent-color`.
    pub role: String,
    /// The token it names.
    pub token: String,
    /// Its operations, empty when the change is outside the design.
    pub ops: Vec<Op>,
    /// What to do by hand when there are no operations.
    pub note: Option<String>,
}

/// A source value not mapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unmapped {
    /// Where in the source (`:root`, `.dark`, `.theme-blue`).
    pub block: String,
    /// The source's name (`--radius`).
    pub name: String,
    /// Its value as written.
    pub value: String,
    /// Why (`kind pending (stage 6)` …).
    pub why: String,
    /// The token it would become, when there is one.
    pub token: Option<String>,
}

/// The source's values, mapped and reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counted {
    /// Every value the source declares.
    pub source: usize,
    /// Those an operation (or an unchanged token) carries.
    pub mapped: usize,
    /// Those in the unmapped report.
    pub reported: usize,
}

/// A source name's segments as a Tamber name under `prefix`: each segment
/// that starts with a digit takes `s` (`blue-500` → `blue.s500`, D3).
pub fn tamber_name(prefix: &str, segments: &[&str]) -> String {
    std::iter::once(prefix.to_string())
        .chain(segments.iter().map(|s| match s.chars().next() {
            Some(c) if c.is_ascii_digit() => format!("s{s}"),
            _ => s.to_string(),
        }))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

/// A token an importer maps a source to: its default value and `when` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    /// Its Tamber name.
    pub name: String,
    /// Its default.
    pub value: String,
    /// Its `when` lines: conditions and value.
    pub whens: Vec<(Vec<(String, String)>, String)>,
}

/// The operations that bring `design` to carry each of `tokens` (and each
/// axis in `axes`): none for a token already so, `set-token` lines for one
/// that differs, `add-token` for one it lacks.
pub fn reconcile(design: &Design, axes: &[(String, Vec<String>)], tokens: &[Mapped]) -> Vec<Op> {
    let mut ops = Vec::new();
    for (name, values) in axes {
        match design.axes.iter().find(|a| a.name.value == *name) {
            None => ops.push(Op::AddAxis {
                name: name.clone(),
                values: values.clone(),
                under_dark: false,
            }),
            Some(a) => {
                for v in values.iter().filter(|v| !a.values.contains(v)) {
                    ops.push(Op::AddAxisValue {
                        axis: name.clone(),
                        value: v.clone(),
                        before: None,
                    });
                }
            }
        }
    }
    for t in tokens {
        let Some(have) = design.tokens.iter().find(|d| d.name.value == t.name) else {
            ops.push(Op::AddToken {
                name: t.name.clone(),
                kind: None,
                value: t.value.clone(),
                meta: Vec::new(),
                whens: t
                    .whens
                    .iter()
                    .map(|(c, v)| -> WhenLine { (c.clone(), v.clone(), None) })
                    .collect(),
                comment: None,
                before: None,
            });
            continue;
        };
        if same(&have.value.value, &t.value) {
            // Unchanged.
        } else {
            ops.push(Op::SetToken {
                name: t.name.clone(),
                when: Vec::new(),
                value: t.value.clone(),
                at: None,
                comment: None,
            });
        }
        for (conds, value) in &t.whens {
            let line = have.whens.iter().find(|w| same_conds(&w.conds, conds));
            if line.is_none_or(|w| !same(&w.value.value, value)) {
                ops.push(Op::SetToken {
                    name: t.name.clone(),
                    when: conds.clone(),
                    value: value.clone(),
                    at: None,
                    comment: None,
                });
            }
        }
    }
    ops
}

/// Two values the same, as written (spacing aside).
fn same(a: &str, b: &str) -> bool {
    let squash = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    squash(a) == squash(b)
}

fn same_conds(a: &[(String, String)], b: &[(String, String)]) -> bool {
    a.len() == b.len() && a.iter().all(|c| b.contains(c))
}

/// The counted check: every source value mapped or reported, exactly.
pub fn counted(source: usize, mapped: usize, reported: usize) -> Result<Counted, Vec<Diagnostic>> {
    if mapped + reported != source {
        return Err(vec![Diagnostic::new(
            "tamber-import-count",
            format!("the source has {source} values but {mapped} were mapped and {reported} reported: an importer drops nothing"),
            1,
            1,
        )]);
    }
    Ok(Counted {
        source,
        mapped,
        reported,
    })
}

/// A stylesheet's custom properties by block: each selector's (or at-rule
/// prelude's) `--name: value` declarations, comments removed, nested
/// at-rules (`@layer base { … }`) entered.
pub fn css_blocks(css: &str) -> Vec<(String, Vec<(String, String)>)> {
    let text = strip_comments(css);
    let mut out = Vec::new();
    blocks(&text, &mut out);
    out
}

fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at + 2..]
            .find("*/")
            .map_or("", |e| &rest[at + 2 + e + 2..]);
    }
    out.push_str(rest);
    out
}

fn blocks(text: &str, out: &mut Vec<(String, Vec<(String, String)>)>) {
    let mut i = 0;
    let b = text.as_bytes();
    while i < b.len() {
        let Some(open) = text[i..].find('{').map(|o| i + o) else {
            return;
        };
        // The prelude is what follows the last `;` or `}` before the brace.
        let prelude = text[i..open]
            .rsplit([';', '}'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let mut depth = 0;
        let mut close = open;
        for (k, c) in text[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = open + k;
                        break;
                    }
                }
                _ => {}
            }
        }
        if close == open {
            return;
        }
        let body = &text[open + 1..close];
        if body.contains('{') {
            blocks(body, out);
        } else {
            let decls: Vec<(String, String)> = body
                .split(';')
                .filter_map(|d| {
                    let (k, v) = d.split_once(':')?;
                    let k = k.trim();
                    k.starts_with("--")
                        .then(|| (k.to_string(), v.trim().to_string()))
                })
                .collect();
            if !decls.is_empty() {
                out.push((prelude, decls));
            }
        }
        i = close + 1;
    }
}
