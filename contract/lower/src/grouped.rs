//! A grouped list (LLP 1084): `list appearance="auto"`. Its children are
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

/// The sheet rows a native button takes (LLP 1069.011 D6), for a row whose
/// class made it one: its inset and height, and nothing it refuses.
pub(crate) fn native_rows(sheet: &mut Vec<Attr>) {
    sheet.retain(|a| matches!(a.name.as_str(), "margin-left" | "min-height"));
}

/// A native button's children without the sheet a row's parts were given:
/// they are its face, which carries no style (LLP 1069.011 D5).
pub(crate) fn unsheet(children: &[Node]) -> Option<Vec<Node>> {
    fn strip(node: &Node) -> Node {
        match node {
            Node::Element {
                tag,
                positional,
                attrs,
                children,
                span,
                instance,
            } => Node::Element {
                tag: tag.clone(),
                positional: positional.clone(),
                attrs: attrs
                    .iter()
                    .filter(|a| !a.name.starts_with(MARK))
                    .cloned()
                    .collect(),
                children: children.iter().map(strip).collect(),
                span: *span,
                instance: *instance,
            },
            Node::When {
                tag,
                cond,
                then,
                otherwise,
                span,
            } => Node::When {
                tag: *tag,
                cond: cond.clone(),
                then: then.iter().map(strip).collect(),
                otherwise: otherwise.iter().map(strip).collect(),
                span: *span,
            },
            other => other.clone(),
        }
    }
    fn marked(node: &Node) -> bool {
        match node {
            Node::Element {
                attrs, children, ..
            } => attrs.iter().any(|a| a.name.starts_with(MARK)) || children.iter().any(marked),
            Node::When {
                then, otherwise, ..
            } => then.iter().chain(otherwise).any(marked),
            _ => false,
        }
    }
    children
        .iter()
        .any(marked)
        .then(|| children.iter().map(strip).collect())
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
            // Only a section written first: one under control flow may
            // share its body with others, or come after nothing.
            let first = i == 0 && matches!(child, Node::Element { .. });
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
            tag,
            cond,
            then,
            otherwise,
            span,
        } => Node::When {
            tag: *tag,
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
            tag,
            subject,
            some,
            none,
            span,
        } => Node::Match {
            tag: *tag,
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
    // `background-color="transparent"` on a section drops its card (§6.2), as
    // Signal's profile and conversation headers sit on the list's background
    // (UIKit's clear cell background). Only that literal: a coloured card or
    // a class's background would draw differently on iOS, which keeps the
    // system's card.
    let mut cardless = false;
    for a in attrs.iter().filter(|a| a.name == "background-color") {
        match &a.value {
            Expr::Str(v, _) if v == "transparent" => cardless = true,
            _ => {
                return err(
                    "lower-grouped-list",
                    "a section's `background-color` is the literal `\"transparent\"`, which drops its card; the system draws the card otherwise",
                    a.span,
                )
            }
        }
    }
    if cardless {
        if let Some(c) = attrs.iter().find(|a| a.name == "class") {
            return err(
                "lower-grouped-list",
                "a section without its card takes no `class`: a class's background would show through",
                c.span,
            );
        }
    }
    sheet.extend(attrs.iter().filter(|a| a.name != "background-color").cloned());
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
    let mut group = vec![s("background-color", if cardless { "transparent" } else { CELL }, span)];
    if style == "grouped" && !cardless {
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
        .map(|r| over(r, &mut |node| Ok(row(node, !cardless))))
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
                "info" => "info.circle",
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
fn row(node: &Node, separated: bool) -> Node {
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
    // A native button row (LLP 1069.011) is the platform's control: only
    // the rows a native button takes, and its face left alone.
    let native = attrs
        .iter()
        .rev()
        .find(|a| a.name == "appearance")
        .is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "auto"));
    if native {
        return with(
            node,
            vec![n("margin-left", 16.0, span), n("min-height", 52.0, span)],
        );
    }
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
                Box::new(inset(then.iter().find(|c| !hidden(c)), span)),
                Box::new(inset(otherwise.iter().find(|c| !hidden(c)), span)),
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
        attr(
            "margin-left",
            inset(children.iter().find(|c| !hidden(c)), span),
            span,
        ),
        n("padding-right", 16.0, span),
        // A card-less section's rows draw no separator (§6.2).
        n("border-bottom-width", if separated { 1.0 } else { 0.0 }, span),
        s("border-bottom-style", "solid", span),
        s("border-bottom-color", SEPARATOR, span),
        n("margin-bottom", if separated { -1.0 } else { 0.0 }, span),
        n("font-size", 17.0, span),
        attr("color", tint(LABEL), span),
        s("text-align", "left", span),
    ];
    let mut texts = Count::Known(0);
    let stack = text_stack(children);
    // Positions among the shown parts: a part written `display="none"` is
    // not the leading or the trailing one (D4).
    let shown: Vec<&Node> = children.iter().filter(|c| !hidden(c)).collect();
    let last = shown.len().saturating_sub(1);
    let mut at_shown = 0;
    let parts = children
        .iter()
        .map(|child| {
            let i = if hidden(child) {
                usize::MAX
            } else {
                at_shown += 1;
                at_shown - 1
            };
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

/// How many texts come before a part: known, or chosen by one `when`'s
/// condition (then, otherwise). Deeper choices count the fewer.
#[derive(Clone)]
enum Count {
    Known(usize),
    Cond(Expr, usize, usize),
}

impl Count {
    fn low(&self) -> usize {
        match self {
            Count::Known(n) => *n,
            Count::Cond(_, a, b) => *a.min(b),
        }
    }
}

fn part(child: &Node, at: Place<'_>, texts: &mut Count) -> Node {
    let Place {
        i,
        last,
        stack,
        tint,
    } = at;
    if let Node::When {
        tag,
        cond,
        then,
        otherwise,
        span,
    } = child
    {
        let start = Count::Known(texts.low());
        *texts = start.clone();
        let then: Vec<Node> = then.iter().map(|c| part(c, at, texts)).collect();
        let after_then = std::mem::replace(texts, start);
        let otherwise: Vec<Node> = otherwise.iter().map(|c| part(c, at, texts)).collect();
        *texts = match (after_then, texts.clone()) {
            (Count::Known(a), Count::Known(b)) if a == b => Count::Known(a),
            (Count::Known(a), Count::Known(b)) => Count::Cond(cond.clone(), a, b),
            (a, b) => Count::Known(a.low().min(b.low())),
        };
        return Node::When {
            tag: *tag,
            cond: cond.clone(),
            then,
            otherwise,
            span: *span,
        };
    }
    if let Node::Match {
        tag,
        subject,
        some,
        none,
        span,
    } = child
    {
        // A `match` has no condition to choose by: its arms count the
        // fewer, and a symbol in first place there is not a leading one.
        let at = Place {
            i: if i == 0 { usize::MAX } else { i },
            ..at
        };
        let start = Count::Known(texts.low());
        *texts = start.clone();
        let arm: Vec<Node> = some.1.iter().map(|c| part(c, at, texts)).collect();
        let after_some = std::mem::replace(texts, start);
        let none: Vec<Node> = none.iter().map(|c| part(c, at, texts)).collect();
        *texts = Count::Known(after_some.low().min(texts.low()));
        return Node::Match {
            tag: *tag,
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
        // Not shown, not counted, as the kernel reads it (D4).
        ("text", _) if hidden(child) => child.clone(),
        ("text", _) => {
            // The first text is the title: it grows, in the row's colour;
            // a later one is the value, in the secondary colour.
            let grow = |title: bool| Expr::Number(if title { 1.0 } else { 0.0 }, at);
            let colour = |title: bool| {
                if title {
                    tint(LABEL)
                } else {
                    Expr::Str(SECONDARY.into(), at)
                }
            };
            let choose = |c: &Expr, a: Expr, b: Expr| {
                Expr::Ternary(Box::new(c.clone()), Box::new(a), Box::new(b), at)
            };
            let (sheet, next) = match texts.clone() {
                Count::Known(count) => (
                    if count == 0 {
                        vec![attr("flex-grow", grow(true), at), n("min-width", 0.0, at)]
                    } else {
                        vec![attr("color", colour(false), at)]
                    },
                    Count::Known(count + 1),
                ),
                Count::Cond(c, a, b) => (
                    vec![
                        attr("flex-grow", choose(&c, grow(a == 0), grow(b == 0)), at),
                        n("min-width", 0.0, at),
                        attr("color", choose(&c, colour(a == 0), colour(b == 0)), at),
                    ],
                    Count::Cond(c, a + 1, b + 1),
                ),
            };
            *texts = next;
            with(child, sheet)
        }
        ("column", _) if stack => subtitle(child),
        _ => child.clone(),
    }
}

/// Whether an element is written `display="none"`.
fn hidden(node: &Node) -> bool {
    matches!(node, Node::Element { attrs, .. }
        if attrs.iter().rev().find(|a| a.name == "display").is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "none")))
}

/// Whether a row's parts are a leading symbol, one `column` of one or two
/// texts and a trailing accessory at most: the subtitle cell the kernel
/// reads. A `column` in any other row is the author's.
fn text_stack(children: &[Node]) -> bool {
    // What the kernel reads at a row's ends, or a condition between it and
    // nothing (a checkmark shown by a `when`).
    fn either(node: &Node, part: &dyn Fn(&Node) -> bool) -> bool {
        match node {
            Node::When {
                then, otherwise, ..
            } => [then, otherwise]
                .iter()
                .all(|arm| matches!(arm.as_slice(), [] | [_]) && arm.iter().all(part)),
            other => part(other),
        }
    }
    let leading = |n: &Node| symbol(n).is_some_and(|s| accessory(s).is_none());
    let trailing = |n: &Node| match n {
        Node::Element { tag, children, .. } => match tag.as_str() {
            "image" => symbol(n).is_some_and(|s| accessory(s).is_some()),
            // A checkbox or switch, as the kernel's toggle is.
            "input" => matches!(n, Node::Element { attrs, .. }
                if attrs.iter().any(|a| a.name == "type" && matches!(&a.value, Expr::Str(v, _) if v.eq_ignore_ascii_case("checkbox")))),
            "button" => {
                matches!(children.as_slice(), [only] if symbol(only).is_some_and(|s| s == "info.circle" || s == "info.circle.fill"))
            }
            _ => false,
        },
        _ => false,
    };
    let visible: Vec<Node> = children.iter().filter(|c| !hidden(c)).cloned().collect();
    let mut rest = &visible[..];
    if let Some((first, after)) = rest.split_first() {
        if either(first, &leading) {
            rest = after;
        }
    }
    if let Some((last, before)) = rest.split_last() {
        if either(last, &trailing) {
            rest = before;
        }
    }
    // One or two texts, a `when` showing one of them included.
    fn lines(nodes: &[Node]) -> Option<(usize, usize)> {
        nodes.iter().try_fold((0, 0), |(low, high), c| match c {
            Node::Element { tag, .. } if tag == "text" && hidden(c) => Some((low, high)),
            Node::Element { tag, .. } if tag == "text" => Some((low + 1, high + 1)),
            Node::When {
                then, otherwise, ..
            } => {
                let (a, b) = (lines(then)?, lines(otherwise)?);
                Some((low + a.0.min(b.0), high + a.1.max(b.1)))
            }
            _ => None,
        })
    }
    matches!(rest, [Node::Element { tag, children, .. }]
        if tag == "column" && lines(children).is_some_and(|(low, high)| low >= 1 && high <= 2))
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
    // The second line, whether written or shown by a condition: a text
    // after the first shown one, chosen by the condition where the arms
    // differ (as `part` counts a row's texts).
    fn line(c: &Node, texts: &mut Count) -> Node {
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
                let then = then.iter().map(|n| line(n, texts)).collect();
                let after = std::mem::replace(texts, start);
                let otherwise = otherwise.iter().map(|n| line(n, texts)).collect();
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
                let size = |title: bool| Expr::Number(if title { 17.0 } else { 15.0 }, at);
                let colour =
                    |title: bool| Expr::Str(if title { LABEL } else { SECONDARY }.into(), at);
                let choose = |c: &Expr, a: Expr, b: Expr| {
                    Expr::Ternary(Box::new(c.clone()), Box::new(a), Box::new(b), at)
                };
                let (sheet, next) = match texts.clone() {
                    Count::Known(0) => (Vec::new(), Count::Known(1)),
                    Count::Known(count) => (
                        vec![n("font-size", 15.0, at), s("color", SECONDARY, at)],
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
                with(c, sheet)
            }
            other => other.clone(),
        }
    }
    let mut texts = Count::Known(0);
    let parts = children.iter().map(|c| line(c, &mut texts)).collect();
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
