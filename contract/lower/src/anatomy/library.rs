//! The library rules on what a library component writes (LLP 1139 D1, D6;
//! stage 12 C3): its spacing, corners and colours are its knobs, never
//! literals (`lower-library-literal`); a control's role is drawn by the
//! native control, which says what its platform needs
//! (`lower-library-control`); and nothing in a library package reads the
//! host it runs on (`lower-library-platform`).

use crate::LowerError;
use contract_syntax::{Attr, Component, Expr, File, Node, Span};
use exact_kernel::StyleId;

/// Whether a row is a knob's (D1): spacing, a corner or a colour.
fn knob(id: StyleId) -> bool {
    use StyleId::*;
    matches!(
        id,
        PaddingTop
            | PaddingRight
            | PaddingBottom
            | PaddingLeft
            | MarginTop
            | MarginRight
            | MarginBottom
            | MarginLeft
            | RowGap
            | ColumnGap
            | BorderRadiusTopLeft
            | BorderRadiusTopRight
            | BorderRadiusBottomRight
            | BorderRadiusBottomLeft
            | TextColor
            | BackgroundColor
            | BorderColorTop
            | BorderColorRight
            | BorderColorBottom
            | BorderColorLeft
            | TintColor
            | AccentColor
    )
}

/// The keywords D1 lets a library write (r6.2): no length or colour.
const KEYWORDS: [&str; 7] = [
    "transparent",
    "currentcolor",
    "auto",
    "none",
    "inherit",
    "initial",
    "unset",
];

/// The functions a knob's value may compute with, over knobs.
const FUNCTIONS: [&str; 6] = ["color-mix", "light-dark", "calc", "min", "max", "clamp"];

/// The words of a colour mix's syntax: its interpolation spaces and hue
/// methods, no colour.
const MIX_WORDS: [&str; 21] = [
    "in",
    "srgb",
    "srgb-linear",
    "display-p3",
    "a98-rgb",
    "prophoto-rgb",
    "rec2020",
    "lab",
    "oklab",
    "lch",
    "oklch",
    "hsl",
    "hwb",
    "xyz",
    "xyz-d50",
    "xyz-d65",
    "hue",
    "shorter",
    "longer",
    "increasing",
    "decreasing",
];

/// Whether `text` names a knob (D1): a token (a param's among them, which
/// the rows name by its token), a role, a base name, the kernel's named
/// length (`-exact-hairline`); a `color-mix`, `light-dark` or `calc`-family
/// function whose every length and colour is one; or is the zero length or
/// a keyword every library may write.
fn named(text: &str) -> bool {
    let t = text.trim();
    // A function is judged by its arguments, though the theme binds its
    // whole text as an entry too.
    if let Some(over) = computed(t) {
        return over;
    }
    t.is_empty()
        || zero(t)
        || KEYWORDS.iter().any(|k| t.eq_ignore_ascii_case(k))
        || exact_kernel::style::theme::names(t)
        || crate::theme::measure_values(t).is_some()
        || crate::theme::role_named(t).is_some()
        || exact_kernel::style::scale::is_base(t)
        || exact_kernel::style::relative::named_length(t).is_some()
}

/// The zero length, `0` or `0px` however written (`0.0`, as a split side
/// is the number 0).
fn zero(t: &str) -> bool {
    t.strip_suffix("px")
        .unwrap_or(t)
        .parse::<f64>()
        .is_ok_and(|n| n == 0.0)
}

/// For a function of [`FUNCTIONS`], whether it is over knobs: each
/// argument word a knob, a number or percentage (a factor or a mix's share,
/// no length), an operator, a parenthesized group of those, or a word of
/// the mix's syntax; and one knob at least (`calc(50%)` is a literal).
fn computed(t: &str) -> Option<bool> {
    let (ok, knob) = function(t)?;
    Some(ok && knob)
}

/// For a function of [`FUNCTIONS`], [`over`] its arguments: a nested one
/// needs no knob of its own (`calc(space.md * calc(1 + 2))`).
fn function(t: &str) -> Option<(bool, bool)> {
    let (name, rest) = t.split_once('(')?;
    let args = rest.strip_suffix(')')?;
    if !FUNCTIONS
        .iter()
        .any(|f| name.trim().eq_ignore_ascii_case(f))
    {
        return None;
    }
    Some(over(args, name.trim().eq_ignore_ascii_case("color-mix")))
}

