//! A grouped list (LLP 1082): `list appearance="auto"`. Its children are
//! `section`s; a section's leading `header` and trailing `footer` are its
//! texts and everything between them its rows. Contract checks that shape
//! and writes the list's look as a user-agent sheet: rows prepended to the
//! author's, so a class or an attribute of the author's replaces any of
//! them. The web, macOS and Linux draw that sheet; iOS draws the platform's
//! own list over the same nodes (`GroupedListIOS.swift`).

use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Node, Span};

/// `listStyle`'s names (D2), UIKit's three list appearances.
pub(crate) const STYLES: &[&str] = &["inset-grouped", "grouped", "plain"];

/// The colours the sheet writes, iOS 27's measured system colours (§2).
const GROUPED_BACKGROUND: &str = "light-dark(#f2f2f7, #000000)";
const PLAIN_BACKGROUND: &str = "light-dark(#ffffff, #000000)";
const CELL: &str = "light-dark(#ffffff, #1c1c1e)";
const LABEL: &str = "light-dark(#000000, #ffffff)";
const SECONDARY: &str = "light-dark(#3c3c4399, #ebebf599)";
const TERTIARY: &str = "light-dark(#3c3c434d, #ebebf54d)";
const SEPARATOR: &str = "light-dark(#3c3c431f, #54545880)";
const ACCENT: &str = "light-dark(#0088ff, #0091ff)";
const RED: &str = "light-dark(#ff383c, #ff4245)";

/// The gap UIKit leaves where a section has no header or no footer.
const SECTION_GAP: f64 = 17.33;
/// The gap above an inset or grouped list's first section without a header.
const FIRST_GAP: f64 = 35.33;

/// The style a grouped list's attributes ask for: `None` for any other
/// element. `appearance` must be a literal (it decides what the node is),
/// and `listStyle` is only a grouped list's.
pub(crate) fn style(tag: &str, attrs: &[Attr]) -> Result<Option<&'static str>, LowerError> {
    let grouped = match attrs.iter().rev().find(|a| a.name == "appearance") {
        Some(a) if tag == "list" => match &a.value {
            Expr::Str(v, _) => v == "auto",
            _ => {
                return err(
                    "lower-grouped-list",
                    "a `list`'s `appearance` is a literal: `\"auto\"` makes it a grouped list. To switch, write `when` with two lists",
                    a.span,
                )
            }
        },
        _ => false,
    };
    let named = attrs.iter().rev().find(|a| a.name == "listStyle");
    if !grouped {
        return match named {
            Some(a) => err(
                "lower-grouped-list",
                "`listStyle` styles a grouped list: `list appearance=\"auto\"`",
                a.span,
            ),
            None => Ok(None),
        };
    }
    if let Some(a) = attrs
        .iter()
        .find(|a| a.name == "virtualized" && !matches!(a.value, Expr::Bool(false, _)))
    {
        return err(
            "lower-grouped-list",
            "a grouped list builds every row (a settings screen, not a feed); it is never `virtualized`",
            a.span,
        );
    }
    match named.map(|a| (&a.value, a.span)) {
        None => Ok(Some(STYLES[0])),
        Some((Expr::Str(v, _), span)) => match STYLES.iter().find(|s| **s == v) {
            Some(s) => Ok(Some(s)),
            None => err(
                "lower-grouped-list",
                format!(
                    "`listStyle=\"{v}\"` is not a list style; styles: {}",
                    STYLES.join(", ")
                ),
                span,
            ),
        },
        Some((_, span)) => err(
            "lower-grouped-list",
            "`listStyle` is a literal: the sheet Contract writes for the other hosts is chosen when the view compiles",
            span,
        ),
    }
}

/// A sheet row's name mark: the lowering takes marked rows out before an
/// element's classes and puts them under them (`split`), so a class, like an
/// attribute, replaces the sheet. No authored name can carry it.
const MARK: &str = "ua:";

fn attr(name: &str, value: Expr, span: Span) -> Attr {
    Attr {
        name: format!("{MARK}{name}"),
        value,
        span,
    }
}

