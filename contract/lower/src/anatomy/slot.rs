//! The slot (LLP 1139 D4, stage 12 C2): an exported library component that
//! declares `slot` places `children` once, unconditionally, never into
//! another component's slot; and no node from its view's root down to
//! that placement (the slot's chain) may hide what fills it, in any branch
//! of a class, any state layer, any tuple of the design or any column of
//! the base scale. A value the check can't fold is refused, never assumed
//! safe.
//!
//! [`denied`] is the one checker, the whole rule for one `(row, value)`: the
//! compiler asks it of each chain row's every folded value, and the design
//! server asks it of a candidate fold's (stage 12 D) from the chain the
//! Tamber IR records ([`chain`]): the same rows, the same values, a value
//! no literal says written [`UNFOLDED`].

use crate::{tags, LowerError};
use contract_syntax::{Attr, Component, Expr, File, Node, Span};
use exact_kernel::style::relative;

/// The rows of a chain node the deny-list reads, as authored.
const ROWS: [&str; 10] = [
    "display",
    "visibility",
    "opacity",
    "width",
    "height",
    "max-width",
    "max-height",
    "scale",
    "font-size",
    "line-height",
];

/// The attributes the deny-list reads.
const ATTRS: [&str; 4] = ["hidden", "inert", "aria-hidden", "popover"];

/// A value no literal says (a computation): what [`denied`] refuses.
pub const UNFOLDED: &str = "?";

/// The `display` values that show a box.
const DISPLAYS: [&str; 10] = [
    "flex",
    "inline-flex",
    "block",
    "inline",
    "inline-block",
    "grid",
    "inline-grid",
    "contents",
    "flow-root",
    "list-item",
];

/// The keywords a length or number row may say that are no zero.
const KEYWORDS: [&str; 12] = [
    "auto",
    "none",
    "normal",
    "inherit",
    "initial",
    "unset",
    "revert",
    "max-content",
    "min-content",
    "fit-content",
    "stretch",
    "full",
];

/// A length's or number's value, `0px` or `0.5` or `0%`: its number, the
/// unit one CSS writes (letters or `%`, never an exponent's `e`); `None` for
/// any other text.
fn number(text: &str) -> Option<f64> {
    let t = text.trim();
    let b = t.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let digits = |i: &mut usize| {
        let start = *i;
        while b.get(*i).is_some_and(u8::is_ascii_digit) {
            *i += 1;
        }
        *i > start
    };
    let whole = digits(&mut i);
    let mut frac = false;
    if b.get(i) == Some(&b'.') {
        i += 1;
        frac = digits(&mut i);
    }
    if !whole && !frac {
        return None;
    }
    // An exponent only where a digit follows (`1e-9`), so `0em` is `em`.
    if matches!(b.get(i), Some(b'e' | b'E')) {
        let mut j = i + 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        if b.get(j).is_some_and(u8::is_ascii_digit) {
            i = j;
            digits(&mut i);
        }
    }
    let (n, unit) = t.split_at(i);
    let unit = unit.to_ascii_lowercase();
    let known = unit.is_empty()
        || unit == "%"
        || [
            "px", "em", "rem", "ch", "lh", "vw", "vh", "vmin", "vmax", "svh", "lvh", "dvh", "svw",
            "lvw", "dvw", "pt", "pc", "in", "cm", "mm", "q",
        ]
        .contains(&unit.as_str());
    known.then(|| n.parse().ok()).flatten()
}

/// A length's number, a math function's folded by the kernel's `calc()`;
/// `None` when the text is neither (a keyword, a token's name, a percentage
/// inside a function, which only layout resolves).
fn length(text: &str) -> Option<f64> {
    number(text).or_else(|| exact_kernel::style::scale::fold(text).map(f64::from))
}