/// Whether `args` are all a function over knobs takes, and whether one is
/// a knob ([`computed`]). A percentage is a colour mix's share (`mix`);
/// in `calc` and its kin it is a length, a literal.
fn over(args: &str, mix: bool) -> (bool, bool) {
    let mut depth = 0;
    let words = args.split(|c: char| {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        depth == 0 && (c.is_ascii_whitespace() || matches!(c, ',' | '*' | '/'))
    });
    let mut knob = false;
    for w in words.filter(|w| !w.is_empty()) {
        let inner = w
            .strip_prefix('(')
            .and_then(|w| w.strip_suffix(')'))
            .map(|inner| over(inner, mix))
            .or_else(|| function(w));
        if let Some((ok, k)) = inner {
            if !ok {
                return (false, knob);
            }
            knob |= k;
            continue;
        }
        let syntax = matches!(w, "+" | "-")
            || MIX_WORDS.iter().any(|m| w.eq_ignore_ascii_case(m))
            || w.parse::<f64>().is_ok()
            || (mix
                && w.strip_suffix('%')
                    .is_some_and(|n| n.parse::<f64>().is_ok()));
        if syntax {
            continue;
        }
        if !named(w) {
            return (false, knob);
        }
        knob |= !zero(w) && !KEYWORDS.iter().any(|k| w.eq_ignore_ascii_case(k));
    }
    (true, knob)
}

/// The literal a knob row's value writes, each leaf of a `?:`, `match` or
/// `let` (a computation is data, no literal).
fn literals(value: &Expr) -> Vec<String> {
    let mut leaves = Vec::new();
    crate::theme::leaves(value, &mut Vec::new(), &mut leaves);
    leaves
        .into_iter()
        .filter_map(|l| match l {
            Expr::Str(s, _) if !named(s) => Some(s.clone()),
            Expr::Number(n, _) if *n != 0.0 => Some(exact_num::Shortest(*n).to_string()),
            // `-12`, and a template no value fills, are literals as written.
            Expr::Unary(_, n, _) => match &**n {
                Expr::Number(n, _) if *n != 0.0 => Some(format!("-{}", exact_num::Shortest(*n))),
                _ => None,
            },
            Expr::Template(..) => whole(l).filter(|t| !named(t)),
            _ => None,
        })
        .collect()
}

/// Each literal knob of library component `c` (`lower-library-literal`):
/// a spacing, corner or colour row of its view or of a style its view
/// applies, and a param default, that names no token, role or base name.
pub(crate) fn literal(c: &Component, file: &File) -> Vec<LowerError> {
    let mut out = Vec::new();
    let comp = &c.name;
    let refuse = |what: String, span: Span| {
        LowerError {
        id: "lower-library-literal",
        message: format!("{what} in library component `{comp}` is a literal: a library's spacing, corners and colours are its knobs, a param, a token, a role or a base name (`0` and `0px` excepted; LLP 1139 D1)"),
        span,
    }
    };
    // Every kind's default names a token (D1): a motion or shadow token is
    // the design's, which the theme table doesn't bind.
    let designed: Vec<String> = file
        .design
        .iter()
        .chain(&file.package_designs)
        .filter_map(|d| exact_tamber::parse(&d.text).ok())
        .flat_map(|d| d.tokens.into_iter().chain(d.icons).map(|t| t.name.value))
        .collect();
    for p in &c.params.list {
        let default = p.default.trim();
        if !named(default)
            && !designed.iter().any(|d| d == default)
            && !default.starts_with(crate::params::PREFIX)
        {
            out.push(refuse(
                format!("param `{}`'s default `{default}`", p.name),
                p.default_span,
            ));
        }
    }
    let mut rows: Vec<(&Attr, Option<&str>)> = Vec::new();
    elements(&c.view, &mut |attrs| {
        for (name, _) in super::classes(attrs) {
            if let Some(s) = file.styles.iter().rev().find(|s| s.name == name) {
                rows.extend(s.attrs.iter().map(|a| (a, Some(s.name.as_str()))));
            }
        }
        rows.extend(attrs.iter().map(|a| (a, None)));
    });
    for (a, style) in rows {
        let mut said: Vec<String> = Vec::new();
        for value in knob_values(a) {
            for lit in literals(&value) {
                if !said.contains(&lit) {
                    said.push(lit);
                }
            }
        }
        if said.is_empty() {
            continue;
        }
        // As written: a split side's text is lowering's (`12px` is `12`).
        let lit = match &a.value {
            Expr::Str(whole, _) => whole.clone(),
            _ => said.join("\", \""),
        };
        let at = style.map_or(String::new(), |s| format!(" (in `style {s}`)"));
        out.push(refuse(format!("`{}=\"{lit}\"`{at}", a.name), a.span));
    }
    out
}

