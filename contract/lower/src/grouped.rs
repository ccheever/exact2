//! A grouped list (LLP 1084): `list appearance="auto"`. Its children are
//! `section`s; a section's leading `header` and trailing `footer` are its
//! texts and everything between them its rows. Contract checks that shape
//! and writes the list's look as a user-agent sheet: rows prepended to the
//! author's, so a class or an attribute of the author's replaces any of
//! them. The web, macOS and Linux draw that sheet; iOS draws the platform's
//! own list over the same nodes (`GroupedListIOS.swift`).
//!
//! The sheet holds no number of a platform's look (LLP 1115.002 D6, D8.4):
//! each is a metric of the kernel's schema (`metrics`, a column a
//! platform), named here and read by the kernel as its value on the
//! platform the plan runs on. So one plan is UIKit's measured list where a
//! host paints it as iOS has it (52-point rows in 26-point cards) and
//! SwiftUI's grouped `Form` on the Mac (36-point rows in 10-point cards, at
//! most 704 points wide). What stays written here is the sheet's structure:
//! which box is a flex row, which part grows, which row is separated.
//!
//! A row spans its card and is `border-box`, the platform's insets its
//! padding (LLP 1084 D7), so a row as wide as its card stays inside it. The
//! separator under a row is no border but the platform's decoration, which
//! the sheet asks for (`groupedRowSeparator`) and each host draws
//! (`Kernel::grouped_separator`, the web's page sheet).

pub(crate) mod actions;
mod cell;
mod inputs;
mod lines;
mod messages;
pub(crate) use messages::folds;
mod native;
mod title;
mod value;

use cell::{lines as cell_lines, standard, text_lines, text_stack};
use inputs::{column_row, field_beside_label, field_kind, hidden, switch, text_field, Field};
use native::native_row;
pub(crate) use native::{margins_said, native_rows};

use crate::{err, swipe_rows, tags, LowerError};
use contract_syntax::{Attr, Expr, Node, Span, TemplatePart};
use exact_kernel::StyleId;

/// `listStyle`'s names (D2), UIKit's three list appearances.
pub(crate) const STYLES: &[&str] = &["inset-grouped", "grouped", "plain"];

/// The colours the sheet writes that are one role on every platform (LLP
/// 1095 D2, LLP 1115 §3, LLP 1115.000 D3.1): its labels. The ones a
/// platform chooses (the ground, a card's fill, the separator, a header's
/// ink, a destructive row's, a symbol's and a checkmark's tint, which in
/// UIKit's list is `AccentColor`: the nearest `accent-color` or else the
/// platform's tint) are metrics. No row of the sheet is written (LLP
/// 1115.000 D2), so a host that draws the platform's own list reads each as
/// unsaid (`kernel/src/grouped.rs`): the values have no second copy.
const LABEL: &str = "-exact-label";
const SECONDARY: &str = "-exact-secondary-label";
const TERTIARY: &str = "-exact-tertiary-label";

/// The sizes the sheet writes are text styles (LLP 1115 D3, LLP 1115.000
/// D3.3), so its text follows the root font size as a hand-built list's
/// follows the platform's body size: a row and a header at the body style
/// (17 on iOS at the default size, 13 on the Mac, 16 on the web), a second
/// line at the subheadline (15, 11, 14). A footer's is the platform's choice
/// of style, a metric.
const BODY: &str = "-exact-body";
const SUBHEADLINE: &str = "-exact-subheadline";

use crate::metrics::grouped as metric;
use actions::{action_cell, disclosure};
use lines::{stack_lines, stack_of_lines, subtitle};
use title::{title_button, title_face};

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
                    "a `list`'s `appearance` is a literal: `\"auto\"` makes it a grouped list. To switch, write `if` with two lists",
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
pub(crate) const MARK: &str = "ua:";
const BUTTON_CONTEXT: &str = "ua:button-context";
const SEPARATED: &str = "ua:grouped-row-separator";

/// Whether the sheet asks for the platform's separator under this row: the
/// lowering sets `groupedRowSeparator`, which no author can write.
pub(crate) fn row_separator(attrs: &[Attr]) -> bool {
    attrs.iter().any(|a| a.name == SEPARATED)
}

/// Rows and their detail accessories belong to the grouped list's press
/// face, unless the author explicitly requests a native button (LLP 1104 D2).
pub(crate) fn button_context(attrs: &[Attr]) -> Option<&str> {
    attrs.iter().find_map(|a| match (&*a.name, &a.value) {
        (BUTTON_CONTEXT, Expr::Str(context, _)) => Some(context.as_str()),
        _ => None,
    })
}

