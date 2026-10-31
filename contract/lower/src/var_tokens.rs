//! @ref LLP 1153 D6 — `var(--name)` in a style row is the design's token
//! `name`. Each one is written as the name before anything reads the row
//! (scoping, the types, the design's retention): by the loader for a file,
//! and by `contract::compile` for a source text. So it lowers exactly as the
//! name does: folded, or on the web the token's `var(--x-name)` where it
//! varies. A browser substitutes `var()` when it computes a value; a token is
//! the build's, which every host draws, so a reference resolves only to a
//! token the app declares.
//!
//! - A name the design declares is the token. A fallback is never used, and
//!   the app is warned once (`tamber-var-fallback`).
//! - Any other `var()` in a style row is refused, fallback or not, so a typo
//!   never ships the fallback (`tamber-unknown-token`, naming the design's
//!   tokens of the kinds the row takes). The refused row is dropped, so the
//!   passes after the loader say nothing more of it.
//! - A component's argument that is one `var()` naming a token is that token,
//!   as a row reads it once the component is expanded. Any other argument is
//!   left as written, since it may be text; where it reaches a style row, a
//!   `var()` in it naming no token is refused there ([`carried`]).

use crate::{tags, LowerError};
use contract_syntax::{Attr, DesignSource, Expr, File, Node, Span, TemplatePart, ThemeDecl};
use exact_tamber as tamber;

/// The tokens an app declares, as a `var()` may name them.
#[derive(Debug, Clone, Default)]
pub struct Tokens {
    /// Each token's name as written, with its kind when the design checks.
    names: Vec<(String, Option<tamber::Kind>)>,
    /// Whether the app has a design (`app.tamber`, a package's, an inline
    /// `theme`).
    designed: bool,
    /// Whether the design parsed. One that does not is refused where the
    /// design is read, so its `var()`s are written as names and refused no
    /// further.
    parsed: bool,
}

impl Tokens {
    /// The tokens of the app's design (`app.tamber`'s text), its packages'
    /// designs and an inline `theme`.
    pub fn of(
        design: Option<&str>,
        packages: &[DesignSource],
        theme: Option<&ThemeDecl>,
    ) -> Tokens {
        let (own, parsed) = match design {
            Some(text) => match tamber::parse(text) {
                Ok(d) => (Some(d), true),
                Err(_) => (None, false),
            },
            None => (theme.map(crate::theme::inline_design), true),
        };
        let designed = design.is_some() || theme.is_some() || !packages.is_empty();
        let Some(mut merged) = crate::theme::packages::merge(own, packages).0 else {
            return Tokens {
                names: Vec::new(),
                designed,
                parsed,
            };
        };
        let names: Vec<String> = merged.tokens.iter().map(|t| t.name.value.clone()).collect();
        crate::theme::stand_in_profiles(&mut merged);
        // A folding's tokens are the design's, in its order.
        let kinds = tamber::check_design(merged, &crate::theme::vocab())
            .ok()
            .map(|f| f.tokens.into_iter().map(|t| t.kind).collect::<Vec<_>>())
            .filter(|kinds| kinds.len() == names.len());
        let names = names
            .into_iter()
            .enumerate()
            .filter(|(_, n)| !n.starts_with(crate::params::PREFIX))
            .map(|(i, n)| (n, kinds.as_ref().map(|k| k[i])))
            .collect();
        Tokens {
            names,
            designed,
            parsed,
        }
    }

    fn declares(&self, name: &str) -> bool {
        !self.parsed || self.names.iter().any(|(n, _)| n == name)
    }
}

/// What [`rewrite`] reports: the rows it refused, and the fallbacks it
/// dropped.
#[derive(Debug, Default)]
pub struct Rewritten {
    /// Each style row refused, and dropped from the file.
    pub refused: Vec<LowerError>,
    /// Each `var()` whose fallback was dropped: where, and the call as
    /// written.
    pub fallbacks: Vec<(Span, String)>,
}

