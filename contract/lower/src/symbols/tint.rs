//! `lower-symbol-tint`: `-exact-tint-color` on a `symbol:` image. A symbol
//! is drawn in its computed `color` on every host (LLP 1081 stage 2;
//! docs/agent-pitfalls.md), and `-exact-tint-color` colours a raster image
//! and a native control's face, so on a symbol it does nothing on Apple,
//! where the symbol falls back to the text colour (theme2's 17:23Z ruling,
//! 2026-10-23: the action row's secondary-label icons drew opaque). The
//! repair is `color=`. The tint is the image's own row or its `class`'s.
//! A native button's face (an `image` among its children, the button one
//! lowering made the platform's own) keeps the button's tint (LLP 1115 D4)
//! and is not warned. A source the compiler can't read as a symbol, such as
//! a computed one with no literal `symbol:` start, a computed `class`, and a
//! button lowered both ways (a component's, used twice) get the benefit of
//! the doubt.

use crate::warnings::TextStyles;
use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, StyleDecl, TemplatePart};
use std::collections::BTreeSet;

/// Every tinted symbol under `nodes`, once per source place (a component used
/// twice is inlined twice): `styles` are the file's classes, `lowered` what
/// lowering made of each element (whether a button is the platform's own).
pub(crate) fn tints(
    nodes: &[Node],
    styles: &[StyleDecl],
    lowered: &TextStyles,
    out: &mut Vec<LowerError>,
) {
    let mut seen = BTreeSet::new();
    walk(
        nodes,
        false,
        &mut Pass {
            styles,
            lowered,
            seen: &mut seen,
            out,
        },
    );
}

struct Pass<'a> {
    styles: &'a [StyleDecl],
    lowered: &'a TextStyles,
    seen: &'a mut BTreeSet<(u32, u32, u32)>,
    out: &'a mut Vec<LowerError>,
}

fn walk(nodes: &[Node], face: bool, pass: &mut Pass<'_>) {
    for node in nodes {
        match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                ..
            } => {
                let span = node.span();
                if let (false, "image", Some(source)) = (face, tag.as_str(), positional.first()) {
                    // (Kept once warned: a component's image may be a raster in one use, a symbol in the next.)
                    let key = (span.source_id, span.line, span.col);
                    if !pass.seen.contains(&key) {
                        if let Some(warning) = symbol_tint(source, attrs, pass.styles) {
                            pass.seen.insert(key);
                            pass.out.push(warning);
                        }
                    }
                }
                let face = tag == "button" && native(node, attrs, pass.lowered);
                walk(children, face, pass);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, face, pass);
                walk(otherwise, face, pass);
            }
            Node::Each { body, .. } => walk(body, face, pass),
            Node::Match { some, none, .. } => {
                walk(&some.1, face, pass);
                walk(none, face, pass);
            }
            Node::Use { children, .. } => walk(children, false, pass),
            Node::Children { .. } => {}
        }
    }
}

/// Whether a button may be the platform's own, whose face keeps its tint: as lowering made it
/// (`TextStyle::native`), unless it made it both ways, or unknown. With nothing lowered (a node
/// lowering did not reach), not a literal `appearance="none"`.
fn native(node: &Node, attrs: &[Attr], lowered: &TextStyles) -> bool {
    let span = node.span();
    let made: Vec<_> = lowered
        .iter()
        .filter(|((at, _, tag), _)| *at == span && tag == "button")
        .map(|(_, style)| style.as_ref().and_then(|s| s.native))
        .collect();
    if made.is_empty() {
        return !attrs
            .iter()
            .rev()
            .find(|a| a.name == "appearance")
            .is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "none"));
    }
    made.iter().any(|native| *native != Some(false))
}

/// The authored `-exact-tint-color` an element is drawn with: its own row, else its literal
/// `class`'s (the last of each wins); none through a computed class.
fn tint<'a>(attrs: &'a [Attr], styles: &'a [StyleDecl]) -> Option<&'a Attr> {
    let named = |rows: &'a [Attr]| {
        rows.iter()
            .rev()
            .find(|a| a.written && a.name == "-exact-tint-color")
    };
    named(attrs).or_else(|| {
        attrs
            .iter()
            .rev()
            .filter(|a| a.name == "class")
            .find_map(|class| match &class.value {
                Expr::Ident(name, _) => styles
                    .iter()
                    .find(|s| &s.name == name)
                    .and_then(|s| named(&s.attrs)),
                _ => None,
            })
    })
}

/// The warning for an `image` whose source is `source` and whose attributes
/// are `attrs`, if it is a symbol drawn with an authored `-exact-tint-color`.
fn symbol_tint(source: &Expr, attrs: &[Attr], styles: &[StyleDecl]) -> Option<LowerError> {
    let tint = tint(attrs, styles)?;
    if !symbol(source) {
        return None;
    }
    let repair = if attrs.iter().any(|a| a.name == "color") {
        "drop it: its `color` is what draws it".to_string()
    } else {
        format!("write `color={}` instead", shown(tint))
    };
    Some(LowerError {
        id: "lower-symbol-tint",
        message: format!("`-exact-tint-color` on a `symbol:` image does nothing: a symbol is drawn in its `color` on every host (Apple draws this one in the text colour), and `-exact-tint-color` colours a raster image or a native control's face; {repair}"),
        span: tint.span,
    })
}

/// Whether a source is a symbol: a literal `symbol:…`, a template that starts
/// so, or a choice between two such.
fn symbol(e: &Expr) -> bool {
    match e {
        Expr::Str(s, _) => s.starts_with("symbol:"),
        Expr::Template(parts, _) => {
            matches!(parts.first(), Some(TemplatePart::Text(t)) if t.starts_with("symbol:"))
        }
        Expr::Ternary(_, a, b, _) => symbol(a) && symbol(b),
        _ => false,
    }
}

/// The tint's value as the repair writes it: a literal quoted, anything else
/// as `…`, which the author knows.
fn shown(tint: &Attr) -> String {
    match &tint.value {
        // (Written on that row: a component's prop is the first use's string by now, whose place is the call's.)
        Expr::Str(s, at) if (at.source_id, at.line) == (tint.span.source_id, tint.span.line) => {
            format!("\"{s}\"")
        }
        _ => "…".to_string(),
    }
}
