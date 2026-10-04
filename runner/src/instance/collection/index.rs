//! Private collection geometry; no host/layout types or row instances.
//!
//! Integration: construct with a positive normal-row estimate, then `replace_keys`
//! only on membership/order changes. Capture an anchor before mutating heights or
//! keys, restore it afterwards, and query `window` using the actual scrollport.
//! Materialize the returned segments and use `prefix` for the intervening spacers.
//! Keep at most the current anchor: it shares the old key order without copying it.
//!
//! Before content changes or a row is remounted, call `invalidate_row`; before a
//! width/typography change, call `invalidate_all`. Both retain provisional heights.
//! Feedback must carry the token from when the row was laid out, plus the caller's
//! collection identity. Tokens are local to this index; fetching a fresh token when
//! old feedback arrives defeats stale-report detection. An accepted measurement
//! does not itself advance a generation; the caller orders reports within a layout.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;
mod gaps;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IndexError {
    InvalidHeight,
    InvalidGeometry,
    DuplicateKey(String),
    UnknownKey(String),
    ExtentOverflow,
    CapacityOverflow,
    GenerationExhausted,
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeight => f.write_str("row height must be finite and nonnegative"),
            Self::InvalidGeometry => f.write_str("scroll geometry must be finite and nonnegative"),
            Self::DuplicateKey(key) => write!(f, "duplicate collection key: {key}"),
            Self::UnknownKey(key) => write!(f, "unknown collection key: {key}"),
            Self::ExtentOverflow => f.write_str("collection height exceeds finite geometry"),
            Self::CapacityOverflow => f.write_str("collection index capacity overflow"),
            Self::GenerationExhausted => f.write_str("collection measurement generation exhausted"),
        }
    }
}

impl std::error::Error for IndexError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MeasurementToken {
    epoch: u64,
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RowHeight {
    height: f64,
    generation: u64,
    measured_epoch: Option<u64>,
}

/// A short-lived anchor around a mutation, sharing an immutable order snapshot.
/// A deleted key uses its next surviving old neighbor, then its previous neighbor,
/// at the same within-row offset. If no old key survives, restore to the start.
#[derive(Debug, Clone)]
pub(crate) struct Anchor {
    order: Rc<[Rc<str>]>,
    row: Option<usize>,
    within: f64,
    follows_end: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Window {
    pub(crate) offset: f64,
    /// Endpoint bounds, possibly containing zero-height gaps; realize `segments`.
    pub(crate) visible: Range<usize>,
    pub(crate) overscan: Range<usize>,
    /// Sorted, nonempty, disjoint positive-height runs plus at most two pins.
    /// Interior zero-height rows are excluded unless explicitly pinned.
    pub(crate) segments: Vec<Range<usize>>,
}

#[derive(Debug)]
pub(crate) struct SizeIndex {
    order: Rc<[Rc<str>]>,
    positions: BTreeMap<Rc<str>, usize>,
    rows: Vec<RowHeight>,
    tree: SumTree,
    estimate: f64,
    epoch: u64,
    next_generation: u64,
    #[cfg(test)]
    rebuilds: usize,
}

impl SizeIndex {
    /// Zero estimates are legal, but cannot bootstrap a visible row by geometry.
    /// Choose a positive estimate for unmeasured content; measured zeroes are fine.
    pub(crate) fn new(estimated_height: f64) -> Result<Self, IndexError> {
        let estimate = valid_height(estimated_height)?;
        Ok(Self {
            order: Rc::from([]),
            positions: BTreeMap::new(),
            rows: Vec::new(),
            tree: SumTree::new(&[])?,
            estimate,
            epoch: 1,
            next_generation: 0,
            #[cfg(test)]
            rebuilds: 0,
        })
    }

