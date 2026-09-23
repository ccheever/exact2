//! Row leases. A lease covers the rows it can hand out: one row (`get`, `require`,
//! positions), every row a query matches, or a whole column (`pages`, engine reads).
//! Leases on different rows coexist; one row takes any number of shared leases or a
//! single exclusive one. Each live lease records its kind and the caller location
//! that took it, nothing per row, so a refusal names both sides and iteration pays
//! nothing. @ref llp/1046.003-game-engine-as-built.explainer.md#row-leases-2026-09-23
use super::RawStorage;
use std::cell::{Cell, UnsafeCell};
use std::panic::Location;
use std::rc::Rc;

pub(crate) type At = &'static Location<'static>;
/// The row index of a whole-column shared hold. Entity indices never reach it.
pub(crate) const EVERY: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Hold {
    index: u32,
    mutable: bool,
    live: bool,
    at: At,
}

/// Live single-row and whole-column holds on one column or resource, as a stack:
/// a guard's slot stays put until it is released, and released tops are popped.
/// The scan is linear in the number of live holds, which are short-lived guards.
#[derive(Default)]
pub(crate) struct Holds {
    stack: UnsafeCell<Vec<Hold>>,
    // Live exclusive holds plus exclusive query terms naming this column.
    writers: Cell<u32>,
}
impl Holds {
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut Vec<Hold>) -> R) -> R {
        // SAFETY: Holds is !Sync, and every caller passes a closure that neither
        // re-enters this Holds nor lets a reference into the stack escape, so this
        // is the only live reference to the Vec.
        f(unsafe { &mut *self.stack.get() })
    }
    #[inline]
    fn conflicts(h: &Hold, index: u32, mutable: bool) -> bool {
        h.live && (mutable || h.mutable) && (h.index == index || h.index == EVERY || index == EVERY)
    }
    /// Take a hold unless a live one on the same row conflicts.
    #[inline]
    pub(crate) fn acquire(&self, index: u32, mutable: bool, at: At) -> Option<u32> {
        let slot = self.with(|stack| {
            if stack.iter().any(|h| Self::conflicts(h, index, mutable)) {
                return None;
            }
            stack.push(Hold {
                index,
                mutable,
                live: true,
                at,
            });
            Some(stack.len() as u32 - 1)
        })?;
        if mutable {
            self.writers.set(self.writers.get() + 1);
        }
        Some(slot)
    }
    /// The live hold that refused `acquire`, if any.
    pub(crate) fn conflict(&self, index: u32, mutable: bool) -> Option<Party> {
        self.with(|stack| {
            let h = stack.iter().find(|h| Self::conflicts(h, index, mutable))?;
            Some(h.party())
        })
    }
    #[inline]
    pub(crate) fn release(&self, slot: u32) {
        let mutable = self.with(|stack| {
            let hold = &mut stack[slot as usize];
            hold.live = false;
            let mutable = hold.mutable;
            while stack.last().is_some_and(|h| !h.live) {
                stack.pop();
            }
            mutable
        });
        if mutable {
            self.writers.set(self.writers.get() - 1);
        }
    }
    /// Whether no exclusive row hold or exclusive query term is live here.
    #[inline]
    pub(crate) fn unwritten(&self) -> bool {
        self.writers.get() == 0
    }
    /// The first live hold a query over `matches` rows conflicts with. `matches`
    /// reads presence masks only, never a Holds.
    fn against(
        &self,
        mutable: bool,
        matches: impl Fn(usize) -> bool,
    ) -> Option<(Option<u32>, Party)> {
        self.with(|stack| {
            stack.iter().find_map(|h| {
                let hit = h.live
                    && (mutable || h.mutable)
                    && (h.index == EVERY || matches(h.index as usize));
                hit.then(|| ((h.index != EVERY).then_some(h.index), h.party()))
            })
        })
    }
    /// An exclusive row hold, for a refused whole-column read.
    pub(crate) fn writer(&self) -> Option<(u32, Party)> {
        let h = self.with(|stack| stack.iter().find(|h| h.live && h.mutable).copied())?;
        Some((h.index, h.party()))
    }
}
impl Hold {
    fn party(&self) -> Party {
        Party {
            mutable: self.mutable,
            via: if self.index == EVERY {
                Via::Column
            } else {
                Via::Row
            },
            at: self.at,
        }
    }
}

