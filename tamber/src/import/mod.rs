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
pub use crate::export_dials::{marks_of, DialChange, Impact, Pin};
use crate::syntax::same_conds;
use crate::{Design, Diagnostic};

pub mod astryx;
pub mod brand;
pub mod dtcg;
mod engine;
mod kinds;
mod landing;
mod reach;
pub mod shadcn;
pub mod tailwind;

/// An import, ready to propose.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    /// A re-import's dial changes: each dial set, and what it moves (D14).
    pub dials: Vec<DialChange>,
    /// A re-import's literals over lines that followed others (D14's mark).
    pub pins: Vec<Pin>,
    /// What decided a choice the import made that the source didn't say
    /// (a Pencil number's kind, by the properties that read it).
    pub notes: Vec<String>,
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

/// Each token's source values, by its name: what is reported when the
/// token is refused where it lands (`landing::settle`).
pub(super) type Sources = Vec<(String, Vec<Unmapped>)>;

/// What an importer's pass for the kinds besides colour writes: the tokens,
/// the report, and each token's sources.
pub(super) type Written<'a> = (&'a mut Vec<Mapped>, &'a mut Vec<Unmapped>, &'a mut Sources);

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
    // (each design token found by name in one step: the first so named, as a scan finds)
    let mut by_name: std::collections::HashMap<&str, &crate::syntax::Token> =
        std::collections::HashMap::new();
    for d in &design.tokens {
        by_name.entry(d.name.value.as_str()).or_insert(d);
    }
    for t in tokens {
        let Some(&have) = by_name.get(t.name.as_str()) else {
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

/// Two values the same, as written (spacing aside, but a string's own).
fn same(a: &str, b: &str) -> bool {
    squash(a) == squash(b)
}

/// `v` with each run of spaces outside a string one space.
pub(crate) fn squash(v: &str) -> String {
    let (mut out, mut quote, mut space) = (String::new(), None, false);
    let mut chars = v.trim().chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                out.push(c);
                if c == '\\' {
                    out.extend(chars.next());
                } else if c == q {
                    quote = None;
                }
            }
            None if c.is_whitespace() => space = true,
            None => {
                if std::mem::take(&mut space) {
                    out.push(' ');
                }
                if c == '"' || c == '\'' {
                    quote = Some(c);
                }
                out.push(c);
            }
        }
    }
    out
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
    css_read(css).0
}

/// A rule whose body is not read, nested past [`NESTING`] or past what its
/// enclosing rules may copy ([`COPIED`]): its prelude (its first 80
/// characters; those around it are not kept, as each such rule would copy
/// them), its line in the source, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deep {
    /// Its prelude, cut at 80 characters.
    pub prelude: String,
    /// Its line, from 1.
    pub line: usize,
    /// Why it is not read.
    pub why: &'static str,
}

/// [`css_blocks`], and the rules nested too deep to read, for an importer
/// to report.
pub fn css_read(css: &str) -> (Vec<Block>, Vec<Deep>) {
    let (text, comments) = stripped(css);
    let mut read = Reading::default();
    let marks = marks(&text);
    // Each `{`'s matching `}`, by mark index, found once (as a scan from
    // the `{` counting braces finds it).
    let mut matched = vec![usize::MAX; marks.len()];
    let mut open = Vec::new();
    for (k, &(_, c)) in marks.iter().enumerate() {
        match c {
            '{' => open.push(k),
            '}' => {
                if let Some(o) = open.pop() {
                    matched[o] = k;
                }
            }
            _ => {}
        }
    }
    let marked = Marked {
        text: &text,
        marks: &marks,
        matched: &matched,
    };
    body(
        &marked,
        (0, text.len()),
        (0, marks.len()),
        0,
        &mut Vec::new(),
        &mut read,
    );
    let Reading { out, mut deep, .. } = read;
    deep.sort_by_key(|d| d.1);
    // Each one's line: the source's, counting a comment's newlines.
    let mut at = (0, 1, comments.iter().peekable());
    let deep = deep
        .into_iter()
        .map(|(prelude, offset, why)| {
            let (from, line, rest) = &mut at;
            *line += text.as_bytes()[*from..offset]
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
            while let Some(&&(o, lines)) = rest.peek() {
                if o >= offset {
                    break;
                }
                *line += lines;
                rest.next();
            }
            *from = offset;
            Deep {
                prelude,
                line: *line,
                why,
            }
        })
        .collect();
    (out, deep)
}

/// How deep rules nest before a deeper one's body is not read but reported:
/// past any stylesheet's, short of the stack (each level reads its body
/// again).
const NESTING: usize = 64;

