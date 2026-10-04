//! One level of nesting (LLP 1070 §4): a virtualized list in a virtualized
//! list's row lives and dies with that row (N1); what the reader scrolled it
//! to is kept, by default, as an item key and an offset into that item (Q1
//! as ruled, §4.2); a pin inside it pins the outer row too (N5).
use super::*;
use std::collections::HashMap;
use traversal::InnerSites;

/// At most this many kept positions per outer list, least recently kept
/// evicted first (~48 bytes an entry beside keys the index shares).
pub(crate) const KEPT_POSITIONS: usize = 4096;

/// Where an inner list's reader was: the key of the item at its port's start
/// and how far into that item. Not pixels, so estimates and re-measured items
/// above it never move what the reader sees when it comes back.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeptPosition {
    pub key: Rc<str>,
    pub within: f64,
    /// The item's start when its row left: measured items above it may not
    /// match the estimates a new list starts from, and the difference is
    /// spread over those so the offset comes back as well as the item.
    pub start: f64,
    stamp: u64,
}

/// An outer list's kept positions: (outer row key, inner site) to the inner
/// list's position, bounded by recency.
#[derive(Debug, Default)]
pub(crate) struct Kept {
    entries: HashMap<(Rc<str>, Rc<str>), KeptPosition>,
    order: BTreeMap<u64, (Rc<str>, Rc<str>)>,
    clock: u64,
}
impl Kept {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    fn put(&mut self, row: Rc<str>, site: Rc<str>, position: Option<(Rc<str>, f64, f64)>) {
        let slot = (row, site);
        if let Some(old) = self.entries.remove(&slot) {
            self.order.remove(&old.stamp);
        }
        let Some((key, within, start)) = position else {
            return;
        };
        self.clock += 1;
        self.order.insert(self.clock, slot.clone());
        self.entries.insert(
            slot,
            KeptPosition {
                key,
                within,
                start,
                stamp: self.clock,
            },
        );
        while self.entries.len() > KEPT_POSITIONS {
            let Some((_, oldest)) = self.order.pop_first() else {
                break;
            };
            self.entries.remove(&oldest);
        }
    }
    fn get(&self, row: &str, site: &str) -> Option<&KeptPosition> {
        self.entries.get(&(Rc::from(row), Rc::from(site)))
    }
    /// Rows whose key left the data take their positions with them.
    fn retain_rows(&mut self, live: impl Fn(&str) -> bool) {
        let gone: Vec<_> = self
            .entries
            .iter()
            .filter(|((row, _), _)| !live(row))
            .map(|(slot, entry)| (slot.clone(), entry.stamp))
            .collect();
        for (slot, stamp) in gone {
            self.entries.remove(&slot);
            self.order.remove(&stamp);
        }
    }
    /// `state`'s rows: outer key, site, item key, offset, newest last.
    pub(crate) fn json(&self, list: ViewId, out: &mut String) {
        use std::fmt::Write;
        for (row, site) in self.order.values() {
            let entry = &self.entries[&(row.clone(), site.clone())];
            if !out.ends_with('[') {
                out.push(',');
            }
            let _ = write!(out, "{{\"list\":{list},\"row\":");
            crate::agent::quote(row, out);
            out.push_str(",\"site\":");
            crate::agent::quote(site, out);
            out.push_str(",\"key\":");
            crate::agent::quote(&entry.key, out);
            let _ = write!(out, ",\"within\":{}}}", exact_num::Shortest(entry.within));
        }
    }
}

/// Every collection in `children` (a row's roots), outside other collections'
/// rows, with its site: its plan node and the keys of any eager `each` rows
/// crossed to reach it, and the frames in force there.
fn inner_lists<'a>(
    children: &'a mut [Child],
    frames: &[Frame],
    site: &str,
    sites: &InnerSites,
    out: &mut Vec<(&'a mut Collection, String, Vec<Frame>)>,
) {
    for child in children {
        if !sites.may_hold(child) {
            continue;
        }
        match child {
            Child::Node(node) => {
                let here = format!("{site}{}", node.node.0);
                match &mut node.collection {
                    Some(c) => out.push((c, here, frames.to_vec())),
                    None => inner_lists(&mut node.children, frames, site, sites, out),
                }
            }
            Child::Region(region) => match &mut region.active {
                Active::Arm { roots, frame, .. } => {
                    let mut inner = frames.to_vec();
                    inner.push(frame.clone());
                    inner_lists(roots, &inner, site, sites, out);
                }
                Active::Rows { rows } => {
                    for row in rows {
                        let mut inner = frames.to_vec();
                        inner.push(row.frame.clone());
                        let key = super::super::ident(&row.key, row.dup).unwrap_or_default();
                        inner_lists(&mut row.roots, &inner, &format!("{site}{key}/"), sites, out);
                    }
                }
            },
        }
    }
}

