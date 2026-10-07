//! Bounded keyed row realization, driven by actual nested-scrollport feedback.
//! @ref LLP 1010 §6 / LLP 1041 §8. No historical instance or row-state cache.
mod api;
mod index;
mod inset;
pub(crate) use inset::Insets;
mod into_view;
mod nest;
mod rekey;
mod reorder;
mod reorder_api;
mod reorder_group;
mod reuse;
pub(crate) mod shown;
use shown::in_collection_row;
mod start;
#[cfg(test)]
mod tests;
mod traversal;
pub(crate) mod views;
mod within;
use super::*;
pub use api::*;
use exact_kernel::PropId;
use exact_plan::EventKind;
use index::{MeasurementToken, SizeIndex};
pub use into_view::{Align, IntoView, IntoViewStatus};
pub use reorder_api::*;
pub(super) use traversal::{collections_json, invalidate_typography};

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
    let k = f64::from_bits(LEAD_SCALE.load(std::sync::atomic::Ordering::Relaxed));
    if velocity > 0.0 {
        [viewport * k, (viewport + extra) * k]
    } else {
        [(viewport + extra) * k, viewport * k]
    }
}

/// 1.0's bits: the process's lead scale (a host may boot on one thread and
/// run on another).
static LEAD_SCALE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x3FF0_0000_0000_0000);

