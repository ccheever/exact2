//! @ref LLP 1121 D3 (stage 6 E) — duration and easing tokens, written into
//! the `transition` and `animation` values that name them at build, before
//! a row's codec, the keyframes' scoping or the longhands' composing read
//! them: the loader does it in each file before scoping ([`substitute`]),
//! and lowering again for a value an expansion carried in ([`attrs`]).
//!
//! A motion token's name is dotted (`duration.fast`; the crate refuses
//! another, `tamber-motion-name`), and no CSS property, keyframes name or
//! easing keyword has a dot: so a dotted word of a motion row that is a
//! token's name is that token wherever it stands in the shorthand. A word
//! inside quotes, or touching a template's `${…}`, is left as written.

use contract_syntax::{Attr, Expr, File, Node, Span, TemplatePart};
use exact_tamber as tamber;

/// Whether `name` is a motion row: a shorthand or a timing longhand, in
/// either spelling.
fn row(name: &str) -> bool {
    matches!(
        name,
        "transition"
            | "animation"
            | "-exact-exit-animation"
            | "-exact-layout-transition"
            | "transition-duration"
            | "transitionDuration"
            | "transition-delay"
            | "transitionDelay"
            | "transition-timing-function"
            | "transitionTimingFunction"
            | "animation-duration"
            | "animationDuration"
            | "animation-delay"
            | "animationDelay"
            | "animation-timing-function"
            | "animationTimingFunction"
    )
}

/// A literal that names a motion token: a row's, which the loader wrote the
/// token into, or a component's argument, which an expansion carries into
/// one. What `uses` reports (`tamber_uses::motion_sites`).
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    /// The literal.
    pub span: Span,
    /// The attribute: the row's name, or the argument's.
    pub attr: String,
    /// The component an argument is given to; `None` for a row.
    pub component: Option<String>,
    /// The tokens it names, each once.
    pub tokens: Vec<String>,
}

/// The design's duration and easing tokens, each name and value; none
/// when it does not check (lowering reports why). An inline `theme`'s names
/// are undotted, so it declares none (`tamber-motion-name`).
pub fn tokens(design: &str) -> Vec<(String, String)> {
    let Ok(mut design) = tamber::parse(design) else {
        return Vec::new();
    };
    crate::theme::stand_in_profiles(&mut design);
    let Ok(folding) = tamber::check_design(design, &crate::theme::vocab()) else {
        return Vec::new();
    };
    folding
        .tokens
        .iter()
        .filter(|t| matches!(t.kind, tamber::Kind::Duration | tamber::Kind::Easing))
        .filter_map(|t| Some((t.name.clone(), t.values.first()?.css())))
        .collect()
}

/// `text` with each motion token it names written as its value, and the
/// tokens it named; `None` when it names none. `glued`: whether a template's
/// hole touches its start and its end, so a word there is no whole word.
/// `quote`: the quote a template's earlier text left open, and this text
/// leaves open, so a quoted name across a hole (`'${name}'`) stays one.
fn text(
    text: &str,
    tokens: &[(String, String)],
    glued: (bool, bool),
    quote: &mut Option<char>,
) -> Option<(String, Vec<String>)> {
    fn word(c: char) -> bool {
        c.is_ascii_alphanumeric() || "-_.".contains(c)
    }
    let (mut out, mut named) = (String::new(), Vec::new());
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        // A quoted name (`animation="'fade.in' 1s"`) is a name.
        if let Some(q) = *quote {
            let end = rest.find(q).map_or(rest.len(), |e| {
                *quote = None;
                e + 1
            });
            out.push_str(&rest[..end]);
            at += end;
            continue;
        }
        let c = rest.chars().next().expect("not at the end");
        if c == '"' || c == '\'' {
            *quote = Some(c);
            out.push(c);
            at += 1;
            continue;
        }
        if !word(c) {
            out.push(c);
            at += c.len_utf8();
            continue;
        }
        let end = rest.find(|c: char| !word(c)).unwrap_or(rest.len());
        let w = &rest[..end];
        let whole = !(at == 0 && glued.0) && !(at + end == text.len() && glued.1);
        match tokens
            .iter()
            .find(|(name, _)| name == w)
            .filter(|_| whole && w.contains('.'))
        {
            Some((name, value)) => {
                out.push_str(value);
                if !named.contains(name) {
                    named.push(name.clone());
                }
            }
            None => out.push_str(w),
        }
        at += end;
    }
    (!named.is_empty()).then_some((out, named))
}