/// The values `a` writes to knob rows, a shorthand's split as lowering
/// splits it (`padding="0 space.md"` per side, `border`'s colour alone);
/// a value lowering can't split is judged whole.
fn knob_values(a: &Attr) -> Vec<Expr> {
    let name = contract_syntax::state::state_of(&a.name).map_or(a.name.as_str(), |(_, row)| row);
    let rows = super::rows_of(a);
    if !rows.iter().copied().any(knob) {
        return Vec::new();
    }
    let split = match crate::tags::attr_valued(name, &a.value) {
        Some(crate::tags::AttrTarget::Styles(_)) => crate::shorthands::style_values(name, &a.value)
            .ok()
            .map(|v| {
                v.into_iter()
                    .filter(|(row, _)| knob(*row))
                    .map(|(_, e)| e)
                    .collect()
            }),
        Some(crate::tags::AttrTarget::Shorthand) => rows
            .iter()
            .enumerate()
            .filter(|(_, row)| knob(**row))
            .map(|(i, _)| crate::shorthands::component(&a.value, name, i).ok())
            .collect(),
        _ => None,
    };
    split.unwrap_or_else(|| vec![a.value.clone()])
}

/// Visit each element of `nodes`' attributes, at any depth (a slot fill's
/// too: the component writes it).
fn elements<'n>(nodes: &'n [Node], f: &mut dyn FnMut(&'n [Attr])) {
    for n in nodes {
        match n {
            Node::Element {
                attrs, children, ..
            } => {
                f(attrs);
                elements(children, f);
            }
            Node::Use { children, .. } | Node::Each { body: children, .. } => elements(children, f),
            Node::When {
                then, otherwise, ..
            } => {
                elements(then, f);
                elements(otherwise, f);
            }
            Node::Match { some, none, .. } => {
                elements(&some.1, f);
                elements(none, f);
            }
            Node::Children { .. } => {}
        }
    }
}

/// Each name under `ns.` a style row of `file` writes, a style's or an
/// element's (a text's words are no row), with its literal's span.
pub(crate) fn rows_naming(file: &File, ns: &str) -> Vec<(String, Span)> {
    let mut attrs: Vec<&Attr> = file.styles.iter().flat_map(|s| &s.attrs).collect();
    for c in &file.components {
        elements(&c.view, &mut |a| attrs.extend(a));
    }
    let prefix = format!("{ns}.");
    let mut out = Vec::new();
    for a in attrs.into_iter().filter(|a| !super::rows_of(a).is_empty()) {
        // Each value the row may take (a choice's leaves, never its
        // condition): a string, a template or a concatenation, its holes
        // marked so a name beside one (`cards.${tone}`) is never read.
        let mut leaves = Vec::new();
        crate::theme::leaves(&a.value, &mut Vec::new(), &mut leaves);
        for leaf in leaves {
            let mut texts = Vec::new();
            written(leaf, false, false, None, &mut texts);
            for (bare, left, right) in texts {
                for (at, w) in word_spans(&bare) {
                    let glued = (left && at == 0) || (right && at + w.len() == bare.len());
                    if w.starts_with(&prefix) && !glued {
                        out.push((w.to_string(), leaf.span()));
                    }
                }
            }
        }
    }
    out
}

/// A quote no text closes: what follows a choice whose branches leave
/// different quotes open.
const UNKNOWN: char = '\0';

/// Each text `e` writes as a row value, with what names no token blanked
/// ([`unquoted`], its quote carried from the text before: `open`), and
/// whether text beside it may glue onto its first and last word (a hole,
/// or a side of a `+`, that may end or start with a word's character): a
/// string; a template's texts and its holes' own; a `+`'s sides; a
/// choice's every leaf. The quote open after it.
fn written(
    e: &Expr,
    left: bool,
    right: bool,
    open: Option<char>,
    out: &mut Vec<(String, bool, bool)>,
) -> Option<char> {
    match e {
        Expr::Str(s, _) => {
            let (bare, after) = unquoted(s, open);
            out.push((bare, left, right));
            after
        }
        Expr::Template(parts, _) => {
            let mut open = open;
            for (i, p) in parts.iter().enumerate() {
                // From the far edge in: an empty part passes on what is
                // beyond it.
                let l = parts[..i]
                    .iter()
                    .fold(left, |beyond, q| part_glues(q, false, beyond));
                let r = parts[i + 1..]
                    .iter()
                    .rev()
                    .fold(right, |beyond, q| part_glues(q, true, beyond));
                open = match p {
                    contract_syntax::TemplatePart::Text(t) => {
                        let (bare, after) = unquoted(t, open);
                        out.push((bare, l, r));
                        after
                    }
                    contract_syntax::TemplatePart::Expr(x) => written(x, l, r, open, out),
                };
            }
            open
        }
        Expr::Binary(contract_syntax::BinOp::Add, a, b, _) => {
            let open = written(a, left, glues(b, true, right), open, out);
            written(b, glues(a, false, left), right, open, out)
        }
        Expr::Ternary(_, a, b, _)
        | Expr::Match {
            some: a, none: b, ..
        } => {
            // Branches that leave different quotes open leave it unknown:
            // the text after them is read as quoted ([`UNKNOWN`]), never
            // guessed.
            let after = written(a, left, right, open, out);
            match written(b, left, right, open, out) == after {
                true => after,
                false => Some(UNKNOWN),
            }
        }
        Expr::Let { body, .. } => written(body, left, right, open, out),
        // A computed value: what it writes is unknown, its quote too.
        _ => open,
    }
}

