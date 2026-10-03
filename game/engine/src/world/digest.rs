//! Paged digests. The world hash and rest observation re-read only the pages
//! written since they were last digested: a component page is keyed by its write
//! generation, a slot page by its spawn/despawn stamp, a parented-global page by
//! the hierarchy's pose epoch, a resource by its revision. The hash is a stream
//! over page digests in type-name and page order; observation keeps each page's
//! row hashes, shared with the cache, so a seek's two samples cost O(pages) plus
//! the rows that changed.
use super::*;
use crate::storage::PAGE;
use std::rc::Rc;

// Observed rows of one page: (index, generation, value hash), ambient rows excluded.
type Rows = Rc<[(u32, u32, u64)]>;

#[derive(Clone, Default)]
struct Page {
    key: Option<(u64, u64, u64)>,
    all: u64,
    len: usize,
    rows: Rows,
}
#[derive(Default)]
pub(crate) struct Digests {
    slot_clock: u64,
    slot_stamps: Vec<u64>,
    slots: Vec<Page>,
    globals: Vec<Page>,
    components: BTreeMap<&'static str, (u64, Vec<Page>)>,
    resources: BTreeMap<&'static str, ((u64, u64), u64)>,
    rng: Option<((u64, u64), u64)>,
}
impl Digests {
    /// A spawn or despawn changed this slot's generation, liveness or name.
    pub(crate) fn touch(&mut self, index: u32) {
        let page = index as usize / PAGE;
        if page >= self.slot_stamps.len() {
            self.slot_stamps.resize(page + 1, 0);
        }
        self.slot_clock += 1;
        self.slot_stamps[page] = self.slot_clock;
    }
}
fn digest(rows: impl Iterator<Item = (u32, u64)>) -> u64 {
    let mut w = hash::Hasher::default();
    for (i, h) in rows {
        w.number(crate::Number::Unsigned(i.into()));
        w.number(crate::Number::Unsigned(h));
    }
    w.finish()
}
type Computed = (u64, usize, Vec<(u32, u32, u64)>);
fn refresh(
    pages: &mut Vec<Page>,
    count: usize,
    key: impl Fn(usize) -> (u64, u64, u64),
    mut compute: impl FnMut(usize) -> Computed,
) {
    pages.resize_with(count, Page::default);
    for (page, slot) in pages.iter_mut().enumerate() {
        let key = key(page);
        if slot.key != Some(key) {
            let (all, len, rows) = compute(page);
            *slot = Page {
                key: Some(key),
                all,
                len,
                rows: rows.into(),
            };
        }
    }
}

