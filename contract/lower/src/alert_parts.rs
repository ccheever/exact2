//! An alert's parts (the `alertPart` prop). An alert-shaped `dialog
//! role="alertdialog"` is grouped into its title, a `message` box holding its
//! explanatory texts and an `actions` box holding its buttons, so a host that
//! draws the alert itself can scroll a long message while the title, a
//! prompt's field and the buttons stay in view (`host/web/index.html`,
//! `kernel/src/arena/alert.rs`). The hosts that present the platform's alert
//! read through the two boxes. The author writes the alert flat; only a shape
//! whose parts the source proves is grouped, and any other stays as written.

use crate::{err, LowerError, Lowerer};
use contract_syntax::{Attr, Expr, Node, Span};

/// The part a row belongs in, in the order the parts show.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Message,
    Field,
    Actions,
}

/// Whether `name` lays out a box's children: written on the dialog, the
/// grouping would change what it means.
fn lays_out_children(name: &str) -> bool {
    matches!(
        name,
        "display" | "gap" | "row-gap" | "column-gap" | "flex-direction" | "flex-wrap" | "flex-flow"
    ) || name.starts_with("grid")
        || ["align-", "justify-", "place-"]
            .iter()
            .any(|p| name.starts_with(p))
            && !name.ends_with("-self")
}

/// Whether `name` places or sizes a box in its parent: written on a row, the
/// row would land in a box that does not read it the same way (a percentage,
/// an offset or a flex item's size is its containing box's).
fn places_itself(name: &str) -> bool {
    matches!(
        name,
        "position"
            | "top"
            | "right"
            | "bottom"
            | "left"
            | "width"
            | "height"
            | "inline-size"
            | "block-size"
            | "aspect-ratio"
            | "order"
            | "flex"
    ) || ["inset", "margin", "min-", "max-", "flex-", "grid-"]
        .iter()
        .any(|p| name.starts_with(p))
        || name.ends_with("-self")
}

/// `alertPart` is the compiler's: an author's would claim a part the shape
/// does not prove.
pub(crate) fn check_authored(attrs: &[Attr]) -> Result<(), LowerError> {
    match attrs.iter().find(|a| a.name == "alertPart" && a.written) {
        Some(a) => err(
            "lower-alert-part",
            "`alertPart` is the compiler's: it groups an alert's title, message and buttons itself. Write the alert's texts and buttons as the dialog's rows",
            a.span,
        ),
        None => Ok(()),
    }
}

impl Lowerer<'_> {
    /// The children of an alert-shaped `dialog` grouped into its parts, or
    /// `None` to keep them as written: when it is no alert, its author lays
    /// it out, or its rows are not texts, then fields (one showing at a time,
    /// as menus.rs holds a prompt to), then buttons, each `if`, `for` or
    /// `match` holding rows of one part. Its title is its `aria-label`, else
    /// its first row (`exact_kernel::confirmation::alert_text`, which leaves
    /// out an empty one), so the title must be literal words, not blank: a
    /// label, or a `text` outside control flow.
    pub(crate) fn alert_parts(
        &self,
        tag: &str,
        attrs: &[Attr],
        children: &[Node],
        alert: bool,
        span: Span,
        instance: u32,
    ) -> Option<Vec<Node>> {
        if tag != "dialog" || !alert || self.lays_out(attrs, lays_out_children) {
            return None;
        }
        let words = |v: &Expr| match v {
            Expr::Str(v, _) if !v.trim().is_empty() => Some(v.clone()),
            _ => None,
        };
        let (title, name, rows) = match attrs.iter().rev().find(|a| a.name == "aria-label") {
            Some(label) => (None, words(&label.value)?, children),
            None => match children.split_first()? {
                (
                    first @ Node::Element {
                        tag,
                        positional,
                        attrs,
                        children: runs,
                        ..
                    },
                    rest,
                ) if tag == "text" && runs.is_empty() && !self.lays_out(attrs, places_itself) => {
                    let [only] = positional.as_slice() else {
                        return None;
                    };
                    (Some(first), words(only)?, rest)
                }
                _ => return None,
            },
        };
        let parts = rows
            .iter()
            .map(|row| self.part(row))
            .collect::<Option<Vec<_>>>()?;
        if !parts.is_sorted() {
            return None;
        }
        let of = |part: Part| -> Vec<Node> {
            rows.iter()
                .zip(&parts)
                .filter(|(_, p)| **p == part)
                .map(|(row, _)| row.clone())
                .collect()
        };
        let group = |tag: &str, attrs: Vec<Attr>, children: Vec<Node>| Node::Element {
            tag: tag.into(),
            positional: Vec::new(),
            attrs,
            children,
            span,
            instance,
        };
        let mut out = Vec::new();
        if let Some(title) = title {
            let mut title = title.clone();
            if let Node::Element { attrs, span, .. } = &mut title {
                attrs.push(implied("alertPart", "title", *span));
            }
            out.push(title);
        }
        // A group named by the title, so a page rendered ahead names it before
        // its runtime loads
        let message = of(Part::Message);
        if !message.is_empty() {
            let attrs = [
                ("alertPart", "message"),
                ("role", "group"),
                ("aria-label", &name),
            ];
            out.push(group(
                "column",
                attrs.map(|(n, v)| implied(n, v, span)).into(),
                message,
            ));
        }
        out.extend(of(Part::Field));
        out.push(group(
            "row",
            vec![implied("alertPart", "actions", span)],
            of(Part::Actions),
        ));
        Some(out)
    }

    /// The part every row `node` is or produces belongs in; `None` when
    /// they differ, it produces none, or one is laid out by its author.
    fn part(&self, node: &Node) -> Option<Part> {
        let all = |nodes: &[&[Node]]| -> Option<Part> {
            let mut parts = nodes.iter().flat_map(|n| n.iter()).map(|n| self.part(n));
            let first = parts.next()??;
            parts.all(|p| p == Some(first)).then_some(first)
        };
        match node {
            Node::Element { tag, attrs, .. } => {
                if self.lays_out(attrs, places_itself) {
                    return None;
                }
                match tag.as_str() {
                    "text" => Some(Part::Message),
                    "input" => Some(Part::Field),
                    "button" => Some(Part::Actions),
                    _ => None,
                }
            }
            Node::When {
                then, otherwise, ..
            } => all(&[then, otherwise]),
            Node::Each { body, .. } => all(&[body]),
            Node::Match { some, none, .. } => all(&[&some.1, none]),
            Node::Use { .. } | Node::Children { .. } => None,
        }
    }

    /// Whether `attrs`, or a class they name, write a property `layout` picks.
    fn lays_out(&self, attrs: &[Attr], layout: fn(&str) -> bool) -> bool {
        let class = self
            .class_rows(attrs)
            .ok()
            .flatten()
            .map_or_else(Vec::new, |(_, rows)| rows);
        attrs
            .iter()
            .filter(|a| a.written)
            .chain(&class)
            .any(|a| layout(&a.name))
    }
}

/// An attribute the compiler writes, which no author did.
fn implied(name: &str, value: &str, span: Span) -> Attr {
    Attr {
        name: name.into(),
        value: Expr::Str(value.into(), span),
        span,
        written: crate::IMPLIED,
    }
}