/// Whether `e` may start (`start`) or end with a word's character, an
/// empty one as what is `beyond` it does: a computed value may.
fn glues(e: &Expr, start: bool, beyond: bool) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
    match e {
        Expr::Str(s, _) => {
            let edge = if start {
                s.chars().next()
            } else {
                s.chars().last()
            };
            edge.map_or(beyond, word)
        }
        Expr::Template(parts, _) => {
            let mut edge: Vec<&contract_syntax::TemplatePart> = parts.iter().collect();
            if start {
                edge.reverse();
            }
            edge.into_iter()
                .fold(beyond, |beyond, p| part_glues(p, start, beyond))
        }
        Expr::Binary(contract_syntax::BinOp::Add, a, b, _) => match start {
            true => glues(a, true, glues(b, true, beyond)),
            false => glues(b, false, glues(a, false, beyond)),
        },
        Expr::Ternary(_, a, b, _)
        | Expr::Match {
            some: a, none: b, ..
        } => glues(a, start, beyond) || glues(b, start, beyond),
        Expr::Let { body, .. } => glues(body, start, beyond),
        _ => true,
    }
}

fn part_glues(p: &contract_syntax::TemplatePart, start: bool, beyond: bool) -> bool {
    match p {
        contract_syntax::TemplatePart::Text(t) => {
            glues(&Expr::Str(t.clone(), Span::default()), start, beyond)
        }
        contract_syntax::TemplatePart::Expr(x) => glues(x, start, beyond),
    }
}

/// [`crate::theme::scan::words`] with each word's byte offset.
fn word_spans(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        let word = c.is_ascii_alphanumeric() || "-_.".contains(c);
        match (word, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// A template's text when no value fills it.
fn whole(e: &Expr) -> Option<String> {
    let Expr::Template(parts, _) = e else {
        return None;
    };
    let mut text = String::new();
    for p in parts {
        match p {
            contract_syntax::TemplatePart::Text(t) => text.push_str(t),
            contract_syntax::TemplatePart::Expr(_) => return None,
        }
    }
    Some(text)
}

/// `text` with what names no token blanked: a quoted string (a family
/// name, `'cards.custom'`), a `url(…)`, and a `#` fragment or colour;
/// `open`, the quote or `url(` still open from the text before it. The
/// one still open after it.
fn unquoted(text: &str, open: Option<char>) -> (String, Option<char>) {
    let mut out = String::with_capacity(text.len());
    let mut open = open;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if let Some(close) = open {
            if c == close {
                open = None;
            }
            out.push(' ');
            continue;
        }
        match c {
            '\'' | '"' => {
                open = Some(c);
                out.push(' ');
            }
            '#' => {
                while chars
                    .peek()
                    .is_some_and(|(_, d)| d.is_ascii_alphanumeric() || "-_.".contains(*d))
                {
                    chars.next();
                }
                out.push(' ');
            }
            'u' | 'U' if text[i..].to_ascii_lowercase().starts_with("url(") => {
                open = Some(')');
                out.push(' ');
            }
            _ => out.push(c),
        }
    }
    (out, open)
}

/// The roles a control is drawn for (D6).
const ROLES: [&str; 8] = [
    "switch",
    "checkbox",
    "slider",
    "progressbar",
    "textbox",
    "spinbutton",
    "combobox",
    "radio",
];

/// An attribute's literal text.
fn text<'a>(attrs: &'a [Attr], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|a| a.name == name)
        .and_then(|a| match &a.value {
            Expr::Str(s, _) => Some(s.as_str()),
            _ => None,
        })
}

/// The texts attribute `name` may be: each string or hole-free template
/// leaf of a `?:`, `match` or `let`; `None` when it has none.
fn leaf_texts(attrs: &[Attr], name: &str) -> Option<Vec<String>> {
    let a = attrs.iter().find(|a| a.name == name)?;
    let mut leaves = Vec::new();
    crate::theme::leaves(&a.value, &mut Vec::new(), &mut leaves);
    let texts: Vec<String> = leaves
        .into_iter()
        .filter_map(|l| match l {
            Expr::Str(s, _) => Some(s.clone()),
            Expr::Template(..) => whole(l),
            _ => None,
        })
        .collect();
    (!texts.is_empty()).then_some(texts)
}

fn has(attrs: &[Attr], name: &str) -> bool {
    attrs.iter().any(|a| a.name == name)
}

/// What [`control`] finds.
#[derive(Default)]
pub(crate) struct Controls {
    /// Each control drawn by hand, or missing what its platform control
    /// needs.
    pub refused: Vec<LowerError>,
    /// Each default `button` with a control's role, refused (with its node's
    /// span) only where lowering draws it as a bare box: whether the
    /// platform draws it is lowering's decision (`button_appearance`).
    pub buttons: Vec<LowerError>,
}