/// One query term, as registered: its column (null when the component has no
/// storage yet), whether it lends exclusively, and whether it constrains the join.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Term {
    pub(crate) column: *const RawStorage,
    pub(crate) name: &'static str,
    pub(crate) mutable: bool,
    pub(crate) optional: bool,
}
/// `with`/`without`/`with_any` membership; `b` is non-null only for `with_any`.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Filter {
    pub(crate) a: *const RawStorage,
    pub(crate) b: *const RawStorage,
    pub(crate) with: bool,
    pub(crate) names: (&'static str, &'static str),
}
const NO_TERM: Term = Term {
    column: std::ptr::null(),
    name: "",
    mutable: false,
    optional: true,
};
const NO_FILTER: Filter = Filter {
    a: std::ptr::null(),
    b: std::ptr::null(),
    with: false,
    names: ("", ""),
};

/// The rows a query matches: required terms and filters, by column identity.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Shape {
    terms: [Term; 8],
    nt: u8,
    filters: [Filter; 4],
    nf: u8,
}
impl Default for Shape {
    fn default() -> Self {
        Self {
            terms: [NO_TERM; 8],
            nt: 0,
            filters: [NO_FILTER; 4],
            nf: 0,
        }
    }
}
// SAFETY (every deref below): a Shape names storages of the World whose Leases
// table stores it. Storages are boxed, never removed, and dropped only with that
// World, so the pointers outlive every stored shape. A live query borrows the World,
// so no `&mut RawStorage` can exist while its shape is read. A leaked query's shape
// is stale after the next structural edit (its stamp differs) and is never
// dereferenced again; it only blocks its columns, as a leaked guard always did.
fn column<'a>(p: *const RawStorage) -> Option<&'a RawStorage> {
    unsafe { p.as_ref() }
}
fn member(p: *const RawStorage, index: usize) -> bool {
    column(p).is_some_and(|c| c.has(index))
}
fn word_of(p: *const RawStorage, word: usize) -> u64 {
    column(p)
        .and_then(|c| c.mask.get(word))
        .copied()
        .unwrap_or(0)
}
impl Shape {
    pub(crate) fn push_term(&mut self, term: Term) {
        self.terms[self.nt as usize] = term;
        self.nt += 1;
    }
    pub(crate) fn set_filters(&mut self, filters: &[Filter]) {
        self.filters[..filters.len()].copy_from_slice(filters);
        self.nf = filters.len() as u8;
    }
    fn terms(&self) -> &[Term] {
        &self.terms[..self.nt as usize]
    }
    fn filters(&self) -> &[Filter] {
        &self.filters[..self.nf as usize]
    }
    fn matches(&self, index: usize) -> bool {
        self.terms()
            .iter()
            .all(|t| t.optional || member(t.column, index))
            && self
                .filters()
                .iter()
                .all(|f| (member(f.a, index) || member(f.b, index)) == f.with)
    }
    fn word(&self, word: usize) -> u64 {
        let mut bits = u64::MAX;
        for t in self.terms().iter().filter(|t| !t.optional) {
            bits &= word_of(t.column, word);
        }
        for f in self.filters() {
            let inside = word_of(f.a, word) | word_of(f.b, word);
            bits &= if f.with { inside } else { !inside };
        }
        bits
    }
    /// `(&mut Transform, &Spin).with::<Enemy>()`, for refusals.
    pub(crate) fn describe(&self) -> String {
        let terms: Vec<_> = self
            .terms()
            .iter()
            .map(|t| {
                let r = if t.mutable { "&mut " } else { "&" };
                if t.optional {
                    format!("Option<{r}{}>", t.name)
                } else {
                    format!("{r}{}", t.name)
                }
            })
            .collect();
        let mut text = format!("({})", terms.join(", "));
        for f in self.filters() {
            text += &match (f.with, f.names.1.is_empty()) {
                (true, true) => format!(".with::<{}>()", f.names.0),
                (true, false) => format!(".with_any::<{}, {}>()", f.names.0, f.names.1),
                (false, _) => format!(".without::<{}>()", f.names.0),
            };
        }
        text
    }
}

struct QueryHold {
    // Zero marks a free entry. The query and each escaped row guard count once.
    // Entries are reused, never removed, so guards may keep this count's address.
    refs: Rc<Cell<u32>>,
    stamp: u64,
    at: At,
    shape: Shape,
}
/// A registered query: its table entry and the count its row guards share.
#[derive(Clone, Copy)]
pub(crate) struct Registration<'a> {
    pub(crate) slot: u32,
    pub(crate) refs: &'a Cell<u32>,
}