/// An element's sheet rows, unmarked, and its other attributes when it has
/// any sheet rows.
pub(crate) fn split(attrs: &[Attr]) -> (Vec<Attr>, Option<Vec<Attr>>) {
    if !attrs.iter().any(|a| a.name.starts_with(MARK)) {
        return (Vec::new(), None);
    }
    let (sheet, rest): (Vec<Attr>, Vec<Attr>) = attrs
        .iter()
        .cloned()
        .partition(|a| a.name.starts_with(MARK));
    let sheet = sheet
        .into_iter()
        .map(|a| Attr {
            name: a.name[MARK.len()..].to_owned(),
            ..a
        })
        .collect();
    (sheet, Some(rest))
}
fn s(name: &str, value: &str, span: Span) -> Attr {
    attr(name, Expr::Str(value.into(), span), span)
}
fn n(name: &str, value: f64, span: Span) -> Attr {
    attr(name, Expr::Number(value, span), span)
}

/// The list's own sheet, before its author's rows; `listStyle` is always
/// written, so a host finds a grouped list by its prop.
pub(crate) fn list_rows(style: &'static str, span: Span) -> Vec<Attr> {
    let background = if style == "plain" {
        PLAIN_BACKGROUND
    } else {
        GROUPED_BACKGROUND
    };
    split(&[
        s("background-color", background, span),
        s("listStyle", style, span),
    ])
    .0
}

/// The list's children with the sheet written into them: each `section`'s
/// header, footer and rows, its rows wrapped in one `column` (the rounded
/// group the web draws; UIKit's section).
pub(crate) fn sections(style: &'static str, children: &[Node]) -> Result<Vec<Node>, LowerError> {
    children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            // Under `each` every section shares one body: none is first.
            let first = i == 0 && !matches!(child, Node::Each { .. });
            over(child, &mut |node| section(style, node, first))
        })
        .collect()
}

