//! Text-style fills for a page without a kernel (LLP 1115.000 D12.2).

use exact_kernel::style::metrics::Column;
use exact_kernel::style::relative::{text_style_leading, text_style_weight, Unit};
use exact_kernel::{NodeFacts, NodeType, PropId, StyleId, StyleProps};
use std::fmt::Write;

/// Presence, not the value: substituting an empty token makes the bare
/// fill inherit its parent's computed row. Forwarding a value such as
/// `2em` would resolve it again against the descendant's font size.
fn said_rows(style: &StyleProps, out: &mut String) {
    for (id, name) in ROWS {
        if style.mask.has(id) {
            write!(out, "--exact-said-{name}: ;").unwrap();
        } else if style.written.has(id) {
            // A clear bypasses this node's start, but says nothing itself.
            write!(out, "--exact-said-{name}:inherit;").unwrap();
        }
    }
}

/// The inherited rows a platform text style can fill.
pub const ROWS: [(StyleId, &str); 2] = [
    (StyleId::FontWeight, "font-weight"),
    (StyleId::LineHeight, "line-height"),
];

/// A shell-only control leading is conditional on an ancestor saying the row.
pub const SHELL_LEADING: &str = "--exact-shell-line-height";
/// The shell's leading when no text style or authored row applies.
pub const CONTROL_LEADING: &str = "--exact-control-line-height";

/// Append the same declarations to a static class and its server document.
pub fn fills(node: &NodeFacts<'_>, out: &mut String) {
    let own = node.style;
    said_rows(own, out);
    let Some((Unit::TextStyle(style), _)) = own.relative.get(StyleId::FontSize) else {
        return;
    };
    let shorthand = node.props.bool(PropId::FontShorthand) == Some(true);
    use exact_kernel::ControlKind;
    let kind = ControlKind::of(node.node_type, node.props);
    let button = kind == Some(ControlKind::Button);
    let shell_leading = kind == Some(ControlKind::Select)
        && node.props.bool(PropId::Multiple) != Some(true)
        || matches!(
            kind,
            Some(ControlKind::Date | ControlKind::Time | ControlKind::DateTimeLocal)
        ) && own.appearance == exact_kernel::Appearance::Auto;
    let field =
        node.node_type == NodeType::TextInput && own.appearance != exact_kernel::Appearance::None;
    for (id, name) in ROWS {
        if own.mask.has(id) || own.written.has(id) {
            continue;
        }
        // The stylesheet supplies a control's start on this node. A bare
        // fill would otherwise inherit past it when its own marker is set.
        if !shorthand && (button || field && id == StyleId::LineHeight) {
            continue;
        }
        let value = match id {
            StyleId::FontWeight => text_style_weight(style, Column::Web).to_string(),
            _ => text_style_leading(style, Column::Web).css(),
        };
        if shorthand {
            write!(out, "{name}:{value};--exact-said-{name}: ;").unwrap();
        } else if id == StyleId::LineHeight && shell_leading {
            write!(out, "{name}:var({SHELL_LEADING},{value});").unwrap();
        } else {
            write!(out, "{name}:var(--exact-said-{name},{value});").unwrap();
        }
    }
}