    /// Transactional membership rebuild. Surviving keys retain heights and tokens;
    /// same-key content changes must separately call `invalidate_row`/`invalidate_all`.
    /// Deleted and later reinserted keys always receive new measurement generations.
    pub(crate) fn replace_keys(&mut self, keys: Vec<Rc<str>>) -> Result<(), IndexError> {
        if self.order.as_ref() == keys.as_slice() {
            return Ok(());
        }
        let mut positions = BTreeMap::new();
        let mut rows = Vec::with_capacity(keys.len());
        let mut generation = self.next_generation;
        for (i, key) in keys.iter().enumerate() {
            if positions.insert(key.clone(), i).is_some() {
                return Err(IndexError::DuplicateKey(key.to_string()));
            }
            let row = if let Some(&old) = self.positions.get(key) {
                self.rows[old]
            } else {
                generation = next_generation(generation)?;
                RowHeight {
                    height: self.estimate,
                    generation,
                    measured_epoch: None,
                }
            };
            rows.push(row);
        }
        let tree = SumTree::new(&rows)?;
        self.order = Rc::from(keys);
        self.positions = positions;
        self.rows = rows;
        self.tree = tree;
        self.next_generation = generation;
        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
        Ok(())
    }

    /// `replace_keys` of the order that keeps rows `..start` and `end..` and
    /// puts `middle` between them, with the same result. Each middle key
    /// comes with the old position it moves from when the caller knows it
    /// (the key at that position, inside `start..end`); the others are
    /// looked up. It moves the kept rows and renumbers positions in one pass
    /// (O(N) copies, no key comparisons), and compares, looks up and
    /// allocates keys only for the looked-up ones: a live insert costs its
    /// own key.
    pub(crate) fn splice_keys(
        &mut self,
        start: usize,
        end: usize,
        middle: Vec<(Rc<str>, Option<usize>)>,
    ) -> Result<(), IndexError> {
        assert!(start <= end && end <= self.len(), "splice inside the order");
        // Per old middle position, its new position if its key survives.
        let mut moved: Vec<Option<usize>> = vec![None; end - start];
        let mut fresh = std::collections::BTreeSet::new();
        // Middle keys new to the index, which `positions` must gain.
        let mut added = Vec::new();
        let mut generation = self.next_generation;
        let mut rows = Vec::with_capacity(self.len() - (end - start) + middle.len());
        rows.extend_from_slice(&self.rows[..start]);
        for (i, (key, from)) in middle.iter().enumerate() {
            let old = match *from {
                Some(old) => {
                    assert!(
                        (start..end).contains(&old)
                            && (Rc::ptr_eq(&self.order[old], key) || self.order[old] == *key),
                        "a moved key is the key at its old position"
                    );
                    Some(old)
                }
                None => {
                    if !fresh.insert(&**key) {
                        return Err(IndexError::DuplicateKey(key.to_string()));
                    }
                    match self.positions.get(&**key) {
                        Some(&old) if (start..end).contains(&old) => Some(old),
                        Some(_) => return Err(IndexError::DuplicateKey(key.to_string())),
                        None => None,
                    }
                }
            };
            rows.push(match old {
                Some(old) => {
                    if moved[old - start].replace(start + i).is_some() {
                        return Err(IndexError::DuplicateKey(key.to_string()));
                    }
                    self.rows[old]
                }
                None => {
                    added.push(i);
                    generation = next_generation(generation)?;
                    RowHeight {
                        height: self.estimate,
                        generation,
                        measured_epoch: None,
                    }
                }
            });
        }
        rows.extend_from_slice(&self.rows[end..]);
        let tree = SumTree::new(&rows)?;
        let after = start + middle.len();
        self.positions.retain(|_, position| {
            if *position >= end {
                *position = *position - end + after;
            } else if *position >= start {
                match moved[*position - start] {
                    Some(new) => *position = new,
                    None => return false,
                }
            }
            true
        });
        for i in added {
            self.positions.insert(middle[i].0.clone(), start + i);
        }
        let mut order = Vec::with_capacity(rows.len());
        order.extend_from_slice(&self.order[..start]);
        order.extend(middle.into_iter().map(|(key, _)| key));
        order.extend_from_slice(&self.order[end..]);
        self.order = Rc::from(order);
        self.rows = rows;
        self.tree = tree;
        self.next_generation = generation;
        Ok(())
    }