/// The literals a value named a token in, each with the tokens it named.
type Named = Vec<(Span, Vec<String>)>;

/// Rewrite a value's literals in place, through `?:`, `match`, `let` (its
/// value too, which its body may name), `+`, and a template's text and
/// holes; each literal rewritten is a site, with the tokens it named.
fn rewrite(e: &mut Expr, tokens: &[(String, String)], sites: &mut Named) {
    match e {
        Expr::Str(s, span) => {
            if let Some((next, named)) = text(s, tokens, (false, false), &mut None) {
                *s = next;
                sites.push((*span, named));
            }
        }
        Expr::Template(parts, span) => {
            let (mut all, mut quote) = (Vec::new(), None);
            let n = parts.len();
            for i in 0..n {
                let glued = (
                    i > 0 && matches!(parts[i - 1], TemplatePart::Expr(_)),
                    i + 1 < n && matches!(parts[i + 1], TemplatePart::Expr(_)),
                );
                // A hole's own value: a prop an expansion made a literal.
                if let TemplatePart::Expr(e) = &mut parts[i] {
                    if quote.is_none() {
                        rewrite(e, tokens, sites);
                    }
                }
                if let TemplatePart::Text(s) = &mut parts[i] {
                    if let Some((next, named)) = text(s, tokens, glued, &mut quote) {
                        *s = next;
                        all.extend(
                            named
                                .into_iter()
                                .filter(|t| !all.contains(t))
                                .collect::<Vec<_>>(),
                        );
                    }
                }
            }
            if !all.is_empty() {
                sites.push((*span, all));
            }
        }
        Expr::Ternary(_, yes, no, _) => {
            rewrite(yes, tokens, sites);
            rewrite(no, tokens, sites);
        }
        Expr::Match { some, none, .. } => {
            rewrite(some, tokens, sites);
            rewrite(none, tokens, sites);
        }
        Expr::Let { value, body, .. } => {
            rewrite(value, tokens, sites);
            rewrite(body, tokens, sites);
        }
        // A concatenation's strings (`"opacity " + "duration.fast"`).
        Expr::Binary(contract_syntax::BinOp::Add, left, right, _) => {
            rewrite(left, tokens, sites);
            rewrite(right, tokens, sites);
        }
        _ => {}
    }
}

/// An attribute with its motion tokens written in, and the literals that
/// named them; `None` when it is no motion row or names none.
fn attr(a: &Attr, tokens: &[(String, String)]) -> Option<(Attr, Named)> {
    if !row(&a.name) || tokens.is_empty() {
        return None;
    }
    let mut out = a.clone();
    let mut sites = Vec::new();
    rewrite(&mut out.value, tokens, &mut sites);
    (!sites.is_empty()).then_some((out, sites))
}

/// A row's value folded to its text (a keyframe's, from a function of
/// literal arguments), with its motion tokens written in; `None` when it is
/// no motion row or names none.
pub(crate) fn folded(name: &str, value: &str, tokens: &[(String, String)]) -> Option<String> {
    row(name)
        .then(|| text(value, tokens, (false, false), &mut None))
        .flatten()
        .map(|(out, _)| out)
}

/// `list` with the motion tokens of its motion rows written in; `None` when
/// none names one. Lowering's, for the rows an expansion (a sheet, a
/// component's prop) carried in, before the longhands are composed.
pub(crate) fn attrs(list: &[Attr], tokens: &[(String, String)]) -> Option<Vec<Attr>> {
    if tokens.is_empty() || !list.iter().any(|a| row(&a.name)) {
        return None;
    }
    let mut out = None;
    for (i, a) in list.iter().enumerate() {
        if let Some((next, _)) = attr(a, tokens) {
            out.get_or_insert_with(|| list.to_vec())[i] = next;
        }
    }
    out
}

