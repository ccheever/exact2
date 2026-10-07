//! A list's padding after its last row (@ref LLP 1010 §6.9): read from the
//! layout before each report, and taken in by the report, whose anchor is
//! taken on the range the reader was in.
use super::*;

impl Collection {
    /// The list's resolved padding after its last row on each axis, `[bottom,
    /// right]`, from the layout a report follows (@ref LLP 1010 §6.9). The
    /// report takes it in: its anchor is taken on the range it was in.
    pub(super) fn set_end_padding(&mut self, [bottom, right]: [f64; 2]) {
        let trailing = match self.axis {
            ListAxis::Vertical => bottom,
            ListAxis::Horizontal => right,
        };
        self.trailing_next = Some(if trailing.is_finite() {
            trailing.max(0.0)
        } else {
            0.0
        });
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