/// `node` with `f` applied to each element it is or holds through `when`,
/// `each` and `match`.
fn over(
    node: &Node,
    f: &mut dyn FnMut(&Node) -> Result<Node, LowerError>,
) -> Result<Node, LowerError> {
    let all = |nodes: &[Node], f: &mut dyn FnMut(&Node) -> Result<Node, LowerError>| {
        nodes
            .iter()
            .map(|n| over(n, f))
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(match node {
        Node::Element { .. } => f(node)?,
        Node::When {
            cond,
            then,
            otherwise,
            span,
        } => Node::When {
            cond: cond.clone(),
            then: all(then, f)?,
            otherwise: all(otherwise, f)?,
            span: *span,
        },
        Node::Each {
            tag,
            var,
            index,
            list,
            key,
            body,
            span,
        } => Node::Each {
            tag: *tag,
            var: var.clone(),
            index: index.clone(),
            list: list.clone(),
            key: key.clone(),
            body: all(body, f)?,
            span: *span,
        },
        Node::Match {
            subject,
            some,
            none,
            span,
        } => Node::Match {
            subject: subject.clone(),
            some: (some.0.clone(), all(&some.1, f)?),
            none: all(none, f)?,
            span: *span,
        },
        Node::Use { .. } | Node::Children { .. } => node.clone(),
    })
}

fn is(node: &Node, name: &str) -> bool {
    matches!(node, Node::Element { tag, .. } if tag == name)
}

/// Whether `node` holds a `header` or `footer` below control flow, where
/// it would be a row's.
fn stray_label(node: &Node) -> Option<Span> {
    match node {
        Node::Element { tag, span, .. } if tag == "header" || tag == "footer" => Some(*span),
        Node::Element { .. } | Node::Use { .. } | Node::Children { .. } => None,
        Node::When {
            then, otherwise, ..
        } => then.iter().chain(otherwise).find_map(stray_label),
        Node::Each { body, .. } => body.iter().find_map(stray_label),
        Node::Match { some, none, .. } => some.1.iter().chain(none).find_map(stray_label),
    }
}

fn section(style: &'static str, node: &Node, first: bool) -> Result<Node, LowerError> {
    let Node::Element {
        tag,
        positional,
        attrs,
        children,
        span,
        instance,
    } = node
    else {
        unreachable!("`over` passes elements")
    };
    let span = *span;
    if tag != "section" {
        return err(
            "lower-grouped-list",
            format!("a grouped list holds `section`s, not `{tag}`: put rows in a `section`"),
            span,
        );
    }
    let head = children.iter().take_while(|c| is(c, "header")).count();
    let tail = children[head..]
        .iter()
        .rev()
        .take_while(|c| is(c, "footer"))
        .count();
    if head > 1 || tail > 1 {
        return err(
            "lower-grouped-list",
            "a section has at most one `header` and one `footer`",
            span,
        );
    }
    let rows = &children[head..children.len() - tail];
    if let Some(stray) = rows.iter().find_map(stray_label) {
        return err(
            "lower-grouped-list",
            "a section's `header` comes first and its `footer` last, outside `when` and `each`",
            stray,
        );
    }
    // UIKit draws a header or footer as one text.
    for label in children[..head]
        .iter()
        .chain(&children[children.len() - tail..])
    {
        if let Node::Element {
            tag,
            children,
            span,
            ..
        } = label
        {
            if !matches!(children.as_slice(), [only] if is(only, "text")) {
                return err(
                    "lower-grouped-list",
                    format!("a section's `{tag}` holds one `text`, the words UIKit draws there"),
                    *span,
                );
            }
        }
    }
    let inset = style == "inset-grouped";
    // UIKit opens an inset or grouped list whose first section has no
    // header with a deeper gap; a plain list's sections meet.
    let plain = style == "plain";
    let top = match (head == 1 || plain, first) {
        (true, _) => 0.0,
        (false, true) => FIRST_GAP,
        (false, false) => SECTION_GAP,
    };
    let bottom = if tail == 1 || plain { 0.0 } else { SECTION_GAP };
    let mut sheet = vec![n("margin-top", top, span), n("margin-bottom", bottom, span)];
    sheet.extend(attrs.iter().cloned());
    let label = |node: &Node, footer: bool| -> Node {
        let Node::Element {
            tag,
            positional,
            attrs,
            children,
            span,
            instance,
        } = node
        else {
            unreachable!("a header or footer is an element")
        };
        let span = *span;
        let mut rows = vec![
            n("padding-left", if inset { 32.0 } else { 16.0 }, span),
            n("padding-right", if inset { 32.0 } else { 16.0 }, span),
            n("padding-top", if footer { 8.0 } else { 10.0 }, span),
            n("padding-bottom", if footer { 6.0 } else { 10.0 }, span),
            n("font-size", if footer { 13.0 } else { 17.0 }, span),
            s("color", SECONDARY, span),
        ];
        if !footer {
            rows.push(n("font-weight", 600.0, span));
        }
        rows.extend(attrs.iter().cloned());
        Node::Element {
            tag: tag.clone(),
            positional: positional.clone(),
            attrs: rows,
            children: children.clone(),
            span,
            instance: *instance,
        }
    };
    let mut group = vec![s("background-color", CELL, span)];
    if style == "grouped" {
        group.extend([
            n("border-top-width", 1.0, span),
            n("border-bottom-width", 1.0, span),
            s("border-top-style", "solid", span),
            s("border-bottom-style", "solid", span),
            s("border-color", SEPARATOR, span),
        ]);
    }
    if inset {
        group.extend([
            n("margin-left", 16.0, span),
            n("margin-right", 16.0, span),
            n("border-radius", 26.0, span),
        ]);
    }
    // Every row draws the separator under it and overlaps the next by its
    // width; the group clips the last one away (`row`).
    group.push(s("overflow", "hidden", span));
    let body = rows
        .iter()
        .map(|r| over(r, &mut |node| Ok(row(node))))
        .collect::<Result<Vec<_>, _>>()?;
    let mut out: Vec<Node> = children[..head].iter().map(|h| label(h, false)).collect();
    out.push(Node::Element {
        tag: "column".into(),
        positional: Vec::new(),
        attrs: group,
        children: body,
        span,
        instance: *instance,
    });
    out.extend(
        children[children.len() - tail..]
            .iter()
            .map(|f| label(f, true)),
    );
    Ok(Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: sheet,
        children: out,
        span,
        instance: *instance,
    })
}

/// A literal `symbol:` image source's Apple name, as the kernel names it.
fn symbol(node: &Node) -> Option<&str> {
    let Node::Element {
        tag, positional, ..
    } = node
    else {
        return None;
    };
    match (tag.as_str(), positional.first()) {
        ("image", Some(Expr::Str(src, _))) => {
            let role = src.strip_prefix("symbol:")?;
            Some(match role {
                "forward-chevron" => "chevron.forward",
                "checkmark" => "checkmark",
                other => other.strip_prefix("sf/").unwrap_or(other),
            })
        }
        _ => None,
    }
}