fn attr(name: &str, value: Expr, span: Span) -> Attr {
    Attr {
        name: format!("{MARK}{name}"),
        value,
        span,
        written: crate::IMPLIED,
    }
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
        .filter(|a| ![BUTTON_CONTEXT, SEPARATED, actions::ACTION_MARK].contains(&a.name.as_str()))
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
/// Row `name` at the metric `metric`.
fn m(name: &str, metric_name: &str, span: Span) -> Attr {
    s(name, &metric(metric_name), span)
}
/// The metric `metric_name` as a value.
fn mv(metric_name: &str, span: Span) -> Expr {
    Expr::Str(metric(metric_name), span)
}

/// The list's own sheet, before its author's rows; `listStyle` is always
/// written, so a host finds a grouped list by its prop.
pub(crate) fn list_rows(style: &'static str, span: Span) -> Vec<Attr> {
    let ground = if style == "plain" {
        "ground-plain"
    } else {
        "ground"
    };
    split(&[
        m("background-color", ground, span),
        s("listStyle", style, span),
    ])
    .0
}

/// Finds the rows a node's `class` gives it (`Lowerer::class_rows`).
pub(crate) type ClassRows<'a> =
    &'a dyn Fn(&[Attr]) -> Result<Option<(String, Vec<Attr>)>, LowerError>;

/// The list's children with the sheet written into them: each `section`'s
/// header, footer and rows, its rows wrapped in one `column` (the rounded
/// group the web draws; UIKit's section). `class` finds the rows a section's
/// `class` gives it.
pub(crate) fn sections(
    style: &'static str,
    children: &[Node],
    class: ClassRows<'_>,
) -> Result<Vec<Node>, LowerError> {
    children
        .iter()
        .enumerate()
        .map(|(i, child)| {
            // Only a section written first: one under control flow may
            // share its body with others, or come after nothing.
            let first = i == 0 && matches!(child, Node::Element { .. });
            over(child, &mut |node| section(style, node, first, class))
        })
        .collect()
}

/// `node` with `f` applied to each element it is or holds through `if`,
/// `for` and `match`.
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

/// Whether a section's `display` only shows or hides it: `"block"` or
/// `"none"`, written or chosen between, or unset (the side of a class choice
/// that sets none).
fn shown_or_hidden(value: &Expr) -> bool {
    match value {
        Expr::Str(v, _) => v == "block" || v == "none",
        Expr::None(_) => true,
        Expr::Ternary(_, yes, no, _) => shown_or_hidden(yes) && shown_or_hidden(no),
        _ => false,
    }
}

/// A `header` or `footer` where a row would be: in the rows, or below control
/// flow (`if`, `for`, `match`, whichever it is).
struct Stray {
    tag: String,
    span: Span,
    flow: Option<&'static str>,
}

/// Whether `node` is or holds a `header` or `footer` among the rows.
fn stray_label(node: &Node) -> Option<Stray> {
    let flow = |flow, found: Option<Stray>| {
        found.map(|s| Stray {
            flow: s.flow.or(Some(flow)),
            ..s
        })
    };
    match node {
        Node::Element { tag, span, .. } if tag == "header" || tag == "footer" => Some(Stray {
            tag: tag.clone(),
            span: *span,
            flow: None,
        }),
        Node::Element { .. } | Node::Use { .. } | Node::Children { .. } => None,
        Node::When {
            then, otherwise, ..
        } => flow("if", then.iter().chain(otherwise).find_map(stray_label)),
        Node::Each { body, .. } => flow("for", body.iter().find_map(stray_label)),
        Node::Match { some, none, .. } => {
            flow("match", some.1.iter().chain(none).find_map(stray_label))
        }
    }
}

fn section(
    style: &'static str,
    node: &Node,
    first: bool,
    class: ClassRows<'_>,
) -> Result<Node, LowerError> {
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
    // LLP 1116.017 D3, before the label's check: copied only when one folds.
    let fold = |c: &Node| {
        (is(c, "header") || is(c, "footer"))
            .then(|| messages::fold_label(c))
            .flatten()
    };
    let folded: Option<Vec<Node>> = children.iter().any(|c| fold(c).is_some()).then(|| {
        children
            .iter()
            .map(|c| fold(c).unwrap_or_else(|| c.clone()))
            .collect()
    });
    let children = folded.as_ref().unwrap_or(children);
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
            messages::stray_message(&stray),
            stray.span,
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
                let message = messages::label(tag, children);
                return err("lower-grouped-list", message, *span);
            }
        }
    }
    // The space around a section is the platform's: UIKit opens an inset or
    // grouped list whose first section has no header with a deeper gap and
    // leaves none beside a header or a footer, whose own padding is the
    // space; the Mac's form spaces a header and a footer by the section. A
    // plain list's sections meet.
    let plain = style == "plain";
    let mut sheet = if plain {
        vec![n("margin-top", 0.0, span), n("margin-bottom", 0.0, span)]
    } else {
        let top = match (head == 1, first) {
            (true, true) => "section-top-header-first",
            (true, false) => "section-top-header",
            (false, true) => "section-top-first",
            (false, false) => "section-top",
        };
        let bottom = if tail == 1 {
            "section-bottom-footer"
        } else {
            "section-bottom"
        };
        vec![
            m("margin-top", top, span),
            m("margin-bottom", bottom, span),
            // Where a platform's form has a widest (the Mac's, centred),
            // the section is that wide at most; no row elsewhere.
            m("width", "section-width", span),
            m("max-width", "section-max-width", span),
            m("margin-left", "section-margin-inline", span),
            m("margin-right", "section-margin-inline", span),
        ]
    };
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
    // The platform lays a section out: UIKit's list draws its rows one
    // under another across the card, whatever the kernel would make of a
    // section laid out otherwise. What it draws of the section's own layout
    // is the space above and below it and whether it shows; its text styles
    // reach the rows as they do in any box.
    let classed = class(attrs)?.map(|(_, rows)| rows).unwrap_or_default();
    let drawn = |a: &Attr, row: &StyleId| match row {
        StyleId::MarginTop | StyleId::MarginBottom => true,
        StyleId::Display => shown_or_hidden(&a.value),
        _ => !row.affects_layout() || row.affects_text() || row.inherited(),
    };
    if let Some(a) = attrs
        .iter()
        .chain(&classed)
        .find(|a| !tags::rows(a).iter().all(|row| drawn(a, row)))
    {
        return err(
            "lower-grouped-list",
            format!(
                "a grouped list's section is laid out by the platform, its rows one under another across the card, so its `{}` would place the rows where the native list cannot draw them; a section takes only `margin-top`, `margin-bottom` and `display=\"none\"` of its own layout: leave the rest to the list, or lay the rows out in a `column` of your own",
                a.name
            ),
            a.span,
        );
    }
    sheet.extend(
        attrs
            .iter()
            .filter(|a| a.name != "background-color")
            .cloned(),
    );
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
        let edge = match style {
            "inset-grouped" => "label-inset",
            "grouped" => "label-inset-edge",
            _ => "label-inset-plain",
        };
        let mut rows = vec![
            s("box-sizing", "border-box", span),
            m("padding-left", edge, span),
            m("padding-right", edge, span),
        ];
        rows.extend(if footer {
            [
                m("padding-top", "footer-padding-top", span),
                m("padding-bottom", "footer-padding-bottom", span),
                m("font-size", "footer-font", span),
                s("color", SECONDARY, span),
            ]
        } else {
            [
                m("padding-top", "header-padding-top", span),
                m("padding-bottom", "header-padding-bottom", span),
                s("font-size", BODY, span),
                m("color", "header-color", span),
            ]
        });
        if !footer {
            rows.push(m("font-weight", "header-weight", span));
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
    // A plain list has no card: its rows are the list's own ground, as
    // UIKit's plain cells and an AppKit table's rows are, not a lighter fill.
    let mut group = vec![if cardless {
        s("background-color", "transparent", span)
    } else if style == "plain" {
        m("background-color", "ground-plain", span)
    } else {
        m("background-color", "card-fill", span)
    }];
    if style == "grouped" && !cardless {
        group.extend([
            m("border-top-width", "edge-width", span),
            m("border-bottom-width", "edge-width", span),
            s("border-top-style", "solid", span),
            s("border-bottom-style", "solid", span),
            m("border-color", "separator", span),
        ]);
    }
    // An inset list's card stands in from the list's edges. A grouped
    // list's runs edge to edge where the platform has that shape (UIKit's);
    // where it has one shape for both (the Mac's form), its inset and its
    // corners are the inset list's, and are no row elsewhere.
    let (edge, corner) = match style {
        "inset-grouped" => (Some("card-inset"), Some("card-radius")),
        "grouped" => (Some("edge-inset"), Some("edge-radius")),
        _ => (None, None),
    };
    if let Some(edge) = edge {
        group.extend([m("margin-left", edge, span), m("margin-right", edge, span)]);
    }
    // No card, no card corners to clip a header image to; their curve is the platform's (the Mac's continuous).
    if let Some(corner) = corner.filter(|_| !cardless) {
        let shape = m("corner-shape", "card-corner-shape", span);
        group.extend([m("border-radius", corner, span), shape]);
    }
    // Where a platform's separator stands between two rows and takes no
    // height from either (the Mac's), the rows are that far apart and the
    // separator is drawn in the gap; no row elsewhere, where it is drawn at
    // the foot of the row above it.
    if !cardless {
        group.push(m("row-gap", "separator-gap", span));
    }
    group.push(s("overflow", "hidden", span));
    let mut styled = |n: &Node| {
        Ok(swipe_rows::grouped(n, &|n, swiped| {
            row(n, !cardless, swiped)
        }))
    };
    let body = rows
        .iter()
        .map(|r| over(r, &mut styled))
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

/// A symbol a prop or an expression names (`symbol:${name}`, a library
/// row's): a symbol, which one known only when it runs, so no accessory (a
/// row writes its chevron or checkmark literally). Its Apple name stands
/// for any.
const NAMED_SYMBOL: &str = "symbol";

/// A `symbol:` image source's Apple name, as the kernel names it; a source
/// a template writes (`symbol:${name}`) is [`NAMED_SYMBOL`].
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
            // @ref LLP 1116.007 D3.1, LLP 1121 D10 — whichever way the role is spelled, or an icon.
            let role = crate::icons::role_or_accessory(role).unwrap_or(role);
            Some(match role {
                "forward-chevron" => "chevron.forward",
                "checkmark" => "checkmark",
                "info" => "info.circle",
                other => other.strip_prefix("sf/").unwrap_or(other),
            })
        }
        ("image", Some(Expr::Template(parts, _))) => match parts.first() {
            Some(TemplatePart::Text(text)) if text.starts_with("symbol:") => Some(NAMED_SYMBOL),
            _ => None,
        },
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

