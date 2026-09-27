//! Per-node-type rules that are semantics, not table data.

use crate::generated::NodeType;

impl NodeType {
    /// Whether `SetChildren` may target this type. A `Text` holds only inline
    /// runs (its `Text` children), which are measured with it, never laid out.
    /// A `Canvas` holds children laid out in its box — the web's
    /// `layoutsubtree` — that never size it (LLP 1014 D1).
    pub fn can_hold_children(self) -> bool {
        matches!(
            self,
            NodeType::View
                | NodeType::ScrollView
                | NodeType::List
                | NodeType::Pressable
                | NodeType::Text
                | NodeType::Canvas
                | NodeType::Svg
                | NodeType::SvgGroup
                | NodeType::SvgViewport
                | NodeType::SvgDefs
                | NodeType::SvgSymbol
                | NodeType::SvgLinearGradient
                | NodeType::SvgRadialGradient
                | NodeType::SvgClipPath
                | NodeType::SvgText
                | NodeType::SvgTSpan
                | NodeType::SvgMarker
                | NodeType::SvgMask
                | NodeType::SvgPattern
                | NodeType::SvgForeignObject
                | NodeType::SvgFilter
                | NodeType::SvgFe
        )
    }

    /// Whether the node is measured by the text measurer.
    pub fn is_text_leaf(self) -> bool {
        matches!(self, NodeType::Text | NodeType::TextInput)
    }

    /// Whether the node's size comes from a measure — text, or a replaced
    /// element (`Image`) with an intrinsic size the host reported.
    pub fn is_measured_leaf(self) -> bool {
        self.is_text_leaf() || self.is_replaced()
    }

    /// An image, video or `svg` whose content has an intrinsic size. An
    /// `svg`'s children are its content, never laid out (LLP 1055 D3).
    pub fn is_replaced(self) -> bool {
        matches!(self, NodeType::Image | NodeType::Video | NodeType::Svg)
    }

    /// Whether this node's children are laid out as boxes: not a paragraph's
    /// inline runs, and not an `svg`'s content (LLP 1055 D3).
    pub fn lays_out_children(self) -> bool {
        self != NodeType::Text && self != NodeType::Svg && !self.is_svg_element()
    }

    /// Whether the node is a scroll container by default (overflow on its
    /// block axis is `scroll` unless the producer says otherwise).
    pub fn scrolls_by_default(self) -> bool {
        matches!(self, NodeType::ScrollView | NodeType::List)
    }

    /// Whether the node describes the document rather than drawing in it
    /// (`head`, LLP 1048.003 D1): it takes no space, whatever its rows say,
    /// and holds no children. Hosts write it where the platform keeps a
    /// page's metadata — the web's `<head>`, a window or scene title.
    pub fn is_metadata(self) -> bool {
        matches!(self, NodeType::Head)
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
        // An image is a replaced element: measured, never a container.
        assert!(NodeType::Image.is_measured_leaf());
        assert!(!NodeType::Image.is_text_leaf());
        assert!(!NodeType::Image.can_hold_children());
        assert!(NodeType::ScrollView.scrolls_by_default());
        // A canvas holds children (LLP 1014 D1) and is never measured: its
        // size is its rows', never its content's.
        assert!(NodeType::Canvas.can_hold_children());
        assert!(!NodeType::Canvas.is_measured_leaf());
        assert!(!NodeType::WebView.can_hold_children());
        // A head is metadata: never a container, never measured.
        assert!(NodeType::Head.is_metadata());
        assert!(!NodeType::Head.can_hold_children());
        assert!(!NodeType::Head.is_measured_leaf());
        assert_eq!(NodeType::ALL.iter().filter(|t| t.is_metadata()).count(), 1);
    }
}
