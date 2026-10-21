//! A gap is certified only by current measured boundary rows, including zero runs.
use super::*;

impl SizeIndex {
    /// Nearest midpoint boundary; rightmost logical boundary for coincident zero
    /// heights. None means measurement is missing, never logical end by guessing.
    /// A foreign target's gap: nothing of its own is dragged (LLP 1094 D4).
    pub(crate) fn certified_gap(&self, y: f64) -> Result<Option<usize>, IndexError> {
        self.gap(y, None)
    }
    pub(crate) fn certified_gap_excluding(
        &self,
        y: f64,
        source: usize,
    ) -> Result<Option<usize>, IndexError> {
        self.gap(y, Some(source))
    }
    fn gap(&self, y: f64, source: Option<usize>) -> Result<Option<usize>, IndexError> {
        if !y.is_finite() {
            return Err(IndexError::InvalidGeometry);
        }
        if self.len() == 0 {
            return Ok(None);
        }
        let y = y.max(0.0).min(self.total_height());
        let raw = match self.row_at(y)? {
            Some(i) => {
                if !self.is_measured(self.key(i).unwrap()) {
                    return Ok(None);
                }
                if source == Some(i) {
                    i
                } else {
                    let mid = self.prefix(i).unwrap() + self.height(i).unwrap() / 2.;
                    if y < mid {
                        i
                    } else {
                        i + 1
                    }
                }
            }
            None => self.len(),
        };
        let top = self.prefix(raw).unwrap();
        let mut right = self.row_at(top)?.unwrap_or(self.len());
        // First positive row ending at top; any following zero rows must also
        // have current measurements. No row scan, including a 25k zero run.
        let mut left = if top > 0. {
            self.tree.find(top, true).unwrap_or(0)
        } else {
            0
        };
        if let Some(source) = source {
            // Collapse the source before resolving coincident boundaries. Both
            // halves of its box name the same gap. Include zero rows on BOTH
            // sides in the proof; the pre-normalized boundary is insufficient.
            if right == source {
                right = self
                    .row_at(self.prefix(source + 1).unwrap())?
                    .unwrap_or(self.len());
            }
            if left == source {
                let before = self.prefix(source).unwrap();
                left = if before > 0.0 {
                    self.tree.find(before, true).unwrap_or(0)
                } else {
                    0
                };
            }
        }
        let end = (right + 1).min(self.len());
        Ok((self.tree.min_epoch(left..end) >= self.epoch).then_some(right))
    }
}
impl SumTree {
    pub(super) fn set_epoch(&mut self, index: usize, epoch: u64) {
        let mut node = self.base + index;
        self.measured[node] = epoch;
        while node > 1 {
            node /= 2;
            self.measured[node] = self.measured[node * 2].min(self.measured[node * 2 + 1]);
        }
    }
    pub(super) fn min_epoch(&self, range: Range<usize>) -> u64 {
        let (mut left, mut right) = (range.start + self.base, range.end + self.base);
        let mut epoch = u64::MAX;
        while left < right {
            #[cfg(test)]
            self.visits.set(self.visits.get() + 1);
            if left & 1 != 0 {
                epoch = epoch.min(self.measured[left]);
                left += 1;
            }
            if right & 1 != 0 {
                right -= 1;
                epoch = epoch.min(self.measured[right]);
            }
            left /= 2;
            right /= 2;
        }
        epoch
    }
}
