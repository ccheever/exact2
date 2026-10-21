//! What an app's view says about its design's tokens (LLP 1121 D3, D7):
//! the names its literals write, the expressions over tokens they write
//! (each a synthetic token), and whether a colour row is computed, when a
//! string from data may name any token.

use super::role_named;
use contract_syntax::{Attr, Component, Expr, File, Node, Span, Stmt, TemplatePart};
use exact_kernel::{StyleCodec, StyleId};
use exact_tamber::Design;
use std::collections::BTreeSet;

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
    // A state part's row (LLP 1121 D8) is the row it is.
    let name = contract_syntax::state::state_of(name).map_or(name, |(_, row)| row);
    match crate::tags::attr(name) {
        Some(crate::tags::AttrTarget::Styles(rows)) => rows.iter().any(|r| colour_codec(*r)),
        Some(crate::tags::AttrTarget::Shorthand) => true,
        _ => false,
    }
}

/// The words of a value, as a name is written in one.
pub(crate) fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || "-_.".contains(c)))
        .filter(|w| !w.is_empty())
}

/// An expression over tokens a literal writes.
#[derive(Clone)]
pub(crate) struct Synthetic {
    /// Its whole text, which names it in the plan.
    pub expr: String,
    /// Where it is first written.
    pub span: Span,
    /// Written in a colour row's value, so it must fold; one only a `fn`, a
    /// state or another attribute writes is dropped when it does not.
    pub strict: bool,
}

#[derive(Default)]
pub(crate) struct Said {
    /// Each expression over a token a literal writes, first where written.
    pub synthetic: Vec<Synthetic>,
    /// A colour row's value is computed somewhere: any token may be named.
    pub computed: bool,
    /// A length or number row's value is computed and may be a string
    /// somewhere: any length or number token may be named (LLP 1121 D7).
    pub computed_measure: bool,
    /// Every word the literals write, a token's name or not: what D7's
    /// retention reads (`tamber::Retention`), for this design or an edit of it.
    pub words: BTreeSet<String>,
    /// Every word a colour keyframe writes: a token one names is folded into
    /// the keyframes, so its value is the plan's structure.
    pub folded: BTreeSet<String>,
    /// Each element's style row whose value is literal (strings or numbers,
    /// or a `?:` or `match` whose arms all are; never a `let`), an SVG
    /// element's as lowering renames and coerces it: what the design server checks
    /// again, the compiler's way, when a write reaches a token it names
    /// ([`super::rows::check`]); a font row with its node's fonts; and each
    /// whose value is computed, with its type where it is known.
    pub rows: Vec<super::rows::Said>,
}

/// Whether lowering's `check_svg` passes the element `tag` with `attrs`
/// under `parent` (as `nodes.rs` gives it: its class's rows, then its own,
/// lengths coerced; `coerced`, its own so); an unknown class, or any
/// animation, is no.
fn prepared(
    file: &File,
    tag: &str,
    in_svg: bool,
    parent: Option<&str>,
    attrs: &[Attr],
    coerced: &[Attr],
    span: Span,
) -> bool {
    let style = |name: &str| {
        file.styles
            .iter()
            .rfind(|s| s.name == name)
            .map(|s| s.attrs.as_slice())
    };
    let Ok(class) = crate::class::rows(attrs, &style) else {
        return false;
    };
    let joined: Vec<Attr>;
    let rejoined: Vec<Attr>;
    let reboxed: Vec<Attr>;
    let expanded = match class {
        Some((_, rows)) => {
            joined = rows
                .into_iter()
                .chain(attrs.iter().filter(|a| a.name != "class").cloned())
                .collect();
            rejoined =
                crate::svg::coerce_lengths(tag, in_svg, &joined).unwrap_or_else(|| joined.clone());
            match crate::box_transform::lower(tag, in_svg, &rejoined) {
                Ok(Some(rows)) => {
                    reboxed = rows;
                    &reboxed
                }
                Ok(None) => &rejoined,
                Err(_) => return false,
            }
        }
        None => coerced,
    };
    // Motion is checked against the keyframes, and composed before this
    // (`compose_animation`): any is taken as refused.
    let refused = |a: &Attr| match a.name.contains("animation") || a.name.contains("transition") {
        true => Err(crate::LowerError {
            id: "",
            message: String::new(),
            span: a.span,
        }),
        false => Ok(()),
    };
    crate::svg::check_svg_with(tag, in_svg, parent, expanded, span, &refused).is_ok()
}