/// The longest custom property name, and value or prelude, read: past them
/// a declaration or a rule is reported, not copied about (no real one is
/// near them).
const NAME: usize = 256;
const VALUE: usize = crate::css::MAX_VALUE_BYTES;
/// The report of a value past [`VALUE`], which a CSS importer refuses as
/// the checker's bound (`source_read`).
const LONG_VALUE: &str = "a custom property's value past 4 KiB";
/// A long name's value nested past the checker's bound, likewise refused.
const DEEP_VALUE: &str = "a custom property's value nested past 32 functions";

/// How many bytes of enclosing preludes the blocks may copy, all told: each
/// block keeps its context, so 64 long preludes around many small rules
/// would multiply.
const COPIED: usize = 1 << 23;

/// What [`body`] has read so far: the blocks, the rules not read (each its
/// prelude, offset and why), and the bytes of context the blocks copied.
#[derive(Default)]
struct Reading {
    out: Vec<Block>,
    deep: Vec<(String, usize, &'static str)>,
    copied: usize,
}

impl Reading {
    /// A block with `context`, or, past [`COPIED`], a rule not read. Its
    /// cost is its context, kept once and copied into the label of each of
    /// its declarations, and each one's report.
    fn push(&mut self, context: &[String], decls: Vec<(String, String)>, at: usize) {
        let cost = context.iter().map(|p| p.len() + 1).sum::<usize>() * (1 + 2 * decls.len());
        if self.copied + cost > COPIED {
            let prelude = context
                .last()
                .map_or(String::new(), |p| p.chars().take(80).collect());
            self.deep
                .push((prelude, at, "its enclosing rules copy past 8 MiB"));
            return;
        }
        self.copied += cost;
        self.out.push(Block {
            context: context.to_vec(),
            decls,
        });
    }
}

/// `css` without `/* … */`, a comment's opener inside a string kept.
pub(crate) fn strip_comments(css: &str) -> String {
    stripped(css).0
}

/// [`strip_comments`], and each comment's place in what is left (the
/// space it leaves) with the newlines it held.
fn stripped(css: &str) -> (String, Vec<(usize, usize)>) {
    let mut comments = Vec::new();
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
                let (mut last, mut lines) = (' ', 0);
                for n in chars.by_ref() {
                    if last == '*' && n == '/' {
                        break;
                    }
                    lines += usize::from(n == '\n');
                    last = n;
                }
                comments.push((out.len(), lines));
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
    (out, comments)
}

/// The byte index of each top-level `{`, `}` and `;` in `text`: outside
/// strings and parentheses (a stray `)` closes none, so every rule's body
/// reads its marks as the whole text does).
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
            ')' | ']' => nesting = (nesting - 1).max(0),
            '{' | '}' | ';' if nesting == 0 => out.push((i, c)),
            _ => {}
        }
    }
    out
}

/// A stylesheet's text with its marks and each `{`'s matching `}`.
struct Marked<'a> {
    text: &'a str,
    marks: &'a [(usize, char)],
    matched: &'a [usize],
}