/// An accessory's image (D4): the chevron or the checkmark UIKit draws.
pub(crate) fn accessory(apple: &str) -> Option<&'static str> {
    match apple {
        "chevron.forward" | "chevron.right" => Some("disclosure"),
        "checkmark" => Some("checkmark"),
        _ => None,
    }
}

fn with(node: &Node, sheet: Vec<Attr>) -> Node {
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
    let mut rows = sheet;
    rows.extend(attrs.iter().cloned());
    Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: rows,
        children: children.clone(),
        span: *span,
        instance: *instance,
    }
}

/// A row and its direct parts: a cell's metrics (52 high, 16 in, the text
/// at 56 after an icon), its separator from the text to the trailing edge,
/// the icon in the leading margin, a value or subtitle in the secondary
/// colour, an accessory's size and colour; red when `destructive`.
fn row(node: &Node) -> Node {
    let Node::Element {
        attrs,
        children,
        span,
        ..
    } = node
    else {
        return node.clone();
    };
    let span = *span;
    let tint = |plain: &str| -> Expr {
        match attrs.iter().rev().find(|a| a.name == "destructive") {
            Some(Attr {
                value: Expr::Bool(true, _),
                ..
            }) => Expr::Str(RED.into(), span),
            Some(Attr {
                value: Expr::Bool(false, _),
                ..
            })
            | None => Expr::Str(plain.into(), span),
            Some(a) => Expr::Ternary(
                Box::new(a.value.clone()),
                Box::new(Expr::Str(RED.into(), span)),
                Box::new(Expr::Str(plain.into(), span)),
                span,
            ),
        }
    };
    // The row starts at its text: after a leading symbol, a symbol shown
    // by a condition included, as `part` styles it.
    fn inset(first: Option<&Node>, span: Span) -> Expr {
        match first {
            Some(Node::When {
                cond,
                then,
                otherwise,
                ..
            }) => Expr::Ternary(
                Box::new(cond.clone()),
                Box::new(inset(then.first(), span)),
                Box::new(inset(otherwise.first(), span)),
                span,
            ),
            other => {
                let icon = other
                    .and_then(symbol)
                    .is_some_and(|name| accessory(name).is_none());
                Expr::Number(if icon { 56.0 } else { 16.0 }, span)
            }
        }
    }
    let sheet = vec![
        s("display", "flex", span),
        s("flex-direction", "row", span),
        s("align-items", "center", span),
        n("gap", 8.0, span),
        n("min-height", 52.0, span),
        attr("margin-left", inset(children.first(), span), span),
        n("padding-right", 16.0, span),
        n("border-bottom-width", 1.0, span),
        s("border-bottom-style", "solid", span),
        s("border-bottom-color", SEPARATOR, span),
        n("margin-bottom", -1.0, span),
        n("font-size", 17.0, span),
        attr("color", tint(LABEL), span),
        s("text-align", "left", span),
    ];
    let mut texts = 0;
    let last = children.len().saturating_sub(1);
    let stack = text_stack(children);
    let parts = children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            let at = Place {
                i,
                last,
                stack,
                tint: &tint,
            };
            part(child, at, &mut texts)
        })
        .collect();
    let Node::Element {
        tag,
        positional,
        span,
        instance,
        ..
    } = node
    else {
        unreachable!()
    };
    let mut rows = sheet;
    rows.extend(attrs.iter().cloned());
    Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: rows,
        children: parts,
        span: *span,
        instance: *instance,
    }
}

/// A row's part at `i` of `last + 1`: a leading icon, a trailing
/// accessory, the title and its value, a subtitle stack. A `when` is read
/// through, so a checkmark shown by a condition is styled as one.
#[derive(Clone, Copy)]
struct Place<'a> {
    i: usize,
    last: usize,
    /// Whether a `column` here is the row's text stack (`text_stack`).
    stack: bool,
    tint: &'a dyn Fn(&str) -> Expr,
}