/// The pins inside `children`'s collections, focus then interaction.
fn inner_pins(children: &[Child], sites: &InnerSites, out: &mut [Option<ViewId>; 2]) {
    for child in children {
        if !sites.may_hold(child) {
            continue;
        }
        match child {
            Child::Node(node) => match &node.collection {
                Some(c) => {
                    if let Some(g) = &c.geometry {
                        out[0] = out[0].or(g.focus_view);
                        out[1] = out[1].or(g.interaction_view);
                    }
                }
                None => inner_pins(&node.children, sites, out),
            },
            Child::Region(region) => match &region.active {
                Active::Arm { roots, .. } => inner_pins(roots, sites, out),
                Active::Rows { rows } => {
                    for row in rows {
                        inner_pins(&row.roots, sites, out);
                    }
                }
            },
        }
    }
}

impl Collection {
    /// The pins this list keeps: its own report's, else a pin an inner list
    /// in one of its rows holds (N5). Hosts report a pin to its nearest
    /// owner only; the chain up is derived here, in one place.
    pub(super) fn pins(&self) -> [Option<ViewId>; 2] {
        let mut out = self
            .geometry
            .as_ref()
            .map_or([None, None], |g| [g.focus_view, g.interaction_view]);
        if let (true, Some(sites)) = (out[0].is_none() || out[1].is_none(), &self.inner) {
            // Walking every mounted row's subtree for an inner list's pin
            // was a fifth of a scrolling list's report; the answer changes
            // only when some list's pins do (`PinEpoch`).
            let epoch = PinEpoch::now();
            let inner = match self.inner_pins.get() {
                Some((at, inner)) if at == epoch => inner,
                _ => {
                    let mut inner = [None, None];
                    for row in &self.mounted {
                        inner_pins(&row.row.roots, sites, &mut inner);
                    }
                    self.inner_pins.set(Some((epoch, inner)));
                    inner
                }
            };
            out = [out[0].or(inner[0]), out[1].or(inner[1])];
        }
        out
    }
    /// Where the reader left this list, if anywhere but its start.
    fn position(&self) -> Option<(Rc<str>, f64, f64)> {
        // Restored and gone again before any host reported it: still there.
        let Some(g) = self.geometry.as_ref() else {
            let (key, within) = self.restored_at.clone()?;
            let start = self.index.prefix(self.index.position(&key)?)?;
            return Some((key, within, start));
        };
        let max = (self.index.total_height() - g.port_main).max(0.0);
        let offset = g.offset.clamp(0.0, max);
        if offset <= 0.0 {
            return None;
        }
        let row = self.index.row_at(offset).ok()??;
        let start = self.index.prefix(row)?;
        Some((self.index.shared_key(row)?.clone(), offset - start, start))
    }
    /// A mounted row leaves the window: its inner lists' positions are kept
    /// under its key, unless a list opted out (`scroll-restoration:
    /// manual`) or the row's item left the data.
    pub(super) fn keep_positions(&mut self, mounted: &mut Mounted, key: &str) {
        let Some(sites) = self.inner.clone() else {
            return;
        };
        let key: Rc<str> = Rc::from(key);
        let mut lists = Vec::new();
        inner_lists(&mut mounted.row.roots, &[], "", &sites, &mut lists);
        for (inner, site, _) in lists {
            if inner.manual {
                continue;
            }
            self.kept.put(key.clone(), Rc::from(site), inner.position());
        }
    }
    /// A row was created: its inner lists learn their parent, and each one
    /// that was scrolled when the row last left starts where it was.
    pub(super) fn adopt_nested(
        &mut self,
        u: &mut Update<'_>,
        row: &mut Row,
        key: &str,
        frames: &[Frame],
    ) -> Result<(), InstanceError> {
        let Some(sites) = self.inner.clone() else {
            return Ok(());
        };
        let mut outer = frames.to_vec();
        outer.push(row.frame.clone());
        let mut lists = Vec::new();
        inner_lists(&mut row.roots, &outer, "", &sites, &mut lists);
        // An inner port is at most the outer port along the same axis, or
        // the outer rows' cross size across it (F2's bootstrap): build that
        // much before any host has laid the inner list out, so the frame
        // that first shows it is covered.
        let port = self.geometry.as_ref().map(|g| (g.port_main, g.cross));
        for (inner, site, frames) in lists {
            inner.parent = Some(self.view);
            let mut again = false;
            if let Some((main, cross)) = port {
                let estimate = if inner.axis == self.axis { main } else { cross };
                // Its own literal size bounds it more tightly (a 399 pt inbox).
                let estimate = inner.declared_port.map_or(estimate, |d| d.min(estimate));
                let rows = ((estimate / inner.estimated_height).ceil() as usize + 1).clamp(1, 64);
                if rows > inner.bootstrap_rows && inner.geometry.is_none() {
                    inner.bootstrap_rows = rows;
                    again = true;
                }
            }
            let kept = (!inner.manual)
                .then(|| self.kept.get(key, &site))
                .flatten()
                .map(|k| (k.key.clone(), k.within, k.start));
            if let Some((anchor, within, start)) = kept {
                inner.restore_position(u, &frames, &anchor, within, start)?;
            } else if again {
                inner.realize_window(u, &frames, false, CollectionFill::default())?;
            }
        }
        Ok(())
    }
    /// A restored list's anchor is its kept item, `within` into it, until the
    /// reader moves it or that item is measured: its estimate may be
    /// shorter than `within`, so the offset alone would name the next item.
    pub(super) fn restoring(&mut self, feedback: &CollectionFeedback) -> Option<index::Anchor> {
        let (key, within) = self.restored_at.clone()?;
        // The reader moved it: from now on, what shows is the anchor. (Not
        // the report's sequence: hosts advance it for their own reasons.)
        // A host that reports before applying the correction reports its
        // start; that is not the reader moving either.
        let moved = (feedback.offset - self.start_offset).abs() > 0.5 && feedback.offset > 0.5;
        if moved || self.index.is_measured(&key) {
            self.restored_at = None;
        }
        if moved {
            return None;
        }
        self.index.anchor_at(&key, within)
    }
    /// Drop the kept positions of rows whose keys are gone.
    pub(super) fn forget_departed(&mut self) {
        if self.kept.len() > 0 {
            let index = &self.index;
            self.kept.retain_rows(|row| index.position(row).is_some());
        }
    }
    /// Start at `key`'s item, `within` into it: the window is built there and
    /// the host is told to move the port there before it paints.
    fn restore_position(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key: &str,
        within: f64,
        start: f64,
    ) -> Result<(), InstanceError> {
        let Some(position) = self.index.position(key) else {
            return Ok(()); // the item left: the list starts at its start
        };
        // The items above were measured when the row left; a new list has
        // only their estimates. Their difference goes to the estimates, so
        // the item starts where it did and the offset comes back exactly.
        let estimated = self.index.prefix(position).unwrap_or(0.0);
        self.index
            .spread_estimates(0..position, start - estimated)
            .map_err(index_error)?;
        let offset = self.index.prefix(position).unwrap_or(0.0) + within;
        self.start_offset = offset;
        self.restored = true;
        self.restored_at = self.index.shared_key(position).map(|k| (k.clone(), within));
        self.correction = Some(AnchorCorrection {
            scroll_sequence: 0,
            offset,
        });
        self.realize_window(u, frames, false, CollectionFill::default())?;
        advance(&mut self.revision)?;
        Ok(())
    }
}

