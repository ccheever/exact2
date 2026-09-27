//! Bounded keyed row realization, driven by actual nested-scrollport feedback.
//! @ref LLP 1010 §6 / LLP 1041 §8. No historical instance or row-state cache.
mod api;
mod index;
mod rekey;
mod reorder;
mod reorder_api;
#[cfg(test)]
mod tests;
mod traversal;
pub(crate) mod views;
use super::*;
pub use api::*;
use exact_kernel::PropId;
use exact_plan::EventKind;
use index::{HeightIndex, MeasurementToken};
pub use reorder_api::*;
pub(super) use traversal::invalidate_typography;

/// A collection's data update, through [`super::LISTS`] (LLP 1047.000 §9).
pub(super) fn update_collection(
    c: &mut Collection,
    u: &mut Update<'_>,
    frames: &[Frame],
    follow: bool,
) -> Result<(), InstanceError> {
    c.follow_end(follow);
    c.update_data(u, frames, false)
}

/// The mounted collections as a batch's JSON, through [`super::LISTS`].
pub(super) fn collections_json(tree: &Tree) -> String {
    snapshots_json(&tree.collections())
}

const BOOTSTRAP_ROWS: usize = 16;
const ESTIMATED_HEIGHT: f64 = 32.0;
/// Travel the window leads by, past its viewport of overscan.
const LEAD_SECONDS: f64 = 0.25;
/// A mounted row farther than this many viewports from what shows retires
/// with any report, whatever its limit.
const FAR_VIEWPORTS: f64 = 2.0;

/// How far the window reaches past the viewport, before and after it: one
/// viewport each side, and toward the side the list travels, a quarter
/// second of that travel more, up to two viewports.
fn lead(viewport: f64, velocity: f64) -> [f64; 2] {
    let extra = (velocity.abs() * LEAD_SECONDS).min(viewport * 2.0);
    if velocity > 0.0 {
        [viewport, viewport + extra]
    } else {
        [viewport + extra, viewport]
    }
}

/// Candidates from one accepted geometry report. The second edge may run only
/// after a pure no-op first action. State changes defer it until settlement.
pub(crate) struct CollectionEdges {
    pub first: EventKind,
    pub end_after_noop: bool,
}

