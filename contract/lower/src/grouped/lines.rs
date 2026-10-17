//! A row's lines drawn as the platform's subtitle cell draws them (LLP
//! 1116.000 D3.2): a title in the row's size and colour, a later line the
//! subheadline in the secondary colour.

use super::{attr, hidden, m, n, s, with, Count, BODY, MARK, SECONDARY, SUBHEADLINE};
use contract_syntax::{Attr, Expr, Node};

/// A stack of lines in a row that is not the subtitle cell, with `sheet`
/// (its margins): its texts are drawn as the subtitle cell draws its lines
/// (LLP 1116.000 D3.2), the first a title in the row's size and colour, a
/// later one the subheadline in the secondary colour. It is presentation left
/// unsaid, which is the platform's (LLP 1115): a text's own `font`,
/// `font-size` or `color` replaces the row it names, as the author's rows
/// replace the sheet's, and so does the stack's own (`stack_lines`).
pub(super) fn stack_of_lines(node: &Node, sheet: Vec<Attr>) -> Node {
    let Node::Element {
        attrs, children, ..
    } = node
    else {
        return node.clone();
    };
    let lines = stack_lines(children, attrs);
    match with(node, sheet) {
        Node::Element {
            tag,
            positional,
            attrs,
            span,
            instance,
            ..
        } => Node::Element {
            tag,
            positional,
            attrs,
            children: lines,
            span,
            instance,
        },
        other => other,
    }
}

/// A title over a subtitle: the platform's subtitle cell (UIKit's has 15
/// above and below), the second line the subheadline in the secondary
/// colour.
pub(super) fn subtitle(node: &Node) -> Node {
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        span,
        instance,
    } = node
    else {
        return node.clone();
    };
    let at = *span;
    let parts = stack_lines(children, attrs);
    let mut rows = vec![
        n("flex-grow", 1.0, at),
        n("min-width", 0.0, at),
        m("padding-top", "lines-padding-block", at),
        m("padding-bottom", "lines-padding-block", at),
        m("row-gap", "subtitle-gap", at),
    ];
    rows.extend(attrs.iter().cloned());
    Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: rows,
        children: parts,
        span: at,
        instance: *instance,
    }
}

/// A subtitle stack's lines: the title in the row's size and colour, the
/// second line the subheadline in the secondary colour. A size or colour the
/// stack's author wrote (`own`, its rows) reaches its lines, which the
/// sheet's would otherwise replace; a class or a style may say either.
pub(super) fn stack_lines(children: &[Node], own: &[Attr]) -> Vec<Node> {
    let says = |names: &[&str]| {
        own.iter().any(|a| {
            names.contains(&a.name.as_str())
                || matches!(a.name.as_str(), "class" | "className" | "style")
        })
    };
    let (size, colour) = (!says(&["font", "font-size"]), !says(&["color"]));
    let keep = move |a: &Attr| match a.name.strip_prefix(MARK) {
        Some("font-size") => size,
        Some("color") => colour,
        _ => true,
    };
    // The second line, whether written or shown by a condition: a text
    // after the first shown one, chosen by the condition where the arms
    // differ (as `part` counts a row's texts).
    fn line(c: &Node, texts: &mut Count, keep: &dyn Fn(&Attr) -> bool) -> Node {
        match c {
            Node::When {
                tag,
                cond,
                then,
                otherwise,
                span,
            } => {
                let start = Count::Known(texts.low());
                *texts = start.clone();
                let then = then.iter().map(|n| line(n, texts, keep)).collect();
                let after = std::mem::replace(texts, start);
                let otherwise = otherwise.iter().map(|n| line(n, texts, keep)).collect();
                *texts = match (after, texts.clone()) {
                    (Count::Known(a), Count::Known(b)) if a == b => Count::Known(a),
                    (Count::Known(a), Count::Known(b)) => Count::Cond(cond.clone(), a, b),
                    (a, b) => Count::Known(a.low().min(b.low())),
                };
                Node::When {
                    tag: *tag,
                    cond: cond.clone(),
                    then,
                    otherwise,
                    span: *span,
                }
            }
            Node::Element { tag, span, .. } if tag == "text" && !hidden(c) => {
                let at = *span;
                let size =
                    |title: bool| Expr::Str(if title { BODY } else { SUBHEADLINE }.into(), at);
                // A title inherits the row's tint and authored colour, as
                // the unconditional first line does by writing no colour.
                let colour =
                    |title: bool| Expr::Str(if title { "inherit" } else { SECONDARY }.into(), at);
                let choose = |c: &Expr, a: Expr, b: Expr| {
                    Expr::Ternary(Box::new(c.clone()), Box::new(a), Box::new(b), at)
                };
                let (mut sheet, next) = match texts.clone() {
                    Count::Known(0) => (Vec::new(), Count::Known(1)),
                    Count::Known(count) => (
                        vec![s("font-size", SUBHEADLINE, at), s("color", SECONDARY, at)],
                        Count::Known(count + 1),
                    ),
                    Count::Cond(c, a, b) => (
                        vec![
                            attr("font-size", choose(&c, size(a == 0), size(b == 0)), at),
                            attr("color", choose(&c, colour(a == 0), colour(b == 0)), at),
                        ],
                        Count::Cond(c, a + 1, b + 1),
                    ),
                };
                *texts = next;
                sheet.retain(keep);
                with(c, sheet)
            }
            other => other.clone(),
        }
    }
    let mut texts = Count::Known(0);
    children
        .iter()
        .map(|c| line(c, &mut texts, &keep))
        .collect()
}
