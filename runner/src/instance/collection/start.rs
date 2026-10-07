//! `scroll-start: end` (LLP 1010 §6.5): a virtualized list that opens at its
//! end — a transcript — builds its last rows first, tells the host to move
//! there before it paints, and anchors the end through measurement until the
//! window there is measured or the reader moves it.
use super::*;

impl Collection {
    /// Whether the window anchors the end: the list follows it, or it is
    /// still opening there.
    pub(super) fn follows(&self) -> bool {
        (self.follow_end || self.at_end) && self.preview.is_none()
    }
    /// Before any host report: the window is the last rows, and the host is
    /// told to start at the end (it clamps the offset to its extent).
    pub(super) fn start_at_end(&mut self) {
        if !self.at_end || self.geometry.is_some() || self.index.len() == 0 {
            return;
        }
        let offset = self.index.total_height();
        self.start_offset = offset;
        self.correction = Some(AnchorCorrection {
            scroll_sequence: 0,
            offset,
            from: None,
            smooth: false,
        });
    }
    /// The first row of the window before any host report.
    pub(super) fn bootstrap_first(&self) -> Result<usize, InstanceError> {
        if self.at_end {
            return Ok(self.index.len().saturating_sub(self.bootstrap_rows));
        }
        Ok(self
            .index
            .row_at(self.start_offset)
            .map_err(index_error)?
            .unwrap_or(0))
    }
    /// The reader's own motion ends the opening: travel a host samples from
    /// a drag, a fling or a wheel, in two reports running, as it cancels a
    /// `scrollIntoView`. Not a report short of the end: a host clamps its
    /// port to rows it laid out at their real size before the runner
    /// measured them, and one may report before it applies the correction.
    pub(super) fn leave_end_if_moved(&mut self, velocity: f64) {
        if !self.at_end {
            return;
        }
        self.end_travel = if velocity != 0.0 {
            self.end_travel + 1
        } else {
            0
        };
        if self.end_travel >= 2 {
            self.at_end = false;
        }
    }
    /// The offset an opening list's anchor is taken at: its end.
    pub(super) fn anchor_offset(&self, offset: f64) -> f64 {
        if self.at_end {
            self.index.total_height()
        } else {
            offset
        }
    }
    /// The list opened: a report at the end that changed nothing — every
    /// mounted row measured, and its measurements left the extent as it was
    /// (`extent`, before them; a first report's are discarded), nothing owed
    /// or corrected. Until then the window there is still
    /// filling, and a host may clamp its port to rows it has laid out before
    /// the runner measured them. From then on `scrollFollowEnd` decides.
    pub(super) fn settle_start(&mut self, extent: f64) {
        let Some(g) = &self.geometry else {
            return;
        };
        let total = self.index.total_height();
        // An empty list has not opened: its rows are yet to come.
        if self.at_end
            && self.index.len() > 0
            && !self.pending
            && self.correction.is_none()
            && (total - extent).abs() < 0.01
            && self.mounted.iter().all(|m| {
                self.index
                    .key(m.position)
                    .is_some_and(|k| self.index.is_measured(k))
            })
            && (total - g.port_main).max(0.0) - g.offset < super::index::END_SLACK
        {
            self.at_end = false;
        }
    }
}
/// Whether a port at `offset` is where the anchor puts it (`corrected`).
/// Hosts round offsets to device pixels (4405.1667 shows as 4405.333 at
/// 3x; WebKit floors a fractional scroll range): a followed end already sent
/// (`sent`) is reached within `END_SLACK`,
/// else the same unreachable end went out with every commit and an opening
/// never settled (LLP 1010 §6.8). An end that moved is sent once; a row
/// anchor keeps 0.01, since its moves add up report on report.
pub(super) fn at_target(
    anchor: &super::index::Anchor,
    corrected: f64,
    offset: f64,
    sent: f64,
) -> bool {
    let gap = (corrected - offset).abs();
    gap <= 0.01
        || (super::index::SizeIndex::follows_end(anchor)
            && gap < super::index::END_SLACK
            && (corrected - sent).abs() <= 0.01)
}