impl Said {
    /// What `file`'s styles, `root`'s view and the `fn`s, states, derives and
    /// actions they read say of `design`'s tokens.
    /// `declared`: the app's fonts, which a font row is judged with;
    /// `typing`: the root's scope and the file's shapes, which type and
    /// judge a computed row as lowering does.
    pub(crate) fn of(
        file: &File,
        root: &Component,
        design: &Design,
        declared: &std::sync::Arc<super::rows::Declared>,
        typing: &super::rows::Typing,
        scope: &contract_types::Scope,
    ) -> Said {
        let mut literals: Vec<(String, Span, bool)> = Vec::new();
        let mut computed = false;
        let mut computed_measure = false;
        let mut rows: Vec<super::rows::Said> = Vec::new();
        let mut attrs = |attrs: &[Attr]| {
            for a in attrs {
                let colour = colour_attr(&a.name);
                computed |= colour && !literal(&a.value);
                computed_measure |=
                    measure_attr(&a.name) && !literal(&a.value) && !number(&a.value);
                // A colour row's own value must fold; a literal elsewhere in
                // it (a condition's, a call's argument) may not be one.
                if colour {
                    values(&a.value, &mut literals);
                }
                exprs(&a.value, &mut |e| strings(e, false, &mut literals));
            }
        };
        for s in &file.styles {
            attrs(&s.attrs);
        }
        // Each node, with whether an `svg` encloses it and its parent's tag,
        // as lowering's `svg::in_svg` reads them (an SVG element's rows are
        // renamed and coerced first, `svg::coerce_lengths`: `fePointLight x`
        // is the prop `lightX`, a unitless `rect x` a number); and whether a
        // `for` or a `match` binds a name around it, which the root's scope
        // does not type.
        // A grouped list's label folded from an `if` chain of texts
        // (`grouped::folds`) holds one text lowering builds anew, at its
        // first arm's place: the chain's rows are left to the build
        // (`left`).
        let mut stack: Vec<(&Node, bool, Option<&str>, bool, bool)> = root
            .view
            .iter()
            .map(|n| (n, false, None, false, false))
            .collect();
        while let Some((node, svg, parent, local, left)) = stack.pop() {
            match node {
                Node::Element {
                    tag,
                    attrs: a,
                    children,
                    span,
                    ..
                } => {
                    attrs(a);
                    let in_svg = crate::svg::in_svg(svg, parent);
                    let coerced = crate::svg::coerce_lengths(tag, in_svg, a);
                    // A box's `transform` as the rows lowering checks
                    // instead (`box_transform::lower`); refused, the build's.
                    let boxed =
                        crate::box_transform::lower(tag, in_svg, coerced.as_deref().unwrap_or(a));
                    // An element lowering refuses before its rows (its SVG
                    // content model and attribute placement, its class's rows
                    // with its own, `svg::check_svg_with`; an animation's
                    // names, the keyframes', taken as refused) is the build's,
                    // with everything inside it.
                    let own = coerced.as_deref().unwrap_or(a);
                    let own = match &boxed {
                        Ok(Some(rows)) => rows.as_slice(),
                        _ => own,
                    };
                    let left = left
                        || boxed.is_err()
                        || !prepared(file, tag, in_svg, parent, a, own, *span);
                    // An element's own rows, which the build checks where it
                    // lowers the element (a style's are checked where a node
                    // applies it, and may be overridden there).
                    // A font row with its node's fonts, or not at all when
                    // they aren't known (an unknown class, the build's). A
                    // computed row with its type and the root's typing, or
                    // not at all when a local name is around (its scope is
                    // the region's) or its form doesn't say its type.
                    let mut fonts = None;
                    // A module tag's row lowering refuses is never judged
                    // (`native::refused`, before any check of it).
                    for r in own
                        .iter()
                        .filter(|r| !left && crate::native::refused(tag, r).is_none())
                    {
                        let computed = match super::rows::kept(r) {
                            Some(false) => None,
                            Some(true) if !local => match super::rows::typed(&r.value, scope) {
                                Some(ty) => Some((ty, typing.clone())),
                                None => continue,
                            },
                            _ => continue,
                        };
                        let fonts = match super::rows::font_row(&r.name) {
                            false => None,
                            true => match fonts.get_or_insert_with(|| {
                                super::rows::Fonts::of(file, a, !children.is_empty(), declared)
                            }) {
                                Some(f) => Some(f.clone()),
                                None => continue,
                            },
                        };
                        rows.push(super::rows::Said {
                            attr: r.clone(),
                            fonts,
                            computed,
                        });
                    }
                    let inside = svg || tag == "svg";
                    let left = left || (parent == Some("section") && crate::grouped::folds(node));
                    stack.extend(
                        children
                            .iter()
                            .map(|c| (c, inside, Some(tag.as_str()), local, left)),
                    );
                }
                Node::When {
                    then, otherwise, ..
                } => stack.extend(
                    then.iter()
                        .chain(otherwise)
                        .map(|c| (c, svg, parent, local, left)),
                ),
                Node::Each { body, .. } => {
                    stack.extend(body.iter().map(|c| (c, svg, parent, true, left)))
                }
                Node::Match { some, none, .. } => {
                    stack.extend(some.1.iter().map(|c| (c, svg, parent, true, left)));
                    stack.extend(none.iter().map(|c| (c, svg, parent, local, left)));
                }
                // An uninlined `use` is lowering's refusal, its children
                // never lowered.
                Node::Use { .. } => {}
                Node::Children { .. } => {}
            }
        }
        let mut folded = BTreeSet::new();
        // A colour keyframe folds its tokens as a row does (`keyframes::tokens`):
        // its value as the app compiles it, through a palette function's
        // template (`shade("chip")` to `light-dark(chip, #ffffff)`).
        for a in file
            .keyframes
            .iter()
            .flat_map(|k| &k.frames)
            .flat_map(|f| &f.attrs)
        {
            if colour_attr(&a.name) {
                if let Some(text) = crate::keyframes::constant(&a.value, &file.fns) {
                    folded.extend(words(&text).map(str::to_string));
                    literals.push((text, a.span, true));
                }
            }
        }
        // A computed row may read a `fn`, a state, a derive or what an
        // action assigns: their literals too.
        let mut loose = |e: &Expr| exprs(e, &mut |e| strings(e, false, &mut literals));
        let bindings = root.derives.iter().chain(&root.states);
        for e in file
            .fns
            .iter()
            .map(|f| &f.body)
            .chain(bindings.map(|b| &b.expr))
        {
            loose(e);
        }
        for a in &root.actions {
            stmts(&a.body, &mut loose);
        }
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
        let mut said = Said {
            computed,
            computed_measure,
            folded,
            rows,
            ..Default::default()
        };
        for (text, span, strict) in literals {
            said.words.extend(words(&text).map(str::to_string));
            for expr in expressions(&text, &is_token) {
                match said.synthetic.iter_mut().find(|s| s.expr == expr) {
                    Some(s) if strict && !s.strict => {
                        s.strict = true;
                        s.span = span;
                    }
                    Some(_) => {}
                    None => said.synthetic.push(Synthetic {
                        expr: expr.to_string(),
                        span,
                        strict,
                    }),
                }
            }
        }
        said
    }
}