    /// Everything a membership change decides, for comparing two paths.
    #[cfg(test)]
    pub(crate) fn fingerprint(&self) -> String {
        format!(
            "{:?} {:?} {:?} {} {} {:?} {:?}",
            self.order,
            self.positions,
            self.rows,
            self.next_generation,
            self.epoch,
            self.tree.sums,
            self.tree.measured
        )
    }

    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn key(&self, index: usize) -> Option<&str> {
        self.order.get(index).map(|key| &**key)
    }

    /// The key at `index`, shared.
    pub(crate) fn shared_key(&self, index: usize) -> Option<&Rc<str>> {
        self.order.get(index)
    }

    pub(crate) fn position(&self, key: &str) -> Option<usize> {
        self.positions.get(key).copied()
    }

    pub(crate) fn height(&self, index: usize) -> Option<f64> {
        self.rows.get(index).map(|row| row.height)
    }

    /// Height of rows `[0, end)`, including `end == len()`; O(log N).
    pub(crate) fn prefix(&self, end: usize) -> Option<f64> {
        (end <= self.len()).then(|| self.tree.prefix(end))
    }

    pub(crate) fn total_height(&self) -> f64 {
        self.tree.total()
    }

    /// First row whose bottom is strictly after `offset`; zero-height prefixes
    /// are skipped in O(log N). At/past total height there is no containing row.
    pub(crate) fn row_at(&self, offset: f64) -> Result<Option<usize>, IndexError> {
        valid_geometry(offset)?;
        Ok(self.tree.find(offset, false))
    }

    pub(crate) fn measurement_token(&self, key: &str) -> Option<MeasurementToken> {
        self.position(key).map(|i| MeasurementToken {
            epoch: self.epoch,
            generation: self.rows[i].generation,
        })
    }

    /// [`SizeIndex::measurement_token`] by position.
    pub(crate) fn measurement_token_at(&self, index: usize) -> Option<MeasurementToken> {
        self.rows.get(index).map(|row| MeasurementToken {
            epoch: self.epoch,
            generation: row.generation,
        })
    }

    pub(crate) fn is_measured(&self, key: &str) -> bool {
        self.position(key)
            .is_some_and(|i| self.rows[i].measured_epoch == Some(self.epoch))
    }

    /// [`SizeIndex::is_measured`] by position (keys are unique), without a
    /// key lookup.
    pub(crate) fn is_measured_at(&self, index: usize) -> bool {
        self.rows
            .get(index)
            .is_some_and(|row| row.measured_epoch == Some(self.epoch))
    }

    /// Current measurements for a nonempty geometric band, in O(log N).
    pub(crate) fn range_measured(&self, range: Range<usize>) -> bool {
        !range.is_empty() && self.tree.min_epoch(range) >= self.epoch
    }

    /// O(1) invalidation for width/typography/global content changes. No row scan
    /// or height reset: existing heights remain estimates until measured again.
    pub(crate) fn invalidate_all(&mut self) -> Result<(), IndexError> {
        self.epoch = next_generation(self.epoch)?;
        Ok(())
    }

    /// O(log N) keyed invalidation for changed content or remounted row roots.
    pub(crate) fn invalidate_row(&mut self, key: &str) -> Result<MeasurementToken, IndexError> {
        let i = self
            .position(key)
            .ok_or_else(|| IndexError::UnknownKey(key.to_owned()))?;
        let generation = next_generation(self.next_generation)?;
        self.next_generation = generation;
        self.rows[i].generation = generation;
        self.rows[i].measured_epoch = None;
        self.tree.set_epoch(i, 0);
        Ok(MeasurementToken {
            epoch: self.epoch,
            generation,
        })
    }

    /// Add `delta` over the rows in `range` whose heights are estimates, in
    /// equal parts, none below zero: a restored position's items above it
    /// (LLP 1070 §4.2) keep the start they had. O(k log N).
    pub(crate) fn spread_estimates(
        &mut self,
        range: Range<usize>,
        delta: f64,
    ) -> Result<(), IndexError> {
        if !delta.is_finite() || delta.abs() < 0.01 {
            return Ok(());
        }
        let range = range.start.min(self.rows.len())..range.end.min(self.rows.len());
        let open: Vec<usize> = range
            .filter(|&i| self.rows[i].measured_epoch != Some(self.epoch))
            .collect();
        if open.is_empty() {
            return Ok(());
        }
        let part = delta / open.len() as f64;
        for i in open {
            let height = (self.rows[i].height + part).max(0.0);
            self.tree.set(i, height)?;
            self.rows[i].height = height;
        }
        Ok(())
    }

