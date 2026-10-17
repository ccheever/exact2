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

pub mod astryx;
pub mod dtcg;
mod engine;
mod kinds;
pub mod shadcn;
pub mod tailwind;

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

/// One offered choice: what it means and its operations (none where the
/// design cannot carry it, and the note says why).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// What it overrides: a role's row spelling.
    pub role: String,
    /// The token it names.
    pub token: String,
    /// Its operations, empty when the change is outside the design.
    pub ops: Vec<Op>,
    /// Why there are no operations, when there are none.
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
    /// Why (`no Tamber kind: a breakpoint …`).
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
    /// Its kind as `add-token` writes it (`length`), or `None` for a
    /// colour, which its value says.
    pub kind: Option<&'static str>,
}

/// The operations that bring `design` to carry each of `tokens` (and each
/// axis in `axes`): none for a token already so, `set-token` lines for one
/// that differs (and `unset-token-when` for a `when` of the importer's own
/// axes, `owned`, that it no longer has), `add-token` for one it lacks.
pub fn reconcile(
    design: &Design,
    axes: &[(String, Vec<String>)],
    owned: &[&str],
    tokens: &[Mapped],
) -> Vec<Op> {
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
                kind: t.kind.map(str::to_string),
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
        // A `when` on the importer's own axes alone that the source no
        // longer gives is the source's to take back; one naming another axis
        // is the author's.
        for w in &have.whens {
            let own = w.conds.iter().all(|(a, _)| owned.contains(&a.as_str()));
            if own && !t.whens.iter().any(|(c, _)| same_conds(&w.conds, c)) {
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

/// Why a colour the CSS parser reads is no Tamber value yet (`none`
/// components, say), or `None` when it folds. Each value is checked once.
pub fn unfolded(value: &str) -> Option<String> {
    thread_local! {
        static SEEN: std::cell::RefCell<std::collections::HashMap<String, Option<String>>> =
            Default::default();
    }
    if let Some(known) = SEEN.with(|s| s.borrow().get(value).cloned()) {
        return known;
    }
    let design = format!("tamber probe\n\ntheme\n  probe = {value}\n");
    let why = match crate::check(&design, &crate::Vocab::default()) {
        Ok(_) => None,
        Err(all) => Some(
            all.first()
                .map_or_else(|| "it does not fold".to_string(), |d| d.message.clone()),
        ),
    };
    SEEN.with(|s| s.borrow_mut().insert(value.to_string(), why.clone()));
    why
}

/// A literal colour as Tamber can hold it, on one line: as written, or
/// with each `none` component written as the zero CSS reads it as (the
/// fold asks for the value written); `currentColor` is none.
pub fn colour_literal(value: &str) -> Option<String> {
    let v = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let parsed = exact_color::parse(&v)?;
    if matches!(parsed, exact_color::Parsed::Current) {
        return None;
    }
    if unfolded(&v).is_none() {
        return Some(v);
    }
    // Each component that is `none`, any case, wherever it stands.
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    let mut zeroed = String::new();
    let mut rest = v.as_str();
    while let Some(at) = rest.to_ascii_lowercase().find("none") {
        let before = rest[..at].chars().next_back();
        let after = rest[at + 4..].chars().next();
        zeroed.push_str(&rest[..at]);
        match before.is_none_or(|c| !ident(c)) && after.is_none_or(|c| !ident(c)) {
            true => zeroed.push('0'),
            false => zeroed.push_str(&rest[at..at + 4]),
        }
        rest = &rest[at + 4..];
    }
    zeroed.push_str(rest);
    (zeroed != v && unfolded(&zeroed).is_none()).then_some(zeroed)
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
pub(crate) fn strip_comments(css: &str) -> String {
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
                } else if c == q || c == '\n' {
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
    let (mut quote, mut nesting, mut escaped) = (None, 0i32, false);
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            match c {
                '\\' => escaped = true,
                // A string a newline ends unescaped is a bad string, ended.
                '\n' => quote = None,
                c if c == q => quote = None,
                _ => {}
            }
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' | '\'' => quote = Some(c),
            '(' | '[' => nesting += 1,
            ')' | ']' => nesting -= 1,
            '{' | '}' | ';' if nesting <= 0 => out.push((i, c)),
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
    // Declarations so far, as one block, before a nested rule's: the order
    // the source has them.
    let flush = |decls: &mut Vec<(String, String)>, out: &mut Vec<Block>| {
        if !decls.is_empty() && !context.is_empty() {
            out.push(Block {
                context: context.to_vec(),
                decls: std::mem::take(decls),
            });
        }
    };
    // The mark index of the `}` matching the `{` at mark `k`, if any.
    let matching = |k: usize| {
        let mut depth = 0;
        for (j, &(_, m)) in marks.iter().enumerate().skip(k) {
            match m {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(j);
                    }
                }
                _ => {}
            }
        }
        None
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
                let prelude = text[start..i].trim();
                let end = matching(k);
                // A custom property's value may hold braces: its declaration
                // runs on to the `;` after them.
                // Without its close, the brace is the value's own.
                if prelude.starts_with("--") && prelude.contains(':') {
                    k = end.map_or(k + 1, |j| j + 1);
                    continue;
                }
                // Unclosed: the rest is its body.
                let (j, close) = match end {
                    Some(j) => (j, marks[j].0),
                    None => (marks.len(), text.len()),
                };
                flush(&mut decls, out);
                let mut inner = context.to_vec();
                inner.push(prelude.to_string());
                let before = out.len();
                body(&text[i + 1..close], &inner, out);
                // An `@theme` with no custom properties still marks where
                // Tailwind writes the theme.
                if prelude.starts_with("@theme") && out.len() == before {
                    out.push(Block {
                        context: inner,
                        decls: Vec::new(),
                    });
                }
                start = (close + 1).min(text.len());
                k = j + 1;
            }
            // A stray close: passed over.
            _ => {
                start = i + 1;
                k += 1;
            }
        }
    }
    // The last declaration may have no `;`.
    if start < text.len() {
        declaration(&text[start..], &mut decls);
    }
    flush(&mut decls, out);
}

