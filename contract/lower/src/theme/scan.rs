//! What an app's view says about its design's tokens (LLP 1121 D3, D7):
//! the names its colour literals write, the expressions over tokens it
//! writes (each a synthetic token), and whether a colour row is computed,
//! when a string from data may name any token.

use super::role_named;
use contract_syntax::{Attr, Component, Expr, File, Node, Span, TemplatePart};
use exact_kernel::{StyleCodec, StyleId};
use exact_tamber::Design;
use std::collections::HashSet;

/// CSS's colour functions: an expression over a token is one of them.
const COLOUR_FNS: &[&str] = &[
    "light-dark",
    "color-mix",
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "color",
    "contrast-color",
    "-exact-platform-color",
];

/// A row whose value is or holds a colour.
pub(crate) fn colour_codec(row: StyleId) -> bool {
    matches!(
        row.codec(),
        StyleCodec::ColorValue
            | StyleCodec::KeywordColor
            | StyleCodec::Rgba8
            | StyleCodec::Color2
            | StyleCodec::SymbolPalette
            | StyleCodec::Paint
            | StyleCodec::BackgroundImage
            | StyleCodec::MaskImage
            | StyleCodec::BoxShadow
            | StyleCodec::TextShadow
            | StyleCodec::Filter
    )
}

/// An attribute that may hold a colour: a colour row, or a shorthand
/// (`border`, `outline`, `background`) that sets one.
fn colour_attr(name: &str) -> bool {
    match crate::tags::attr(name) {
        Some(crate::tags::AttrTarget::Styles(rows)) => rows.iter().any(|r| colour_codec(*r)),
        Some(crate::tags::AttrTarget::Shorthand) => true,
        _ => false,
    }
}

/// The words of a value, as a name is written in one.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
        .filter(|w| !w.is_empty())
}

#[derive(Default)]
pub(crate) struct Said {
    /// Each expression over a token a literal writes, first where written.
    pub synthetic: Vec<(String, Span)>,
    /// A colour row's value is computed somewhere: any token may be named.
    pub computed: bool,
    /// Every word the colour literals write.
    words: HashSet<String>,
}

impl Said {
    /// What `file`'s styles, `root`'s view and the `fn`s and derives they
    /// read say of `design`'s tokens.
    pub(crate) fn of(file: &File, root: &Component, design: &Design) -> Said {
        let declared: Vec<&str> = design
            .tokens
            .iter()
            .map(|t| t.name.value.as_str())
            .collect();
        let roles: Vec<u8> = declared
            .iter()
            .filter_map(|n| role_named(n).map(|(id, _)| id))
            .collect();
        let is_token = |w: &str| {
            declared.contains(&w) || role_named(w).is_some_and(|(id, _)| roles.contains(&id))
        };
        let mut said = Said::default();
        let mut literals: Vec<(String, Span)> = Vec::new();
        // Every literal is read for names (a token kept that no row needs
        // costs a line); a computed colour is what keeps them all.
        let mut attrs = |attrs: &[Attr], said: &mut Said| {
            for a in attrs {
                let computed = leaves(&a.value, &mut literals);
                said.computed |= computed && colour_attr(&a.name);
            }
        };
        for s in &file.styles {
            attrs(&s.attrs, &mut said);
        }
        let mut stack: Vec<&Node> = root.view.iter().collect();
        while let Some(node) = stack.pop() {
            match node {
                Node::Element {
                    attrs: a, children, ..
                } => {
                    attrs(a, &mut said);
                    stack.extend(children);
                }
                Node::When {
                    then, otherwise, ..
                } => stack.extend(then.iter().chain(otherwise)),
                Node::Each { body, .. } => stack.extend(body),
                Node::Match { some, none, .. } => stack.extend(some.1.iter().chain(none)),
                Node::Use { children, .. } => stack.extend(children),
                Node::Children { .. } => {}
            }
        }
        // A computed row may read a `fn`, a derive or a state: their literals too.
        let bindings = root.derives.iter().chain(&root.states).map(|d| &d.expr);
        for body in file.fns.iter().map(|f| &f.body).chain(bindings) {
            leaves(body, &mut literals);
        }
        for (text, span) in literals {
            said.words
                .extend(words(&text).filter(|w| is_token(w)).map(str::to_string));
            for expr in expressions(&text, &is_token) {
                if !said.synthetic.iter().any(|(e, _)| e == expr) {
                    said.synthetic.push((expr.to_string(), span));
                }
            }
        }
        said
    }

    /// Whether the view names `name`: a token a literal writes, or a
    /// synthetic token's expression.
    pub(crate) fn names(&self, name: &str) -> bool {
        self.words.contains(name) || self.synthetic.iter().any(|(e, _)| e == name)
    }
}

/// `e`'s string leaves, through `?:`, `match`, `let` and a template's text;
/// whether any leaf is computed.
fn leaves(e: &Expr, out: &mut Vec<(String, Span)>) -> bool {
    match e {
        Expr::Str(text, span) => {
            out.push((text.clone(), *span));
            false
        }
        Expr::Ternary(_, yes, no, _) => leaves(yes, out) | leaves(no, out),
        Expr::Match { some, none, .. } => leaves(some, out) | leaves(none, out),
        Expr::Let { body, .. } => leaves(body, out),
        Expr::Template(parts, span) => {
            for p in parts {
                if let TemplatePart::Text(text) = p {
                    out.push((text.clone(), *span));
                }
            }
            true
        }
        _ => true,
    }
}

/// Each outermost colour function in `text` that names a token.
fn expressions<'t>(text: &'t str, is_token: &dyn Fn(&str) -> bool) -> Vec<&'t str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(open) = text[from..].find('(').map(|k| from + k) {
        let start = text[..open]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .map_or(0, |k| k + 1);
        let mut depth = 0;
        let mut end = None;
        for (k, c) in text[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + k + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        let call = &text[start..end];
        let name = &text[start..open];
        if COLOUR_FNS.iter().any(|f| f.eq_ignore_ascii_case(name))
            && words(&text[open..end]).any(is_token)
        {
            out.push(call);
            from = end;
        } else {
            from = open + 1;
        }
    }
    out
}
