//! What a grouped row and its parts are: whether it lays out as a column,
//! and which `input` is a field, a switch, or hidden.

use contract_syntax::{Attr, Expr, Node};

/// Whether a row lays out as a column: a `column`, or a row whose authored
/// `flex-direction` is a literal column; a `button`'s parts stay a row.
pub(super) fn column_row(tag: &str, attrs: &[Attr]) -> bool {
    match attrs.iter().rev().find(|a| a.name == "flex-direction") {
        Some(a) => matches!(&a.value, Expr::Str(v, _) if v.starts_with("column")),
        None => tag == "column",
    }
}

/// An `input`'s literal `type`, lowercased; `text` when it has none.
pub(super) fn input_type(node: &Node) -> Option<String> {
    let Node::Element { attrs, .. } = node else {
        return None;
    };
    match attrs.iter().rev().find(|a| a.name == "type") {
        Some(a) => match &a.value {
            Expr::Str(v, _) => Some(v.to_ascii_lowercase()),
            _ => None,
        },
        None => Some("text".into()),
    }
}

/// Whether an `input` is a switch: a checkbox written with HTML's word
/// `switch`, or `role="switch"` (LLP 1069.001 D1, D3).
pub(super) fn switch(node: &Node) -> bool {
    input_type(node).as_deref() == Some("checkbox")
        && matches!(node, Node::Element { tag, positional, attrs, .. }
            if positional.iter().any(|word| contract_syntax::is_input_switch(tag, word))
                || attrs.iter().any(|a| a.name == "role"
                    && matches!(&a.value, Expr::Str(v, _) if v == "switch")))
}

/// What a row's field beside its label is: one line of text, or a date,
/// time or menu (a `select`) the platform shows as a compact picker.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Field {
    Text,
    Picker,
}

pub(super) fn field_kind(node: &Node) -> Option<Field> {
    match node {
        Node::Element { tag, .. } if tag == "select" => return Some(Field::Picker),
        Node::Element { tag, .. } if tag == "input" => {}
        _ => return None,
    }
    if text_field(node) {
        return Some(Field::Text);
    }
    matches!(
        input_type(node).as_deref(),
        Some("date" | "time" | "datetime-local" | "month" | "week")
    )
    .then_some(Field::Picker)
}

/// A row of a label and the field after it, written as such: its first
/// shown part a `text`, and a text, date or time `input` or a `select` among
/// the rest.
pub(super) fn field_beside_label(children: &[Node]) -> Option<Field> {
    let mut shown = children.iter().filter(|c| !hidden(c));
    if !matches!(shown.next(), Some(Node::Element { tag, .. }) if tag == "text") {
        return None;
    }
    shown.find_map(field_kind)
}

/// Whether an `input` is a field of text: one line a person types into.
pub(super) fn text_field(node: &Node) -> bool {
    matches!(
        input_type(node).as_deref(),
        Some("text" | "email" | "url" | "tel" | "password" | "number")
    )
}

/// Whether an element is written `display="none"`.
pub(super) fn hidden(node: &Node) -> bool {
    matches!(node, Node::Element { attrs, .. }
        if attrs.iter().rev().find(|a| a.name == "display").is_some_and(|a| matches!(&a.value, Expr::Str(v, _) if v == "none")))
}