/// Write each `var()` of `file`'s style rows that names a token as its
/// name, refuse and drop each row with another, and write a component
/// argument that is one such `var()` as the name.
pub fn rewrite(file: &mut File, tokens: &Tokens) -> Rewritten {
    let mut out = Rewritten::default();
    for s in file
        .styles
        .iter_mut()
        .chain(&mut file.decorations)
        .chain(&mut file.atoms)
    {
        rows(&mut s.attrs, tokens, &mut out);
    }
    for k in &mut file.keyframes {
        for frame in &mut k.frames {
            rows(&mut frame.attrs, tokens, &mut out);
        }
    }
    for c in file.components.iter_mut().chain(file.members.as_mut()) {
        nodes(&mut c.view, tokens, &mut out);
    }
    out
}

/// An app's design as [`Tokens::of`] reads it: `app.tamber`'s text, its
/// packages' designs and an inline `theme`.
type Noted = (Option<String>, Vec<DesignSource>, Option<ThemeDecl>);

thread_local! {
    /// The design a `var()` may name while the app's styles are lowered,
    /// read into [`Tokens`] only when a row needs them ([`carried`]).
    static DESIGN: std::cell::RefCell<Option<Noted>> = const { std::cell::RefCell::new(None) };
}

/// Note `file`'s design for the rows [`carried`] checks, as lowering starts.
pub(crate) fn note(file: &File) {
    let design = file.design.as_ref().map(|d| d.text.clone());
    let noted = (design, file.package_designs.clone(), file.theme.clone());
    DESIGN.with(|d| *d.borrow_mut() = Some(noted));
}

/// A style row's `var()` that reached it through a component's argument,
/// which the loader writes only when the argument is one `var()` naming a
/// token: one naming no token is refused as the loader refuses it in a row,
/// naming the tokens the row takes. A `var()` among other words of an
/// argument, naming a token, is left for the row's own check.
pub(crate) fn carried(a: &Attr) -> Result<(), LowerError> {
    fn mentions(e: &Expr) -> bool {
        let named = |s: &str| {
            s.as_bytes()
                .windows(4)
                .any(|w| w.eq_ignore_ascii_case(b"var("))
        };
        match e {
            Expr::Str(s, _) => named(s),
            Expr::Template(parts, _) => parts.iter().any(|p| match p {
                TemplatePart::Text(t) => named(t),
                TemplatePart::Expr(e) => mentions(e),
            }),
            Expr::Ternary(_, yes, no, _) => mentions(yes) || mentions(no),
            Expr::Match { some, none, .. } => mentions(some) || mentions(none),
            Expr::Let { value, body, .. } => mentions(value) || mentions(body),
            Expr::Binary(contract_syntax::BinOp::Add, l, r, _) => mentions(l) || mentions(r),
            _ => false,
        }
    }
    if !tags::style(&a.name) || !mentions(&a.value) {
        return Ok(());
    }
    let Some((design, packages, theme)) = DESIGN.with(|d| d.borrow().clone()) else {
        return Ok(());
    };
    let tokens = Tokens::of(design.as_deref(), &packages, theme.as_ref());
    let (mut out, mut refused) = (Rewritten::default(), None);
    value(
        &mut a.value.clone(),
        &tokens,
        Mode::Row,
        &mut out,
        &mut refused,
    );
    match refused {
        Some((why, literal, span)) => Err(refusal(a, why, &literal, span, &tokens)),
        None => Ok(()),
    }
}

/// The one warning for every fallback [`rewrite`] dropped, at the first.
pub fn fallback_warning(fallbacks: &[(Span, String)]) -> Option<LowerError> {
    let (span, call) = fallbacks.first()?;
    let name = call_name(call);
    let others = match fallbacks.len() - 1 {
        0 => String::new(),
        1 => "; one other `var()` in this app has a fallback, ignored likewise".into(),
        n => format!("; {n} other `var()`s in this app have fallbacks, ignored likewise"),
    };
    Some(LowerError {
        id: "tamber-var-fallback",
        message: format!("`{call}`: `{name}` is a token of the design, so the fallback is never used and is ignored (Exact never leaves a token undefined): write `var(--{name})`{others}"),
        span: *span,
    })
}