    /// Returns false for stale, deleted, or mismatched rows. Errors never mutate
    /// metadata; NaN/infinite/negative heights are rejected even for stale reports.
    pub(crate) fn set_measured_height(
        &mut self,
        key: &str,
        token: MeasurementToken,
        height: f64,
    ) -> Result<bool, IndexError> {
        match self.position(key) {
            Some(i) => self.set_measured_height_at(i, token, height),
            None => valid_height(height).map(|_| false),
        }
    }

    /// [`SizeIndex::set_measured_height`] by position (keys are unique).
    pub(crate) fn set_measured_height_at(
        &mut self,
        i: usize,
        token: MeasurementToken,
        height: f64,
    ) -> Result<bool, IndexError> {
        let height = valid_height(height)?;
        if self.measurement_token_at(i) != Some(token) {
            return Ok(false);
        }
        // The same height measured again this epoch: nothing to write.
        if self.rows[i].height.to_bits() == height.to_bits()
            && self.rows[i].measured_epoch == Some(self.epoch)
        {
            return Ok(true);
        }
        self.tree.set(i, height)?;
        self.rows[i].height = height;
        self.rows[i].measured_epoch = Some(self.epoch);
        self.tree.set_epoch(i, self.epoch);
        Ok(true)
    }

    /// Geometry is nonnegative and finite. Clamp overscroll to the logical extent;
    /// a zero-height scrollport has no window (explicit pins can remain mounted).
    /// Missing pin keys are ignored; the array makes the two-pin budget explicit.
    pub(crate) fn window(
        &self,
        offset: f64,
        viewport: f64,
        pins: [Option<&str>; 2],
    ) -> Result<Window, IndexError> {
        self.window_led(offset, viewport, [viewport, viewport], pins)
    }
    /// [`SizeIndex::window`] with its overscan `lead` before and after the
    /// viewport given, rather than one viewport each side.
    pub(crate) fn window_led(
        &self,
        offset: f64,
        viewport: f64,
        lead: [f64; 2],
        pins: [Option<&str>; 2],
    ) -> Result<Window, IndexError> {
        let offset = self.clamp_offset(offset, viewport)?;
        let end = (offset + viewport).min(self.total_height());
        let visible = self.band(offset, end);
        let overscan = self.band(
            (offset - lead[0]).max(0.0),
            (end + lead[1]).min(self.total_height()),
        );
        let mut ranges = self.tree.positive_ranges(&overscan);
        for key in pins.into_iter().flatten() {
            if let Some(i) = self.position(key) {
                ranges.push(i..i + 1);
            }
        }
        ranges.sort_unstable_by_key(|range| range.start);
        let mut segments: Vec<Range<usize>> = Vec::with_capacity(3);
        for range in ranges {
            if let Some(last) = segments.last_mut() {
                if range.start <= last.end {
                    last.end = last.end.max(range.end);
                    continue;
                }
            }
            segments.push(range);
        }
        Ok(Window {
            offset,
            visible,
            overscan,
            segments,
        })
    }

    /// O(log N), no key copy. `follow_end` opts into following only if the clamped
    /// offset is at the end of a nonzero scrollport within host rounding tolerance.
    /// Otherwise preserve
    /// the first visible key's top relative to the scrollport (even if it shrinks).
    pub(crate) fn capture_anchor(
        &self,
        offset: f64,
        viewport: f64,
        follow_end: bool,
    ) -> Result<Anchor, IndexError> {
        let offset = self.clamp_offset(offset, viewport)?;
        let row = self.tree.find(offset, false);
        let within = row.map_or(0.0, |i| (offset - self.tree.prefix(i)).max(0.0));
        // Browser scroll ranges round fractional CSS extents to whole pixels;
        // native document geometry also rounds through f32. Admit up to half a
        // logical pixel/point on every host, independent of extent (including
        // small resident windows); never follow a reader beyond that tolerance.
        let tolerance = 0.5;
        Ok(Anchor {
            order: Rc::clone(&self.order),
            row,
            within,
            follows_end: follow_end
                && viewport > 0.0
                && self.max_offset(viewport) - offset <= tolerance,
        })
    }