/// The controls of library component `c` (`lower-library-control`, D6).
pub(crate) fn control(c: &Component) -> Controls {
    let mut out = Controls::default();
    walk_controls(&c.view, &c.name, &mut out);
    let states: Vec<&str> = c
        .props
        .iter()
        .filter(|p| STATES.contains(&p.name.as_str()))
        .filter(|p| match &p.ty {
            Some(contract_syntax::TypeExpr::Named(n, _)) => n == "bool",
            Some(contract_syntax::TypeExpr::Option(inner, _)) => {
                matches!(inner.as_ref(), contract_syntax::TypeExpr::Named(n, _) if n == "bool")
            }
            _ => false,
        })
        .map(|p| p.name.as_str())
        .collect();
    let mut labels = std::collections::HashMap::new();
    texts_by_id(&c.view, &mut Vec::new(), &mut labels);
    let cx = Buttons {
        comp: &c.name,
        states: &states,
        derives: &c.derives,
        labels,
    };
    walk_buttons(&c.view, &cx, &mut Vec::new(), &mut out.refused);
    out
}

/// The bool props a pressable's look may show as a state a person needs
/// said (theme2 2026-10-22 04:40): a button whose look reads one must say
/// it aloud (`walk_buttons`). Others (a size, an ink) are presentation.
pub(crate) const STATES: [&str; 7] = [
    "selected", "current", "pressed", "checked", "on", "active", "expanded",
];

/// The attributes a button says a state through, besides its label.
const SAID: [&str; 5] = [
    "aria-selected",
    "aria-pressed",
    "aria-current",
    "aria-checked",
    "aria-expanded",
];

/// Texts by literal `id`: their words and the names bound around them.
type Labels<'a> = std::collections::HashMap<&'a str, Option<(&'a Expr, Vec<String>)>>;

/// Each `text` of `nodes` with a literal `id`, by that id: its words and
/// the names `bound` around it, or `None` for an id two texts take (in two
/// arms, which of them names a button the view doesn't settle).
fn texts_by_id<'a>(nodes: &'a [Node], bound: &mut Vec<String>, out: &mut Labels<'a>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                positional,
                children,
                ..
            } => {
                if let (true, Some(id), Some(words)) =
                    (tag == "text", text(attrs, "id"), positional.first())
                {
                    let seen = out.contains_key(id);
                    out.insert(id, (!seen).then(|| (words, bound.clone())));
                }
                texts_by_id(children, bound, out);
            }
            Node::Use { children, .. } => texts_by_id(children, bound, out),
            Node::Each {
                var, index, body, ..
            } => {
                let mark = bound.len();
                bound.push(var.clone());
                bound.extend(index.clone());
                texts_by_id(body, bound, out);
                bound.truncate(mark);
            }
            Node::When {
                then, otherwise, ..
            } => {
                texts_by_id(then, bound, out);
                texts_by_id(otherwise, bound, out);
            }
            Node::Match { some, none, .. } => {
                let mark = bound.len();
                bound.push(some.0.clone());
                texts_by_id(&some.1, bound, out);
                bound.truncate(mark);
                texts_by_id(none, bound, out);
            }
            Node::Children { .. } => {}
        }
    }
}

/// What the button rules read of a component: its name, its state props,
/// and its derives, through which a look or a label may read a state.
struct Buttons<'a> {
    comp: &'a str,
    states: &'a [&'a str],
    derives: &'a [contract_syntax::Binding],
    /// Each text of the view with a literal `id`: what `aria-labelledby`
    /// names (`None` where two texts take one id, in two arms).
    labels: Labels<'a>,
}

impl Buttons<'_> {
    /// `names` and every name a derive among them reads, transitively.
    fn expand(&self, names: &mut Vec<String>) {
        let mut i = 0;
        while i < names.len() {
            if let Some(d) = self.derives.iter().find(|d| d.name == names[i]) {
                let mut read = Vec::new();
                super::reads(&d.expr, &mut Vec::new(), true, &mut read);
                for r in read {
                    if !names.contains(&r) {
                        names.push(r);
                    }
                }
            }
            i += 1;
        }
    }
    /// The names `e` reads as a look: inside a `?:` or `match`, or inside one
    /// a derive it names holds (`opacity=fade` where `derive fade = (selected
    /// ? 1 : 0.5)`), a derive read inside one adding all it reads. A derive
    /// reads in its declaration's scope, which `bound` doesn't reach.
    fn look_names(
        &self,
        e: &Expr,
        bound: &mut Vec<String>,
        out: &mut Vec<String>,
        seen: &mut Vec<String>,
    ) {
        let mut inside = Vec::new();
        super::reads(e, bound, false, &mut inside);
        let mut all = Vec::new();
        super::reads(e, bound, true, &mut all);
        // `seen`: the derives being followed now, a stack (a cycle stops; the
        // same derive met again elsewhere is followed again).
        for name in all {
            let Some(d) = self.derives.iter().find(|d| d.name == name) else {
                continue;
            };
            if inside.contains(&name) {
                let mut through = vec![name.clone()];
                self.expand(&mut through);
                out.extend(through);
            } else if !seen.contains(&name) {
                seen.push(name.clone());
                self.look_names(&d.expr, &mut Vec::new(), out, seen);
                seen.pop();
            }
        }
        out.extend(inside);
    }
    /// Whether `e` reads `state`, itself or through a derive, where a
    /// binding in `bound` doesn't hide it.
    fn mentions(&self, e: &Expr, state: &str, bound: &[String]) -> bool {
        let mut read = Vec::new();
        super::reads(e, &mut bound.to_vec(), true, &mut read);
        self.expand(&mut read);
        read.iter().any(|r| r == state)
    }
}

