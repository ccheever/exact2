//! Images in a paragraph's text (LLP 1011.002).
use super::*;
use crate::generated::Display;

impl NodeArena {
    /// Whether this slot is an image in a paragraph's text: an `Image`
    /// whose parent is a paragraph (a `Text` that is no inline run). Its
    /// paragraph measures it and places it; it has no layout node of its own.
    pub fn is_inline_image(&self, slot: u32) -> bool {
        self.node_types[slot as usize] == NodeType::Image
            && self.parents[slot as usize].is_some_and(|p| {
                self.node_types[p as usize] == NodeType::Text && !self.is_inline_run(p)
            })
    }

    /// Whether this inline image is one of its paragraph's runs: it is
    /// displayed.
    pub(crate) fn is_run_image(&self, slot: u32) -> bool {
        self.is_inline_image(slot) && self.style(slot).display != Display::None
    }

    /// Whether a change to these rows of `slot` measures its paragraph
    /// again: a text row, or any layout row of an image in the text, whose
    /// box is the paragraph's measurement.
    pub(crate) fn remeasures(&self, slot: u32, mask: StyleMask) -> bool {
        mask.intersects(StyleMask::TEXT)
            || mask.intersects(StyleMask::LAYOUT) && self.is_inline_image(slot)
    }

    /// Whether this slot is measured with its paragraph and has no layout
    /// node of its own: an inline run or an inline image.
    pub fn in_paragraph(&self, slot: u32) -> bool {
        self.is_inline_run(slot) || self.is_inline_image(slot)
    }

    /// The images the paragraph `owner` measures, in run order, appended to
    /// `out`: one per run whose `inline` is set.
    pub fn inline_images(&self, owner: u32, out: &mut Vec<u32>) {
        if self.node_types[owner as usize] != NodeType::Text
            || self.props[owner as usize].str(PropId::Text).is_some()
        {
            return;
        }
        // Its own children, the runs' order: an image is never a run's.
        out.extend(
            self.children[owner as usize]
                .iter()
                .filter(|&&c| self.is_run_image(c)),
        );
    }
}