/// An attribute that sets a row a length or number token may name.
fn measure_attr(name: &str) -> bool {
    let name = contract_syntax::state::state_of(name).map_or(name, |(_, row)| row);
    match crate::tags::attr(name) {
        Some(crate::tags::AttrTarget::Styles(rows)) => rows.iter().any(|r| {
            use exact_kernel::style::theme::{admits, Kind};
            [
                Kind::Length,
                Kind::Number,
                Kind::BoxShadow,
                Kind::TextShadow,
            ]
            .into_iter()
            .any(|k| admits(k, *r))
        }),
        _ => false,
    }
}

/// Whether a value is a number however it is chosen: its leaves through
/// `?:`, `match` and `let` are number literals.
fn number(e: &Expr) -> bool {
    match e {
        Expr::Number(..) => true,
        Expr::Ternary(_, yes, no, _) => number(yes) && number(no),
        Expr::Match { some, none, .. } => number(some) && number(none),
        Expr::Let { body, .. } => number(body),
        _ => false,
    }
}

/// Whether a value is literal: its leaves through `?:`, `match` and `let`
/// are string literals.
fn literal(e: &Expr) -> bool {
    match e {
        Expr::Str(..) => true,
        Expr::Ternary(_, yes, no, _) => literal(yes) && literal(no),
        Expr::Match { some, none, .. } => literal(some) && literal(none),
        Expr::Let { body, .. } => literal(body),
        _ => false,
    }
}