/// Write the design's motion tokens into every motion row of `file`'s
/// styles, keyframes and views, and give back the sites: those rows'
/// literals, and the component arguments whose literals name a token (an
/// expansion writes those in, [`attrs`]).
pub fn substitute(file: &mut File, tokens: &[(String, String)]) -> Vec<Site> {
    let mut sites = Vec::new();
    if tokens.is_empty() {
        return sites;
    }
    let mut rows = |list: &mut Vec<Attr>, sites: &mut Vec<Site>| {
        for a in list.iter_mut() {
            if let Some((next, found)) = attr(a, tokens) {
                *a = next;
                sites.extend(found.into_iter().map(|(span, tokens)| Site {
                    span,
                    attr: a.name.clone(),
                    component: None,
                    tokens,
                }));
            }
        }
    };
    for s in &mut file.styles {
        rows(&mut s.attrs, &mut sites);
    }
    // A keyframe's own timing (`animation-timing-function` in a step).
    for k in &mut file.keyframes {
        for frame in &mut k.frames {
            rows(&mut frame.attrs, &mut sites);
        }
    }
    fn nodes(
        list: &mut [Node],
        tokens: &[(String, String)],
        rows: &mut dyn FnMut(&mut Vec<Attr>, &mut Vec<Site>),
        sites: &mut Vec<Site>,
    ) {
        for node in list {
            match node {
                Node::Element {
                    attrs: a, children, ..
                } => {
                    rows(a, sites);
                    nodes(children, tokens, rows, sites);
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    nodes(then, tokens, rows, sites);
                    nodes(otherwise, tokens, rows, sites);
                }
                Node::Each { body, .. } => nodes(body, tokens, rows, sites),
                Node::Match { some, none, .. } => {
                    nodes(&mut some.1, tokens, rows, sites);
                    nodes(none, tokens, rows, sites);
                }
                Node::Use {
                    name,
                    args,
                    children,
                    ..
                } => {
                    // Read, not written: the argument may be no motion row's.
                    for a in args.iter() {
                        let mut value = a.value.clone();
                        let mut found = Vec::new();
                        rewrite(&mut value, tokens, &mut found);
                        sites.extend(found.into_iter().map(|(span, tokens)| Site {
                            span,
                            attr: a.name.clone(),
                            component: Some(name.clone()),
                            tokens,
                        }));
                    }
                    nodes(children, tokens, rows, sites);
                }
                Node::Children { .. } => {}
            }
        }
    }
    for c in &mut file.components {
        nodes(&mut c.view, tokens, &mut rows, &mut sites);
    }
    sites
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Vec<(String, String)> {
        vec![
            ("duration.fast".into(), "150ms".into()),
            (
                "easing.standard".into(),
                "cubic-bezier(0.2, 0, 0, 1)".into(),
            ),
        ]
    }

    #[test]
    fn a_dotted_token_is_written_wherever_it_stands() {
        let t = tokens();
        let got = |s: &str| text(s, &t, (false, false), &mut None).map(|(s, _)| s);
        assert_eq!(
            got("opacity duration.fast easing.standard").as_deref(),
            Some("opacity 150ms cubic-bezier(0.2, 0, 0, 1)")
        );
        assert_eq!(
            got("duration.fast opacity").as_deref(),
            Some("150ms opacity")
        );
        assert_eq!(
            got("a 1s, b duration.fast").as_deref(),
            Some("a 1s, b 150ms")
        );
        // A quoted name, a longer word and a time are left as written.
        assert_eq!(got("'duration.fast' 1s"), None);
        assert_eq!(got("duration.fast2 1s"), None);
        assert_eq!(got("opacity 1.5s"), None);
    }

    #[test]
    fn a_word_touching_a_hole_is_no_whole_word() {
        let t = tokens();
        assert_eq!(text("duration.fast 1s", &t, (true, false), &mut None), None);
        assert_eq!(
            text("opacity duration.fast", &t, (false, true), &mut None),
            None
        );
        // A quote a hole's text left open closes here: what follows is read.
        let mut quote = Some('\'');
        assert_eq!(
            text("' duration.fast", &t, (true, false), &mut quote)
                .map(|(s, _)| s)
                .as_deref(),
            Some("' 150ms")
        );
        assert_eq!(quote, None);
        assert_eq!(
            text(" duration.fast ", &t, (true, true), &mut None)
                .map(|(s, _)| s)
                .as_deref(),
            Some(" 150ms ")
        );
    }
}