fn call_name(call: &str) -> &str {
    let inner = call.get(4..).unwrap_or_default().trim_start();
    let inner = inner.strip_prefix("--").unwrap_or(inner);
    inner.split([',', ')']).next().unwrap_or_default().trim()
}

fn nodes(list: &mut [Node], tokens: &Tokens, out: &mut Rewritten) {
    for node in list {
        match node {
            Node::Element {
                attrs, children, ..
            } => {
                rows(attrs, tokens, out);
                nodes(children, tokens, out);
            }
            Node::Use { args, children, .. } => {
                for a in args.iter_mut() {
                    let mut refused = None;
                    value(&mut a.value, tokens, Mode::Whole, out, &mut refused);
                }
                nodes(children, tokens, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                nodes(then, tokens, out);
                nodes(otherwise, tokens, out);
            }
            Node::Each { body, .. } => nodes(body, tokens, out),
            Node::Match { some, none, .. } => {
                nodes(&mut some.1, tokens, out);
                nodes(none, tokens, out);
            }
            Node::Children { .. } => {}
        }
    }
}

/// A list's style rows rewritten; one with a `var()` no token answers is
/// refused and dropped.
fn rows(list: &mut Vec<Attr>, tokens: &Tokens, out: &mut Rewritten) {
    list.retain_mut(|a| {
        if !tags::style(&a.name) {
            return true;
        }
        let mut refused = None;
        value(&mut a.value, tokens, Mode::Row, out, &mut refused);
        match refused {
            None => true,
            Some((why, literal, span)) => {
                out.refused.push(refusal(a, why, &literal, span, tokens));
                false
            }
        }
    });
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// A style row's value: every `var()` written or refused.
    Row,
    /// A component's argument: a literal that is one `var()` naming a
    /// token, written; the rest left.
    Whole,
}

/// Why a `var()` is refused.
#[derive(Debug, PartialEq)]
enum Why {
    /// It names no token; whether it has a fallback.
    Unknown(String, bool),
    /// Its argument is no custom property name.
    Unnamed,
    /// Its name is computed (a template's hole).
    Computed,
}

/// The first refusal in a value: why, the literal's text, where.
type Refused = Option<(Why, String, Span)>;

/// Rewrite a value's literals in place, through `?:`, `match`, `let`, `+`
/// and a template's text.
fn value(e: &mut Expr, tokens: &Tokens, mode: Mode, out: &mut Rewritten, refused: &mut Refused) {
    match e {
        Expr::Str(s, span) => {
            if mode == Mode::Whole && whole_var(s).is_none() {
                return;
            }
            match text(s, tokens, false, &mut None) {
                Ok(Some((next, fallbacks))) => {
                    out.fallbacks
                        .extend(fallbacks.into_iter().map(|f| (*span, f)));
                    *s = next;
                }
                Ok(None) => {}
                Err(why) if mode == Mode::Row && refused.is_none() => {
                    *refused = Some((why, s.clone(), *span));
                }
                Err(_) => {}
            }
        }
        Expr::Template(parts, span) if mode == Mode::Row => {
            let mut quote = None;
            let n = parts.len();
            for i in 0..n {
                let hole = i + 1 < n && matches!(parts[i + 1], TemplatePart::Expr(_));
                match &mut parts[i] {
                    TemplatePart::Text(s) => match text(s, tokens, hole, &mut quote) {
                        Ok(Some((next, fallbacks))) => {
                            out.fallbacks
                                .extend(fallbacks.into_iter().map(|f| (*span, f)));
                            *s = next;
                        }
                        Ok(None) => {}
                        Err(why) => {
                            if refused.is_none() {
                                *refused = Some((why, s.clone(), *span));
                            }
                        }
                    },
                    TemplatePart::Expr(e) => value(e, tokens, mode, out, refused),
                }
            }
        }
        Expr::Ternary(_, yes, no, _) => {
            value(yes, tokens, mode, out, refused);
            value(no, tokens, mode, out, refused);
        }
        Expr::Match { some, none, .. } => {
            value(some, tokens, mode, out, refused);
            value(none, tokens, mode, out, refused);
        }
        Expr::Let { value: v, body, .. } => {
            value(v, tokens, mode, out, refused);
            value(body, tokens, mode, out, refused);
        }
        Expr::Binary(contract_syntax::BinOp::Add, left, right, _) => {
            value(left, tokens, mode, out, refused);
            value(right, tokens, mode, out, refused);
        }
        _ => {}
    }
}

/// `text` as one `var()` call and nothing else, its parentheses balanced:
/// the name it gives and whether it writes a fallback; `None` for any other
/// text.
pub fn whole_var(text: &str) -> Option<(String, bool)> {
    let text = text.trim();
    match calls(text, false, &mut None).as_slice() {
        [Found::Call { at, name, fallback }] if at.start == 0 && at.end == text.len() => {
            Some((name.clone(), fallback.is_some()))
        }
        _ => None,
    }
}

/// A `var(` in a literal's text.
#[derive(Debug, PartialEq)]
enum Found {
    /// A whole call: its bytes, its name and its fallback as written.
    Call {
        at: std::ops::Range<usize>,
        name: String,
        fallback: Option<String>,
    },
    /// One with no name, or whose name a template's hole computes.
    Bad(Why),
}

/// `text` with each `var()` naming a token written as the name, and each
/// call whose fallback it dropped; `None` when it has no `var()`. `hole`:
/// a template's hole follows the text. `quote`: the quote a template's
/// earlier text left open, and this text leaves open.
fn text(
    text: &str,
    tokens: &Tokens,
    hole: bool,
    quote: &mut Option<char>,
) -> Result<Option<(String, Vec<String>)>, Why> {
    let found = calls(text, hole, quote);
    if found.is_empty() {
        return Ok(None);
    }
    let (mut out, mut fallbacks, mut at) = (String::new(), Vec::new(), 0);
    for f in found {
        match f {
            Found::Call {
                at: range,
                name,
                fallback,
            } => {
                if !tokens.declares(&name) {
                    return Err(Why::Unknown(name, fallback.is_some()));
                }
                // A design that does not parse names no token: its refusal
                // fails the build, and no fallback is said to be unused.
                if fallback.is_some() && tokens.parsed {
                    fallbacks.push(text[range.clone()].to_string());
                }
                out.push_str(&text[at..range.start]);
                out.push_str(&name);
                at = range.end;
            }
            Found::Bad(why) => return Err(why),
        }
    }
    out.push_str(&text[at..]);
    Ok(Some((out, fallbacks)))
}

/// Each `var(` in `text` outside a quoted string, in order.
fn calls(text: &str, hole: bool, quote: &mut Option<char>) -> Vec<Found> {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    let skip = |j: usize| j + text[j..].len() - text[j..].trim_start().len();
    let mut found = Vec::new();
    let mut i = 0;
    while let Some(c) = text[i..].chars().next() {
        if let Some(q) = *quote {
            if c == q {
                *quote = None;
            }
            i += c.len_utf8();
            continue;
        }
        if c == '"' || c == '\'' {
            *quote = Some(c);
            i += 1;
            continue;
        }
        let opens = text
            .get(i..i + 4)
            .is_some_and(|s| s.eq_ignore_ascii_case("var("))
            && !text[..i].chars().next_back().is_some_and(word);
        if !opens {
            i += c.len_utf8();
            continue;
        }
        let start = skip(i + 4);
        // The name ends where its characters do; a name the hole computes
        // reaches the text's end with a hole after it.
        let ends_open = |j: usize| j >= text.len() && hole;
        let Some(rest) = text[start..].strip_prefix("--") else {
            found.push(Found::Bad(if ends_open(start) {
                Why::Computed
            } else {
                Why::Unnamed
            }));
            i = start.max(i + 4);
            continue;
        };
        let name_at = text.len() - rest.len();
        let name_end = name_at
            + rest
                .find(|c: char| !(word(c) || c == '.'))
                .unwrap_or(rest.len());
        let name = &text[name_at..name_end];
        let after = skip(name_end);
        match text.as_bytes().get(after) {
            Some(b')') if !name.is_empty() => {
                found.push(Found::Call {
                    at: i..after + 1,
                    name: name.to_string(),
                    fallback: None,
                });
                i = after + 1;
            }
            Some(b',') if !name.is_empty() => match close(text, after + 1) {
                Some(end) => {
                    found.push(Found::Call {
                        at: i..end + 1,
                        name: name.to_string(),
                        fallback: Some(text[after + 1..end].trim().to_string()),
                    });
                    i = end + 1;
                }
                None => {
                    found.push(Found::Bad(if hole { Why::Computed } else { Why::Unnamed }));
                    i = text.len();
                }
            },
            _ => {
                found.push(Found::Bad(if ends_open(after) {
                    Why::Computed
                } else {
                    Why::Unnamed
                }));
                i = after.max(i + 4);
            }
        }
    }
    found
}

/// The index of the `)` that closes a function whose argument starts at
/// `from`, past nested parentheses and quoted strings.
fn close(text: &str, from: usize) -> Option<usize> {
    let (mut depth, mut quote) = (0usize, None);
    for (k, c) in text[from..].char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') if depth == 0 => return Some(from + k),
            (None, ')') => depth -= 1,
            _ => {}
        }
    }
    None
}