/// Each button of a library view (a `button`, or a node of
/// `role="button"`), named (a non-empty `aria-label`, `aria-labelledby`, or
/// a text of its own in every arm), and saying each state of `states` its
/// look reads (a `?:` or `match` in a row, a class, or a used component's
/// argument, itself or through a derive) through one of [`SAID`], a label
/// or a text of its own that reads it (theme2 2026-10-22 04:40, LLP 1139
/// D9's condition (b)). `bound`: the names a `for` or `match` around binds,
/// which hide a prop of the same name.
fn walk_buttons(nodes: &[Node], cx: &Buttons, bound: &mut Vec<String>, out: &mut Vec<LowerError>) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                span,
                ..
            } => {
                let button = tag == "button"
                    || leaf_texts(attrs, "role").is_some_and(|r| r.iter().any(|r| r == "button"));
                if button {
                    check_button(tag, attrs, children, cx, bound, *span, out);
                }
                walk_buttons(children, cx, bound, out);
            }
            Node::Use { children, .. } => walk_buttons(children, cx, bound, out),
            Node::Each {
                var, index, body, ..
            } => {
                let mark = bound.len();
                bound.push(var.clone());
                bound.extend(index.clone());
                walk_buttons(body, cx, bound, out);
                bound.truncate(mark);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk_buttons(then, cx, bound, out);
                walk_buttons(otherwise, cx, bound, out);
            }
            Node::Match { some, none, .. } => {
                let mark = bound.len();
                bound.push(some.0.clone());
                walk_buttons(&some.1, cx, bound, out);
                bound.truncate(mark);
                walk_buttons(none, cx, bound, out);
            }
            Node::Children { .. } => {}
        }
    }
}

fn check_button(
    tag: &str,
    attrs: &[Attr],
    children: &[Node],
    cx: &Buttons,
    bound: &[String],
    span: Span,
    out: &mut Vec<LowerError>,
) {
    let comp = cx.comp;
    let error = |message: String| LowerError {
        id: "lower-library-control",
        message: format!("{message} in library component `{comp}` (LLP 1139 D9)"),
        span,
    };
    let what = if tag == "button" {
        "a `button`".to_string()
    } else {
        format!("a `{tag}` of `role=\"button\"`")
    };
    let labelled = (has(attrs, "aria-label")
        && text(attrs, "aria-label").is_none_or(|t| !t.trim().is_empty()))
        || has(attrs, "aria-labelledby");
    if !labelled && !texted(children) {
        out.push(error(format!(
            "{what} without a name: write `aria-label` (or `aria-labelledby`), or give it a text, which assistive technology reads as the control it stands in for"
        )));
    }
    // The states its look reads: inside a `?:` or `match` anywhere in it but
    // the attributes that say things aloud, itself or through a derive.
    // (A derive reached reads in its own scope: a binding here doesn't hide it.)
    let mut look = Vec::new();
    looks(attrs, children, cx, &mut bound.to_vec(), &mut look);
    for state in cx.states.iter().filter(|s| look.iter().any(|l| l == *s)) {
        let says = |name: &str| {
            attrs
                .iter()
                .filter(|a| a.name == name)
                .any(|a| cx.mentions(&a.value, state, bound))
        };
        // What names it says it: `aria-labelledby` first (the texts its ids
        // name in this view; one the view doesn't settle is taken as saying
        // it), else a non-empty `aria-label`, else its own text.
        // (A text it names reads in its own scope, with its own bindings,
        // not the button's; an empty `aria-labelledby` names nothing.)
        let name_says = match text(attrs, "aria-labelledby").filter(|ids| !ids.trim().is_empty()) {
            Some(ids) => ids.split_whitespace().any(|id| {
                cx.labels
                    .get(id)
                    .and_then(Option::as_ref)
                    .is_none_or(|(e, bound)| cx.mentions(e, state, bound))
            }),
            None if has(attrs, "aria-labelledby") && text(attrs, "aria-labelledby").is_none() => {
                true
            }
            None if has(attrs, "aria-label")
                && text(attrs, "aria-label").is_none_or(|t| !t.trim().is_empty()) =>
            {
                says("aria-label")
            }
            None => text_says(children, cx, state, &mut bound.to_vec()),
        };
        if !SAID.iter().any(|a| says(a)) && !name_says {
            out.push(error(format!(
                "{what} whose look says a state it doesn't say aloud: its look reads `{state}`, so say it through `aria-selected`, `aria-pressed`, `aria-current`, `aria-checked` or `aria-expanded` from `{state}`, or a label that reads it"
            )));
        }
    }
}

