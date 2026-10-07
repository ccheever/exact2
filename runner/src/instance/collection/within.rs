//! A report that moves nothing a commit would show: travel inside the
//! realized window.
use super::*;

impl Collection {
    /// Travel inside the realized window, which is most reports while a list
    /// moves: the same port and pins, nothing owed, no correction before or
    /// after, no height that changes, and every row of the window it leads
    /// to mounted. Realizing it would reuse every row and emit nothing, so
    /// only the geometry moves (@ref LLP 1050.000 §6). None: realize.
    ///
    /// A row's first measurement at the height the index already holds for
    /// it (a row as tall as its estimate) is such a report: it is recorded
    /// and nothing moves. So is, where rows are rebound (LLP 1078), one that
    /// only leaves rows past the window, no more than a list holds for its
    /// next rows ([`reuse::HOLD`]): they stay, held.
    pub(super) fn travel_within(
        &mut self,
        u: &mut Update<'_>,
        feedback: &CollectionFeedback,
        by_view: &BTreeMap<ViewId, usize>,
        fill: CollectionFill,
    ) -> Result<Option<Option<CollectionEdges>>, InstanceError> {
        let Some(g) = &self.geometry else {
            return Ok(None);
        };
        if self.restored_at.is_some()
            || self.at_end
            || self.target.is_some()
            || self.pending
            || self.preview.is_some()
            || self.incoming.is_some()
            || self.hidden.is_some()
            || self.correction.is_some()
            || (
                g.port_cross,
                g.port_main,
                g.cross,
                g.focus_view,
                g.interaction_view,
            ) != (
                feedback.port_cross,
                feedback.port_main,
                feedback.cross,
                feedback.focus_view,
                feedback.interaction_view,
            )
        {
            return Ok(None);
        }
        // A host reports every mounted row's height each time; one the
        // index already holds, measured, changes nothing (`feedback`'s own
        // loop would set it again). One it holds as an estimate is a first
        // measurement: recorded below when it is that estimate, else realize.
        let mut first = Vec::new();
        for (i, m) in feedback.measurements.iter().enumerate() {
            let row = &self.mounted[by_view[&m.view]];
            // By position: a lookup by key hashed every mounted row's key,
            // every report.
            let zero = || {
                self.zero_heights
                    .contains(self.index.key(row.position).unwrap())
            };
            if self.index.measurement_token_at(row.position) != Some(row.token)
                || (self.index.noise_at(row.position, m.size)
                    && (m.size == 0.0) == (!self.zero_heights.is_empty() && zero()))
            {
                continue;
            }
            let same = self.index.height(row.position).map(f64::to_bits) == Some(m.size.to_bits());
            if !same || m.size == 0.0 || zero() {
                return Ok(None);
            }
            first.push(i);
        }
        let anchor = self
            .index
            .capture_anchor(feedback.offset, g.port_main, self.follow_end)
            .map_err(index_error)?;
        let corrected = self
            .index
            .restore_anchor(&anchor, g.port_main)
            .map_err(index_error)?;
        if !start::at_target(&anchor, corrected, feedback.offset.max(0.0), self.end_sent) {
            return Ok(None);
        }
        let pins = self.pins();
        let (focus, interaction) = (self.pin(pins[0]), self.pin(pins[1]));
        let window = self
            .index
            .window_led(
                feedback.offset,
                feedback.port_main,
                lead(feedback.port_main, fill.velocity),
                [focus.as_deref(), interaction.as_deref()],
            )
            .map_err(index_error)?;
        // Every row of the window mounted, in order; the rows past it are
        // those a list holds, where it holds any.
        let inside = |p: usize| window.segments.iter().any(|s| s.contains(&p));
        let mut within = self.mounted.iter().filter(|r| inside(r.position));
        let whole = window
            .segments
            .iter()
            .cloned()
            .flatten()
            .all(|p| within.next().is_some_and(|r| r.position == p));
        let past = || self.mounted.iter().filter(|r| !inside(r.position));
        let hold = if self.reusing(u) { reuse::HOLD } else { 0 };
        if !whole || past().count() > hold || past().any(|r| !self.spare(r)) {
            return Ok(None);
        }
        for i in 0..self.mounted.len() {
            self.mounted[i].held = !inside(self.mounted[i].position);
        }
        for i in first {
            self.measure(by_view, feedback.measurements[i])?;
        }
        u.work.rows_reused += self.mounted.len();
        self.set_geometry(CollectionFeedback {
            measurements: Vec::new(),
            ..feedback.clone()
        });
        Ok(Some(self.edge_event(fill)?))
    }
}