/// A row's refusal: what its `var()` names, or why it names nothing.
fn refusal(a: &Attr, why: Why, literal: &str, span: Span, tokens: &Tokens) -> LowerError {
    let written = format!("`{}=\"{literal}\"`", a.name);
    let (id, message) = match why {
        Why::Unnamed => (
            "lower-attr-value",
            format!("{written}: a `var()` names a token of the design as a custom property, `var(--name)`"),
        ),
        Why::Computed => (
            "lower-attr-value",
            format!("{written}: a `var()` names its token whole; a token's name from data is the value itself (`{}=name`), which the runtime reads against the design", a.name),
        ),
        Why::Unknown(name, fallback) => {
            let kinds = kinds(a);
            let said = kinds
                .iter()
                .map(|k| kind_words(*k))
                .collect::<Vec<_>>()
                .join(" or ");
            let declared: Vec<&str> = tokens
                .names
                .iter()
                .filter(|(_, k)| k.is_none_or(|k| kinds.contains(&k)))
                .map(|(n, _)| n.as_str())
                .collect();
            let listed = match declared.len() {
                0 if !tokens.designed => "this app has no design: a `var(--name)` names a token its `app.tamber` declares (`bun exact.mjs tamber import css <file>` writes a stylesheet's custom properties there)".to_string(),
                0 => format!("the design declares no {said} token"),
                n => {
                    let shown: Vec<String> = declared.iter().take(20).map(|n| format!("`{n}`")).collect();
                    let more = if n > 20 { format!(" and {} more", n - 20) } else { String::new() };
                    format!("its {said} tokens are {}{more}", shown.join(", "))
                }
            };
            let dotted = tokens
                .names
                .iter()
                .find(|(n, _)| n.contains('.') && n.replace('.', "-") == name)
                .map(|(n, _)| format!("; `var(--{n})` names `{n}`"))
                .unwrap_or_default();
            let role = crate::theme::role_named(&format!("-{name}"))
                .map(|(_, row)| format!("; `{row}` is a role, which a row names bare (`{}=\"{row}\"`)", a.name))
                .unwrap_or_default();
            let fallback = if fallback {
                "; the fallback is not used for a name the design lacks, so a misspelt name never ships it"
            } else {
                ""
            };
            (
                "tamber-unknown-token",
                format!("{written}: `--{name}` is no token of this app's design; {listed}{dotted}{role}{fallback}"),
            )
        }
    };
    LowerError { id, message, span }
}