/// The literals a value is, through `?:`, `match` and `let`: strict.
fn values(e: &Expr, out: &mut Vec<(String, Span, bool)>) {
    match e {
        Expr::Ternary(_, yes, no, _) => {
            values(yes, out);
            values(no, out);
        }
        Expr::Match { some, none, .. } => {
            values(some, out);
            values(none, out);
        }
        Expr::Let { body, .. } => values(body, out),
        other => strings(other, true, out),
    }
}

/// The string literals a value is, through `?:`, `match` and `let`.
pub(crate) fn literal_leaves(e: &Expr, out: &mut Vec<(String, Span)>) {
    match e {
        Expr::Str(text, span) => out.push((text.clone(), *span)),
        Expr::Ternary(_, yes, no, _) => {
            literal_leaves(yes, out);
            literal_leaves(no, out);
        }
        Expr::Match { some, none, .. } => {
            literal_leaves(some, out);
            literal_leaves(none, out);
        }
        Expr::Let { body, .. } => literal_leaves(body, out),
        _ => {}
    }
}

/// A string literal's text, or a template's, with its place.
fn strings(e: &Expr, strict: bool, out: &mut Vec<(String, Span, bool)>) {
    match e {
        Expr::Str(text, span) => out.push((text.clone(), *span, strict)),
        Expr::Template(parts, span) => {
            for p in parts {
                if let TemplatePart::Text(text) = p {
                    out.push((text.clone(), *span, strict));
                }
            }
        }
        _ => {}
    }
}

/// Visit `e` and every expression inside it.
pub(crate) fn exprs(e: &Expr, f: &mut dyn FnMut(&Expr)) {
    f(e);
    match e {
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) | Expr::None(..) | Expr::Ident(..) => {}
        Expr::Template(parts, _) => {
            for p in parts {
                if let TemplatePart::Expr(x) = p {
                    exprs(x, f);
                }
            }
        }
        Expr::List(items, _) => items.iter().for_each(|a| exprs(a, f)),
        Expr::Some(x, _)
        | Expr::Member(x, _, _)
        | Expr::NamedArg(_, x, _)
        | Expr::Typed(x, _, _)
        | Expr::Unary(_, x, _) => exprs(x, f),
        Expr::Call(_, args, _) => args.iter().for_each(|a| exprs(a, f)),
        Expr::Binary(_, a, b, _) => {
            exprs(a, f);
            exprs(b, f);
        }
        Expr::Ternary(a, b, c, _) => {
            exprs(a, f);
            exprs(b, f);
            exprs(c, f);
        }
        Expr::Match {
            subject,
            some,
            none,
            ..
        } => {
            exprs(subject, f);
            exprs(some, f);
            exprs(none, f);
        }
        Expr::Let { value, body, .. } => {
            exprs(value, f);
            exprs(body, f);
        }
        Expr::Arrow { body, .. } => exprs(body, f),
    }
}