/// A field whose authored `flex` is a number alone (`flex=1`, CSS's `1 1
/// 0%`) with no `flex-basis` after it, written as its grow and shrink: the
/// row's sheet keeps the basis. `None` for any other field.
fn grown_field(node: &Node) -> Option<Node> {
    let Node::Element { attrs, .. } = node else {
        return None;
    };
    let flex = attrs.iter().rposition(|a| a.name == "flex")?;
    let grow = match &attrs[flex].value {
        Expr::Str(v, _) => v.trim().parse::<f64>().ok()?,
        v => crate::values::numeric_literal(v)?,
    };
    if attrs[flex..].iter().any(|a| a.name == "flex-basis") {
        return None;
    }
    let mut node = node.clone();
    let Node::Element { attrs, .. } = &mut node else {
        return None;
    };
    let a = attrs.remove(flex);
    let longhand = |name: &str, value: f64| Attr {
        name: name.into(),
        value: Expr::Number(value, a.span),
        ..a.clone()
    };
    attrs.splice(
        flex..flex,
        [longhand("flex-grow", grow), longhand("flex-shrink", 1.0)],
    );
    Some(node)
}

/// `yes` where the boolean attribute `name` is true, else `otherwise`; a
/// bound one chooses as it changes.
fn by_flag(attrs: &[Attr], name: &str, yes: &str, otherwise: Expr, span: Span) -> Expr {
    flagged(attrs, name, Expr::Str(yes.into(), span), otherwise, span)
}

