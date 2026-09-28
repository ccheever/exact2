//! Form controls by their HTML names (LLP 1069.001): which widget a
//! `Control` node is, the size a host shows before it reports its own, and a
//! `select`'s options as every host reads them.
//!
//! The rules here are HTML's, so the runner can hold every host to them: a
//! select's value is one of its enabled options' values.

use crate::generated::{NodeType, PropId};
use crate::id::ViewId;
use crate::kernel::{Kernel, NodeRef};
use crate::props::PropList;

/// Which platform control a `Control` node is, by its `type` prop (and a
/// checkbox's role).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    /// `input type="checkbox"`.
    Checkbox,
    /// `input type="checkbox" switch`.
    Switch,
    /// `input type="file"` (LLP 1069.002).
    File,
    /// `select`, its options the node's children.
    Select,
}

impl ControlKind {
    /// The kind of a node, or `None` when it is not a `Control`.
    pub fn of(node_type: NodeType, props: &PropList) -> Option<ControlKind> {
        if node_type != NodeType::Control {
            return None;
        }
        Some(match props.str(PropId::Type) {
            Some("file") => ControlKind::File,
            Some("select") => ControlKind::Select,
            _ if props.str(PropId::AccessibilityRole) == Some("switch") => ControlKind::Switch,
            _ => ControlKind::Checkbox,
        })
    }

    /// The content size a host that reports none shows (LLP 1069.001 D3):
    /// Chrome's 13×13 checkbox, Safari's 38×22 desktop switch, and a select
    /// one line of Chrome's 13.33 px control font tall. A host that knows
    /// its control's size reports it.
    pub fn default_size(self) -> (f32, f32) {
        match self {
            ControlKind::Checkbox | ControlKind::File => (13.0, 13.0),
            ControlKind::Switch => (38.0, 22.0),
            ControlKind::Select => (64.0, 19.0),
        }
    }
}

/// One `option` of a `select`, as a menu shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// The option's node.
    pub view: ViewId,
    /// Its `value`, or its label when it has none (HTML).
    pub value: String,
    /// Its text, white space stripped and collapsed (HTML's `option.text`).
    pub label: String,
    /// Whether it may not be chosen.
    pub disabled: bool,
}

/// Whether `node` is an `option`: a text node the `option` tag made.
pub fn is_option(node: &NodeRef<'_>) -> bool {
    node.node_type == NodeType::Text && node.props.str(PropId::SemanticTag) == Some("option")
}

impl Kernel {
    /// A select's options in order, from its children (LLP 1069.001 D2);
    /// empty for any other node.
    pub fn select_choices(&self, view: ViewId) -> Vec<Choice> {
        let Some(select) = self.node(view) else {
            return Vec::new();
        };
        if ControlKind::of(select.node_type, select.props) != Some(ControlKind::Select) {
            return Vec::new();
        }
        let disabled = select.props.bool(PropId::Disabled) == Some(true);
        select
            .children()
            .into_iter()
            .filter_map(|id| self.node(id))
            .filter(is_option)
            .map(|option| {
                let text: String = option.text_runs().iter().map(|r| &*r.text).collect();
                let label = text.split_whitespace().collect::<Vec<_>>().join(" ");
                Choice {
                    view: option.id,
                    value: option
                        .props
                        .str(PropId::Value)
                        .map_or_else(|| label.clone(), str::to_owned),
                    label,
                    disabled: disabled || option.props.bool(PropId::Disabled) == Some(true),
                }
            })
            .collect()
    }

    /// The option a select shows: the one whose value its `value` names;
    /// with no `value`, the first enabled option, as an uncontrolled HTML
    /// select starts. `None` when its value names no option.
    pub fn select_chosen(&self, view: ViewId) -> Option<Choice> {
        let choices = self.select_choices(view);
        match self.node(view)?.props.str(PropId::Value) {
            Some(value) => choices.into_iter().find(|c| c.value == value),
            None => choices.into_iter().find(|c| !c.disabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::Op;
    use crate::PropValue;

    fn select() -> Kernel {
        let mut k = Kernel::with_monospace();
        let mut ops = vec![
            Op::CreateView {
                id: 1,
                node_type: NodeType::View,
            },
            Op::CreateView {
                id: 2,
                node_type: NodeType::Control,
            },
            Op::SetProp {
                id: 2,
                prop: PropId::Type,
                value: PropValue::Str("select".into()),
            },
        ];
        for (id, value, text) in [(3, Some("s"), "  System\n default "), (4, None, "Dark")] {
            ops.push(Op::CreateView {
                id,
                node_type: NodeType::Text,
            });
            ops.push(Op::SetProp {
                id,
                prop: PropId::SemanticTag,
                value: PropValue::Str("option".into()),
            });
            ops.push(Op::SetProp {
                id,
                prop: PropId::Text,
                value: PropValue::Str(text.into()),
            });
            if let Some(v) = value {
                ops.push(Op::SetProp {
                    id,
                    prop: PropId::Value,
                    value: PropValue::Str(v.into()),
                });
            }
        }
        ops.push(Op::SetProp {
            id: 4,
            prop: PropId::Disabled,
            value: PropValue::Bool(true),
        });
        ops.push(Op::SetChildren {
            id: 2,
            children: vec![3, 4],
        });
        ops.push(Op::SetChildren {
            id: 1,
            children: vec![2],
        });
        ops.push(Op::AttachRoot { id: 1 });
        k.apply(0, 1, &ops).unwrap();
        k
    }

    #[test]
    fn a_selects_options_are_its_children_by_htmls_rules() {
        let k = select();
        let choices = k.select_choices(2);
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].value, "s");
        assert_eq!(choices[0].label, "System default");
        assert_eq!(choices[1].value, "Dark", "no value: the label");
        assert!(choices[1].disabled);
        assert_eq!(k.select_chosen(2).map(|c| c.value), Some("s".into()));
        assert!(k.select_choices(1).is_empty());
        assert_eq!(
            ControlKind::of(NodeType::Control, k.node(2).unwrap().props),
            Some(ControlKind::Select)
        );
    }
}