/// Why `value` on chain row `row` (a deny-list row, `font`, `transform`, or
/// one of its attributes) hides the slot's fill, or why the check can't
/// tell ([`UNFOLDED`], a text it can't read); `None` when it doesn't (LLP
/// 1139 D4). The whole rule: the compiler's and the server's.
pub fn denied(row: &str, value: &str) -> Option<String> {
    let v = value.trim();
    if v == UNFOLDED {
        return Some(format!("`{row}` is computed, so the check can't fold it"));
    }
    let unread = || Some(format!("`{row}: {v}` is a value the check can't read"));
    let keyword = || KEYWORDS.iter().any(|k| v.eq_ignore_ascii_case(k));
    match row {
        "display" | "visibility" => {
            let hides = if row == "display" { "none" } else { "hidden" };
            if v.eq_ignore_ascii_case(hides) {
                return Some(format!("`{row}: {v}` hides it"));
            }
            let shows: &[&str] = if row == "display" {
                &DISPLAYS
            } else {
                &["visible", "collapse"]
            };
            match shows
                .iter()
                .chain(&["inherit", "initial", "unset", "revert"])
                .any(|k| v.eq_ignore_ascii_case(k))
            {
                true => None,
                false => unread(),
            }
        }
        "opacity" | "scale" => match number(v) {
            Some(n) if n <= 0.0 => Some(format!("`{row}: {v}` makes it vanish")),
            Some(_) => None,
            None if keyword() => None,
            None => unread(),
        },
        "width" | "height" | "max-width" | "max-height" | "font-size" | "line-height" => {
            match length(v) {
                Some(n) => (n == 0.0).then(|| format!("`{row}: {v}` gives it no room")),
                None if keyword() => None,
                // A text style's (its WebKit alias too) or a named length's.
                None if relative::text_style(v).is_some()
                    || relative::named_length(v).is_some() =>
                {
                    None
                }
                None => unread(),
            }
        }
        "font" => match exact_kernel::style::theme::typeface::parts(v) {
            Some(p) => denied("font-size", p.size).or_else(|| denied("line-height", p.leading?)),
            // A text style alone (`font="-apple-system-body"`).
            None if relative::text_style(v).is_some() => None,
            None => unread(),
        },
        "transform" => match exact_kernel::svg::transform::TransformList::parse(v) {
            Some(t) => {
                let [a, b, c, d, ..] = t.matrix().map(f64::from);
                let det = a * d - b * c;
                match det.is_finite() {
                    true => {
                        (det.abs() < 1e-12).then(|| format!("`transform: {v}` collapses its area"))
                    }
                    false => unread(),
                }
            }
            None => unread(),
        },
        "popover" => Some("`popover` takes it out of the page until shown".into()),
        "hidden" | "inert" | "aria-hidden" => match v {
            "false" => None,
            "true" | "" => Some(format!("`{row}` hides it")),
            _ => unread(),
        },
        _ => None,
    }
}