impl World {
    // Bring page digests up to the current state. Callers hold no exclusive lease.
    // Observation leaves ambient resources (executors) unread; the hash reads them.
    fn refresh_digests(&self, ambient_resources: bool) {
        let mut d = self.digests.borrow_mut();
        let d = &mut *d;
        let ambient_storage = self.storage::<crate::Ambient>();
        let ambient = |i: usize| ambient_storage.is_some_and(|s| s.has(i));
        let ambient_key = |p: usize| {
            ambient_storage.map_or(0, |s| s.page_generation(p) ^ s.instance().rotate_left(32))
        };
        let len = self.state.slots.len();
        let stamps = &d.slot_stamps;
        refresh(
            &mut d.slots,
            len.div_ceil(PAGE),
            |p| {
                let fill = ((p + 1) * PAGE).min(len) - p * PAGE;
                (
                    stamps.get(p).copied().unwrap_or(0),
                    ambient_key(p),
                    fill as u64,
                )
            },
            |p| {
                let mut w = hash::Hasher::default();
                let mut rows = Vec::new();
                let end = ((p + 1) * PAGE).min(len);
                for i in p * PAGE..end {
                    let slot = &self.state.slots[i];
                    slot.write(&mut w);
                    if slot.alive && !ambient(i) {
                        rows.push((i as u32, slot.generation, 0));
                    }
                }
                (w.finish(), end - p * PAGE, rows)
            },
        );
        let epochs = &self.hierarchy.epochs;
        let parented = self.storage::<crate::Parent>();
        // Follower subtrees move with their rig's Pose and Transform: re-read on
        // each new tick or pose (pose_cursor's own condition), never by epoch alone.
        let sockets = self.socket_pages();
        let moved = (
            self.tick(),
            self.revision::<crate::Transform>(),
            self.revision::<crate::Pose>(),
        );
        let moved = moved.0 ^ moved.1.rotate_left(21) ^ moved.2.rotate_left(42);
        refresh(
            &mut d.globals,
            epochs.len().max(sockets.last().map_or(0, |p| p + 1)),
            |p| {
                let rig = if sockets.contains(&p) { moved } else { 0 };
                let epoch = epochs.get(p).copied().unwrap_or(0);
                (
                    epoch,
                    ambient_key(p),
                    self.presentation_generation ^ rig.rotate_left(7),
                )
            },
            |p| {
                let mut rows = Vec::new();
                for i in p * PAGE..(p + 1) * PAGE {
                    if parented.is_none_or(|s| !s.has(i)) || ambient(i) {
                        continue;
                    }
                    let e = self.entity_at(i);
                    if let Some(pose) = self.global(e) {
                        rows.push((i as u32, e.generation, hash::of(&pose.to_cols_array())));
                    }
                }
                (0, rows.len(), rows)
            },
        );
        d.components
            .retain(|name, _| self.components.contains_key(name));
        for (&name, storage) in &self.components {
            let (instance, pages) = d.components.entry(name).or_default();
            if *instance != storage.instance() {
                *instance = storage.instance();
                pages.clear();
            }
            refresh(
                pages,
                storage.page_count(),
                |p| (storage.page_generation(p), ambient_key(p), 0),
                |p| {
                    let mut all = Vec::new();
                    let mut rows = Vec::new();
                    storage.digest_page(p, &mut |i, h| {
                        all.push((i as u32, h));
                        if !ambient(i) {
                            rows.push((i as u32, self.state.slots[i].generation, h));
                        }
                    });
                    (digest(all.iter().copied()), all.len(), rows)
                },
            );
        }
        d.resources
            .retain(|name, _| self.resources.contains_key(name));
        for (&name, storage) in &self.resources {
            if !ambient_resources && self.registry[name].ambient {
                continue;
            }
            let key = (storage.instance(), storage.revision());
            let entry = d.resources.entry(name).or_insert(((u64::MAX, 0), 0));
            if entry.0 != key {
                let mut w = hash::Hasher::default();
                storage.write_one(0, &mut w);
                *entry = (key, w.finish());
            }
        }
        let key = (self.rng.instance(), self.rng.revision());
        if d.rng.is_none_or(|(k, _)| k != key) {
            let mut w = hash::Hasher::default();
            self.rng.write_one(0, &mut w);
            d.rng = Some((key, w.finish()));
        }
    }
    /// Hash simulation state in type-name and page order, excluding saved delivery
    /// queues and caches. Only pages written since the last hash are re-read.
    #[track_caller]
    pub fn hash(&self) -> u64 {
        if let Some((epoch, hash)) = self.hash_cache.get() {
            if epoch == self.mutation_epoch() {
                return hash;
            }
        }
        self.check_reads(Location::caller());
        self.refresh_digests(true);
        let d = self.digests.borrow();
        let mut w = hash::Hasher::default();
        w.begin_struct();
        w.field("state");
        for n in [self.state.tick, self.state.hz.into(), self.state.seed] {
            n.write(&mut w);
        }
        self.state.busy.borrow().write(&mut w);
        w.begin_seq(d.slots.len());
        for page in &d.slots {
            w.item();
            page.all.write(&mut w);
        }
        w.end_seq();
        w.field("rng");
        d.rng.map_or(0, |(_, h)| h).write(&mut w);
        w.field("components");
        w.begin_struct();
        for (name, (_, pages)) in &d.components {
            w.key(name);
            let used = pages.iter().enumerate().filter(|(_, page)| page.len != 0);
            w.begin_seq(used.clone().count());
            for (p, page) in used {
                w.item();
                (p as u64).write(&mut w);
                page.all.write(&mut w);
            }
            w.end_seq();
        }
        w.end_struct();
        w.field("resources");
        w.begin_struct();
        for (name, (_, h)) in &d.resources {
            w.key(name);
            h.write(&mut w);
        }
        w.end_struct();
        w.end_struct();
        let hash = w.finish();
        self.hash_cache.set(Some((self.mutation_epoch(), hash)));
        hash
    }

