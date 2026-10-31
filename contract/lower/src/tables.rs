//! HTML's table and description list (LLP 1153 D7), carried as what they
//! are: each element is a `View` whose `semanticTag` names it, a cell's
//! `colspan`, `rowspan` and `scope` are props, and the compiler holds them to
//! HTML's content model. The web makes the elements themselves; a native
//! host's kernel lays a table out as a grid (`kernel/src/layout/table.rs`).

use crate::tags::Tag;
use crate::{err, LowerError};
use contract_syntax::{Attr, Expr, Node, Span};
use exact_kernel::{NodeType, PropId, StyleId};

/// The tag for one of the elements: a `View` with its `semanticTag`. A
/// `th` is bold and centred and a `caption` centred, as HTML's UA sheet
/// sets them; the rest of what the browser's sheet gives these elements
/// is layout, which the kernel supplies on native hosts.
pub(crate) fn tag(name: &str) -> Tag {
    const CENTRED: &[(StyleId, &str)] = &[(StyleId::TextAlign, "center")];
    const HEADER: &[(StyleId, &str)] =
        &[(StyleId::FontWeight, "700"), (StyleId::TextAlign, "center")];
    let (fixed_styles, fixed_props): (&'static [_], &'static [_]) = match name {
        "table" => (&[], &[(PropId::SemanticTag, "table")]),
        "caption" => (CENTRED, &[(PropId::SemanticTag, "caption")]),
        "thead" => (&[], &[(PropId::SemanticTag, "thead")]),
        "tbody" => (&[], &[(PropId::SemanticTag, "tbody")]),
        "tfoot" => (&[], &[(PropId::SemanticTag, "tfoot")]),
        "tr" => (&[], &[(PropId::SemanticTag, "tr")]),
        "th" => (HEADER, &[(PropId::SemanticTag, "th")]),
        "td" => (&[], &[(PropId::SemanticTag, "td")]),
        "dl" => (&[], &[(PropId::SemanticTag, "dl")]),
        "dt" => (&[], &[(PropId::SemanticTag, "dt")]),
        _ => (&[], &[(PropId::SemanticTag, "dd")]),
    };
    Tag {
        node_type: NodeType::View,
        fixed_styles,
        fixed_props,
        positional: None,
    }
}

/// The element a table's part stands in, and what HTML's content model
/// says may stand there.
fn belongs(tag: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match tag {
        "caption" | "thead" | "tbody" | "tfoot" => (&["table"], "a `table`"),
        "tr" => (
            &["table", "thead", "tbody", "tfoot"],
            "a `table`, `thead`, `tbody` or `tfoot`",
        ),
        "th" | "td" => (&["tr"], "a `tr`"),
        "dt" | "dd" => (&["dl"], "a `dl`"),
        _ => return None,
    })
}

/// What an element of these holds: the tags, and how to say them.
fn holds(parent: &str) -> Option<(&'static [&'static str], &'static str)> {
    Some(match parent {
        "table" => (
            &["caption", "thead", "tbody", "tfoot", "tr"],
            "`caption`, `thead`, `tbody`, `tfoot` and `tr`",
        ),
        "thead" | "tbody" | "tfoot" => (&["tr"], "`tr`s"),
        "tr" => (&["th", "td"], "`th` and `td`"),
        "dl" => (&["dt", "dd"], "`dt` and `dd`"),
        _ => return None,
    })
}

fn content_id(tag: &str) -> &'static str {
    if matches!(tag, "dl" | "dt" | "dd") {
        "lower-dl-content"
    } else {
        "lower-table-content"
    }
}

/// HTML's content model for a table's and a description list's elements,
/// and their cells' attributes, for `tag` under `parent` (the nearest
/// enclosing element; a `for` or `if` between them does not count).
pub(crate) fn check(
    tag: &str,
    parent: Option<&str>,
    attrs: &[Attr],
    children: &[Node],
    span: Span,
) -> Result<(), LowerError> {
    if let Some((parents, said)) = belongs(tag) {
        if !parent.is_some_and(|p| parents.contains(&p)) {
            return err(content_id(tag), format!("`{tag}` belongs in {said}"), span);
        }
    }
    if let Some((parent, (allowed, list))) = parent.and_then(|p| Some((p, holds(p)?))) {
        if !allowed.contains(&tag) {
            return err(
                content_id(parent),
                format!("a `{parent}` holds {list}, not `{tag}`"),
                span,
            );
        }
    }
    for a in attrs {
        attribute(tag, a)?;
    }
    if tag == "tr" {
        if let Some(at) = repeated(children) {
            return err(
                "lower-table-cells",
                "a `tr`'s cells are written out, one element per cell, so a table's columns are the ones its markup names (LLP 1153 D7): a `for` inside a `tr` makes the row's cell count vary with the data. Repeat the `tr` with `for` instead, or write each cell out, with `if` around one that comes and goes",
                at,
            );
        }
    }
    if tag == "dl" {
        grouped(children)?;
    }
    Ok(())
}