/// Where the component places `children`, refused unless once and
/// unconditionally (`lower-anatomy-slot-placement`); with the chain, each
/// element from the view's root down to the one holding it.
fn placement<'c>(c: &'c Component, out: &mut Vec<LowerError>) -> Option<Vec<&'c Node>> {
    struct Found<'c> {
        span: Span,
        chain: Vec<&'c Node>,
        why: Option<&'static str>,
    }
    fn walk<'c>(
        nodes: &'c [Node],
        path: &mut Vec<&'c Node>,
        why: Option<&'static str>,
        found: &mut Vec<Found<'c>>,
    ) {
        for n in nodes {
            match n {
                Node::Children { span } => found.push(Found {
                    span: *span,
                    chain: path.clone(),
                    why,
                }),
                Node::Element { children, .. } => {
                    path.push(n);
                    walk(children, path, why, found);
                    path.pop();
                }
                Node::Use { children, .. } => walk(
                    children,
                    path,
                    why.or(Some(
                        "passed into another component's slot (no forwarding in v1)",
                    )),
                    found,
                ),
                Node::When {
                    then, otherwise, ..
                } => {
                    let w = why.or(Some("inside an `if`"));
                    walk(then, path, w, found);
                    walk(otherwise, path, w, found);
                }
                Node::Each { body, .. } => walk(body, path, why.or(Some("inside a `for`")), found),
                Node::Match { some, none, .. } => {
                    let w = why.or(Some("inside a `match`"));
                    walk(&some.1, path, w, found);
                    walk(none, path, w, found);
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(&c.view, &mut Vec::new(), None, &mut found);
    let comp = &c.name;
    let refuse = |message: String, span: Span| LowerError {
        id: "lower-anatomy-slot-placement",
        message,
        span,
    };
    if found.is_empty() {
        out.push(refuse(
            format!("`{comp}` declares `slot` and places no `children`: a library component places its slot once (LLP 1139 D4)"),
            c.span,
        ));
        return None;
    }
    for f in &found[1..] {
        out.push(refuse(
            format!("`{comp}` places `children` again (the first at line {}): a library component places its slot once (LLP 1139 D4)", found[0].span.line),
            f.span,
        ));
    }
    if let Some(why) = found[0].why {
        out.push(refuse(
            format!("`children` in `{comp}` is {why}: a library component places its slot unconditionally (LLP 1139 D4)"),
            found[0].span,
        ));
    }
    (found.len() == 1 && found[0].why.is_none()).then(|| found.swap_remove(0).chain)
}

/// Whether a tag draws SVG, where `transform` is the element's own (a
/// box's is refused already, `lower-attr-tag`).
fn is_svg(tag: &str) -> bool {
    tags::tag(tag).is_some_and(|t| t.node_type.name().starts_with("Svg"))
}

/// The chain rows an attribute stands for: a deny-list row (a shorthand's
/// longhands among them), `font`, an SVG element's `transform`, or one of
/// the attributes.
fn deny_rows(tag: &str, svg: bool, a: &Attr) -> Vec<String> {
    let row = contract_syntax::state::state_of(&a.name).map_or(a.name.as_str(), |(_, row)| row);
    if ATTRS.contains(&row) || row == "font" || ROWS.contains(&row) {
        return vec![row.to_string()];
    }
    if row == "transform" {
        return match svg || is_svg(tag) {
            true => vec![row.to_string()],
            false => Vec::new(),
        };
    }
    let ids = tags::rows(&Attr {
        name: row.to_string(),
        ..a.clone()
    });
    ROWS.iter()
        .filter(|r| {
            tags::rows(&Attr {
                name: r.to_string(),
                ..a.clone()
            })
            .first()
            .is_some_and(|id| ids.contains(id))
        })
        .map(|r| r.to_string())
        .collect()
}

/// One row of a chain node: what [`denied`] reads, the value as written
/// (each leaf of a `?:`, `match` or `let`; [`UNFOLDED`] for a computation),
/// and where.
struct Row {
    row: String,
    value: String,
    span: Span,
}

/// Each chain node's tag and rows: every row its classes bring (each style
/// a choice names, each state layer) that the node doesn't set itself in
/// the same layer, and its own (LLP 1121 D8's precedence).
fn chain_rows(
    c: &Component,
    file: &File,
    out: &mut Vec<LowerError>,
) -> Option<Vec<(String, Vec<Row>)>> {
    let chain = placement(c, out)?;
    let mut in_svg = false;
    Some(
        chain
            .into_iter()
            .filter_map(|node| {
                let Node::Element { tag, attrs, .. } = node else {
                    return None;
                };
                // Inside an `svg`, a `text` is SVG text, whose `transform`
                // is its own.
                in_svg |= tag == "svg";
                let svg = in_svg || is_svg(tag);
                let mut rows = Vec::new();
                let classes = super::classes(attrs)
                    .into_iter()
                    .filter_map(|(name, _)| file.styles.iter().rev().find(|s| s.name == name))
                    .flat_map(|s| {
                        // A class's `font` always: a type token's leading is
                        // a longhand `surviving` can't see, so the check
                        // keeps it (failing closed).
                        let kept = super::surviving(&s.attrs, attrs);
                        s.attrs.iter().filter(move |a| {
                            a.name == "font" || kept.iter().any(|k| std::ptr::eq(*k, *a))
                        })
                    });
                for a in classes.chain(attrs.iter()) {
                    let mut leaves = Vec::new();
                    crate::theme::leaves(&a.value, &mut Vec::new(), &mut leaves);
                    for r in deny_rows(tag, svg, a) {
                        for leaf in &leaves {
                            let value = match leaf {
                                Expr::Str(s, _) => s.clone(),
                                Expr::Number(n, _) => exact_num::Shortest(*n).to_string(),
                                Expr::Bool(b, _) => b.to_string(),
                                Expr::None(_) => continue,
                                _ => UNFOLDED.to_string(),
                            };
                            rows.push(Row {
                                row: r.clone(),
                                value,
                                span: a.span,
                            });
                        }
                    }
                }
                Some((tag.clone(), rows))
            })
            .collect(),
    )
}

/// Each refusal of `c`'s slot (LLP 1139 D4): its placement, then each
/// chain node's rows, every value [`denied`] asked: a token's every value
/// over the tuples, a base name's in every column of the scale (`fold`).
pub(crate) fn check(
    c: &Component,
    file: &File,
    fold: &dyn Fn(&str) -> Option<Vec<String>>,
) -> Vec<LowerError> {
    let mut out = Vec::new();
    let Some(chain) = chain_rows(c, file, &mut out) else {
        return out;
    };
    let comp = &c.name;
    for (tag, rows) in chain {
        let mut said: Vec<(Span, String)> = Vec::new();
        for r in rows {
            let values = fold(&r.value).unwrap_or_else(|| vec![r.value.clone()]);
            for v in values {
                let Some(why) = denied(&r.row, &v) else {
                    continue;
                };
                let why = match v == r.value {
                    true => why,
                    false => format!("{why} (`{}` is `{v}` in some tuple or column)", r.value),
                };
                if said.iter().any(|(s, w)| *s == r.span && *w == why) {
                    continue;
                }
                said.push((r.span, why.clone()));
                out.push(LowerError {
                    id: "lower-anatomy-slot",
                    message: format!("a `{tag}` on the slot's chain in `{comp}`: {why}, and the slot's fill would not show (LLP 1139 D4)"),
                    span: r.span,
                });
            }
        }
    }
    out
}

/// The slot's chain as the Tamber IR records it ([`super::Described`]):
/// each node's tag and its rows, `(row, value)` as written, for the server
/// to fold and ask [`denied`] of.
pub(crate) fn chain(c: &Component, file: &File) -> Vec<(String, Vec<(String, String)>)> {
    chain_rows(c, file, &mut Vec::new())
        .unwrap_or_default()
        .into_iter()
        .map(|(tag, rows)| (tag, rows.into_iter().map(|r| (r.row, r.value)).collect()))
        .collect()
}

/// One exported library component's slot chain, as the Tamber IR records
/// it ([`chain`]): what the design server re-checks on a write.
#[derive(Debug, Clone, Default)]
pub struct SlotChain {
    /// The component, as diagnostics name it.
    pub component: String,
    /// Each node, root first: its tag and its rows, `(row, value)`.
    pub nodes: Vec<(String, Vec<(String, String)>)>,
}

/// Every exported library component's slot chain, indexed by the token
/// names their rows' values reach, built once per snapshot.
#[derive(Debug, Clone, Default)]
pub struct SlotChains {
    chains: Vec<SlotChain>,
    by_token: std::collections::HashMap<String, Vec<usize>>,
}

impl SlotChains {
    /// From the Tamber IR file's `library` block: each entry `library:
    /// true` with its `chain` (`tag`, `rows`, `tokens`).
    pub fn from_ir(library: &serde_json::Value) -> SlotChains {
        let mut out = SlotChains::default();
        let empty = Vec::new();
        for entry in library.as_array().unwrap_or(&empty) {
            if entry["library"] != serde_json::Value::Bool(true) {
                continue;
            }
            let mut chain = SlotChain {
                component: entry["name"].as_str().unwrap_or_default().to_string(),
                nodes: Vec::new(),
            };
            let mut tokens: Vec<String> = Vec::new();
            for node in entry["chain"].as_array().unwrap_or(&empty) {
                let rows = node["rows"]
                    .as_array()
                    .unwrap_or(&empty)
                    .iter()
                    .filter_map(|r| Some((r[0].as_str()?.to_string(), r[1].as_str()?.to_string())))
                    .collect::<Vec<_>>();
                tokens.extend(rows.iter().map(|(_, v)| v.trim().to_string()));
                tokens.extend(
                    node["tokens"]
                        .as_array()
                        .unwrap_or(&empty)
                        .iter()
                        .filter_map(|t| t.as_str().map(str::to_string)),
                );
                chain
                    .nodes
                    .push((node["tag"].as_str().unwrap_or_default().to_string(), rows));
            }
            let i = out.chains.len();
            for t in tokens {
                let list = out.by_token.entry(t).or_default();
                if !list.contains(&i) {
                    list.push(i);
                }
            }
            out.chains.push(chain);
        }
        out
    }
}

/// The one checker on a candidate fold (LLP 1139 D4; stage 12 D's server):
/// each chain row whose value names a token of `folding` (a base name the
/// design leaves unset in every column of the scale), asked [`denied`] of
/// every value, a refusal in the compiler's words at the token's line.
/// Only the chains `touched` names reach, every chain when it is empty.
/// `before` is the fold the write replaces: a refusal whose token has no
/// line in the candidate (a write that removed the app's override) is
/// said at the line it had there. Pure: no I/O.
pub fn check_slots(
    folding: &exact_tamber::Folding,
    before: Option<&exact_tamber::Folding>,
    chains: &SlotChains,
    touched: &[&str],
) -> Vec<exact_tamber::Diagnostic> {
    // Closed over references (theme2's D2 condition): a chain is touched
    // when a token its rows name reaches a touched name through any chain
    // of tokens, a base name among them (`cards.pad = space.md` and a write
    // to `space.md`).
    let reached = |name: &str| reach(folding, name);
    let which: Vec<usize> = match touched.is_empty() {
        true => (0..chains.chains.len()).collect(),
        false => {
            let mut all: Vec<usize> = chains
                .by_token
                .iter()
                .filter(|(token, _)| {
                    touched.contains(&token.as_str())
                        || reached(token).iter().any(|r| touched.contains(&r.as_str()))
                })
                .flat_map(|(_, list)| list.iter().copied())
                .collect();
            all.sort_unstable();
            all.dedup();
            all
        }
    };
    let columns = |v: &str| -> Option<Vec<String>> {
        crate::scale::ranged(v)
            .map(|cols| cols.into_iter().map(|(_, n)| crate::scale::px(n)).collect())
    };
    let mut out = Vec::new();
    for chain in which.into_iter().map(|i| &chains.chains[i]) {
        for (tag, rows) in &chain.nodes {
            for (row, value) in rows {
                let value = value.trim();
                let values: Vec<String> = match folding.tokens.iter().find(|t| t.name == value) {
                    Some(t) => t
                        .values
                        .iter()
                        .flat_map(|v| {
                            let css = v.css();
                            columns(&css).unwrap_or_else(|| vec![css])
                        })
                        .collect(),
                    // A base name the design leaves unset (one a write just
                    // removed), or an expression over base names: the
                    // scale's columns, as the compiler folds it.
                    // Anything else as written, as the compiler judges a
                    // row it can't fold (an expression over tokens is a
                    // value the check can't read; a literal is itself).
                    None => columns(value).unwrap_or_else(|| vec![value.to_string()]),
                };
                let at = line(folding, before, value);
                // In the compiler's words: the folded value said only when
                // it isn't the row's own.
                if let Some(why) = values.iter().find_map(|v| {
                    denied(row, v).map(|w| match v == value {
                        true => w,
                        false => format!("{w} (`{value}` is `{v}` in some tuple or column)"),
                    })
                }) {
                    out.push(exact_tamber::Diagnostic {
                        id: "lower-anatomy-slot",
                        message: format!("a `{tag}` on the slot's chain in `{}`: {why}, and the slot's fill would not show (LLP 1139 D4)", chain.component),
                        line: at.0,
                        col: at.1,
                    });
                }
            }
        }
    }
    out
}

/// Where a refusal of a row naming `value` is said: the line, in
/// `before`, of a name the row reaches there (the token, what it reaches,
/// the base names an expression reads) that the candidate no longer writes
/// (the removed override); else its token's line in the candidate, or an
/// expression's first token's, or the first app line a package token it
/// names reaches; else the design's start.
fn line(
    folding: &exact_tamber::Folding,
    before: Option<&exact_tamber::Folding>,
    value: &str,
) -> (u32, u32) {
    // The app design's lines only: a package's are banded past them
    // (`theme::packages::LINE`), no line of the app's file.
    let at = |f: &exact_tamber::Folding, name: &str| {
        f.design
            .tokens
            .iter()
            .find(|d| d.name.value == name)
            .map(|d| (d.name.line, d.name.col))
            .filter(|(line, _)| *line < crate::theme::packages::LINE)
    };
    let removed = before.and_then(|before| {
        let bases = exact_kernel::style::scale::named_in(value);
        let words: Vec<String> = crate::theme::scan::words(value)
            .flat_map(|w| std::iter::once(w.to_string()).chain(reach(before, w)))
            .collect();
        std::iter::once(value.to_string())
            .chain(reach(before, value))
            .chain(bases.into_iter().map(str::to_string))
            .chain(words)
            .filter(|n| at(folding, n).is_none())
            .find_map(|n| at(before, &n))
    });
    removed
        .or_else(|| at(folding, value))
        .or_else(|| crate::theme::scan::words(value).find_map(|w| at(folding, w)))
        // A package token over an app line (`lib.w = space.md` with the
        // app's `space.md`): the first name it reaches that the app writes.
        .or_else(|| {
            crate::theme::scan::words(value)
                .flat_map(|w| reach(folding, w))
                .find_map(|n| at(folding, &n))
        })
        .unwrap_or((1, 1))
}

/// Every token and base name `name` reaches through `folding`'s tokens:
/// the names its values name, theirs in turn, and the unset base names
/// each reads.
pub fn reach(folding: &exact_tamber::Folding, name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut todo = vec![name.to_string()];
    while let Some(n) = todo.pop() {
        let Some(t) = folding.tokens.iter().find(|t| t.name == n) else {
            continue;
        };
        for r in t.names.iter().chain(&t.bases) {
            if !out.contains(r) && r != name {
                out.push(r.clone());
                todo.push(r.clone());
            }
        }
    }
    out
}