fn part(child: &Node, at: Place<'_>, texts: &mut usize) -> Node {
    let Place {
        i,
        last,
        stack,
        tint,
    } = at;
    let place = at;
    if let Node::When {
        cond,
        then,
        otherwise,
        span,
    } = child
    {
        let start = *texts;
        let then: Vec<Node> = then.iter().map(|c| part(c, place, texts)).collect();
        let after_then = *texts;
        *texts = start;
        let otherwise: Vec<Node> = otherwise.iter().map(|c| part(c, place, texts)).collect();
        // A text after the condition is the title unless both arms wrote one.
        *texts = (*texts).min(after_then);
        return Node::When {
            cond: cond.clone(),
            then,
            otherwise,
            span: *span,
        };
    }
    if let Node::Match {
        subject,
        some,
        none,
        span,
    } = child
    {
        let start = *texts;
        let arm: Vec<Node> = some.1.iter().map(|c| part(c, place, texts)).collect();
        let after_some = *texts;
        *texts = start;
        let none: Vec<Node> = none.iter().map(|c| part(c, place, texts)).collect();
        *texts = (*texts).min(after_some);
        return Node::Match {
            subject: subject.clone(),
            some: (some.0.clone(), arm),
            none,
            span: *span,
        };
    }
    let Node::Element { tag, span: at, .. } = child else {
        return child.clone();
    };
    let at = *at;
    match (tag.as_str(), symbol(child)) {
        ("image", Some(name)) if i == 0 && accessory(name).is_none() => with(
            child,
            vec![
                n("width", 24.0, at),
                n("height", 20.0, at),
                s("object-fit", "contain", at),
                n("margin-left", -40.0, at),
                n("margin-right", 8.0, at),
                n("flex-shrink", 0.0, at),
                attr("tint-color", tint(ACCENT), at),
            ],
        ),
        ("image", Some(name)) if i == last && accessory(name).is_some() => {
            let check = accessory(name) == Some("checkmark");
            with(
                child,
                vec![
                    n("width", if check { 19.0 } else { 14.0 }, at),
                    n("height", if check { 17.0 } else { 14.0 }, at),
                    s("object-fit", "contain", at),
                    n("font-weight", 600.0, at),
                    n("flex-shrink", 0.0, at),
                    s("tint-color", if check { ACCENT } else { TERTIARY }, at),
                ],
            )
        }
        ("text", _) => {
            *texts += 1;
            if *texts == 1 {
                with(
                    child,
                    vec![n("flex-grow", 1.0, at), n("min-width", 0.0, at)],
                )
            } else {
                with(child, vec![s("color", SECONDARY, at)])
            }
        }
        ("column", _) if stack => subtitle(child),
        _ => child.clone(),
    }
}

/// Whether a row's parts are a leading symbol, one `column` of one or two
/// texts and a trailing accessory at most: the subtitle cell the kernel
/// reads. A `column` in any other row is the author's.
fn text_stack(children: &[Node]) -> bool {
    let mut rest = children;
    if let Some((first, after)) = rest.split_first() {
        if symbol(first).is_some_and(|n| accessory(n).is_none()) {
            rest = after;
        }
    }
    if let Some((last, before)) = rest.split_last() {
        let trailing = symbol(last).is_some_and(|n| accessory(n).is_some())
            || matches!(last, Node::Element { tag, .. } if tag == "input" || tag == "button");
        if trailing {
            rest = before;
        }
    }
    matches!(rest, [Node::Element { tag, children, .. }]
        if tag == "column" && (1..=2).contains(&children.len()) && children.iter().all(|c| is(c, "text")))
}

/// A title over a subtitle: UIKit's subtitle cell, 15 above and below, the
/// second line 15 points in the secondary colour.
fn subtitle(node: &Node) -> Node {
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
    let mut texts = 0;
    let parts = children
        .iter()
        .map(|c| {
            if !is(c, "text") {
                return c.clone();
            }
            texts += 1;
            if texts == 2 {
                let Node::Element { span, .. } = c else {
                    unreachable!()
                };
                with(
                    c,
                    vec![n("font-size", 15.0, *span), s("color", SECONDARY, *span)],
                )
            } else {
                c.clone()
            }
        })
        .collect();
    let mut rows = vec![
        n("flex-grow", 1.0, at),
        n("min-width", 0.0, at),
        n("padding-top", 15.0, at),
        n("padding-bottom", 15.0, at),
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