/// `colspan` on a cell, a whole number from 1 to HTML's limit, and
/// `rowspan` one from 0, which is the rest of the cell's row group (HTML
/// §4.9.11); `scope` on a `th`, `col` or `row`.
fn attribute(tag: &str, a: &Attr) -> Result<(), LowerError> {
    let cell = matches!(tag, "th" | "td");
    let (least, most, said) = match a.name.as_str() {
        "colspan" => (1.0, 1000.0, "columns from 1"),
        "rowspan" => (0.0, 65534.0, "rows from 0 (the rest of its row group)"),
        "scope" => {
            if tag != "th" {
                return err("lower-attr-tag", "`scope` belongs to `th`", a.span);
            }
            if matches!(&a.value, Expr::Str(s, _) if s != "col" && s != "row")
                || matches!(a.value, Expr::Number(..) | Expr::Bool(..))
            {
                return err(
                    "lower-attr-value",
                    "`scope` takes \"col\" (the header of its column) or \"row\" (of its row)",
                    a.span,
                );
            }
            return Ok(());
        }
        _ => return Ok(()),
    };
    if !cell {
        return err(
            "lower-attr-tag",
            format!("`{}` belongs to `th` or `td`", a.name),
            a.span,
        );
    }
    let whole = |n: f64| n.fract() == 0.0 && (least..=most).contains(&n);
    match &a.value {
        Expr::Number(n, _) if whole(*n) => Ok(()),
        Expr::Number(..) | Expr::Str(..) | Expr::Bool(..) => err(
            "lower-attr-value",
            format!(
                "`{}` is a whole number of {said} to {most} (HTML's limit), written as a number: `{}=2`",
                a.name, a.name
            ),
            a.span,
        ),
        _ => Ok(()),
    }
}

/// A table's children with each run of rows it holds directly in a `tbody`
/// of its own, as HTML's parser puts them (HTML §13.2.6.4.9): the DOM a
/// page rendered ahead is parsed into is then the one the runtime builds,
/// and every host has the row group HTML's table model has. `None` where
/// there is no such run. The `tbody` has the table's place in the source.
pub(crate) fn implied_bodies(
    tag: &str,
    children: &[Node],
    span: Span,
    instance: u32,
) -> Option<Vec<Node>> {
    if tag != "table" || !children.iter().any(rows) {
        return None;
    }
    let body = |run: Vec<Node>| Node::Element {
        tag: "tbody".into(),
        positional: Vec::new(),
        attrs: Vec::new(),
        children: run,
        span,
        instance,
    };
    let mut out = Vec::with_capacity(children.len());
    let mut run = Vec::new();
    for c in children {
        if rows(c) {
            run.push(c.clone());
            continue;
        }
        if !run.is_empty() {
            out.push(body(std::mem::take(&mut run)));
        }
        out.push(c.clone());
    }
    if !run.is_empty() {
        out.push(body(run));
    }
    Some(out)
}

/// Whether `n` is a `tr`, or a `for`, `if` or `match` that makes nothing
/// else.
fn rows(n: &Node) -> bool {
    let all = |nodes: &[Node]| nodes.iter().all(rows);
    match n {
        Node::Element { tag, .. } => tag == "tr",
        Node::When {
            then, otherwise, ..
        } => all(then) && all(otherwise),
        Node::Each { body, .. } => all(body),
        Node::Match { some, none, .. } => all(&some.1) && all(none),
        _ => false,
    }
}

/// Where a `for` stands among `children`, an `if`'s or `match`'s arms
/// looked into: the cells of the row it is in would vary in number.
fn repeated(children: &[Node]) -> Option<Span> {
    children.iter().find_map(|c| match c {
        Node::Each { span, .. } => Some(*span),
        Node::When {
            then, otherwise, ..
        } => repeated(then).or_else(|| repeated(otherwise)),
        Node::Match { some, none, .. } => repeated(&some.1).or_else(|| repeated(none)),
        _ => None,
    })
}

/// HTML's groups in a `dl` (HTML §4.4.9): one or more `dt`, then one or
/// more `dd`. Any run of them that starts with a `dt` and ends with a `dd`
/// is such groups, so what is written must start the list with a `dt` and
/// end it with a `dd`; between, a `for` or an `if` makes groups of the
/// data's, which the written ones before and after it join.
fn grouped(children: &[Node]) -> Result<(), LowerError> {
    fn tag(n: Option<&Node>) -> Option<(&str, Span)> {
        match n {
            Some(Node::Element { tag, span, .. }) => Some((tag.as_str(), *span)),
            _ => None,
        }
    }
    if let Some((_, span)) = tag(children.first()).filter(|(t, _)| *t == "dd") {
        return err(
            "lower-dl-content",
            "a `dl`'s first group starts with its `dt`: each group is one or more `dt` (the terms) then one or more `dd` (their descriptions)",
            span,
        );
    }
    if let Some((_, span)) = tag(children.last()).filter(|(t, _)| *t == "dt") {
        return err(
            "lower-dl-content",
            "a `dt` is followed by its `dd`: each group in a `dl` is one or more `dt` then one or more `dd`",
            span,
        );
    }
    Ok(())
}