/// Whether `nodes` name a button whatever their arms: a text of its own
/// (not empty, not `aria-hidden`), the slot (its caller's content, which
/// can't be known here), or a used component given a text to draw (`text`,
/// `label` or `title`).
fn texted(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element {
            tag,
            attrs,
            positional,
            children,
            ..
        } => {
            let hidden = matches!(
                attrs
                    .iter()
                    .find(|a| a.name == "aria-hidden")
                    .map(|a| &a.value),
                Some(Expr::Bool(true, _))
            );
            let empty = matches!(positional.first(), Some(Expr::Str(s, _)) if s.trim().is_empty());
            (tag == "text" && !hidden && !empty) || (!hidden && texted(children))
        }
        Node::Each { body, .. } => texted(body),
        Node::When {
            then, otherwise, ..
        } => texted(then) && texted(otherwise),
        Node::Match { some, none, .. } => texted(&some.1) && texted(none),
        Node::Use { args, children, .. } => {
            args.iter()
                .any(|a| matches!(a.name.as_str(), "text" | "label" | "title"))
                || texted(children)
        }
        Node::Children { .. } => true,
    })
}

/// Whether a text under `nodes`, one that names the button, reads `state`
/// whatever their arms (`text (following ? "Unfollow" : "Follow")` says it
/// as a label does): not an `aria-hidden` one, and in both arms of an `if`
/// or `match`, or beside them. `bound`: the names a `for` or `match` binds.
fn text_says(nodes: &[Node], cx: &Buttons, state: &str, bound: &mut Vec<String>) -> bool {
    nodes.iter().any(|n| match n {
        Node::Element {
            tag,
            attrs,
            positional,
            children,
            ..
        } => {
            let hidden = matches!(
                attrs
                    .iter()
                    .find(|a| a.name == "aria-hidden")
                    .map(|a| &a.value),
                Some(Expr::Bool(true, _))
            );
            !hidden
                && ((tag == "text" && positional.iter().any(|e| cx.mentions(e, state, bound)))
                    || text_says(children, cx, state, bound))
        }
        Node::Each {
            var, index, body, ..
        } => {
            let mark = bound.len();
            bound.push(var.clone());
            bound.extend(index.clone());
            let says = text_says(body, cx, state, bound);
            bound.truncate(mark);
            says
        }
        Node::When {
            then, otherwise, ..
        } => text_says(then, cx, state, bound) && text_says(otherwise, cx, state, bound),
        Node::Match { some, none, .. } => {
            let mark = bound.len();
            bound.push(some.0.clone());
            let says = text_says(&some.1, cx, state, bound);
            bound.truncate(mark);
            says && text_says(none, cx, state, bound)
        }
        Node::Use { .. } | Node::Children { .. } => false,
    })
}

/// The names read inside a `?:` or `match` by `attrs` (but those saying
/// things aloud, its handlers and its names) and everything under
/// `children`, a used component's arguments included, but those `bound`
/// (and a `for` or `match` within) hides.
fn looks(
    attrs: &[Attr],
    children: &[Node],
    cx: &Buttons,
    bound: &mut Vec<String>,
    out: &mut Vec<String>,
) {
    // What draws: not what it says aloud, its handlers (`press=(selected ?
    // deselect : select)` draws nothing) nor its names.
    let drawn = |a: &&Attr| {
        !a.name.starts_with("aria-")
            && !matches!(a.name.as_str(), "id" | "testId" | "key")
            && !matches!(
                crate::tags::attr(&a.name),
                Some(crate::tags::AttrTarget::Handler(_))
            )
    };
    for a in attrs.iter().filter(drawn) {
        cx.look_names(&a.value, bound, out, &mut Vec::new());
    }
    for n in children {
        match n {
            Node::Element {
                attrs, children, ..
            } => looks(attrs, children, cx, bound, out),
            Node::Use { args, children, .. } => looks(args, children, cx, bound, out),
            Node::Each {
                var, index, body, ..
            } => {
                let mark = bound.len();
                bound.push(var.clone());
                bound.extend(index.clone());
                looks(&[], body, cx, bound, out);
                bound.truncate(mark);
            }
            Node::When {
                then, otherwise, ..
            } => {
                looks(&[], then, cx, bound, out);
                looks(&[], otherwise, cx, bound, out);
            }
            Node::Match { some, none, .. } => {
                let mark = bound.len();
                bound.push(some.0.clone());
                looks(&[], &some.1, cx, bound, out);
                bound.truncate(mark);
                looks(&[], none, cx, bound, out);
            }
            Node::Children { .. } => {}
        }
    }
}