/// [`by_flag`] between two values.
fn flagged(attrs: &[Attr], name: &str, yes: Expr, otherwise: Expr, span: Span) -> Expr {
    match attrs.iter().rev().find(|a| a.name == name) {
        Some(Attr {
            value: Expr::Bool(true, _),
            ..
        }) => yes,
        Some(Attr {
            value: Expr::Bool(false, _),
            ..
        })
        | None => otherwise,
        Some(a) => Expr::Ternary(
            Box::new(a.value.clone()),
            Box::new(yes),
            Box::new(otherwise),
            span,
        ),
    }
}

/// The sheet's request for the platform's separator under a row.
fn separator(separated: bool, span: Span) -> Option<Attr> {
    separated.then(|| Attr {
        name: SEPARATED.into(),
        value: Expr::Bool(true, span),
        span,
        written: crate::IMPLIED,
    })
}

fn s_val(value: &str, span: Span) -> Expr {
    Expr::Str(value.into(), span)
}

/// A row and its direct parts: a cell's metrics (on iOS 52 high, 16 in, the
/// text at 56 after an icon; on the Mac 36 high with 10 around its content),
/// its separator, the icon, a value or subtitle in the secondary colour, an
/// accessory's size and colour, and what a `destructive` row is: red text
/// where the platform's list shows that, the platform's push button where
/// its form shows one (the Mac's, LLP 1115.002 D6).
/// `swiped`: the row is a swipe's content, which the platform's list
/// carries as a custom row.
/// A grouped list's row: the sheet's rows, without the look of an element
/// that yields it to the row (`row_look`).
fn row(node: &Node, separated: bool, swiped: bool) -> Node {
    match crate::row_look::yielded(node) {
        Some(own) => styled(&own, separated, swiped),
        None => styled(node, separated, swiped),
    }
}