/// Registered queries, world-wide, and the structural stamp that retires the
/// shapes of leaked ones. Owned by the World beside the storages it names.
#[derive(Default)]
pub(crate) struct Leases {
    queries: UnsafeCell<Vec<QueryHold>>,
    live: Cell<u32>,
    stamp: Cell<u64>,
}
impl Leases {
    #[inline]
    fn with<R>(&self, f: impl FnOnce(&mut Vec<QueryHold>) -> R) -> R {
        // SAFETY: Leases is !Sync, and every caller passes a closure that neither
        // re-enters this table (it reads masks and writes column counters only) nor
        // lets a reference into it escape, so this is the only live reference.
        f(unsafe { &mut *self.queries.get() })
    }
    /// Called before any `&mut` access to component storages; only leaked
    /// queries can be registered then, and their shapes are never read again.
    pub(crate) fn restructure(&self) {
        self.stamp.set(self.stamp.get().wrapping_add(1));
    }
    /// Whether no live query holds this row in a way a row request conflicts with.
    #[inline]
    pub(crate) fn admits(&self, column: &RawStorage, index: usize, mutable: bool) -> bool {
        self.live.get() == 0 || self.holder(column, index, mutable).is_none()
    }
    /// The live query holding a row, for a refused row request.
    pub(crate) fn holder(&self, column: &RawStorage, index: usize, mutable: bool) -> Option<Party> {
        let stamp = self.stamp.get();
        self.with(|queries| {
            queries.iter().filter(|q| q.refs.get() != 0).find_map(|q| {
                let t = q.shape.terms().iter().find(|t| {
                    std::ptr::eq(t.column, column)
                        && (mutable || t.mutable)
                        && (q.stamp != stamp || q.shape.matches(index))
                })?;
                Some(q.party(t.mutable))
            })
        })
    }
    /// An exclusive query term on this column, for a refused whole-column read.
    pub(crate) fn writer(&self, column: &RawStorage) -> Option<Party> {
        self.with(|queries| {
            queries.iter().find_map(|q| {
                let t =
                    q.shape.terms().iter().find(|t| {
                        q.refs.get() != 0 && t.mutable && std::ptr::eq(t.column, column)
                    })?;
                Some(q.party(t.mutable))
            })
        })
    }
    /// Register a query's shape, refusing overlap with live holds and queries. The
    /// shape is built in place in a free entry, which stays free if refused.
    pub(crate) fn register(
        &self,
        at: At,
        build: impl FnOnce(&mut Shape),
    ) -> Result<Registration<'_>, Conflict> {
        let stamp = self.stamp.get();
        let (slot, refs) = self.with(|queries| {
            let slot = match queries.iter().position(|q| q.refs.get() == 0) {
                Some(free) => free,
                None => {
                    queries.push(QueryHold {
                        refs: Rc::default(),
                        stamp,
                        at,
                        shape: Shape::default(),
                    });
                    queries.len() - 1
                }
            };
            let q = &mut queries[slot];
            (q.stamp, q.at, q.shape.nt, q.shape.nf) = (stamp, at, 0, 0);
            build(&mut q.shape);
            let queries = &*queries;
            let shape = &queries[slot].shape;
            let requested = |mutable| Party {
                mutable,
                via: Via::Query(shape.describe()),
                at,
            };
            for t in shape.terms() {
                let Some(c) = column(t.column) else {
                    continue;
                };
                let refuse = |row, held| Conflict {
                    component: t.name,
                    row,
                    held,
                    requested: requested(t.mutable),
                };
                let matches = |i| c.has(i) && shape.matches(i);
                if let Some((row, held)) = c.holds.against(t.mutable, matches) {
                    return Err(refuse(row, held));
                }
                let live = queries.iter().filter(|q| q.refs.get() != 0);
                let overlap = live.into_iter().find_map(|q| {
                    q.shape.terms().iter().find_map(|u| {
                        if !std::ptr::eq(u.column, t.column) || !(t.mutable || u.mutable) {
                            return None;
                        }
                        if q.stamp != stamp {
                            return Some((None, q.party(u.mutable)));
                        }
                        let row = c.mask.iter().enumerate().find_map(|(w, &bits)| {
                            let both = bits & shape.word(w) & q.shape.word(w);
                            (both != 0).then(|| (w * 64) as u32 + both.trailing_zeros())
                        })?;
                        Some((Some(row), q.party(u.mutable)))
                    })
                });
                if let Some((row, held)) = overlap {
                    return Err(refuse(row, held));
                }
            }
            for t in shape.terms().iter().filter(|t| t.mutable) {
                if let Some(c) = column(t.column) {
                    c.holds.writers.set(c.holds.writers.get() + 1);
                }
            }
            queries[slot].refs.set(1);
            Ok((slot as u32, Rc::as_ptr(&queries[slot].refs)))
        })?;
        self.live.set(self.live.get() + 1);
        // SAFETY: entries are never removed and their counts never replaced, so the
        // Rc allocation lives as long as this table.
        let refs = unsafe { &*refs };
        Ok(Registration { slot, refs })
    }
    /// Narrow a registered query's filters. Narrowing never adds rows.
    pub(crate) fn refilter(&self, slot: u32, filters: &[Filter]) {
        self.with(|queries| queries[slot as usize].shape.set_filters(filters));
    }
    #[inline]
    pub(crate) fn split(&self, query: Registration<'_>) {
        let refs = query.refs.get().checked_add(1).expect("too many borrows");
        query.refs.set(refs);
    }
    #[inline]
    pub(crate) fn release(&self, query: Registration<'_>) {
        let refs = query.refs.get() - 1;
        query.refs.set(refs);
        if refs == 0 {
            self.retire(query.slot);
        }
    }
    fn retire(&self, slot: u32) {
        self.with(|queries| {
            // A released query was live until now, so its stamp is current.
            for t in queries[slot as usize]
                .shape
                .terms()
                .iter()
                .filter(|t| t.mutable)
            {
                if let Some(c) = column(t.column) {
                    c.holds.writers.set(c.holds.writers.get() - 1);
                }
            }
        });
        self.live.set(self.live.get() - 1);
    }
}
impl QueryHold {
    fn party(&self, mutable: bool) -> Party {
        Party {
            mutable,
            via: Via::Query(self.shape.describe()),
            at: self.at,
        }
    }
}