#[derive(Debug)]
struct Mounted {
    position: usize,
    wrapper: ViewId,
    epoch: u64,
    token: MeasurementToken,
    preview_target: Option<f64>,
    /// The position and count the wrapper last published (`aria-posinset`,
    /// `aria-setsize`), for hosts that select and copy across rows.
    published: (usize, usize),
    row: Row,
}
#[derive(Debug)]
pub(crate) struct Collection {
    preview: Option<reorder::Preview>,
    view: ViewId,
    region: RegionsId,
    index: HeightIndex,
    estimated_height: f64,
    bootstrap_rows: usize,
    items: Rc<[Value]>,
    keys: Vec<Value>,
    /// Positions whose key repeats an earlier one, and which repeat.
    dups: BTreeMap<usize, u32>,
    string_keys: bool,
    mounted: Vec<Mounted>,
    spacers: Vec<(ViewId, f64)>,
    children: Vec<ViewId>,
    revision: u64,
    next_epoch: u64,
    zero_heights: std::collections::BTreeSet<String>,
    geometry: Option<CollectionFeedback>,
    correction: Option<AnchorCorrection>,
    follow_end: bool,
    edge_handlers: [bool; 2],
    edge_armed: [bool; 2],
    /// The last realization left window rows unbuilt or kept rows past it.
    pending: bool,
}
fn index_error(e: index::IndexError) -> InstanceError {
    InstanceError::Collection(e.to_string())
}
fn invalid(message: &str) -> InstanceError {
    InstanceError::Collection(message.into())
}
// Only valid scalar keys may carry the prior ordered uniqueness proof. Keep
// signed-zero changes on the normal path so the stored Value stays exact.
fn same_key(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(a), Value::Str(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a.is_finite() && a.to_bits() == b.to_bits(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        _ => false,
    }
}
fn advance(n: &mut u64) -> Result<u64, InstanceError> {
    *n = n
        .checked_add(1)
        .ok_or_else(|| invalid("collection generation exhausted"))?;
    Ok(*n)
}
impl Collection {
    pub(super) fn follow_end(&mut self, enabled: bool) {
        self.follow_end = enabled;
    }
    fn invalidate_height_estimates(&mut self) -> Result<(), InstanceError> {
        // A confirmed zero cannot remain the estimate of invalidated content:
        // it would hide newly nonempty rows from every geometric window. Track
        // only these compact keys, never instances. Ordinary invalidation is
        // O(1); invalidating Z previously zero rows is O(Z log N), with no key
        // expression evaluation or data/resource access.
        for key in std::mem::take(&mut self.zero_heights) {
            if let Some(token) = self.index.measurement_token(&key) {
                self.index
                    .set_measured_height(&key, token, self.estimated_height)
                    .map_err(index_error)?;
            }
        }
        self.index.invalidate_all().map_err(index_error)
    }
    fn invalidate_measurements(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        self.end_preview(u)?;
        let anchor = self.anchor()?;
        self.invalidate_height_estimates()?;
        self.restore(anchor)?;
        // Invalidated zeroes now have positive estimates. Re-select the window
        // and emit its spacers in this commit, even if no old rows were mounted;
        // otherwise the host has nothing to lay out and cannot measure again.
        self.realize_window(u, frames, false, CollectionFill::default())?;
        advance(&mut self.revision)?;
        Ok(())
    }
    pub(super) fn create(
        u: &mut Update<'_>,
        node: NodesId,
        view: ViewId,
        frames: &[Frame],
    ) -> Result<Option<Box<Self>>, InstanceError> {
        let plan = u.env.plan;
        let descriptor = plan.node(node);
        let mut enabled = false;
        let mut follow_end = false;
        let mut estimated_height = ESTIMATED_HEIGHT;
        for binding in descriptor.bindings.iter().map(|b| plan.binding(b)) {
            if binding.kind == BindingKind::Prop && binding.id == PropId::Virtualized as u16 {
                enabled = u.eval(binding.expr, frames)? == Value::Bool(true);
            }
            if binding.kind == BindingKind::Prop && binding.id == PropId::ScrollFollowEnd as u16 {
                follow_end = u.eval(binding.expr, frames)? == Value::Bool(true);
            }
            if binding.kind == BindingKind::Prop && binding.id == PropId::EstimatedItemHeight as u16
            {
                let Value::Number(height) = u.eval(binding.expr, frames)? else {
                    return Err(invalid("estimated item height must be a number"));
                };
                if !height.is_finite() || height <= 0.0 {
                    return Err(invalid("estimated item height must be positive and finite"));
                }
                estimated_height = height;
            }
        }
        if !enabled {
            return Ok(None);
        }
        if descriptor.node_type != exact_kernel::NodeType::List as u8 {
            return Err(invalid("virtualized requires List"));
        }
        let child_sites = u.sites.children(Some(node), descriptor.arm);
        let [(_, Site::Region(region))] = child_sites else {
            return Err(invalid("collection needs one direct each"));
        };
        let region = *region;
        let row = plan.region(region);
        if row.kind != RegionKind::Each || row.arms.len != 1 {
            return Err(invalid("collection needs one each arm"));
        }
        if !matches!(
            u.sites.children(None, row.arms.iter().next()),
            [(_, Site::Node(_))]
        ) {
            return Err(invalid("collection row needs one flow root"));
        }
        traversal::validate_no_nested(plan, u.sites, region)?;
        let mut this = Box::new(Self {
            preview: None,
            view,
            region,
            index: HeightIndex::new(estimated_height).map_err(index_error)?,
            estimated_height,
            // Keep the original provisional pixel budget, capped at sixteen
            // rows. Actual nested-scrollport feedback determines the real window.
            bootstrap_rows: ((BOOTSTRAP_ROWS as f64 * ESTIMATED_HEIGHT / estimated_height)
                .ceil()
                .clamp(1.0, BOOTSTRAP_ROWS as f64)) as usize,
            items: Rc::from([]),
            keys: Vec::new(),
            dups: BTreeMap::new(),
            string_keys: true,
            mounted: Vec::new(),
            spacers: Vec::new(),
            children: Vec::new(),
            revision: 0,
            next_epoch: 0,
            zero_heights: Default::default(),
            geometry: None,
            correction: None,
            follow_end,
            edge_handlers: [EventKind::Reachstart, EventKind::Reachend].map(|event| {
                descriptor
                    .handlers
                    .iter()
                    .any(|h| plan.handler(h).event == event)
            }),
            edge_armed: [true; 2],
            pending: false,
        });
        this.update_data(u, frames, true)?;
        Ok(Some(this))
    }
    /// Bring the rows up to date when the subject, the keys' other inputs or
    /// anything a mounted row reads changed; `fresh` on creation.
    pub(super) fn update_data(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        fresh: bool,
    ) -> Result<(), InstanceError> {
        let deps = &u.sites.deps;
        let index = self.region.0 as usize;
        let keys_stale = fresh || u.stale_outside(&deps.keys[index], 1);
        let data_changed = keys_stale || u.stale(&deps.subjects[index]);
        let body_changed = u.stale_outside(&deps.bodies[index], 1);
        if !data_changed && !body_changed {
            u.work.regions_skipped += 1;
            return Ok(());
        }
        // What an input actually changed, not the full evaluation's "all of
        // it", decides the protocol: ending a reorder preview, re-measuring
        // rows and the revision are the same in both modes.
        let rows_changed =
            u.changed_outside(&deps.keys[index], 1) || u.changed_outside(&deps.bodies[index], 1);
        let changed = fresh || rows_changed || u.changed_outside(&deps.subjects[index], 0);
        if changed {
            self.end_preview(u)?;
        }
        let anchor = if changed { self.anchor()? } else { None };
        // Items that changed in place: the same keys in the same order, and
        // no input of the rows' bodies or keys changed (a live tick's prices).
        let mut in_place: Option<Vec<usize>> = None;
        if data_changed {
            let descriptor = u.env.plan.region(self.region);
            let Value::List(items) = u.eval(descriptor.subject, frames)? else {
                return Err(InstanceError::SubjectKind {
                    region: self.region,
                });
            };
            let compare_previous = !fresh && items.len() == self.items.len();
            // A different length with the same key inputs: keep the keys of
            // the items that are the same objects, key the rest.
            let shared = !fresh
                && !keys_stale
                && !compare_previous
                && self.rekey_shared(u, frames, descriptor.key, &items)?;
            let mut rekeyed = true;
            if !shared {
                rekeyed = self.rekey_full(
                    u,
                    frames,
                    descriptor.key,
                    &items,
                    compare_previous,
                    keys_stale,
                )?;
            }
            if compare_previous && !keys_stale && !rekeyed && !rows_changed {
                in_place = Some(
                    (0..items.len())
                        .filter(|&p| !crate::compare::same(&items[p], &self.items[p]))
                        .collect(),
                );
            }
            self.items = items;
        }
        let previous = in_place.as_ref().map(|_| self.snapshot());
        if changed {
            match &in_place {
                // Only the changed rows' heights become estimates: the rows
                // that show and did not change keep their measurements, so
                // the host measures again only what changed.
                Some(positions) => self.invalidate_rows(positions)?,
                // O(1): old heights remain estimates; stale measurements cannot confirm them.
                None => self.invalidate_height_estimates()?,
            }
            if self.index.len() == 0 {
                self.edge_armed = [true; 2];
            }
            self.restore(anchor)?;
        }
        self.realize_window(u, frames, true, CollectionFill::default())?;
        if changed {
            let unchanged = previous.is_some_and(|mut before| {
                let now = self.snapshot();
                before.scroll_sequence = now.scroll_sequence;
                before == now
            });
            if !unchanged {
                advance(&mut self.revision)?;
            }
        }
        Ok(())
    }
    /// Keyed invalidation of the rows at `positions` (O(k log N)); a
    /// confirmed zero among them goes back to the estimate, as in
    /// [`Self::invalidate_height_estimates`].
    fn invalidate_rows(&mut self, positions: &[usize]) -> Result<(), InstanceError> {
        for &p in positions {
            let key = self.index.shared_key(p).unwrap().clone();
            if self.zero_heights.remove(&*key) {
                if let Some(token) = self.index.measurement_token(&key) {
                    self.index
                        .set_measured_height(&key, token, self.estimated_height)
                        .map_err(index_error)?;
                }
            }
            self.index.invalidate_row(&key).map_err(index_error)?;
        }
        Ok(())
    }
    /// Key every item, reusing a key only where the list kept its length and
    /// an item its position ([`Self::rekey_shared`] handles the rest).
    /// Whether any key changed.
    fn rekey_full(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key_code: exact_plan::Code,
        items: &[Value],
        compare_previous: bool,
        keys_stale: bool,
    ) -> Result<bool, InstanceError> {
        let reuse_items = compare_previous && !keys_stale;
        // No candidate N-vector or canonical strings until an actual key
        // mismatch. All equal results reuse the existing uniqueness proof.
        let mut changed = !compare_previous;
        let mut keys = Vec::new();
        let mut text_keys = Vec::new();
        if changed {
            keys.reserve(items.len());
            text_keys.reserve(items.len());
        }
        let mut unique = std::collections::BTreeSet::new();
        let mut dups = BTreeMap::new();
        let mut inner = frames.to_vec();
        inner.push(Frame::default());
        for (position, item) in items.iter().enumerate() {
            let key = if reuse_items && crate::compare::same(item, &self.items[position]) {
                if !changed {
                    continue;
                }
                self.keys[position].clone()
            } else {
                u.work.rows_keyed += 1;
                inner.last_mut().unwrap().item = Some(item.clone());
                inner.last_mut().unwrap().index = Some(position);
                u.eval(key_code, &inner)?
            };
            if !changed && same_key(&key, &self.keys[position]) {
                continue;
            }
            if !changed {
                // Seed only the already-validated prefix. Checking each
                // following row now preserves duplicate-before-later-trap.
                keys.reserve(items.len());
                text_keys.reserve(items.len());
                for prefix in 0..position {
                    let text = self.index.shared_key(prefix).unwrap().clone();
                    unique.insert(text.to_string());
                    text_keys.push(text);
                    keys.push(self.keys[prefix].clone());
                }
                dups.extend(self.dups.range(..position).map(|(p, d)| (*p, *d)));
                changed = true;
            }
            let text = key_text(&key).ok_or(InstanceError::KeyKind {
                region: self.region,
            })?;
            // A repeated key is the data's error: the repeat takes the
            // next identity in order (as an `each` does).
            let mut dup = 0;
            let mut ident = text.clone();
            while unique.contains(&ident) {
                dup += 1;
                ident = super::disambiguate(text.clone(), dup);
            }
            unique.insert(ident.clone());
            if dup > 0 {
                dups.insert(position, dup);
            }
            keys.push(key);
            text_keys.push(Rc::from(ident));
        }
        if changed {
            self.index.replace_keys(text_keys).map_err(index_error)?;
            self.string_keys = keys.iter().all(|key| key.as_str().is_some());
            self.keys = keys;
            self.dups = dups;
        }
        Ok(changed)
    }
    fn anchor(&self) -> Result<Option<index::Anchor>, InstanceError> {
        self.geometry
            .as_ref()
            .map(|g| {
                self.index
                    .capture_anchor(
                        g.scroll_top,
                        g.port_height,
                        self.follow_end && self.preview.is_none(),
                    )
                    .map_err(index_error)
            })
            .transpose()
    }
    fn restore(&mut self, anchor: Option<index::Anchor>) -> Result<(), InstanceError> {
        if let (Some(anchor), Some(g)) = (anchor, &mut self.geometry) {
            let corrected = self
                .index
                .restore_anchor(&anchor, g.port_height)
                .map_err(index_error)?;
            if (corrected - g.scroll_top).abs() > 0.01 {
                self.correction = Some(AnchorCorrection {
                    scroll_sequence: g.scroll_sequence,
                    scroll_top: corrected,
                });
                g.scroll_top = corrected;
            }
        }
        Ok(())
    }
    fn pin(&self, view: Option<ViewId>) -> Option<String> {
        let view = view?;
        if let Some(p) = self
            .preview
            .as_ref()
            .filter(|p| p.pin_owned && p.handle == view)
        {
            if self.index.position(&p.source).is_some() {
                return Some(p.source.clone());
            }
        }
        self.mounted
            .iter()
            .find(|row| row.wrapper == view || super::find::contains(&row.row.roots, view))
            .and_then(|row| super::ident(&row.row.key, row.row.dup))
    }
    /// Pins never qualify an edge. Re-arm only after the geometric window is
    /// measured: replacement estimates cannot manufacture a temporary edge exit.
    fn geometric_edges(&mut self) -> Result<[bool; 2], InstanceError> {
        let mut reached = [false; 2];
        if let Some(g) = &self.geometry {
            if self.index.len() > 0 {
                let window = self
                    .index
                    .window(g.scroll_top, g.port_height, [None, None])
                    .map_err(index_error)?;
                reached = [0, self.index.len() - 1]
                    .map(|i| window.segments.iter().any(|range| range.contains(&i)));
                if self.edge_armed != [true; 2] && self.index.range_measured(window.overscan) {
                    for (armed, reached) in self.edge_armed.iter_mut().zip(reached) {
                        *armed |= !reached;
                    }
                }
            }
        }
        Ok(reached)
    }
    /// Realize the window: every row it owes (visible and pinned), then, on
    /// a limited report, at most `fill.limit` more, nearest the viewport on
    /// the side of travel first, retiring at most `max(2·limit, 4)` rows past
    /// the window (none for a limit of zero), farthest first, more when the
    /// rows kept past the window would outnumber the window's own, and every
    /// row more than two viewports from what shows (@ref LLP 1050.000 §6). A
    /// data update realizes the whole window. What a limit leaves undone is
    /// `pending`.
    fn realize_window(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        update: bool,
        fill: CollectionFill,
    ) -> Result<(), InstanceError> {
        let limit = fill.limit.filter(|_| !update);
        let mut owed: Vec<std::ops::Range<usize>> = Vec::new();
        let mut port = None;
        let ranges = if let Some(g) = &self.geometry {
            let focus = self.pin(g.focus_view);
            let interaction = self
                .preview
                .as_ref()
                .filter(|p| p.pin_owned)
                .map(|p| p.source.clone())
                .or_else(|| self.pin(g.interaction_view));
            let window = self
                .index
                .window_led(
                    g.scroll_top,
                    g.port_height,
                    lead(g.port_height, fill.velocity),
                    [focus.as_deref(), interaction.as_deref()],
                )
                .map_err(index_error)?;
            owed.push(window.visible.clone());
            for key in [&focus, &interaction].into_iter().flatten() {
                if let Some(i) = self.index.position(key) {
                    owed.push(i..i + 1);
                }
            }
            port = Some((window.offset, window.offset + g.port_height));
            window.segments
        } else {
            let first = self.index.row_at(0.0).map_err(index_error)?.unwrap_or(0);
            std::iter::once(first..self.index.len().min(first + self.bootstrap_rows)).collect()
        };
        let mut old: BTreeMap<String, Mounted> = std::mem::take(&mut self.mounted)
            .into_iter()
            .map(|row| {
                (
                    super::ident(&row.row.key, row.row.dup).expect("validated"),
                    row,
                )
            })
            .collect();
        let limited = limit.zip(port);
        let is_owed = |p: usize| owed.iter().any(|r| r.contains(&p));
        let toward_start = fill.velocity < 0.0;
        let mut pending = false;
        let mut admitted = std::collections::BTreeSet::new();
        if let Some((limit, (top, end))) = limited {
            let mut optional = Vec::new();
            for p in ranges.iter().cloned().flatten() {
                if !is_owed(p) && !old.contains_key(self.index.key(p).unwrap()) {
                    let (before, distance) = self.distance(p, top, end);
                    optional.push((before != toward_start, distance, p));
                }
            }
            optional.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
            pending = optional.len() > limit as usize;
            admitted.extend(optional.into_iter().take(limit as usize).map(|(_, _, p)| p));
        }
        for position in ranges.into_iter().flatten() {
            let text = self.index.key(position).unwrap().to_owned();
            let mounted = match old.remove(&text) {
                Some(mut mounted) => {
                    let dirty = self.reposition(&mut mounted, position);
                    if update {
                        let body = &u.sites.deps.bodies[self.region.0 as usize];
                        update_row(u, &mut mounted.row, frames, dirty, body)?;
                    } else {
                        u.work.rows_reused += 1;
                    }
                    mounted
                }
                None => {
                    if limited.is_some() && !is_owed(position) && !admitted.contains(&position) {
                        continue;
                    }
                    let token = self.index.invalidate_row(&text).map_err(index_error)?;
                    let row = self.create_row(u, position, frames)?;
                    let wrapper = views::row_wrapper(u, roots_of(&row.roots), &text)?;
                    Mounted {
                        position,
                        wrapper,
                        epoch: advance(&mut self.next_epoch)?,
                        token,
                        preview_target: None,
                        published: (usize::MAX, usize::MAX),
                        row,
                    }
                }
            };
            self.settle_mounted(u, mounted, &text)?;
        }
        // Rows past the window: all retire, unless a limited report bounds it.
        let mut leaving: Vec<(f64, String, Mounted)> = Vec::new();
        for (text, mounted) in old {
            match (limited, self.index.position(&text)) {
                (Some((_, (top, end))), Some(p)) => {
                    leaving.push((self.distance(p, top, end).1, text, mounted))
                }
                (_, position) => {
                    if position.is_none() {
                        views::item_left(u, mounted.wrapper);
                    }
                    u.ops.push(Op::DestroyView {
                        id: mounted.wrapper,
                    })
                }
            }
        }
        if let Some((limit, (top, end))) = limited {
            // A rescue (no limit past what shows) only builds; slices retire,
            // but a row two viewports past what shows always goes: travel
            // faster than slices retire must not keep every row it passed.
            let cap = if limit == 0 {
                0
            } else {
                (2 * limit as usize).max(4)
            };
            // Rows kept past the window never outnumber the window's own.
            // Travel that outruns the fill makes every report a rescue, and
            // a rescue retires nothing: without this bound each one added a
            // viewport of rows (a thousand on an iPad at 48,000 pt/s, each
            // with its canvas's Metal layer and surface) until the list
            // stopped, and every report and frame walked them all.
            let cap = cap.max(leaving.len().saturating_sub(self.mounted.len()));
            leaving.sort_by(|a, b| b.0.total_cmp(&a.0));
            let far = leaving.partition_point(|row| row.0 > FAR_VIEWPORTS * (end - top));
            let kept = leaving.split_off(far.max(cap).min(leaving.len()));
            for (_, _, gone) in leaving {
                u.ops.push(Op::DestroyView { id: gone.wrapper });
            }
            pending |= !kept.is_empty();
            for (_, text, mut mounted) in kept {
                let position = self.index.position(&text).unwrap();
                self.reposition(&mut mounted, position);
                self.settle_mounted(u, mounted, &text)?;
            }
            self.mounted.sort_by_key(|row| row.position);
        }
        self.pending = pending;
        self.emit_children(u)?;
        self.emit_preview(u)?;
        Ok(())
    }
    /// Move a mounted row to `position`; which fields of its item changed
    /// ([`crate::compare::changed_fields`]), every one when its position
    /// changed (a row that moved reads its new one, LLP 1062 D8). An
    /// equivalent item keeps its object for nested memos.
    fn reposition(&self, mounted: &mut Mounted, position: usize) -> u64 {
        mounted.position = position;
        let item = Some(self.items[position].clone());
        let dirty = crate::compare::changed_fields(&mounted.row.frame.item, &item);
        if dirty != 0 {
            mounted.row.frame.item = item;
        }
        let moved = mounted.row.frame.index != Some(position);
        mounted.row.frame.index = Some(position);
        if moved {
            !0
        } else {
            dirty
        }
    }
    /// Publish a row's position and measurement epoch, and mount it.
    fn settle_mounted(
        &mut self,
        u: &mut Update<'_>,
        mut mounted: Mounted,
        text: &str,
    ) -> Result<(), InstanceError> {
        views::validate_row(u.env.plan, &mounted.row.roots)?;
        let position = mounted.position;
        let count = self.index.len();
        if mounted.published != (position, count) {
            views::publish_position(u, mounted.wrapper, position, count);
            mounted.published = (position, count);
        }
        let token = self.index.measurement_token(text).unwrap();
        if token != mounted.token {
            mounted.token = token;
            mounted.epoch = advance(&mut self.next_epoch)?;
        }
        self.mounted.push(mounted);
        Ok(())
    }
    /// Whether row `p` lies before the port, and its distance from it.
    fn distance(&self, p: usize, top: f64, end: f64) -> (bool, f64) {
        let start = self.index.prefix(p).unwrap();
        let finish = start + self.index.height(p).unwrap();
        if finish <= top {
            (true, top - finish)
        } else {
            (false, (start - end).max(0.0))
        }
    }
    fn create_row(
        &self,
        u: &mut Update<'_>,
        position: usize,
        frames: &[Frame],
    ) -> Result<Row, InstanceError> {
        let plan = u.env.plan;
        let slots: RowSlots = Rc::new(RefCell::new(BTreeMap::new()));
        let frame = Frame {
            item: Some(self.items[position].clone()),
            index: Some(position),
            bound: None,
            region: Some(self.region.0),
            row: Some(slots.clone()),
        };
        let mut inner = frames.to_vec();
        inner.push(frame.clone());
        for (i, s) in plan.slots.iter().enumerate() {
            if s.owner == Some(self.region) {
                let value = u.eval(s.init, &inner)?;
                if !value.conforms(plan, s.ty) {
                    return Err(InstanceError::SlotType {
                        slot: plan.str(s.name).to_string(),
                    });
                }
                slots.borrow_mut().insert(i as u32, value);
            }
        }
        let roots = realize(u, None, plan.region(self.region).arms.iter().next(), &inner)?;
        let dup = self.dups.get(&position).copied().unwrap_or(0);
        if dup > 0 {
            let ident = self.index.key(position).unwrap_or_default();
            u.notes
                .push(super::repeated(self.region, &self.keys[position], ident));
        }
        Ok(Row {
            dup,
            key: self.keys[position].clone(),
            frame,
            roots,
            slots,
        })
    }
    /// Preflight the entire report before releasing any other collection's pins.
    fn prepare_feedback(
        &self,
        feedback: &CollectionFeedback,
    ) -> Result<Option<BTreeMap<ViewId, usize>>, InstanceError> {
        feedback
            .validate()
            .map_err(|_| invalid("invalid collection feedback"))?;
        if feedback.view != self.view
            || feedback.revision != self.revision
            || self
                .geometry
                .as_ref()
                .is_some_and(|g| feedback.scroll_sequence < g.scroll_sequence)
        {
            return Ok(None);
        }
        if [feedback.focus_view, feedback.interaction_view]
            .into_iter()
            .flatten()
            .any(|view| self.pin(Some(view)).is_none())
        {
            return Ok(None);
        }
        // Reject batches mentioning retired/unpublished wrappers or epochs atomically.
        let by_view: BTreeMap<_, _> = self
            .mounted
            .iter()
            .enumerate()
            .map(|(i, r)| (r.wrapper, i))
            .collect();
        if feedback.measurements.iter().any(|m| {
            by_view
                .get(&m.view)
                .is_none_or(|i| self.mounted[*i].epoch != m.epoch)
        }) {
            return Ok(None);
        }
        let changed_width = self
            .geometry
            .as_ref()
            .is_none_or(|g| g.row_width != feedback.row_width);
        if !changed_width {
            // Each height can fit the kernel's f32 geometry while their sum
            // does not. Validate the complete replacement against retained
            // heights before changing epochs, geometry, the index or view IDs.
            // Only mounted rows are visited; no clone/scan of the N-row index.
            let mut removed = 0.0;
            let mut added = 0.0;
            for measurement in &feedback.measurements {
                let position = self.mounted[by_view[&measurement.view]].position;
                removed += self.index.height(position).unwrap();
                added += measurement.height;
            }
            let extent = (self.index.total_height() - removed).max(0.0) + added;
            if !extent.is_finite() || extent > f32::MAX as f64 {
                return Err(InstanceError::InvalidCollectionFeedback);
            }
        }
        Ok(Some(by_view))
    }
    /// Called only after preflight and release of the transferred pin categories.
    fn feedback(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        mut feedback: CollectionFeedback,
        by_view: &BTreeMap<ViewId, usize>,
        mut fill: CollectionFill,
    ) -> Result<(bool, Option<CollectionEdges>), InstanceError> {
        if let Some(edge) = self.travel_within(u, &feedback, by_view, fill)? {
            return Ok((false, edge));
        }
        let changed_width = self
            .geometry
            .as_ref()
            .is_none_or(|g| g.row_width != feedback.row_width);
        // A new port size, width or pin retires every row that left and
        // builds the whole window: only travel is sliced.
        if self.geometry.as_ref().is_none_or(|g| {
            (
                g.port_width,
                g.port_height,
                g.row_width,
                g.focus_view,
                g.interaction_view,
            ) != (
                feedback.port_width,
                feedback.port_height,
                feedback.row_width,
                feedback.focus_view,
                feedback.interaction_view,
            )
        }) {
            fill.limit = None;
        }
        if changed_width {
            self.end_preview(u)?;
        }
        if self
            .geometry
            .as_ref()
            .is_some_and(|g| g.interaction_view != feedback.interaction_view)
        {
            self.lose_preview_pin(u)?;
        }
        let previous = self.snapshot();
        let anchor_height = self
            .geometry
            .as_ref()
            .map_or(feedback.port_height, |g| g.port_height);
        let anchor = Some(
            self.index
                .capture_anchor(
                    feedback.scroll_top,
                    anchor_height,
                    self.follow_end && self.preview.is_none(),
                )
                .map_err(index_error)?,
        );
        self.geometry = Some(CollectionFeedback {
            measurements: Vec::new(),
            ..feedback.clone()
        });
        self.correction = None;
        if changed_width {
            self.invalidate_height_estimates()?;
        }
        // Measurements from the previous width are deliberately discarded. A fresh
        // snapshot supplies the new epoch for the next post-layout feedback.
        if !changed_width {
            for measurement in feedback.measurements.drain(..) {
                let row = &self.mounted[by_view[&measurement.view]];
                let key = self.index.key(row.position).unwrap().to_owned();
                self.index
                    .set_measured_height(&key, row.token, measurement.height)
                    .map_err(index_error)?;
                if measurement.height == 0.0 {
                    self.zero_heights.insert(key);
                } else {
                    self.zero_heights.remove(&key);
                }
            }
        }
        self.check_preview_height(u)?;
        self.restore(anchor)?;
        self.realize_window(u, frames, false, fill)?;
        let mut now = self.snapshot();
        // Receiving a newer sequence without changing rows/extent/correction is
        // a fact update, not a new frame (avoids post-layout feedback loops).
        now.scroll_sequence = previous.scroll_sequence;
        let changed = previous != now;
        if changed {
            advance(&mut self.revision)?;
        }
        Ok((changed, self.edge_event()?))
    }
    /// The edge a report's geometry reached, if armed and handled.
    fn edge_event(&mut self) -> Result<Option<CollectionEdges>, InstanceError> {
        let reached = self.geometric_edges()?;
        let ready = [0, 1].map(|i| reached[i] && self.edge_armed[i] && self.edge_handlers[i]);
        let edge = (0..2).find(|&i| ready[i]);
        Ok(edge.map(|i| {
            self.edge_armed[i] = false;
            CollectionEdges {
                first: [EventKind::Reachstart, EventKind::Reachend][i],
                // End remains armed until an action actually consumes it.
                end_after_noop: ready[0] && ready[1],
            }
        }))
    }
    /// Travel inside the realized window, which is most reports while a list
    /// moves: the same port, pins and heights, no measurement, nothing owed,
    /// no correction before or after, and the window it leads to is the
    /// mounted rows. Realizing it would reuse every row and emit nothing, so
    /// only the geometry moves (@ref LLP 1050.000 §6). None: realize.
    fn travel_within(
        &mut self,
        u: &mut Update<'_>,
        feedback: &CollectionFeedback,
        by_view: &BTreeMap<ViewId, usize>,
        fill: CollectionFill,
    ) -> Result<Option<Option<CollectionEdges>>, InstanceError> {
        let Some(g) = &self.geometry else {
            return Ok(None);
        };
        // A host reports every mounted row's height each time; one the
        // index already holds, measured, changes nothing (`feedback`'s own
        // loop would set it again).
        let remeasures = |m: &RowMeasurement| {
            let row = &self.mounted[by_view[&m.view]];
            let key = self.index.key(row.position).unwrap();
            self.index.measurement_token(key) == Some(row.token)
                && !(self.index.is_measured(key)
                    && self.index.height(row.position) == Some(m.height)
                    && (m.height == 0.0) == self.zero_heights.contains(key))
        };
        if feedback.measurements.iter().any(remeasures)
            || self.pending
            || self.preview.is_some()
            || self.correction.is_some()
            || (
                g.port_width,
                g.port_height,
                g.row_width,
                g.focus_view,
                g.interaction_view,
            ) != (
                feedback.port_width,
                feedback.port_height,
                feedback.row_width,
                feedback.focus_view,
                feedback.interaction_view,
            )
        {
            return Ok(None);
        }
        let anchor = self
            .index
            .capture_anchor(feedback.scroll_top, g.port_height, self.follow_end)
            .map_err(index_error)?;
        let corrected = self
            .index
            .restore_anchor(&anchor, g.port_height)
            .map_err(index_error)?;
        if (corrected - feedback.scroll_top).abs() > 0.01 {
            return Ok(None);
        }
        let (focus, interaction) = (self.pin(g.focus_view), self.pin(g.interaction_view));
        let window = self
            .index
            .window_led(
                feedback.scroll_top,
                feedback.port_height,
                lead(feedback.port_height, fill.velocity),
                [focus.as_deref(), interaction.as_deref()],
            )
            .map_err(index_error)?;
        let mut mounted = self.mounted.iter().map(|row| row.position);
        let same = window
            .segments
            .iter()
            .cloned()
            .flatten()
            .all(|p| mounted.next() == Some(p))
            && mounted.next().is_none();
        if !same {
            return Ok(None);
        }
        u.work.rows_reused += self.mounted.len();
        self.geometry = Some(CollectionFeedback {
            measurements: Vec::new(),
            ..feedback.clone()
        });
        Ok(Some(self.edge_event()?))
    }
    fn release_pins(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        categories: [bool; 2],
    ) -> Result<bool, InstanceError> {
        if categories[1] {
            self.lose_preview_pin(u)?;
        }
        let Some(g) = &mut self.geometry else {
            return Ok(false);
        };
        let changed = (categories[0] && g.focus_view.is_some())
            || (categories[1] && g.interaction_view.is_some());
        if !changed {
            return Ok(false);
        }
        if categories[0] {
            g.focus_view = None;
        }
        if categories[1] {
            g.interaction_view = None;
        }
        self.realize_window(u, frames, false, CollectionFill::default())?;
        // Invalidate queued reports that still claim the former ownership,
        // even when its old pinned row happens to remain inside the window.
        advance(&mut self.revision)?;
        Ok(true)
    }
    fn snapshot(&self) -> CollectionSnapshot {
        CollectionSnapshot {
            view: self.view,
            revision: self.revision,
            scroll_sequence: self.geometry.as_ref().map_or(0, |g| g.scroll_sequence),
            count: self.index.len(),
            total_extent: self.index.total_height(),
            rows: self
                .mounted
                .iter()
                .map(|row| CollectionRow {
                    view: row.wrapper,
                    root: roots_of(&row.row.roots)[0],
                    index: row.position,
                    top: self.index.prefix(row.position).unwrap(),
                    height: self.index.height(row.position).unwrap(),
                    epoch: row.epoch,
                    measured: self
                        .index
                        .is_measured(self.index.key(row.position).unwrap()),
                })
                .collect(),
            correction: self.correction,
            pending: self.pending,
        }
    }
}