    /// An anchor at `key`'s item, `within` into it, however that compares
    /// with the item's estimate: a restored position (LLP 1070 §4.2), held
    /// until the item is measured.
    pub(crate) fn anchor_at(&self, key: &str, within: f64) -> Option<Anchor> {
        Some(Anchor {
            order: Rc::clone(&self.order),
            row: Some(self.position(key)?),
            within,
            follows_end: false,
        })
    }

    /// O(log N) while the anchor survives. Deletion fallback scans the old order
    /// only after structural edits (which already permit an index rebuild).
    /// Drop/recapture the anchor after applying the correction; retaining many old
    /// anchors would retain their old O(N) key snapshots too.
    pub(crate) fn restore_anchor(&self, anchor: &Anchor, viewport: f64) -> Result<f64, IndexError> {
        valid_geometry(viewport)?;
        let max = self.max_offset(viewport);
        if anchor.follows_end {
            return Ok(max);
        }
        let Some(old) = anchor.row else {
            return Ok(0.0);
        };
        let row = self.position(&anchor.order[old]).or_else(|| {
            anchor.order[old + 1..]
                .iter()
                .chain(anchor.order[..old].iter().rev())
                .find_map(|key| self.position(key))
        });
        Ok(row.map_or(0.0, |i| (self.tree.prefix(i) + anchor.within).min(max)))
    }

    fn band(&self, start: f64, end: f64) -> Range<usize> {
        let first = self.tree.find(start, false).unwrap_or(self.len());
        if start >= end {
            return first..first;
        }
        let last = self.tree.find(end, true).map_or(self.len(), |i| i + 1);
        first..last
    }

    fn max_offset(&self, viewport: f64) -> f64 {
        (self.total_height() - viewport).max(0.0)
    }

    fn clamp_offset(&self, offset: f64, viewport: f64) -> Result<f64, IndexError> {
        valid_geometry(offset)?;
        valid_geometry(viewport)?;
        Ok(offset.min(self.max_offset(viewport)).max(0.0))
    }
}

fn next_generation(generation: u64) -> Result<u64, IndexError> {
    generation
        .checked_add(1)
        .ok_or(IndexError::GenerationExhausted)
}

fn valid_height(value: f64) -> Result<f64, IndexError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value.max(0.0))
    } else {
        Err(IndexError::InvalidHeight)
    }
}

fn valid_geometry(value: f64) -> Result<(), IndexError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(IndexError::InvalidGeometry)
    }
}

/// A sum tree avoids subtractive Fenwick updates leaving negative residue after
/// many measurements. Only O(log N) ancestors are recomputed from their children.
#[derive(Debug)]
struct SumTree {
    sums: Vec<f64>,
    measured: Vec<u64>,
    base: usize,
    len: usize,
    zeros: usize,
    #[cfg(test)]
    visits: std::cell::Cell<usize>,
}

impl SumTree {
    fn new(rows: &[RowHeight]) -> Result<Self, IndexError> {
        let base = rows
            .len()
            .max(1)
            .checked_next_power_of_two()
            .ok_or(IndexError::CapacityOverflow)?;
        let capacity = base.checked_mul(2).ok_or(IndexError::CapacityOverflow)?;
        let mut sums = vec![0.0; capacity];
        let mut measured = vec![u64::MAX; capacity];
        let mut zeros = 0;
        for (i, row) in rows.iter().enumerate() {
            sums[base + i] = row.height;
            measured[base + i] = row.measured_epoch.unwrap_or(0);
            zeros += usize::from(row.height == 0.0);
        }
        for node in (1..base).rev() {
            sums[node] = sums[node * 2] + sums[node * 2 + 1];
            measured[node] = measured[node * 2].min(measured[node * 2 + 1]);
        }
        if !sums[1].is_finite() {
            return Err(IndexError::ExtentOverflow);
        }
        Ok(Self {
            sums,
            measured,
            base,
            len: rows.len(),
            zeros,
            #[cfg(test)]
            visits: std::cell::Cell::new(0),
        })
    }

