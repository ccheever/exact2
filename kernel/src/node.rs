//! Per-node-type rules that are semantics, not table data.

use crate::generated::NodeType;
use crate::kernel::NodeRef;
use crate::{PropList, StyleProps, ViewId};

/// A node's own facts, as a document projection reads them (LLP 1048.004):
/// a kernel node's ([`NodeRef::facts`]), or those of a node a render host
/// holds for a document it writes from the runner's instance tree, with no
/// kernel. Its children and the ids it references are the tree's to answer.
#[derive(Debug, Clone, Copy)]
pub struct NodeFacts<'a> {
    /// Wire id.
    pub id: ViewId,
    /// Type.
    pub node_type: NodeType,
    /// Style rows.
    pub style: &'a StyleProps,
    /// Props.
    pub props: &'a PropList,
    /// Whether the node is a root.
    pub is_root: bool,
    /// A `Text` inside a `Text`: an inline run of its paragraph.
    pub inline_run: bool,
}

impl NodeFacts<'_> {
    /// Whether the node is an inline run of its parent paragraph.
    pub fn is_inline_run(&self) -> bool {
        self.inline_run
    }
}

impl<'a> NodeRef<'a> {
    /// This node's [`NodeFacts`].
    pub fn facts(&self) -> NodeFacts<'a> {
        NodeFacts {
            id: self.id,
            node_type: self.node_type,
            style: self.style,
            props: self.props,
            is_root: self.is_root,
            inline_run: self.is_inline_run(),
        }
    }
}

impl NodeType {
    /// Whether `SetChildren` may target this type. A `Text` holds only inline
    /// runs (its `Text` children), which are measured with it, never laid out.
    /// A `Canvas` holds children laid out in its box — the web's
    /// `layoutsubtree` — that never size it (LLP 1014 D1). A `Control` holds
    /// a `select`'s options, which a closed select never lays out (LLP
    /// 1069.001 D2).
    pub fn can_hold_children(self) -> bool {
        matches!(
            self,
            NodeType::View
                | NodeType::ScrollView
                | NodeType::List
                | NodeType::Pressable
                | NodeType::Text
                | NodeType::Control
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

    /// Whether the node's size comes from a measure — text, a replaced
    /// element sized from its natural or default object size (a canvas's
    /// children then laid out in the measured box, Taffy patch 16), or a
    /// form control whose size the platform decides (LLP 1069.001 D3).
    pub fn is_measured_leaf(self) -> bool {
        self.is_text_leaf() || self.is_replaced() || self == NodeType::Control
    }

    /// An image, video, `svg`, canvas or iframe: a replaced element, sized
    /// from what it shows, never from children. An `svg`'s children are its
    /// content, never laid out (LLP 1055 D3); a canvas's are laid out in its
    /// box and never size it (LLP 1014 D1).
    pub fn is_replaced(self) -> bool {
        matches!(
            self,
            NodeType::Image
                | NodeType::Video
                | NodeType::Svg
                | NodeType::Canvas
                | NodeType::WebView
        )
    }

    /// The size a replaced element has before (or without) a host-reported
    /// one, and whether it is natural. CSS Images 3 §5's default object
    /// size is 300×150, with no natural ratio, for an iframe, a video before
    /// its metadata and an `svg` without a view box. A canvas's natural size
    /// is its bitmap's, whose `width`/`height` attributes default to 300×150
    /// (HTML §4.12.5), so it has the natural ratio 2:1. A broken image has
    /// neither and is 0×0.
    pub fn default_object_size(self) -> Option<((f32, f32), bool)> {
        match self {
            NodeType::Canvas => Some(((300.0, 150.0), true)),
            NodeType::Video | NodeType::Svg | NodeType::WebView => Some(((300.0, 150.0), false)),
            _ => None,
        }
    }

    /// Whether this node's children are laid out as boxes: not a paragraph's
    /// inline runs, not an `svg`'s content (LLP 1055 D3), and not a
    /// `select`'s options (LLP 1069.001 D2).
    pub fn lays_out_children(self) -> bool {
        !matches!(self, NodeType::Text | NodeType::Svg | NodeType::Control)
            && !self.is_svg_element()
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
        // A form control is a measured leaf with no natural ratio.
        assert!(NodeType::Control.is_measured_leaf());
        assert!(!NodeType::Control.is_replaced());
        // Its children are a select's options, never laid out.
        assert!(NodeType::Control.can_hold_children());
        assert!(!NodeType::Control.lays_out_children());
        assert!(NodeType::ScrollView.scrolls_by_default());
        // A canvas holds children (LLP 1014 D1) and is measured as a
        // replaced element: its size is never its content's.
        assert!(NodeType::Canvas.can_hold_children());
        assert!(NodeType::Canvas.is_measured_leaf());
        assert!(!NodeType::WebView.can_hold_children());
        // A head is metadata: never a container, never measured.
        assert!(NodeType::Head.is_metadata());
        assert!(!NodeType::Head.can_hold_children());
        assert!(!NodeType::Head.is_measured_leaf());
        assert_eq!(NodeType::ALL.iter().filter(|t| t.is_metadata()).count(), 1);
    }
}