/// How a lease reaches its rows.
pub(crate) enum Via {
    Row,
    Column,
    Query(String),
    Resource,
    Read,
}
/// One side of a refusal: its kind, its reach and where it was taken.
pub(crate) struct Party {
    pub mutable: bool,
    pub via: Via,
    pub at: At,
}
impl Party {
    fn describe(&self, verb: &str) -> String {
        let kind = if self.mutable { "exclusive" } else { "shared" };
        let reach = match &self.via {
            Via::Row => "borrow of one row (get/require/position)".to_string(),
            Via::Column => "borrow of every row (pages)".to_string(),
            Via::Query(shape) => format!("query {shape}"),
            Via::Resource => "borrow".to_string(),
            Via::Read => "engine read of every row (hash/save/inspection)".to_string(),
        };
        format!("{kind} {reach}, {verb} {}", self.at)
    }
}
/// A refused lease. The World names the entity; storage knows only its index.
pub(crate) struct Conflict {
    pub component: &'static str,
    pub row: Option<u32>,
    pub held: Party,
    pub requested: Party,
}
impl Conflict {
    pub(crate) fn row(component: &'static str, row: u32, held: Party, requested: Party) -> Self {
        Self {
            component,
            row: Some(row),
            held,
            requested,
        }
    }
    /// The refusal text; `entity` describes `row` when there is one.
    pub(crate) fn message(&self, entity: Option<String>) -> String {
        let subject = match (&self.requested.via, entity) {
            (Via::Resource, _) => format!("resource {}", self.component),
            (_, Some(entity)) => format!("{} of {entity}", self.component),
            (_, None) => format!("{} (every row)", self.component),
        };
        let rule = if matches!(self.requested.via, Via::Resource) {
            "A resource takes any number of shared borrows or a single exclusive one."
        } else {
            "Rows of one component borrow independently: a row takes any number of shared \
             borrows or a single exclusive one. A query holds every row it matches until it, \
             and every row guard it yielded, drops."
        };
        format!(
            "borrow conflict on {subject}\n  requested: {}\n  held by:   {}\n  {rule}",
            self.requested.describe("at"),
            self.held.describe("taken at"),
        )
    }
}