    fn total(&self) -> f64 {
        self.sums[1]
    }

    /// Prune whole zero-height subtrees, including gaps inside the endpoint band.
    /// Work depends on selected rows and tree depth, not on the gap's row count.
    fn positive_ranges(&self, band: &Range<usize>) -> Vec<Range<usize>> {
        let mut ranges = Vec::with_capacity(3);
        if !band.is_empty() {
            if self.zeros == 0 {
                // Keep the usual all-positive window lookup O(log N).
                ranges.push(band.clone());
            } else {
                self.collect_positive(1, 0..self.base, band, &mut ranges);
            }
        }
        ranges
    }

    fn collect_positive(
        &self,
        node: usize,
        span: Range<usize>,
        band: &Range<usize>,
        ranges: &mut Vec<Range<usize>>,
    ) {
        #[cfg(test)]
        self.visits.set(self.visits.get() + 1);
        if self.sums[node] == 0.0 || span.end <= band.start || span.start >= band.end {
            return;
        }
        if node >= self.base {
            if let Some(last) = ranges.last_mut() {
                if last.end == span.start {
                    last.end = span.end;
                    return;
                }
            }
            ranges.push(span);
            return;
        }
        let middle = span.start + (span.end - span.start) / 2;
        self.collect_positive(node * 2, span.start..middle, band, ranges);
        self.collect_positive(node * 2 + 1, middle..span.end, band, ranges);
    }

    fn set(&mut self, index: usize, height: f64) -> Result<(), IndexError> {
        // Check the new root before publishing any writes; overflow is atomic.
        let mut node = self.base + index;
        let mut sum = height;
        while node > 1 {
            sum += self.sums[node ^ 1];
            node /= 2;
        }
        if !sum.is_finite() {
            return Err(IndexError::ExtentOverflow);
        }
        let mut node = self.base + index;
        self.zeros = self.zeros - usize::from(self.sums[node] == 0.0) + usize::from(height == 0.0);
        self.sums[node] = height;
        while node > 1 {
            node /= 2;
            self.sums[node] = self.sums[node * 2] + self.sums[node * 2 + 1];
        }
        Ok(())
    }

    fn prefix(&self, end: usize) -> f64 {
        if end == self.len {
            return self.total();
        }
        let mut node = 1;
        let mut span = self.base;
        let mut remaining = end;
        let mut before = 0.0;
        let mut after = self.total();
        while node < self.base {
            #[cfg(test)]
            self.visits.set(self.visits.get() + 1);
            span /= 2;
            let middle = self.split(node, before, after);
            node *= 2;
            if remaining >= span {
                before = middle;
                node += 1;
                remaining -= span;
            } else {
                after = middle;
            }
        }
        before
    }

    // Different associations of floating-point sums can disagree by an ulp.
    // Share the same parent-bounded splits between prefix and lookup so prefix
    // positions never decrease, and never assign rounding residue to zero rows.
    fn split(&self, node: usize, before: f64, after: f64) -> f64 {
        let left = self.sums[node * 2];
        if left == 0.0 {
            before
        } else if self.sums[node * 2 + 1] == 0.0 {
            after
        } else {
            (before + left).min(after)
        }
    }

    /// inclusive=true finds the first bottom >= offset (end of a half-open band),
    /// false finds the first bottom > offset (start of that band).
    fn find(&self, offset: f64, inclusive: bool) -> Option<usize> {
        if self.len == 0 || self.total() == 0.0 {
            return None;
        }
        let contains = |bottom: f64| {
            if inclusive {
                bottom >= offset
            } else {
                bottom > offset
            }
        };
        if !contains(self.total()) {
            return None;
        }
        let mut node = 1;
        let mut before = 0.0;
        let mut after = self.total();
        while node < self.base {
            #[cfg(test)]
            self.visits.set(self.visits.get() + 1);
            let bottom = self.split(node, before, after);
            node *= 2;
            if contains(bottom) {
                after = bottom;
            } else {
                before = bottom;
                node += 1;
            }
        }
        let index = node - self.base;
        (index < self.len).then_some(index)
    }
}

#[cfg(test)]
#[path = "index_tests.rs"]
mod tests;