impl Tree {
    /// Every outer list's kept positions, for `state`.
    pub fn kept_positions_json(&self) -> String {
        let mut out = String::from("[");
        let mut stack: Vec<_> = self.children.iter().collect();
        while let Some(child) = stack.pop() {
            match child {
                Child::Node(node) => {
                    if let Some(c) = &node.collection {
                        c.kept.json(c.view, &mut out);
                        c.add_children(&mut stack);
                    }
                    stack.extend(node.children.iter());
                }
                Child::Region(region) => match &region.active {
                    Active::Arm { roots, .. } => stack.extend(roots.iter()),
                    Active::Rows { rows } => {
                        for row in rows {
                            stack.extend(row.roots.iter());
                        }
                    }
                },
            }
        }
        out.push(']');
        out
    }
}

/// A count that moves whenever any list's own pins change or a list
/// holding one goes: an outer list's cached answer for the pins inside its
/// rows is good while the count stands. One count for the process, so a
/// runner booted on one thread and run on another reads the same count
/// (it only ever grows; a bump on another runner only costs a recount).
pub(super) struct PinEpoch;
static PIN_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
impl PinEpoch {
    pub(super) fn now() -> u64 {
        PIN_EPOCH.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub(super) fn bump() {
        PIN_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kept_positions_are_bounded_by_recency_and_leave_with_their_rows() {
        let mut kept = Kept::default();
        let at = |n: usize| Some((Rc::from(format!("n:{n}").as_str()), 4.0, 0.0));
        for row in 0..KEPT_POSITIONS + 10 {
            kept.put(Rc::from(format!("r{row}").as_str()), Rc::from("7"), at(row));
        }
        assert_eq!(kept.len(), KEPT_POSITIONS);
        assert!(kept.get("r0", "7").is_none(), "the least recent went first");
        assert!(kept.get("r10", "7").is_some());
        // Kept again, r10 is the newest: the next eviction takes r11.
        kept.put(Rc::from("r10"), Rc::from("7"), at(1));
        kept.put(Rc::from("new"), Rc::from("7"), at(2));
        assert!(kept.get("r10", "7").is_some() && kept.get("r11", "7").is_none());
        // Back at its start: nothing kept.
        kept.put(Rc::from("r10"), Rc::from("7"), None);
        assert!(kept.get("r10", "7").is_none());
        kept.retain_rows(|row| row == "new");
        assert_eq!(kept.len(), 1);
    }
}