/// The token kinds a row takes, by its rows (LLP 1121 D3).
fn kinds(a: &Attr) -> Vec<tamber::Kind> {
    use exact_kernel::style::theme::{admits, Kind as K};
    use tamber::Kind as T;
    let name = contract_syntax::state::state_of(&a.name).map_or(a.name.as_str(), |(_, row)| row);
    if crate::motion::is_row(name) {
        return vec![T::Duration, T::Easing];
    }
    match name {
        "font" => return vec![T::Type],
        "font-family" => return vec![T::Family],
        _ => {}
    }
    let rows = tags::rows(&Attr {
        name: name.to_string(),
        ..a.clone()
    });
    [
        (K::Color, T::Color),
        (K::Length, T::Length),
        (K::Number, T::Number),
        (K::BoxShadow, T::BoxShadow),
        (K::TextShadow, T::TextShadow),
    ]
    .into_iter()
    .filter(|(k, _)| rows.iter().any(|r| admits(*k, *r)))
    .map(|(_, t)| t)
    .collect()
}

/// A kind in an author's words.
fn kind_words(kind: tamber::Kind) -> &'static str {
    match kind {
        tamber::Kind::Color => "colour",
        tamber::Kind::BoxShadow => "box shadow",
        tamber::Kind::TextShadow => "text shadow",
        other => other.name(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(names: &[&str]) -> Tokens {
        Tokens {
            names: names.iter().map(|n| (n.to_string(), None)).collect(),
            designed: true,
            parsed: true,
        }
    }

    fn written(s: &str) -> Result<Option<String>, Why> {
        text(s, &tokens(&["ink", "space.row"]), false, &mut None).map(|o| o.map(|(s, _)| s))
    }

    #[test]
    fn a_declared_var_is_its_name_wherever_it_stands() {
        assert_eq!(written("var(--ink)").unwrap().as_deref(), Some("ink"));
        assert_eq!(written("VAR( --ink )").unwrap().as_deref(), Some("ink"));
        assert_eq!(
            written("1px solid var(--ink, #000)").unwrap().as_deref(),
            Some("1px solid ink")
        );
        assert_eq!(
            written("color-mix(in srgb, var(--ink) 50%, var(--ink, rgb(0 0 0)))")
                .unwrap()
                .as_deref(),
            Some("color-mix(in srgb, ink 50%, ink)")
        );
        assert_eq!(
            written("var(--space.row)").unwrap().as_deref(),
            Some("space.row")
        );
        // Quoted text, and a word ending in `var(`, are not calls.
        assert_eq!(written("'var(--x)', serif").unwrap(), None);
        assert_eq!(written("somevar(--x)").unwrap(), None);
    }

    #[test]
    fn any_other_var_is_refused() {
        assert_eq!(
            written("var(--inc)"),
            Err(Why::Unknown("inc".into(), false))
        );
        assert_eq!(
            written("var(--inc, red)"),
            Err(Why::Unknown("inc".into(), true))
        );
        assert_eq!(written("var(ink)"), Err(Why::Unnamed));
        assert_eq!(written("var(--ink"), Err(Why::Unnamed));
        // A name a template's hole completes.
        let t = tokens(&["ink"]);
        assert_eq!(text("var(--", &t, true, &mut None), Err(Why::Computed));
    }

    #[test]
    fn a_fallback_is_kept_for_the_warning() {
        let t = tokens(&["ink"]);
        let (_, fallbacks) = text("var(--ink, #111)", &t, false, &mut None)
            .unwrap()
            .unwrap();
        assert_eq!(fallbacks, ["var(--ink, #111)"]);
        assert_eq!(call_name("var(--ink, #111)"), "ink");
        assert_eq!(whole_var(" var(--ink) "), Some(("ink".into(), false)));
        assert_eq!(
            whole_var("var(--ink, rgb(0 0 0))"),
            Some(("ink".into(), true))
        );
        assert_eq!(whole_var("color: var(--ink);"), None);
        assert_eq!(whole_var("var(--ink, #000) var(--paper)"), None);
    }
}
