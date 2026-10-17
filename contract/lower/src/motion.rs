//! @ref LLP 1121 D3 (stage 6 E) — duration and easing tokens, written into
//! the `transition` and `animation` values that name them at build, before
//! a row's codec, the keyframes' scoping or the longhands' composing read
//! them: the loader does it in each file before scoping ([`substitute`]),
//! and lowering again for a value an expansion carried in ([`attr`]).
//!
//! A word is a token only in a timing's place: never a transition's
//! property (its first word) nor an animation's keyframes name, so a token
//! named `opacity` or `fast` leaves `transition="opacity 1s"` and
//! `animation="fast 1s"` as written.

use contract_syntax::{Attr, Expr, File, Node, TemplatePart};
use exact_tamber as tamber;

/// The rows a motion token is a component of: the shorthands and the
/// timing longhands, in either spelling.
fn row(name: &str) -> Option<Row> {
    match name {
        "transition" => Some(Row::Transition),
        "animation" | "-exact-exit-animation" => Some(Row::Animation),
        // A layout transition is a timing alone: no property first.
        "-exact-layout-transition"
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
        | "animationTimingFunction" => Some(Row::Longhand),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Transition,
    Animation,
    Longhand,
}

/// The design's duration and easing tokens, each name and value; none
/// when it has none, or does not check (lowering reports why).
pub fn tokens(design: &str) -> Vec<(String, String)> {
    let Ok(folding) = tamber::check(design, &crate::theme::vocab()) else {
        return Vec::new();
    };
    folding
        .tokens
        .iter()
        .filter(|t| matches!(t.kind, tamber::Kind::Duration | tamber::Kind::Easing))
        .filter_map(|t| Some((t.name.clone(), t.values.first()?.css())))
        .collect()
}

/// `text`, a `row` value, with each token in a timing's place written as
/// its value; `None` when it names none.
fn text(row: Row, text: &str, tokens: &[(String, String)], keyframes: &[String]) -> Option<String> {
    fn word(c: char) -> bool {
        c.is_ascii_alphanumeric() || "-_.".contains(c)
    }
    let mut out = String::new();
    let mut moved = false;
    for (i, item) in text.split(',').enumerate() {
        if i > 0 {
            out.push(',');
        }
        let mut first = true;
        let mut rest = item;
        while !rest.is_empty() {
            let end = rest.find(|c: char| !word(c)).unwrap_or(rest.len());
            if end == 0 {
                let c = rest.chars().next().unwrap_or(' ');
                out.push(c);
                rest = &rest[c.len_utf8()..];
                continue;
            }
            let w = &rest[..end];
            // A transition's property, an animation's keyframes name.
            let kept = match row {
                Row::Transition => first,
                Row::Animation => keyframes.iter().any(|k| k == w),
                Row::Longhand => false,
            };
            first = false;
            match tokens.iter().find(|(name, _)| name == w).filter(|_| !kept) {
                Some((_, value)) => {
                    out.push_str(value);
                    moved = true;
                }
                None => out.push_str(w),
            }
            rest = &rest[end..];
        }
    }
    moved.then_some(out)
}

/// Rewrite a value's literals in place, through `?:`, `match`, `let` (its
/// value too, which its body may name) and a template's text.
fn rewrite(e: &mut Expr, f: &dyn Fn(&str) -> Option<String>) -> bool {
    match e {
        Expr::Str(s, _) => match f(s) {
            Some(next) => {
                *s = next;
                true
            }
            None => false,
        },
        Expr::Template(parts, _) => {
            let mut moved = false;
            for part in parts {
                if let TemplatePart::Text(s) = part {
                    if let Some(next) = f(s) {
                        *s = next;
                        moved = true;
                    }
                }
            }
            moved
        }
        Expr::Ternary(_, yes, no, _) => rewrite(yes, f) | rewrite(no, f),
        Expr::Match { some, none, .. } => rewrite(some, f) | rewrite(none, f),
        Expr::Let { value, body, .. } => rewrite(value, f) | rewrite(body, f),
        _ => false,
    }
}

/// An attribute with its motion tokens written in; `None` when it is no
/// motion row or names none.
pub(crate) fn attr(a: &Attr, tokens: &[(String, String)], keyframes: &[String]) -> Option<Attr> {
    let row = row(&a.name)?;
    if tokens.is_empty() {
        return None;
    }
    let mut out = a.clone();
    rewrite(&mut out.value, &|s| text(row, s, tokens, keyframes)).then_some(out)
}

/// Write the design's motion tokens into every motion row of `file`'s
/// styles and views; `keyframes` are every file's keyframes names, as
/// declared (before scoping renames them).
pub fn substitute(file: &mut File, tokens: &[(String, String)], keyframes: &[String]) {
    if tokens.is_empty() {
        return;
    }
    let mut attrs = |attrs: &mut Vec<Attr>| {
        for a in attrs.iter_mut() {
            if let Some(next) = attr(a, tokens, keyframes) {
                *a = next;
            }
        }
    };
    for s in &mut file.styles {
        attrs(&mut s.attrs);
    }
    // A keyframe's own timing (`animation-timing-function` in a step).
    for k in &mut file.keyframes {
        for frame in &mut k.frames {
            attrs(&mut frame.attrs);
        }
    }
    fn nodes(list: &mut [Node], attrs: &mut dyn FnMut(&mut Vec<Attr>)) {
        for node in list {
            match node {
                Node::Element {
                    attrs: a, children, ..
                } => {
                    attrs(a);
                    nodes(children, attrs);
                }
                Node::When {
                    then, otherwise, ..
                } => {
                    nodes(then, attrs);
                    nodes(otherwise, attrs);
                }
                Node::Each { body, .. } => nodes(body, attrs),
                Node::Match { some, none, .. } => {
                    nodes(&mut some.1, attrs);
                    nodes(none, attrs);
                }
                Node::Use { children, .. } => nodes(children, attrs),
                Node::Children { .. } => {}
            }
        }
    }
    for c in &mut file.components {
        nodes(&mut c.view, &mut attrs);
    }
}