fn styled(node: &Node, separated: bool, swiped: bool) -> Node {
    let Node::Element {
        tag,
        attrs,
        children,
        span,
        ..
    } = node
    else {
        return node.clone();
    };
    let span = *span;
    let context = if tag == "button" {
        vec![s("button-context", "grouped-list row", span)]
    } else {
        Vec::new()
    };
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
            [context, native_row(span, margins_said(attrs))].concat(),
        );
    }
    // A destructive row's ink is the platform's (red in UIKit's list, the
    // label's in the Mac's form, whose button says it); a disabled row is the
    // tertiary label's grey, its symbols too, as UIKit draws one
    // (`GroupedListIOS.swift`).
    let tint = |plain: Expr| -> Expr {
        let ink = flagged(
            attrs,
            "destructive",
            mv("destructive-color", span),
            plain,
            span,
        );
        by_flag(attrs, "disabled", TERTIARY, ink, span)
    };
    let label = || Expr::Str(LABEL.into(), span);
    // @ref LLP 1116.019 D6 — a `button` row that is an action cell (a title,
    // a symbol before it at most, nothing at its trailing edge) is in the
    // platform's action ink: the tint in UIKit's list, as SwiftUI's
    // `List { Button }` and a hand-built action cell draw it. The kernel
    // reads the same shape (`GroupedRow::action`). A row that says whether
    // it is selected is a choice, whose mark comes and goes: never one.
    let choice = attrs.iter().any(|a| a.name == "aria-selected");
    let acting = tag == "button" && !choice && action_cell(children);
    let ink = if acting {
        mv("action-color", span)
    } else {
        label()
    };
    // A destructive `button` row's shape, where the platform's form shows
    // its push button: the button's height, its title at the button's
    // padding and no wider than its words, and `appearance`, which is how a
    // host is told the platform draws it. Elsewhere each is the row's own.
    let destructive = tag == "button" && attrs.iter().any(|a| a.name == "destructive");
    let button = |row: &str, pushed: &str, plain: Expr| -> Attr {
        attr(
            row,
            flagged(attrs, "destructive", mv(pushed, span), plain, span),
            span,
        )
    };
    let unset = || Expr::Str("unset".into(), span);
    // The row starts at its text: after a leading symbol, a symbol shown
    // by a condition included, as `part` styles it. `names` are the inset's
    // metrics without a symbol and with one.
    fn inset(first: Option<&Node>, names: [&str; 2], span: Span) -> Expr {
        match first {
            Some(Node::When {
                cond,
                then,
                otherwise,
                ..
            }) => Expr::Ternary(
                Box::new(cond.clone()),
                Box::new(inset(then.iter().find(|c| !hidden(c)), names, span)),
                Box::new(inset(otherwise.iter().find(|c| !hidden(c)), names, span)),
                span,
            ),
            other => {
                let icon = other
                    .and_then(symbol)
                    .is_some_and(|name| accessory(name).is_none());
                mv(names[usize::from(icon)], span)
            }
        }
    }
    let first = children.iter().find(|c| !hidden(c));
    let start = inset(first, ["row-inset", "row-inset-symbol"], span);
    // A `column` row keeps its direction (LLP 1116 D2): its children stack
    // as written, from the text's margin, centred in the cell's height, and
    // a subtitle cell's space above and below (on iOS 15, 52 high for one
    // line). One or two texts are the subtitle cell's lines, which the kernel
    // reads as one; anything else is the author's own layout.
    if column_row(tag, attrs) {
        let stack = cell_lines(children);
        let mut rows = context;
        rows.extend([
            s("display", "flex", span),
            s("flex-direction", "column", span),
            s("justify-content", "center", span),
            m("gap", if stack { "stack-gap" } else { "row-gap" }, span),
            s("box-sizing", "border-box", span),
            m("min-height", "stack-min-height", span),
            m("padding-left", "row-inset", span),
            m("padding-right", "row-padding-end", span),
            m("padding-top", "stack-padding-block", span),
            m("padding-bottom", "stack-padding-block", span),
            s("font-size", BODY, span),
            attr("color", tint(label()), span),
            s("text-align", "start", span),
        ]);
        rows.extend(separator(separated, span));
        rows.extend(attrs.iter().cloned());
        let Node::Element {
            positional,
            instance,
            ..
        } = node
        else {
            unreachable!()
        };
        return Node::Element {
            tag: tag.clone(),
            positional: positional.clone(),
            attrs: rows,
            children: if stack {
                stack_lines(children, attrs)
            } else {
                children.clone()
            },
            span,
            instance: *instance,
        };
    }
    let stack = text_stack(children);
    // A stack of lines in the row (an author's `column` that is not the
    // subtitle cell) keeps the cell's margins, as a hand-built cell pins its
    // content to its content view's: the subtitle cell's space above and
    // below (15 on iOS), as a `column` row has it. On the stack, not the row,
    // so a taller part beside it (an avatar with its own margins) sets the
    // height as before; a row whose author wrote its vertical padding has
    // said the space. Without them its first line touched the card and its
    // last the separator. Where the row itself is padded (the Mac's), the
    // stack's share is nothing.
    let lines = !stack
        && !attrs.iter().any(|a| {
            matches!(
                a.name.as_str(),
                "padding" | "padding-top" | "padding-bottom"
            )
        });
    let row_height = mv("row-min-height", span);
    // A label beside a field: the row wraps, so where both do not fit (the
    // accessibility text sizes) the field goes under the label, as in a
    // SwiftUI `Form`, rather than squeezing the label to nothing.
    let field = field_beside_label(children);
    let mut sheet = vec![
        s("display", "flex", span),
        s("flex-direction", "row", span),
        s("align-items", "center", span),
        m("gap", "row-gap", span),
        if destructive {
            button("min-height", "button-min-height", row_height)
        } else {
            attr("min-height", row_height, span)
        },
        m("padding-top", "row-padding-block", span),
        m("padding-bottom", "row-padding-block", span),
        s("font-size", BODY, span),
        attr("color", tint(ink), span),
        s("text-align", "start", span),
        // A word longer than its line breaks rather than running over the
        // value beside it, as a UILabel's does.
        s("overflow-wrap", "break-word", span),
    ];
    // The standard cell's labels hyphenate (UIListContentConfiguration's
    // text has a hyphenationFactor of 1, iOS 27: "Alice Ander-son"). An
    // author's own content is a custom cell's, whose UILabel's factor is 0,
    // as SwiftUI's `Text` is: CSS's `manual`.
    if standard(children) {
        sheet.push(s("hyphens", "auto", span));
    }
    if field.is_some() {
        sheet.push(s("flex-wrap", "wrap", span));
    }
    // Its box: the card's width and its insets inside it. A destructive
    // row the platform's form shows as its push button (the Mac's) is that
    // button's box instead, at the row's inset and the button's padding.
    if destructive {
        sheet.extend([
            button("box-sizing", "button-box-sizing", s_val("border-box", span)),
            button("margin-left", "button-margin-inline", unset()),
            button("margin-right", "button-margin-inline", unset()),
            attr(
                "padding-left",
                flagged(
                    attrs,
                    "destructive",
                    inset(
                        first,
                        ["button-padding-start", "button-padding-start-symbol"],
                        span,
                    ),
                    start,
                    span,
                ),
                span,
            ),
            button(
                "padding-right",
                "button-padding-end",
                mv("row-padding-end", span),
            ),
            button(
                "appearance",
                "button-appearance",
                Expr::Str("none".into(), span),
            ),
        ]);
    } else {
        sheet.extend([
            s("box-sizing", "border-box", span),
            attr("padding-left", start, span),
            m("padding-right", "row-padding-end", span),
        ]);
    }
    // A card-less section's rows draw no separator (§6.2).
    sheet.extend(separator(separated, span));
    sheet.extend(actions::mark(tag == "button", acting, span));
    // The title grows to push a value to the far edge; a push button's is
    // as wide as its words.
    let title_grow = || {
        let grows = Expr::Number(1.0, span);
        if destructive {
            flagged(
                attrs,
                "destructive",
                mv("button-title-grow", span),
                grows,
                span,
            )
        } else {
            grows
        }
    };
    let mut texts = Count::Known(0);
    // Positions among the shown parts: a part written `display="none"` is
    // not the leading or the trailing one (D4).
    let shown: Vec<&Node> = children.iter().filter(|c| !hidden(c)).collect();
    let last = shown.len().saturating_sub(1);
    let mut at_shown = 0;
    let titled = title_button(children);
    let mut parts = Vec::with_capacity(children.len());
    let mut counts = Vec::with_capacity(children.len());
    for (k, child) in children.iter().enumerate() {
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
            lines,
            field,
            titled: titled == Some(k),
            tint: &tint,
            grow: &title_grow,
        };
        let before = texts.clone();
        parts.push(part(child, at, &mut texts));
        counts.push((before, texts.clone()));
    }
    let parts = value::boxed(children, parts, &counts, swiped, lines, title_grow(), span);
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
    let mut rows = context;
    rows.extend(sheet);
    rows.extend(attrs.iter().cloned());
    Node::Element {
        tag: tag.clone(),
        positional: positional.clone(),
        attrs: rows,
        children: parts.iter().map(disclosure).collect(),
        span: *span,
        instance: *instance,
    }
}