    /// Sample the observed state: O(pages), sharing row hashes with the digests.
    #[track_caller]
    pub(crate) fn observe(&self, out: &mut Observation) {
        self.check_reads(std::panic::Location::caller());
        self.refresh_digests(false);
        let d = self.digests.borrow();
        let published = self.published.borrow();
        out.published.clear();
        for (name, value) in published.iter() {
            if !self.derived_publications.contains(name) {
                out.published.push((name.clone(), crate::hash::of(value)));
            }
        }
        out.exists = d.slots.iter().map(|p| p.rows.clone()).collect();
        out.globals = d.globals.iter().map(|p| p.rows.clone()).collect();
        out.components = d
            .components
            .iter()
            .map(|(&name, (_, pages))| (name, pages.iter().map(|p| p.rows.clone()).collect()))
            .collect();
        out.rng = d.rng.map_or(0, |(_, h)| h);
        out.resources = d
            .resources
            .iter()
            .filter(|(name, _)| !self.registry[*name].ambient)
            .map(|(&name, &(_, h))| (name, h))
            .collect();
    }
    pub(crate) fn compare(&mut self, before: &Observation, after: &Observation) {
        self.observed_epoch = self.mutation_epoch();
        self.changing.clear();
        let mut reasons = Vec::new();
        if before.published != after.published {
            for (key, _) in before.published.iter().chain(&after.published) {
                if reasons.len() >= 8 {
                    break;
                }
                if before.published.iter().find(|v| &v.0 == key)
                    != after.published.iter().find(|v| &v.0 == key)
                {
                    let reason = format!("published.{key}");
                    if !reasons.contains(&reason) {
                        reasons.push(reason);
                    }
                }
            }
        }
        let mut still = before.published == after.published;
        let mut pages = |label: &str, a: &[Rows], b: &[Rows], reasons: &mut Vec<String>| {
            let empty: Rows = Rc::from([]);
            for p in 0..a.len().max(b.len()) {
                let (x, y) = (a.get(p).unwrap_or(&empty), b.get(p).unwrap_or(&empty));
                if Rc::ptr_eq(x, y) || x[..] == y[..] {
                    continue;
                }
                still = false;
                diff(self, x, y, label, reasons);
            }
        };
        pages("exists", &before.exists, &after.exists, &mut reasons);
        pages("global", &before.globals, &after.globals, &mut reasons);
        let names: BTreeSet<_> = before
            .components
            .iter()
            .chain(&after.components)
            .map(|(n, _)| *n)
            .collect();
        for name in names {
            fn find<'o>(o: &'o Observation, name: &str) -> &'o [Rows] {
                o.components
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map_or(&[][..], |(_, p)| &p[..])
            }
            pages(name, find(before, name), find(after, name), &mut reasons);
        }
        if before.rng != after.rng {
            still = false;
            if reasons.len() < 8 {
                reasons.push("resource.Rng".into());
            }
        }
        if before.resources != after.resources {
            still = false;
            let names: BTreeSet<_> = before
                .resources
                .iter()
                .chain(&after.resources)
                .map(|(n, _)| *n)
                .collect();
            for name in names {
                let find =
                    |o: &Observation| o.resources.iter().find(|(n, _)| *n == name).map(|v| v.1);
                if find(before) != find(after) && reasons.len() < 8 {
                    reasons.push(format!("resource.{name}"));
                }
            }
        }
        self.changing = reasons;
        self.observation = if still {
            ObservationState::Still
        } else {
            ObservationState::Changing
        };
    }
}
// Name changed rows in entity order, as `name.label`, up to eight reasons in all.
fn diff(
    w: &World,
    a: &[(u32, u32, u64)],
    b: &[(u32, u32, u64)],
    label: &str,
    reasons: &mut Vec<String>,
) {
    let (mut i, mut j) = (0, 0);
    while (i < a.len() || j < b.len()) && reasons.len() < 8 {
        let (x, y) = (a.get(i), b.get(j));
        if x == y {
            i += 1;
            j += 1;
            continue;
        }
        let key = |v: &(u32, u32, u64)| (v.0, v.1);
        let entry = match (x, y) {
            (Some(x), Some(y)) => match key(x).cmp(&key(y)) {
                std::cmp::Ordering::Less => {
                    i += 1;
                    x
                }
                std::cmp::Ordering::Greater => {
                    j += 1;
                    y
                }
                std::cmp::Ordering::Equal => {
                    i += 1;
                    j += 1;
                    y
                }
            },
            (Some(x), None) => {
                i += 1;
                x
            }
            (None, Some(y)) => {
                j += 1;
                y
            }
            _ => break,
        };
        let e = Entity {
            index: entry.0,
            generation: entry.1,
        };
        let name = w
            .name(e)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("#{}", e.index()));
        reasons.push(format!("{name}.{label}"));
    }
}

/// One sample of observed state; pages share row hashes with the world's digests.
#[derive(Default)]
pub(crate) struct Observation {
    exists: Vec<Rows>,
    globals: Vec<Rows>,
    components: Vec<(&'static str, Vec<Rows>)>,
    rng: u64,
    resources: Vec<(&'static str, u64)>,
    published: Vec<(String, u64)>,
}
