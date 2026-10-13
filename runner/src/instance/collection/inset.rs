//! A list's padding along its axis (@ref LLP 1010 §6.9): the padding
//! before the first row and after the last that its host laid out, taken in
//! with the report that carries it, whose anchor is taken on the range the
//! reader was in.
use super::*;

impl Collection {
    /// The padding before the first row and after the last, as the index
    /// has it.
    pub(super) fn padding(&self) -> [f64; 2] {
        [self.index.leading(), self.index.trailing()]
    }
    pub(super) fn set_padding(&mut self, [leading, trailing]: [f64; 2]) {
        self.index.set_leading(leading);
        self.index.set_trailing(trailing);
    }
    /// A report's anchor, where the padding moves to `padding` with it:
    /// taken on the old range, so a followed end the host left where it was
    /// (the padding grew: a rotation's safe area) still follows; or, when
    /// that misses a followed end, on the new one (the host clamped the port
    /// to the end of a range that shrank). On the old range the port is
    /// where the host left it on the page: offsets count from the first
    /// row, which a grown padding before it moved down by as much. The
    /// index takes `padding`.
    pub(super) fn report_anchor(
        &mut self,
        offset: f64,
        port: f64,
        padding: [f64; 2],
    ) -> Result<index::Anchor, InstanceError> {
        let capture = |c: &Self, offset: f64| {
            c.index
                .capture_anchor(c.anchor_offset(offset), port, c.follows())
                .map_err(index_error)
        };
        let was = self.padding();
        // Before a first report the runner knew no padding to move.
        let moved_down = if self.geometry.is_some() {
            padding[0] - was[0]
        } else {
            0.0
        };
        let mut anchor = capture(self, offset + moved_down)?;
        self.set_padding(padding);
        if padding != was && self.follows() && !index::SizeIndex::follows_end(&anchor) {
            let at_new = capture(self, offset)?;
            if index::SizeIndex::follows_end(&at_new) {
                anchor = at_new;
            }
        }
        Ok(anchor)
    }
}
