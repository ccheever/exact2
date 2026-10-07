//! A list's padding and scroll padding along its axis (@ref LLP 1010
//! §6.9), read from its style before each report. The padding after the
//! last row is taken in by the report, whose anchor is taken on the range
//! the reader was in; the padding before the first row and the scroll
//! padding are where a `scrollIntoView` aligns.
use super::*;

/// A list's `padding` and `scroll-padding`, each top, right, bottom, left,
/// in points.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Insets {
    pub(crate) padding: [f64; 4],
    pub(crate) scroll: [f64; 4],
}

impl Insets {
    /// The start and end of `sides` along `axis`.
    fn main(sides: [f64; 4], axis: ListAxis) -> [f64; 2] {
        let [top, right, bottom, left] =
            sides.map(|n| if n.is_finite() { n.max(0.0) } else { 0.0 });
        match axis {
            ListAxis::Vertical => [top, bottom],
            ListAxis::Horizontal => [left, right],
        }
    }
}

impl Collection {
    /// The list's insets as its style resolves them now. The report takes
    /// the end padding in: its anchor is taken on the range it was in.
    pub(super) fn set_insets(&mut self, insets: Insets) {
        let [leading, trailing] = Insets::main(insets.padding, self.axis);
        self.leading = leading;
        self.scroll_padding = Insets::main(insets.scroll, self.axis);
        self.trailing_next = Some(trailing);
    }
    /// A report's anchor, where the end padding moves to `trailing` with it:
    /// taken on the old range, so a followed end the host left where it was
    /// (the padding grew: a rotation's safe area) still follows; or, when
    /// that misses a followed end, on the new one (the host clamped the port
    /// to the end of a range that shrank). The index takes `trailing`.
    pub(super) fn report_anchor(
        &mut self,
        offset: f64,
        port: f64,
        trailing: f64,
    ) -> Result<index::Anchor, InstanceError> {
        let capture = |c: &Self| {
            c.index
                .capture_anchor(c.anchor_offset(offset), port, c.follows())
                .map_err(index_error)
        };
        let mut anchor = capture(self)?;
        let moved = trailing != self.index.trailing();
        self.index.set_trailing(trailing);
        if moved && self.follows() && !index::SizeIndex::follows_end(&anchor) {
            let at_new = capture(self)?;
            if index::SizeIndex::follows_end(&at_new) {
                anchor = at_new;
            }
        }
        Ok(anchor)
    }
}
