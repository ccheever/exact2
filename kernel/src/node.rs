//! Per-node-type rules that are semantics, not table data.

use crate::generated::NodeType;

impl NodeType {
    /// Whether `SetChildren` may target this type. A `Text` holds only inline
    /// runs (its `Text` children), which are measured with it, never laid out.
    pub fn can_hold_children(self) -> bool {
        matches!(
            self,
            NodeType::View
                | NodeType::ScrollView
                | NodeType::List
                | NodeType::Pressable
                | NodeType::Text
        )
    }

    /// Whether the node is measured by the text measurer.
    pub fn is_text_leaf(self) -> bool {
        matches!(self, NodeType::Text | NodeType::TextInput)
    }

    /// Whether the node is a scroll container by default (overflow on its
    /// block axis is `scroll` unless the producer says otherwise).
    pub fn scrolls_by_default(self) -> bool {
        matches!(self, NodeType::ScrollView | NodeType::List)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_cover_every_type() {
        for t in NodeType::ALL {
            // Every type answers each question without panicking, and text leaves never hold layout children.
            let _ = (
                t.can_hold_children(),
                t.is_text_leaf(),
                t.scrolls_by_default(),
            );
            if t == NodeType::TextInput {
                assert!(!t.can_hold_children());
            }
        }
        assert!(NodeType::Text.can_hold_children());
        assert!(NodeType::Text.is_text_leaf());
        assert!(NodeType::ScrollView.scrolls_by_default());
    }
}