/// Every expression an action's statements hold.
pub(crate) fn stmts(body: &[Stmt], f: &mut dyn FnMut(&Expr)) {
    for s in body {
        match s {
            Stmt::Let { expr, .. } | Stmt::Assign { expr, .. } => f(expr),
            Stmt::Command { args, .. } | Stmt::Send { args, .. } => args.iter().for_each(&mut *f),
            Stmt::If {
                cond,
                then,
                otherwise,
                ..
            } => {
                f(cond);
                stmts(then, f);
                stmts(otherwise, f);
            }
            Stmt::Match {
                subject,
                some,
                none,
                ..
            } => {
                f(subject);
                stmts(&some.1, f);
                stmts(none, f);
            }
            Stmt::Call { args, body, .. } => {
                args.iter().for_each(&mut *f);
                stmts(body, f);
            }
            Stmt::Refresh { .. } => {}
        }
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
        let name = &text[start..open];
        if COLOUR_FNS.iter().any(|f| f.eq_ignore_ascii_case(name))
            && words(&text[open..end]).any(is_token)
        {
            out.push(&text[start..end]);
            from = end;
        } else {
            from = open + 1;
        }
    }
    out
}

/// Whether `word` is a dotted identifier, a name a design's token may have
/// (`eu.glow`, `_eu.glow`): Tamber's own name grammar (`is_name`), with a
/// `.`. A number, a dimension (`-1.5px`) or a function (`rgb(0.5 0 0)`) is
/// none (stage 12 B3, round 1, Astra 4; round 2, Astra 2).
pub(crate) fn dotted_name(word: &str) -> bool {
    word.contains('.') && exact_tamber::is_name(word)
}

/// `tamber-unknown-token` or `tamber-kind` for a word inside a literal the
/// kernel refused in a row that holds a colour among other words (a shadow,
/// a border, a gradient; LLP 1121 stage 12 B3, QUEUE's item): a dotted word
/// naming no token of the design and no role, in a colour row's words; in a
/// shadow, a length token or a base name, which a literal's px lengths
/// don't read, pointing to a shadow token. `None` otherwise: the kernel's
/// refusal stands.
pub(crate) fn literal_word(
    attr: &str,
    text: &str,
    rows: &[StyleId],
    span: Span,
) -> Option<crate::LowerError> {
    use exact_kernel::style::{scale, theme};
    if !rows.iter().any(|r| colour_codec(*r)) {
        return None;
    }
    let shadow = rows.iter().find_map(|r| match r.codec() {
        StyleCodec::BoxShadow => Some("box-shadow"),
        StyleCodec::TextShadow => Some("text-shadow"),
        _ => None,
    });
    let words = text
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')' | '/'))
        .filter(|w| w.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_'));
    for word in words {
        let length =
            scale::is_base(word) || super::measure_named(word) == Some(exact_tamber::Kind::Length);
        let (id, message) = match shadow {
            Some(kind) if length => (
                "tamber-kind",
                format!("`{attr}=\"{text}\"`: a shadow row's lengths are px: name a shadow token (`x: {kind} = 0 {word} 4px black`) to read `{word}`"),
            ),
            _ if dotted_name(word) && theme::bound() && !theme::names(word) && !length => (
                "tamber-unknown-token",
                format!("`{attr}=\"{text}\"`: `{word}` is no token of this app's design and no role"),
            ),
            _ => continue,
        };
        return Some(crate::LowerError { id, message, span });
    }
    None
}
