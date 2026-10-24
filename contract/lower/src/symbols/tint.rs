//! `lower-symbol-tint`: `-exact-tint-color` on a `symbol:` image. A symbol
//! is drawn in its computed `color` on every host (LLP 1081 stage 2;
//! docs/agent-pitfalls.md), and `-exact-tint-color` colours a raster image
//! and a native control's face, so on a symbol it does nothing on Apple,
//! where the symbol falls back to the text colour (theme2's 17:23Z ruling,
//! 2026-10-23: the action row's secondary-label icons drew opaque). The
//! repair is `color=`. A native button's face (an `image` among its
//! children, the button not `appearance="none"`) keeps the button's tint
//! (LLP 1115 D4) and is not warned; a source the compiler can't read as a
//! symbol, such as a computed one with no literal `symbol:` start, and a
//! computed `appearance`, get the benefit of the doubt.

use crate::LowerError;
use contract_syntax::{Attr, Expr, Node, TemplatePart};
use std::collections::BTreeSet;

/// Every tinted symbol under `nodes`, once per source line (a component used
/// twice is inlined twice).
pub(crate) fn tints(nodes: &[Node], out: &mut Vec<LowerError>) {
    walk(nodes, false, &mut BTreeSet::new(), out);
}

fn walk(
    nodes: &[Node],
    face: bool,
    seen: &mut BTreeSet<(u32, u32, u32)>,
    out: &mut Vec<LowerError>,
) {
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
                    if seen.insert((span.source_id, span.line, span.col)) {
                        out.extend(symbol_tint(source, attrs));
                    }
                }
                walk(children, tag == "button" && native(attrs), seen, out);
            }
            Node::When {
                then, otherwise, ..
            } => {
                walk(then, face, seen, out);
                walk(otherwise, face, seen, out);
            }
            Node::Each { body, .. } => walk(body, face, seen, out),
            Node::Match { some, none, .. } => {
                walk(&some.1, face, seen, out);
                walk(none, face, seen, out);
            }
            Node::Use { children, .. } => walk(children, false, seen, out),
            Node::Children { .. } => {}
        }
    }
}

/// Whether a button may be the platform's own, whose face keeps its tint: not a literal `appearance="none"`.
fn native(attrs: &[Attr]) -> bool {
    !attrs
        .iter()
        .rev()
        .find(|a| a.name == "appearance")
        .is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "none"))
}

/// The warning for an `image` whose source is `source` and whose attributes
/// are `attrs`, if it is a symbol with an authored `-exact-tint-color`.
fn symbol_tint(source: &Expr, attrs: &[Attr]) -> Option<LowerError> {
    let tint = attrs
        .iter()
        .rev()
        .find(|a| a.written && a.name == "-exact-tint-color")?;
    if !symbol(source) {
        return None;
    }
    let repair = if attrs.iter().any(|a| a.name == "color") {
        "drop it: its `color` is what draws it".to_string()
    } else {
        format!("write `color={}` instead", shown(&tint.value))
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
fn shown(value: &Expr) -> String {
    match value {
        Expr::Str(s, _) => format!("\"{s}\""),
        _ => "…".to_string(),
    }
}