/// A row's part at `i` of `last + 1`: a leading icon, a trailing
/// accessory, the title and its value, a subtitle stack. An `if` is read
/// through, so a checkmark shown by a condition is styled as one.
#[derive(Clone, Copy)]
struct Place<'a> {
    i: usize,
    last: usize,
    /// Whether a `column` here is the row's text stack (`text_stack`).
    stack: bool,
    /// Whether a stack of lines here takes the cell's margins (`stacks`).
    lines: bool,
    /// The field beside the row's label, if it is such a row.
    field: Option<Field>,
    /// Whether this part is the button that holds the row's title (D4).
    titled: bool,
    /// The row's ink over a colour: its destructive or disabled one.
    tint: &'a dyn Fn(Expr) -> Expr,
    /// The title's `flex-grow`.
    grow: &'a dyn Fn() -> Expr,
}

/// How many texts come before a part: known, or chosen by one `if`'s
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
        lines,
        field,
        titled,
        tint,
        grow: title_grow,
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
        // LLP 1116.021 D4: the button that holds the title is the title.
        _ if titled => {
            *texts = Count::Known(1);
            title_face(child)
        }
        // The leading symbol: in UIKit's cell it stands in the margin the
        // text is inset by, pulled back into it; in the Mac's form it is the
        // row's first part. Its tint is the platform's too.
        ("image", Some(name)) if i == 0 && accessory(name).is_none() => with(
            child,
            vec![
                m("width", "symbol-width", at),
                m("height", "symbol-height", at),
                s("object-fit", "contain", at),
                m("margin-left", "symbol-pull", at),
                m("margin-right", "symbol-gap", at),
                n("flex-shrink", 0.0, at),
                attr("-exact-tint-color", tint(mv("symbol-tint", at)), at),
            ],
        ),
        ("image", Some(name)) if i == last && accessory(name).is_some() => {
            let check = accessory(name) == Some("checkmark");
            let (width, height) = if check {
                ("checkmark-width", "checkmark-height")
            } else {
                ("chevron-width", "chevron-height")
            };
            with(
                child,
                vec![
                    m("width", width, at),
                    m("height", height, at),
                    s("object-fit", "contain", at),
                    m("font-weight", "accessory-weight", at),
                    n("flex-shrink", 0.0, at),
                    // A checkmark's ink is the platform's: the tint in
                    // UIKit's cell, the label's in the Mac's form. A
                    // symbol is drawn in its `color` (LLP 1081 stage 2),
                    // which a custom row's chevron or checkmark shows.
                    if check {
                        m("color", "checkmark-tint", at)
                    } else {
                        s("color", TERTIARY, at)
                    },
                ],
            )
        }
        // Not shown, not counted, as the kernel reads it (D4).
        ("text", _) if hidden(child) => child.clone(),
        ("text", _) => {
            // The first text is the title: it grows, in the row's colour;
            // a later one is the value, in the secondary colour.
            let grow = |title: bool| {
                if title {
                    title_grow()
                } else {
                    Expr::Number(0.0, at)
                }
            };
            let colour = |title: bool| {
                if title {
                    tint(Expr::Str(LABEL.into(), at))
                } else {
                    Expr::Str(SECONDARY.into(), at)
                }
            };
            let choose = |c: &Expr, a: Expr, b: Expr| {
                Expr::Ternary(Box::new(c.clone()), Box::new(a), Box::new(b), at)
            };
            // Beside a field the title keeps its words' width, and the row
            // wraps instead (`field_beside_label`).
            let squeeze = || field.is_none().then(|| n("min-width", 0.0, at));
            let (sheet, next) = match texts.clone() {
                Count::Known(count) => (
                    if count == 0 {
                        [Some(attr("flex-grow", grow(true), at)), squeeze()]
                            .into_iter()
                            .flatten()
                            .collect()
                    } else {
                        vec![attr("color", colour(false), at)]
                    },
                    Count::Known(count + 1),
                ),
                Count::Cond(c, a, b) => (
                    [
                        Some(attr(
                            "flex-grow",
                            choose(&c, grow(a == 0), grow(b == 0)),
                            at,
                        )),
                        squeeze(),
                        Some(attr(
                            "color",
                            choose(&c, colour(a == 0), colour(b == 0)),
                            at,
                        )),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                    Count::Cond(c, a + 1, b + 1),
                ),
            };
            *texts = next;
            with(child, sheet)
        }
        ("button", _)
            if i == last
                && matches!(child, Node::Element { children, .. }
            if matches!(children.as_slice(), [only] if symbol(only).is_some_and(|s| s == "info.circle" || s == "info.circle.fill"))) =>
        {
            with(
                child,
                vec![s("button-context", "grouped-list detail accessory", at)],
            )
        }
        // A control beside the title, where the platform's form draws it
        // its own way (the Mac's: the small switch, a field's text at the
        // far edge); no row elsewhere.
        ("input", _) if i == last && switch(child) => {
            with(child, vec![m("-exact-control-size", "switch-size", at)])
        }
        // A text field beside its label is 8rem at the far edge, so the
        // fields of a section line up and the label, the author's `flex=1`
        // one too, keeps the rest; 8rem and a short label fit on one line up
        // to the largest size before the accessibility ones, and past it the
        // row wraps the field under the label. A date, time or menu keeps its
        // own width. Under the label either is as narrow as the line. An
        // author's `flex=1` on the field (the number-field recipe writes it)
        // grows it from the same basis: CSS's `1 1 0%` let the label squeeze
        // it to "…" at the accessibility sizes instead of wrapping.
        ("input", _) if texts.low() >= 1 && text_field(child) => with(
            &grown_field(child).unwrap_or_else(|| child.clone()),
            vec![
                m("text-align", "field-align", at),
                n("flex-grow", 0.0, at),
                n("flex-shrink", 1.0, at),
                s("flex-basis", "8rem", at),
                n("min-width", 0.0, at),
            ],
        ),
        // An author's `flex=1` on a menu or date grows it from its own width,
        // as on a text field: from CSS's 0% basis the row never wrapped it
        // under its label, and at the accessibility sizes it took a sliver
        // its title broke into letters in (grocery-list's Aisle, round 24).
        ("input" | "select", _) if texts.low() >= 1 && field_kind(child) == Some(Field::Picker) => {
            with(
                &grown_field(child).unwrap_or_else(|| child.clone()),
                vec![n("min-width", 0.0, at)],
            )
        }
        ("column", _) if stack => subtitle(child),
        // A column around the title's one text is the title, as a stack of
        // lines is: a text after it is the value at the far edge, not a
        // second title sharing the row's room (grocery-list's quantity
        // mid-row, and over the name at the accessibility sizes, round 24).
        _ if !stack && matches!(texts, Count::Known(0)) && one_line(child) => {
            *texts = Count::Known(1);
            if sized(child) {
                child.clone()
            } else {
                with(
                    child,
                    vec![attr("flex-grow", title_grow(), at), n("min-width", 0.0, at)],
                )
            }
        }
        _ if !stack && stacks(child) => {
            let mut margins = if lines {
                vec![
                    m("margin-top", "lines-padding-block", at),
                    m("margin-bottom", "lines-padding-block", at),
                ]
            } else {
                Vec::new()
            };
            // LLP 1116.020 D5: a stack before any text is the title, and
            // grows as a title does, so a text after it is the value at the
            // far edge (`RowText` straight in a section drew it mid-row). A
            // stack its author sized is laid out as written: three `flex=1`
            // columns of figures keep CSS's `min-width: auto`, and a first
            // one given `min-width: 0` here was squeezed under its caption's
            // width at the largest text size (event-rsvp, round 21).
            if matches!(texts, Count::Known(0)) {
                if !sized(child) {
                    margins.extend([attr("flex-grow", title_grow(), at), n("min-width", 0.0, at)]);
                }
                *texts = Count::Known(1);
            }
            // A control's lines are its label, in its own colour.
            match tag.as_str() {
                "button" | "link" => with(child, margins),
                _ => stack_of_lines(child, margins),
            }
        }
        // The row's one part, a box the author laid its content out in (a
        // button's face in a `row`), fills the cell, as a hand-built cell
        // pins its stack to its content view's edges. As wide as its
        // content, a bar in it was a share of its label's width (polls,
        // round 29).
        ("row" | "column" | "view", _) if i == 0 && last == 0 && !sized(child) => with(
            child,
            vec![attr("flex-grow", title_grow(), at), n("min-width", 0.0, at)],
        ),
        _ => child.clone(),
    }
}

/// Whether the author wrote how wide `node` is in its row.
fn sized(node: &Node) -> bool {
    matches!(node, Node::Element { attrs, .. } if attrs.iter().any(|a| matches!(a.name.as_str(), "flex" | "flex-grow" | "flex-basis" | "width" | "min-width")))
}

/// Whether a row's part is a column of one shown text.
fn one_line(node: &Node) -> bool {
    matches!(node, Node::Element { tag, attrs, children, .. }
        if !hidden(node) && column_row(tag, attrs) && text_lines(children) == Some((1, 1)))
}

/// Whether a row's part is lines stacked in the row: a shown element that
/// lays out as a column around two parts or more (a column with nothing in
/// it is a box, as a cover's placeholder is). `part` reads an `if` through.
fn stacks(node: &Node) -> bool {
    matches!(node, Node::Element { tag, attrs, children, .. }
        if !hidden(node) && column_row(tag, attrs) && children.iter().filter(|c| !hidden(c)).count() >= 2)
}