/// The blocks of the body `bytes` (start, end) of `css`, whose marks are
/// `range` (start, end) of its marks, under `context`; `at` is where its
/// prelude starts, for a report. Each level reads its own marks once and
/// passes over a nested rule's, so the whole stylesheet is read once.
fn body(
    css: &Marked<'_>,
    (lo, hi): (usize, usize),
    (from, to): (usize, usize),
    at: usize,
    context: &mut Vec<String>,
    read: &mut Reading,
) {
    let (text, marks) = (css.text, css.marks);
    let mut decls = Vec::new();
    let (mut start, mut k) = (lo, from);
    // A custom property, or, its name or value past what one may be, a
    // value reported.
    let declaration =
        |(from, to): (usize, usize), decls: &mut Vec<(String, String)>, read: &mut Reading| {
            let segment = &text[from..to];
            if let Some((name, value)) = segment.split_once(':') {
                let name = name.trim();
                if name.starts_with("--") && name.len() > 2 {
                    let value = value.trim();
                    if name.len() > NAME || value.len() > VALUE {
                        let at = from + (segment.len() - segment.trim_start().len());
                        // (the value first: one past the checker's bound
                        // refuses the import, however long its name)
                        let why = match (value.len() > VALUE, crate::css::over_limit(value)) {
                            (true, _) => LONG_VALUE,
                            (false, Some(_)) => DEEP_VALUE,
                            (false, None) => "a custom property's name past 256 bytes",
                        };
                        read.deep.push((name.chars().take(80).collect(), at, why));
                        return;
                    }
                    decls.push((name.to_string(), value.to_string()));
                }
            }
        };
    // Declarations so far, as one block, before a nested rule's: the order
    // the source has them.
    let flush = |decls: &mut Vec<(String, String)>, context: &[String], read: &mut Reading| {
        if !decls.is_empty() && !context.is_empty() {
            read.push(context, std::mem::take(decls), at);
        }
    };
    // The mark index of the `}` matching the `{` at mark `k`, if any, in
    // this body.
    let matching = |k: usize| Some(css.matched[k]).filter(|&j| j < to);
    while k < to {
        let (i, c) = marks[k];
        match c {
            ';' => {
                declaration((start, i), &mut decls, read);
                start = i + 1;
                k += 1;
            }
            '{' => {
                let raw = &text[start..i];
                let prelude = raw.trim();
                let prelude_at = start + (raw.len() - raw.trim_start().len());
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
                    None => (to, hi),
                };
                flush(&mut decls, context, read);
                let past = match (context.len() >= NESTING, prelude.len() > VALUE) {
                    (true, _) => Some("nested past 64 rules"),
                    (_, true) => Some("its prelude past 4 KiB"),
                    _ => None,
                };
                if let Some(why) = past {
                    let prelude = prelude.chars().take(80).collect();
                    read.deep.push((prelude, prelude_at, why));
                    start = (close + 1).min(hi);
                    k = j + 1;
                    continue;
                }
                // The context borrowed down, copied only into a block kept.
                context.push(prelude.to_string());
                let before = read.out.len();
                body(css, (i + 1, close), (k + 1, j), prelude_at, context, read);
                // An `@theme` with no custom properties still marks where
                // Tailwind writes the theme.
                if prelude.starts_with("@theme") && read.out.len() == before {
                    read.push(context, Vec::new(), prelude_at);
                }
                context.pop();
                start = (close + 1).min(hi);
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
    if start < hi {
        declaration((start, hi), &mut decls, read);
    }
    flush(&mut decls, context, read);
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
    // Over the design's tokens, each naming the tokens its values name:
    // those reaching a token the import sets or a role an offer rebinds, and
    // those reaching a `when` line, for every token at once.
    let index = reach::Index::new(design, roles);
    let edges: Vec<Vec<usize>> = design
        .tokens
        .iter()
        .map(|t| words_of(t).filter_map(|w| index.get(w)).collect())
        .collect();
    let set: std::collections::HashSet<&str> = tokens.iter().map(|m| m.name.as_str()).collect();
    let reaches = reach::reaching(
        &edges,
        design.tokens.iter().enumerate().filter_map(|(i, t)| {
            let n = &t.name.value;
            (set.contains(n.as_str())
                || key(n).is_some_and(|k| rebound(&k))
                || words_of(t).any(|w| set.contains(w)))
            .then_some(i)
        }),
    );
    let varies = reach::reaching(
        &edges,
        design
            .tokens
            .iter()
            .enumerate()
            .filter_map(|(i, t)| (!t.whens.is_empty()).then_some(i)),
    );
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
        if index
            .get(&t.name.value)
            .is_some_and(|i| reaches[i] && varies[i])
        {
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

/// The names a token's default and `when` values may hold.
fn words_of(t: &crate::Token) -> impl Iterator<Item = &str> {
    std::iter::once(t.value.value.as_str())
        .chain(t.whens.iter().map(|w| w.value.value.as_str()))
        .flat_map(reach::words)
}

/// The dial whose only use is the role `t`: rebinding `t` would leave it
/// unused, which the design refuses (`tamber-dial-inert`).
pub(super) fn orphaned_dial<'a>(design: &'a Design, t: &crate::Token) -> Option<&'a str> {
    let others: std::collections::HashSet<&str> = design
        .tokens
        .iter()
        .filter(|o| !std::ptr::eq(*o, t))
        .flat_map(words_of)
        .collect();
    design
        .dials
        .iter()
        .map(|d| d.name.value.as_str())
        .find(|d| words_of(t).any(|w| w == *d) && !others.contains(d))
}

/// The design checker's bound at import (theme2's condition on the checker
/// bound): a source value past `css::over_limit`, mapped or reported,
/// refuses the import with that bound's id, naming the source's name and
/// never echoing the value. The importers' own budget bounds their work
/// reading the source; this bounds what they hand the checker.
pub(crate) fn bounded(import: Result<Import, Vec<Diagnostic>>) -> Result<Import, Vec<Diagnostic>> {
    let import = import?;
    let over = |name: &str, value: &str| {
        crate::css::over_limit(value).map(|(id, why)| {
            let name: String = name.chars().take(80).collect();
            vec![Diagnostic::new(id, format!("`{name}`: {why}"), 1, 1)]
        })
    };
    // (an unmapped value is a report's text, which an importer may join
    // from two source values each within the bound: the source values are
    // checked where they are read, `source_bound`)
    let ops = import
        .ops
        .iter()
        .chain(import.offers.iter().flat_map(|o| &o.ops));
    for op in ops {
        let values: Vec<(&str, &str)> = match op {
            Op::SetToken { name, value, .. }
            | Op::SetDial { name, value }
            | Op::SetParam { name, value, .. } => vec![(name, value)],
            Op::AddToken {
                name, value, whens, ..
            } => std::iter::once((name.as_str(), value.as_str()))
                .chain(whens.iter().map(|(_, v, _)| (name.as_str(), v.as_str())))
                .collect(),
            _ => Vec::new(),
        };
        for (name, value) in values {
            if let Some(no) = over(name, value) {
                return Err(no);
            }
        }
    }
    Ok(import)
}

/// Each source value an importer reads, before it maps any: one past
/// `css::over_limit` refuses the import with that bound's id (theme2's
/// condition), naming the source's name and never echoing the value, though
/// the importer might have folded it to something short.
pub(crate) fn source_bound<'a>(
    values: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<(), Vec<Diagnostic>> {
    for (name, value) in values {
        if let Some((id, why)) = crate::css::over_limit(value) {
            let name: String = name.chars().take(80).collect();
            return Err(vec![Diagnostic::new(id, format!("`{name}`: {why}"), 1, 1)]);
        }
    }
    Ok(())
}

/// Every string a JSON value holds, depth first (a composite token's parts).
pub(crate) fn json_strings<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match v {
        serde_json::Value::String(s) => out.push(s),
        serde_json::Value::Array(a) => a.iter().for_each(|v| json_strings(v, out)),
        serde_json::Value::Object(o) => o.values().for_each(|v| json_strings(v, out)),
        _ => {}
    }
}

/// A CSS importer's source as `css_read` read it: a value past the checker's
/// bound, which the reader reports rather than copies, or one it copied,
/// refuses the import with `tamber-value-size` ([`source_bound`]).
pub(crate) fn source_read(blocks: &[Block], deep: &[Deep]) -> Result<(), Vec<Diagnostic>> {
    if let Some(d) = deep
        .iter()
        .find(|d| d.why == LONG_VALUE || d.why == DEEP_VALUE)
    {
        let name: String = d.prelude.chars().take(80).collect();
        let (id, why) = match d.why == LONG_VALUE {
            true => (
                "tamber-value-size",
                format!("a value is at most {VALUE} bytes"),
            ),
            false => (
                "tamber-value-depth",
                format!(
                    "a value nests at most {} functions deep",
                    crate::css::MAX_VALUE_DEPTH
                ),
            ),
        };
        return Err(vec![Diagnostic::new(
            id,
            format!("`{name}`: {why}"),
            d.line as u32,
            1,
        )]);
    }
    source_bound(
        blocks
            .iter()
            .flat_map(|b| b.decls.iter().map(|(n, v)| (n.as_str(), v.as_str()))),
    )
}

/// A reimport's source read as JSON (`refuse` saying why it isn't), its
/// `$value`s against the checker's bound ([`source_bound`]): past it, the
/// reimport is refused with the bound's id, as an import is.
pub(crate) fn dtcg_doc(
    text: &str,
    refuse: &dyn Fn(String) -> Vec<Diagnostic>,
) -> Result<serde_json::Value, Vec<Diagnostic>> {
    let doc = serde_json::from_str(text).map_err(|e| refuse(format!("not JSON: {e}")))?;
    // Every string it holds (an export keeps a mode's values in its
    // `$extensions`, beside each `$value`), and every key, which a refusal
    // may name (a context's).
    let mut strings = Vec::new();
    json_strings(&doc, &mut strings);
    source_bound(strings.into_iter().map(|v| ("the reimported file", v)))?;
    if let Some((id, why)) = json_keys(&doc).find_map(|k| crate::css::over_bytes("a name", k)) {
        return Err(vec![Diagnostic::new(id, why, 1, 1)]);
    }
    Ok(doc)
}

/// Every key a JSON value's objects hold, depth first.
fn json_keys(v: &serde_json::Value) -> Box<dyn Iterator<Item = &str> + '_> {
    match v {
        serde_json::Value::Object(o) => Box::new(
            o.iter()
                .flat_map(|(k, v)| std::iter::once(k.as_str()).chain(json_keys(v))),
        ),
        serde_json::Value::Array(a) => Box::new(a.iter().flat_map(json_keys)),
        _ => Box::new(std::iter::empty()),
    }
}