/// How much of a window's lead past its viewport collections realize: 1,
/// the default, all of it; a host drawing its first frame may ask for 0 (the
/// rows that show), then 1 once that frame is out, so the rows past the
/// viewport mount after the first frame instead of before it.
pub fn set_lead_scale(scale: f64) {
    LEAD_SCALE.store(
        scale.clamp(0.0, 1.0).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// The provisional extent (px along the axis) a list with no port yet
/// realizes rows for: by default sixteen 32 px rows' worth; a host that knows
/// its viewport sets it ([`set_bootstrap_extent`]).
static BOOTSTRAP_EXTENT: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new((BOOTSTRAP_ROWS as f64 * ESTIMATED_HEIGHT).to_bits());

/// How far a new list's first rows reach before its port is reported: a
/// host passes its viewport's extent, so the rows that can show are built
/// with the list instead of in a second commit after the first layout (at
/// most sixteen rows; a list's `initial-item-count` still wins).
pub fn set_bootstrap_extent(px: f64) {
    if px.is_finite() && px > 0.0 {
        BOOTSTRAP_EXTENT.store(px.to_bits(), std::sync::atomic::Ordering::Relaxed);
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
    /// Hidden while a ghost stands for it (LLP 1094 D6).
    preview_hidden: bool,
    /// The position and count the wrapper last published (`aria-posinset`,
    /// `aria-setsize`), for hosts that select and copy across rows.
    published: (usize, usize),
    row: Row,
    /// Mounted out of the port and not shown since ([`shown`]).
    awaiting: bool,
    /// Past the window, kept for the next row it needs ([`reuse`]).
    held: bool,
}
#[derive(Debug)]
pub(crate) struct Collection {
    preview: Option<reorder::Preview>,
    /// The gap a grouped session opens here as its target (LLP 1094 D4).
    incoming: Option<reorder_group::Incoming>,
    /// The dragged row's identity while a ghost stands for it (D6).
    hidden: Option<String>,
    /// Emit preview offsets at once: the rows moved with them (D8).
    instant: bool,
    view: ViewId,
    /// Fixed at creation from the list's style (LLP 1070 H1).
    axis: ListAxis,
    region: RegionsId,
    index: SizeIndex,
    estimated_height: f64,
    bootstrap_rows: usize,
    /// The list's literal size along its axis, when it declares one.
    declared_port: Option<f64>,
    items: Items,
    keys: Vec<Value>,
    /// Positions whose key repeats an earlier one, and which repeat.
    dups: BTreeMap<usize, u32>,
    string_keys: bool,
    mounted: Vec<Mounted>,
    any_awaiting: bool,
    spacers: Vec<(ViewId, f64)>,
    children: Vec<ViewId>,
    revision: u64,
    /// Moves only when the list's keys changed (an item put in, taken out or
    /// moved), never for its window, a measurement or a scroll: a host tells
    /// authored row moves from the list's own by it, with a row's own resize
    /// ([`Tree::collection_data`]).
    data_generation: u64,
    next_epoch: u64,
    zero_heights: std::collections::BTreeSet<String>,
    geometry: Option<CollectionFeedback>,
    correction: Option<AnchorCorrection>,
    follow_end: bool,
    /// The list's `scroll-behavior: smooth`: its end-follow and its smooth
    /// `scrollIntoView` corrections ask the host to animate (LLP 1070.000 §6.2).
    smooth: bool,
    edge_handlers: [bool; 2],
    reorderable: bool,
    edge_armed: [bool; 2],
    /// The last realization left window rows unbuilt or kept rows past it.
    pending: bool,
    /// The outer list whose row holds this one (LLP 1070 N1).
    parent: Option<ViewId>,
    /// Whether this list's rows can hold virtualized lists.
    nested: bool,
    /// Where in a row they can be (`nested` lists only).
    inner: Option<Rc<traversal::InnerSites>>,
    /// The pins inside its rows, as of a [`nest::PinEpoch`] count.
    inner_pins: std::cell::Cell<Option<(u64, [Option<ViewId>; 2])>>,
    /// Where its inner lists were when their rows left (Q1 as ruled).
    kept: nest::Kept,
    /// `scroll-restoration: manual`: the app keeps this list's position.
    manual: bool,
    /// This list started where a kept position said.
    restored: bool,
    /// That position, until a host reports one.
    restored_at: Option<(Rc<str>, f64)>,
    /// A `scrollIntoView` under way (LLP 1070.000).
    target: Option<into_view::Target>,
    /// The latest request here and how it stands, for `state`.
    into_view_status: Option<(Rc<str>, IntoViewStatus)>,
    /// Where the window starts before the host reports: 0, a restored
    /// position, or the end.
    start_offset: f64,
    /// `scroll-start: end`, until the list has opened there.
    at_end: bool,
    /// Consecutive reports that said the port was travelling, while it opens.
    end_travel: u8,
    /// The followed end last sent as a correction (`start::at_target`).
    end_sent: f64,
    /// What a retiring row may be rebound to another item under (LLP 1078):
    /// `None` when no row of this list can be.
    reuse: Option<Rc<reuse::Reuse>>,
    /// The end padding the next report brings ([`Collection::set_insets`]).
    trailing_next: Option<f64>,
    /// The padding before the first row, and the scroll padding at each
    /// end, along the axis (@ref LLP 1010 §6.9).
    leading: f64,
    scroll_padding: [f64; 2],
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
        (a @ exact_plan::str_value!(), b @ exact_plan::str_value!()) => a.text() == b.text(),
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
impl Drop for Collection {
    fn drop(&mut self) {
        // A list holding a pin goes: its outer list's cached pins are stale.
        if self
            .geometry
            .as_ref()
            .is_some_and(|g| g.focus_view.is_some() || g.interaction_view.is_some())
        {
            nest::PinEpoch::bump();
        }
    }
}

impl Collection {
    /// A host report becomes the geometry; a change of its pins moves the
    /// pin count ([`nest::PinEpoch`]).
    fn set_geometry(&mut self, g: CollectionFeedback) {
        let pins = |g: Option<&CollectionFeedback>| g.map(|g| (g.focus_view, g.interaction_view));
        if pins(self.geometry.as_ref()) != pins(Some(&g)) {
            nest::PinEpoch::bump();
        }
        self.geometry = Some(g);
    }

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
        let mut smooth = false;
        let mut at_end = false;
        let mut estimated_height = ESTIMATED_HEIGHT;
        let mut initial: Option<usize> = None;
        let mut axis = ListAxis::Vertical;
        // The list's own literal size on each axis (`height=399`), which
        // bounds the rows its first frame needs: [vertical, horizontal].
        let mut declared: [Option<f64>; 2] = [None, None];
        for binding in descriptor.bindings.iter().map(|b| plan.binding(b)) {
            if binding.kind == BindingKind::Style {
                use exact_kernel::StyleId;
                let slot = match binding.id {
                    id if id == StyleId::Height as u16 || id == StyleId::MaxHeight as u16 => {
                        Some(0)
                    }
                    id if id == StyleId::Width as u16 || id == StyleId::MaxWidth as u16 => Some(1),
                    _ => None,
                };
                if let Some(slot) = slot {
                    if let Value::Number(n) = u.eval(binding.expr, frames)? {
                        if n.is_finite() && n > 0.0 {
                            declared[slot] = Some(declared[slot].map_or(n, |d: f64| d.min(n)));
                        }
                    }
                }
            }
            // A flex list is CSS's row (the compiler refuses the other
            // directions); its main axis is horizontal.
            if binding.kind == BindingKind::Style
                && binding.id == exact_kernel::StyleId::Display as u16
                && u.eval(binding.expr, frames)?.as_str() == Some("flex")
            {
                axis = ListAxis::Horizontal;
            }
            if binding.kind == BindingKind::Style
                && binding.id == exact_kernel::StyleId::ScrollBehavior as u16
                && u.eval(binding.expr, frames)?.as_str() == Some("smooth")
            {
                smooth = true;
            }
            if binding.kind == BindingKind::Prop && binding.id == PropId::Virtualized as u16 {
                enabled = u.eval(binding.expr, frames)? == Value::Bool(true);
            }
            if binding.kind == BindingKind::Prop && binding.id == PropId::ScrollFollowEnd as u16 {
                follow_end = u.eval(binding.expr, frames)? == Value::Bool(true);
            }
            if binding.kind == BindingKind::Prop && binding.id == PropId::ScrollStart as u16 {
                at_end = u.eval(binding.expr, frames)?.as_str() == Some("end");
            }
            if binding.kind == BindingKind::Prop
                && (binding.id == PropId::EstimatedItemHeight as u16
                    || binding.id == PropId::EstimatedItemWidth as u16)
            {
                let Value::Number(height) = u.eval(binding.expr, frames)? else {
                    return Err(invalid("estimated item height must be a number"));
                };
                if !height.is_finite() || height <= 0.0 {
                    return Err(invalid("estimated item height must be positive and finite"));
                }
                estimated_height = height;
            }
            if binding.kind == BindingKind::Prop && binding.id == PropId::InitialItemCount as u16 {
                let Value::Number(count) = u.eval(binding.expr, frames)? else {
                    return Err(invalid("initial item count must be a number"));
                };
                if !(count.fract() == 0.0 && (1.0..=64.0).contains(&count)) {
                    return Err(invalid(
                        "initial item count must be a whole number from 1 to 64",
                    ));
                }
                initial = Some(count as usize);
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
        let inner = traversal::validate_nesting(plan, u.sites, region)?;
        let reuse = reuse::Reuse::new(plan, u.sites, region);
        let nested = inner.is_some();
        let port = declared[match axis {
            ListAxis::Vertical => 0,
            ListAxis::Horizontal => 1,
        }];
        // A trap here poisons like every other binding's (LLP 1090 D6).
        let mut manual = false;
        for b in descriptor.bindings.iter().map(|b| plan.binding(b)) {
            if b.kind == BindingKind::Prop && b.id == PropId::ScrollRestoration as u16 {
                manual |= u.eval(b.expr, frames)?.as_str() == Some("manual");
            }
        }
        let mut this = Box::new(Self {
            preview: None,
            incoming: None,
            hidden: None,
            instant: false,
            view,
            axis,
            region,
            index: SizeIndex::new(estimated_height).map_err(index_error)?,
            estimated_height,
            // The provisional extent ([`set_bootstrap_extent`]), capped at
            // sixteen rows, unless the list says how many (`initial-item-count`).
            // Actual nested-scrollport feedback determines the real window.
            bootstrap_rows: initial.unwrap_or(
                ((f64::from_bits(BOOTSTRAP_EXTENT.load(std::sync::atomic::Ordering::Relaxed))
                    / estimated_height)
                    .ceil()
                    .min(
                        port.filter(|_| in_collection_row(plan, frames))
                            .map_or(f64::INFINITY, |p| (p / estimated_height).ceil() + 1.0),
                    )
                    .clamp(1.0, BOOTSTRAP_ROWS as f64)) as usize,
            ),
            declared_port: port,
            items: Items::default(),
            keys: Vec::new(),
            dups: BTreeMap::new(),
            string_keys: true,
            mounted: Vec::new(),
            any_awaiting: false,
            spacers: Vec::new(),
            children: Vec::new(),
            revision: 0,
            data_generation: 0,
            next_epoch: 0,
            zero_heights: Default::default(),
            geometry: None,
            correction: None,
            follow_end,
            smooth,
            edge_handlers: [EventKind::Reachstart, EventKind::Reachend].map(|event| {
                descriptor
                    .handlers
                    .iter()
                    .any(|h| plan.handler(h).event == event)
            }),
            reorderable: descriptor
                .handlers
                .iter()
                .any(|h| plan.handler(h).event == EventKind::Reorderdrop),
            edge_armed: [true; 2],
            pending: false,
            parent: None,
            nested,
            inner,
            inner_pins: Default::default(),
            kept: Default::default(),
            manual,
            restored: false,
            restored_at: None,
            target: None,
            into_view_status: None,
            start_offset: 0.0,
            at_end,
            end_travel: 0,
            end_sent: f64::NAN,
            reuse,
            trailing_next: None,
            leading: 0.0,
            scroll_padding: [0.0; 2],
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
        // A grouped session's offsets move with its rows, at once (LLP 1094
        // D8): the layout moved by what they gave.
        let instant = changed
            && (self.incoming.is_some() || self.preview.as_ref().is_some_and(|p| p.grouped));
        if changed {
            self.end_preview(u)?;
            self.unsettle_incoming();
        }
        self.instant = instant;
        let anchor = if changed { self.anchor()? } else { None };
        // Items that changed in place: the same keys in the same order, and
        // no input of the rows' bodies or keys changed (a live tick's prices).
        let mut in_place: Option<Vec<usize>> = None;
        let (last, count) = (self.last_key(), self.index.len());
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
            // `keys_stale` only says the keys were evaluated again (always,
            // in full evaluation); `rekeyed` says whether one changed. Asking
            // it here made the revision differ between modes when a re-ask
            // answered the same rows (exact-live, a build-time answer asked
            // again at data_ready, 2026-10-04).
            // A key put in, taken out or moved: the rows that move are the
            // data's (`data_generation`).
            if fresh || shared || rekeyed {
                self.data_generation += 1;
            }
            if compare_previous && !rekeyed && !rows_changed {
                in_place = Some(
                    (0..items.len())
                        .filter(|&p| !crate::compare::same(&items[p], &self.items[p]))
                        .collect(),
                );
            }
            self.items = items;
            self.forget_departed();
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
            // @ref LLP 1010 (2026-09-29 ruling, provisional) — rows that
            // arrived are a new end: the old last row is still here with rows
            // after it, or the list grew and kept it (a page inserted before
            // a trailing row that stays last, a feed's "loading" tail), so the
            // edge re-arms and a reader still there when a page lands is
            // offered the next one. An empty page, a replaced last row or a
            // window that slid past it re-arm nothing.
            let kept = last.as_deref().and_then(|key| self.index.position(key));
            if kept.is_some_and(|p| p + 1 < self.index.len() || self.index.len() > count) {
                self.edge_armed[1] = true;
            }
            self.restore(anchor)?;
        }
        self.start_at_end();
        let realized = self.realize_window(u, frames, true, CollectionFill::default());
        self.instant = false;
        realized?;
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
        // Membership only: hashed, and holding the identities the index
        // keeps (no copy of each key's text).
        // Each identity's position: the index takes it as its key map.
        let mut unique: std::collections::HashMap<Rc<str>, usize> = Default::default();
        if changed {
            unique.reserve(items.len());
        }
        let mut dups = BTreeMap::new();
        // Each key's text, written here before its identity is made.
        let mut text = String::new();
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
                unique.reserve(items.len());
                for prefix in 0..position {
                    let text = self.index.shared_key(prefix).unwrap().clone();
                    unique.insert(text.clone(), prefix);
                    text_keys.push(text);
                    keys.push(self.keys[prefix].clone());
                }
                dups.extend(self.dups.range(..position).map(|(p, d)| (*p, *d)));
                changed = true;
            }
            text.clear();
            if !super::key_text_into(&key, &mut text) {
                return Err(InstanceError::KeyKind {
                    region: self.region,
                });
            }
            // A repeated key is the data's error: the repeat takes the
            // next identity in order (as an `each` does).
            let mut dup = 0;
            let ident: Rc<str> = if unique.contains_key(text.as_str()) {
                let mut ident = text.clone();
                while unique.contains_key(ident.as_str()) {
                    dup += 1;
                    ident = super::disambiguate(text.clone(), dup);
                }
                Rc::from(ident)
            } else {
                Rc::from(text.as_str())
            };
            unique.insert(ident.clone(), position);
            if dup > 0 {
                dups.insert(position, dup);
            }
            keys.push(key);
            text_keys.push(ident);
        }
        if changed {
            self.index
                .replace_keys_indexed(text_keys, unique)
                .map_err(index_error)?;
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
                    .capture_anchor(self.anchor_offset(g.offset), g.port_main, self.follows())
                    .map_err(index_error)
            })
            .transpose()
    }
    fn restore(&mut self, anchor: Option<index::Anchor>) -> Result<(), InstanceError> {
        if let (Some(anchor), Some(g)) = (anchor, &mut self.geometry) {
            let corrected = self
                .index
                .restore_anchor(&anchor, g.port_main)
                .map_err(index_error)?;
            if !start::at_target(&anchor, corrected, g.offset, self.end_sent) {
                if index::SizeIndex::follows_end(&anchor) {
                    self.end_sent = corrected;
                }
                // Relative only where the anchor's row stayed put (an end
                // followed or clamped is absolute: the host's own clamp has
                // moved it). One not yet acknowledged by a report is still
                // the host's to apply: this one moves on from where it began.
                let from = match self.correction {
                    _ if !self.index.kept_row(&anchor, g.port_main, corrected) => None,
                    Some(c) if c.scroll_sequence == g.scroll_sequence => c.from,
                    Some(_) => None,
                    None => Some(g.offset),
                };
                // A followed end that moved, once the list has opened, is the
                // reader's own content arriving (a message sent): smooth if
                // the list says so.
                let smooth = self.smooth
                    && index::SizeIndex::follows_end(&anchor)
                    && !self.at_end
                    && from.is_none();
                self.correction = Some(AnchorCorrection {
                    scroll_sequence: g.scroll_sequence,
                    offset: corrected,
                    from,
                    smooth,
                });
                g.offset = corrected;
                if self.restored_at.is_some() {
                    // Where a restored list now expects the host to be.
                    self.start_offset = corrected;
                }
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
    /// The last supplied row's key, if any.
    fn last_key(&self) -> Option<String> {
        self.index
            .len()
            .checked_sub(1)
            .and_then(|i| self.index.key(i))
            .map(str::to_owned)
    }
    /// Pins never qualify an edge. Re-arm only after the geometric window is
    /// measured: replacement estimates cannot manufacture a temporary edge exit.
    fn geometric_edges(&mut self) -> Result<[bool; 2], InstanceError> {
        let mut reached = [false; 2];
        if let Some(g) = &self.geometry {
            if self.index.len() > 0 {
                let window = self
                    .index
                    .window(g.offset, g.port_main, [None, None])
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
            let pins = self.pins();
            let focus = self.pin(pins[0]);
            let interaction = self
                .preview
                .as_ref()
                .filter(|p| p.pin_owned)
                .map(|p| p.source.clone())
                .or_else(|| self.pin(pins[1]));
            let window = self
                .index
                .window_led(
                    g.offset,
                    g.port_main,
                    lead(g.port_main, fill.velocity),
                    [focus.as_deref(), interaction.as_deref()],
                )
                .map_err(index_error)?;
            owed.push(window.visible.clone());
            for key in [&focus, &interaction].into_iter().flatten() {
                if let Some(i) = self.index.position(key) {
                    owed.push(i..i + 1);
                }
            }
            port = Some((window.offset, window.offset + g.port_main));
            window.segments
        } else {
            let first = self.bootstrap_first()?;
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
        // A retire-only report builds what it owes and nothing optional.
        let building = if fill.no_build {
            port.map(|p| (0, p))
        } else {
            limited
        };
        if let Some((limit, (top, end))) = building {
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
        // With reuse, the rows nothing mounts are built once the retiring
        // rows are known, so one can be rebound to each (LLP 1078).
        let reusing = self.reusing(u);
        let mut needed: Vec<(usize, String)> = Vec::new();
        for position in ranges.into_iter().flatten() {
            let text = self.index.key(position).unwrap().to_owned();
            let mounted = match old.remove(&text) {
                Some(mut mounted) => {
                    mounted.held = false;
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
                    if building.is_some() && !is_owed(position) && !admitted.contains(&position) {
                        continue;
                    }
                    if reusing {
                        needed.push((position, text));
                        continue;
                    }
                    self.build_row(u, position, &text, frames)?
                }
            };
            self.settle_mounted(u, mounted, &text)?;
        }
        // A build-only report retires nothing (LLP 1072 §5): rows past the
        // window stay, and the report is pending until an immediate one.
        if fill.create_only && !update {
            self.build_needed(u, needed, Vec::new(), Vec::new(), None, frames)?;
            let mut kept = false;
            for (text, mut mounted) in old {
                match self.index.position(&text) {
                    Some(position) => {
                        kept = true;
                        self.reposition(&mut mounted, position);
                        self.settle_mounted(u, mounted, &text)?;
                    }
                    // An item that left the data went with the update that
                    // removed it; a build-only report never sees one.
                    None => {
                        debug_assert!(false, "a build-only report met a row whose item left");
                        views::item_left(u, mounted.wrapper);
                        u.ops.push(Op::DestroyView {
                            id: mounted.wrapper,
                        });
                    }
                }
            }
            self.mounted.sort_by_key(|row| row.position);
            self.pending = pending || kept;
            self.emit_children(u)?;
            self.emit_preview(u)?;
            return Ok(());
        }
        // Rows past the window: all retire, unless a limited report bounds it.
        let mut leaving: Vec<(f64, String, Mounted)> = Vec::new();
        let mut retiring: Vec<Mounted> = Vec::new();
        let mut kept: Vec<(f64, String, Mounted)> = Vec::new();
        for (text, mut mounted) in old {
            match (limited, self.index.position(&text)) {
                (Some((_, (top, end))), Some(p)) => {
                    leaving.push((self.distance(p, top, end).1, text, mounted))
                }
                (_, None) => {
                    views::item_left(u, mounted.wrapper);
                    u.ops.push(Op::DestroyView {
                        id: mounted.wrapper,
                    })
                }
                (_, Some(_)) => {
                    self.keep_positions(&mut mounted, &text);
                    retiring.push(mounted);
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
            let cap = cap.max(
                leaving
                    .len()
                    .saturating_sub(self.mounted.len() + needed.len()),
            );
            leaving.sort_by(|a, b| b.0.total_cmp(&a.0));
            let far = leaving.partition_point(|row| row.0 > FAR_VIEWPORTS * (end - top));
            kept = leaving.split_off(far.max(cap).min(leaving.len()));
            for (_, text, mut gone) in leaving {
                self.keep_positions(&mut gone, &text);
                retiring.push(gone);
            }
        }
        // Rows still needed after the retiring ones may take the kept rows
        // past the window, farthest first.
        let hold = port.filter(|_| reusing && !update);
        let kept = self.build_needed(u, needed, retiring, kept, hold, frames)?;
        pending |= !kept.is_empty();
        for (_, text, mut mounted) in kept {
            let position = self.index.position(&text).unwrap();
            self.reposition(&mut mounted, position);
            self.settle_mounted(u, mounted, &text)?;
        }
        self.mounted.sort_by_key(|row| row.position);
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
        debug_assert_eq!(self.index.key(position), Some(text));
        let token = self.index.measurement_token_at(position).unwrap();
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
    /// Build the row for `position` (key `text`) and its wrapper.
    fn build_row(
        &mut self,
        u: &mut Update<'_>,
        position: usize,
        text: &str,
        frames: &[Frame],
    ) -> Result<Mounted, InstanceError> {
        let token = self.index.invalidate_row(text).map_err(index_error)?;
        let mut row = self.create_row(u, position, frames)?;
        self.adopt_nested(u, &mut row, text, frames)?;
        let wrapper =
            views::row_wrapper(u, self.axis, roots_of(&row.roots), text, self.reorderable)?;
        let mut mounted = Mounted {
            position,
            wrapper,
            epoch: advance(&mut self.next_epoch)?,
            token,
            preview_target: None,
            preview_hidden: false,
            published: (usize::MAX, usize::MAX),
            row,
            awaiting: false,
            held: false,
        };
        self.mounted_shown(u, &mut mounted);
        Ok(mounted)
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
        let arm = plan.region(self.region).arms.iter().next();
        for (i, s) in plan.slots.iter().enumerate() {
            if s.owner.is_some() && s.owner == arm {
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
            .is_none_or(|g| g.cross != feedback.cross);
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
                added += measurement.size;
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
        // @ref LLP 1072 §5 — a build-only report keeps the port, width and
        // pins: a report that changes them may retire, so it is immediate.
        if fill.create_only
            && self.geometry.as_ref().is_none_or(|g| {
                (
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
            })
        {
            return Err(InstanceError::InvalidCollectionFeedback);
        }
        // @ref LLP 1010 §6.9 — a new end padding (a rotation's safe area)
        // is no travel: the anchor is taken on the old range and restored
        // on the new, so a followed end follows it.
        let trailing = self.trailing_next.take().unwrap_or(self.index.trailing());
        if trailing == self.index.trailing() {
            if let Some(edge) = self.travel_within(u, &feedback, by_view, fill)? {
                self.reveal_shown(u);
                return Ok((false, edge));
            }
        }
        let changed_width = self
            .geometry
            .as_ref()
            .is_none_or(|g| g.cross != feedback.cross);
        // A new port size, width or pin retires every row that left and
        // builds the whole window: only travel is sliced. A list's first
        // report retires nothing, so a slice there builds what shows and
        // leaves its lead pending (a tap's response frame, LLP 1072 §5).
        if self.geometry.as_ref().is_some_and(|g| {
            (
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
        }) {
            fill.limit = None;
        }
        // An enclosing list is moving (LLP 1070 F2): build what this list
        // owes and nothing else, first report and resize included.
        if fill.ancestor_moving {
            fill.limit = Some(0);
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
        self.follow_into_view(fill);
        // @ref LLP 1070 H4, Q3 (a): a row list anchors only an estimate
        // replaced by a first measurement, the jump virtualization makes. A
        // card measured again moves what follows it, as Chrome, which does
        // not anchor on the inline axis, moves it. So its re-measurements
        // land before the anchor is taken.
        if self.axis == ListAxis::Horizontal && !changed_width {
            let (again, first): (Vec<_>, Vec<_>) = feedback.measurements.drain(..).partition(|m| {
                let row = &self.mounted[by_view[&m.view]];
                self.index.is_measured_at(row.position)
            });
            for measurement in again {
                self.measure(by_view, measurement)?;
            }
            feedback.measurements = first;
        }
        let anchor_height = self
            .geometry
            .as_ref()
            .map_or(feedback.port_main, |g| g.port_main);
        self.leave_end_if_moved(fill.velocity);
        let extent = self.index.total_height();
        let anchor = Some(match self.restoring(&feedback) {
            Some(anchor) => anchor,
            None => self.report_anchor(feedback.offset, anchor_height, trailing)?,
        });
        self.index.set_trailing(trailing);
        self.set_geometry(CollectionFeedback {
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
                self.measure(by_view, measurement)?;
            }
        }
        self.check_preview_height(u)?;
        self.restore(anchor)?;
        self.settle_into_view(u.env.plan, feedback.offset);
        self.realize_window(u, frames, false, fill)?;
        self.reveal_shown(u);
        self.settle_start(extent);
        let mut now = self.snapshot();
        // Receiving a newer sequence without changing rows/extent/correction is
        // a fact update, not a new frame (avoids post-layout feedback loops).
        now.scroll_sequence = previous.scroll_sequence;
        let changed = previous != now;
        if changed {
            advance(&mut self.revision)?;
        }
        Ok((changed, self.edge_event(fill)?))
    }
    /// One mounted row's measured size into the index.
    fn measure(
        &mut self,
        by_view: &BTreeMap<ViewId, usize>,
        measurement: RowMeasurement,
    ) -> Result<(), InstanceError> {
        let row = &self.mounted[by_view[&measurement.view]];
        let key = self.index.shared_key(row.position).unwrap().clone();
        let size = self.index.denoised(row.position, measurement.size);
        self.index
            .set_measured_height_at(row.position, row.token, size)
            .map_err(index_error)?;
        if measurement.size == 0.0 {
            self.zero_heights.insert(key.to_string());
        } else if !self.zero_heights.is_empty() {
            self.zero_heights.remove(&*key);
        }
        Ok(())
    }
    /// The edge a report's geometry reached, if armed and handled.
    fn edge_event(
        &mut self,
        fill: CollectionFill,
    ) -> Result<Option<CollectionEdges>, InstanceError> {
        let reached = self.geometric_edges()?;
        let ready = [0, 1].map(|i| reached[i] && self.edge_armed[i] && self.edge_handlers[i]);
        let edge = (0..2).find(|&i| ready[i]);
        // A build-only report runs no action (LLP 1072 §5): the edge stays
        // armed and the report is pending, so the next report runs it.
        if fill.create_only && edge.is_some() {
            self.pending = true;
            return Ok(None);
        }
        Ok(edge.map(|i| {
            self.edge_armed[i] = false;
            CollectionEdges {
                first: [EventKind::Reachstart, EventKind::Reachend][i],
                // End remains armed until an action actually consumes it.
                end_after_noop: ready[0] && ready[1],
            }
        }))
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
        nest::PinEpoch::bump();
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
}
