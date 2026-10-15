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
/// that differs (and `unset-token-when` for a `when` it no longer has),
/// `add-token` for one it lacks.
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
        // A `when` the source no longer gives is the source's to take back.
        for w in &have.whens {
            if !t.whens.iter().any(|(c, _)| same_conds(&w.conds, c)) {
                ops.push(Op::UnsetTokenWhen {
                    name: t.name.clone(),
                    when: w.conds.clone(),
                });
            }
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

/// One block's custom properties: the preludes enclosing it, outermost
/// first (`@layer base`, `@media (prefers-color-scheme: dark)`, `:root`), and
/// its own `--name: value` declarations in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The preludes, outermost first; the last is the block's own selector.
    pub context: Vec<String>,
    /// Its custom properties.
    pub decls: Vec<(String, String)>,
}

/// A stylesheet's custom properties by block, read with CSS's own rules:
/// comments, strings (with escapes) and parentheses hide braces and
/// semicolons; a block's own declarations count beside its nested rules.
pub fn css_blocks(css: &str) -> Vec<Block> {
    let text = strip_comments(css);
    let mut out = Vec::new();
    body(&text, &[], &mut out);
    out
}

/// `css` without `/* … */`, a comment's opener inside a string kept.
fn strip_comments(css: &str) -> String {
    let mut out = String::new();
    let mut chars = css.chars().peekable();
    let mut quote = None;
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                out.push(c);
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        out.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '/' && chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for n in chars.by_ref() {
                    if last == '*' && n == '/' {
                        break;
                    }
                    last = n;
                }
                out.push(' ');
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                }
                out.push(c);
            }
        }
    }
    out
}

/// The byte index of each top-level `{`, `}` and `;` in `text`: outside
/// strings and parentheses.
fn marks(text: &str) -> Vec<(usize, char)> {
    let mut out = Vec::new();
    let (mut quote, mut parens, mut escaped) = (None, 0i32, false);
    for (i, c) in text.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '(' => parens += 1,
            ')' => parens -= 1,
            '{' | '}' | ';' if parens <= 0 => out.push((i, c)),
            _ => {}
        }
    }
    out
}

fn body(text: &str, context: &[String], out: &mut Vec<Block>) {
    let marks = marks(text);
    let mut decls = Vec::new();
    let (mut start, mut k) = (0, 0);
    let declaration = |segment: &str, decls: &mut Vec<(String, String)>| {
        if let Some((name, value)) = segment.split_once(':') {
            let name = name.trim();
            if name.starts_with("--") && name.len() > 2 {
                decls.push((name.to_string(), value.trim().to_string()));
            }
        }
    };
    while k < marks.len() {
        let (i, c) = marks[k];
        match c {
            ';' => {
                declaration(&text[start..i], &mut decls);
                start = i + 1;
                k += 1;
            }
            '{' => {
                // Its matching close, counting the marks between.
                let mut depth = 0;
                let mut end = None;
                for (j, &(at, m)) in marks.iter().enumerate().skip(k) {
                    match m {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                end = Some((j, at));
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Some((j, close)) = end else { break };
                let prelude = text[start..i].trim().to_string();
                let mut inner = context.to_vec();
                inner.push(prelude);
                body(&text[i + 1..close], &inner, out);
                start = close + 1;
                k = j + 1;
            }
            // A stray close: what follows is past this body.
            _ => break,
        }
    }
    // The last declaration may have no `;`.
    if start < text.len() {
        declaration(&text[start..], &mut decls);
    }
    if !decls.is_empty() && !context.is_empty() {
        out.push(Block {
            context: context.to_vec(),
            decls,
        });
    }
}