/// The design's own role overrides a changed value could split: one whose
/// chain (any spelling) reaches a token the import sets or a role an offer
/// in `offered` rebinds, and holds a `when` line anywhere, can fold to a
/// different colour per tuple once accepted (LLP 1121 D5). Each is named,
/// with no operations; one an offer rebinds, or one `offered` already
/// names under any spelling, is left out.
pub(super) fn split_roles(
    design: &Design,
    roles: &[(String, bool)],
    tokens: &[Mapped],
    offered: &[Offer],
) -> Vec<Offer> {
    let key = |name: &str| crate::edit::role_key(roles, name);
    let rebound = |k: &String| {
        offered
            .iter()
            .any(|o| !o.ops.is_empty() && key(&o.role).as_ref() == Some(k))
    };
    let mut out = Vec::new();
    for t in &design.tokens {
        let Some(k) = key(&t.name.value) else {
            continue;
        };
        if rebound(&k)
            || offered
                .iter()
                .any(|o| o.ops.is_empty() && key(&o.role).as_ref() == Some(&k))
        {
            continue;
        }
        let mut seen = Vec::new();
        chain(&t.name.value, design, roles, &mut seen);
        let reaches = seen.iter().any(|&i| {
            let n = &design.tokens[i].name.value;
            tokens.iter().any(|m| m.name == *n) || key(n).is_some_and(|k| rebound(&k))
        }) || seen
            .iter()
            .flat_map(|&i| words_of(&design.tokens[i]))
            .any(|w| tokens.iter().any(|m| m.name == w));
        if reaches && seen.iter().any(|&i| !design.tokens[i].whens.is_empty()) {
            out.push(Offer {
                role: t.name.value.clone(),
                token: t.value.value.clone(),
                ops: Vec::new(),
                note: Some(format!("the design's `{}` reaches a value this import changes and a `when` line: a role has one value (LLP 1121 D5), so the build may refuse it (`tamber-role-axis`) once accepted", t.name.value)),
            });
        }
    }
    out
}

/// The design's tokens `name` reaches, itself included, by their indices:
/// through every value it and they name, under any spelling of a role.
fn chain(name: &str, design: &Design, roles: &[(String, bool)], seen: &mut Vec<usize>) {
    let Ok(i) = crate::edit::token_index(design, roles, name) else {
        return;
    };
    if seen.contains(&i) {
        return;
    }
    seen.push(i);
    for w in words_of(&design.tokens[i]) {
        chain(w, design, roles, seen);
    }
}

/// The names a token's default and `when` values may hold.
fn words_of(t: &crate::Token) -> impl Iterator<Item = &str> {
    std::iter::once(t.value.value.as_str())
        .chain(t.whens.iter().map(|w| w.value.value.as_str()))
        .flat_map(|v| {
            v.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')))
                .filter(|w| !w.is_empty())
        })
}

/// The dial whose only use is the role `t`: rebinding `t` would leave it
/// unused, which the design refuses (`tamber-dial-inert`).
pub(super) fn orphaned_dial<'a>(design: &'a Design, t: &crate::Token) -> Option<&'a str> {
    design
        .dials
        .iter()
        .map(|d| d.name.value.as_str())
        .find(|d| {
            words_of(t).any(|w| w == *d)
                && !design
                    .tokens
                    .iter()
                    .filter(|o| !std::ptr::eq(*o, t))
                    .any(|o| words_of(o).any(|w| w == *d))
        })
}