fn walk_controls(nodes: &[Node], comp: &str, out: &mut Controls) {
    for n in nodes {
        match n {
            Node::Element {
                tag,
                attrs,
                children,
                span,
                ..
            } => {
                check_control(tag, attrs, children, comp, *span, out);
                walk_controls(children, comp, out);
            }
            Node::Use { children, .. } | Node::Each { body: children, .. } => {
                walk_controls(children, comp, out)
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk_controls(then, comp, out);
                walk_controls(otherwise, comp, out);
            }
            Node::Match { some, none, .. } => {
                walk_controls(&some.1, comp, out);
                walk_controls(none, comp, out);
            }
            Node::Children { .. } => {}
        }
    }
}

fn check_control(
    tag: &str,
    attrs: &[Attr],
    children: &[Node],
    comp: &str,
    span: Span,
    controls: &mut Controls,
) {
    let error = |message: String| LowerError {
        id: "lower-library-control",
        message: format!("{message} in library component `{comp}` (LLP 1139 D6)"),
        span,
    };
    let native = match tag {
        "input" | "textarea" | "select" | "progress" => true,
        "button" => text(attrs, "appearance") != Some("none"),
        _ => false,
    };
    // Any role the value may be: each leaf of a `?:`, `match` or `let`.
    let roles = leaf_texts(attrs, "role").unwrap_or_default();
    if let Some(role) = roles.iter().find(|r| ROLES.contains(&r.as_str())) {
        let drawn = |why: &str| {
            error(format!("a `{tag}` drawn as `role=\"{role}\"`{why}: a control's role is the native control's (`input`, `textarea`, `select`, `progress`, a native `button`), which the platform draws"))
        };
        if !native {
            controls.refused.push(drawn(""));
            return;
        }
        if tag == "button" {
            controls
                .buttons
                .push(drawn(", which lowering draws as a bare box here"));
        }
    }
    let out = &mut controls.refused;
    let refuse = |out: &mut Vec<LowerError>, message: String| out.push(error(message));
    // Contract has no `label` element: a field is named by `aria-label`, or
    // `aria-labelledby` with its text's `id`.
    let named = has(attrs, "aria-label") || has(attrs, "aria-labelledby");
    let mut needed: Vec<String> = Vec::new();
    let mut need = |what: &str, ok: bool| {
        if !ok && !needed.iter().any(|n| n == what) {
            needed.push(what.to_string());
            refuse(out, format!("a `{tag}` without {what}"));
        }
    };
    match tag {
        // Each type the value may be (a choice's every leaf), each held
        // to its row.
        "input" => {
            for kind in leaf_texts(attrs, "type").unwrap_or_else(|| vec!["text".into()]) {
                match kind.to_ascii_lowercase().as_str() {
                    // The platform's picker: nothing D6's table asks of it.
                    "file" => {}
                    "checkbox" | "radio" => {
                        need("`checked`", has(attrs, "checked"));
                        need("`aria-label` or `aria-labelledby`", named);
                    }
                    "range" => {
                        for a in ["value", "min", "max"] {
                            need(&format!("`{a}`"), has(attrs, a));
                        }
                        need("`aria-label` or `aria-labelledby`", named);
                    }
                    "date" | "time" | "datetime-local" => {
                        need("`value`", has(attrs, "value"));
                        need("`aria-label` or `aria-labelledby`", named);
                    }
                    _ => {
                        need("`value`", has(attrs, "value"));
                        need(
                            "`aria-label`, `aria-labelledby` or `placeholder`",
                            named || has(attrs, "placeholder"),
                        );
                    }
                }
            }
        }
        "textarea" => {
            need("`value`", has(attrs, "value"));
            need(
                "`aria-label`, `aria-labelledby` or `placeholder`",
                named || has(attrs, "placeholder"),
            );
        }
        "select" => {
            need("`value`", has(attrs, "value"));
            need(
                "its `option`s",
                children.iter().any(|c| {
                    matches!(c, Node::Element { tag, .. } if tag == "option")
                        || !matches!(c, Node::Element { .. })
                }),
            );
            need("`aria-label` or `aria-labelledby`", named);
        }
        "progress" => {
            need("`aria-label` or `aria-labelledby`", named);
            // Determinate: its share is `value` of `max`.
            if has(attrs, "value") {
                need(
                    "`max` (it shows progress, `value` of `max`)",
                    has(attrs, "max"),
                );
            }
        }
        _ => {}
    }
}
