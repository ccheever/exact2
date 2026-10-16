//! The table's slots reclaimed (LLP 1121 §5 stage 4, as built: mark and
//! sweep, where the RFC says reference-counted). `ColorValue` is `Copy`, so
//! an id has no owner to count; instead every holder of ids registers a
//! [`Holder`] and publishes the slots it holds as a [`WideMark`], on its own
//! thread, whenever what it holds changes: each kernel at the end of each
//! commit that changed it, and a [`Keep`] beside anything else in a kernel
//! or a runner that keeps ids across a commit (a runner's queued patches and
//! saved overrides, a thread's role overrides). No host holds an id: each
//! converts colours at its boundary (the Linux host's retained paint is
//! settled at capture, `Settle`, or keeps its values, `Values`). A mark is therefore always a superset of
//! what its holder holds, and an idle holder never holds a sweep up.
//!
//! A painting host's accent scope (`roles::authored_accent`) is no holder:
//! it lasts only while the host paints a node of a kernel it borrows, and
//! that kernel's mark holds the colour all the while.
//!
//! An id interned but not yet in any mark (parsed, its commit not yet made)
//! is in flight. A thread that made a live [`Holder`] (a kernel's thread)
//! opens a batch at its first intern after it last published, and the batch
//! closes when a holder on the thread publishes again (its commit made), its
//! last holder goes, or it ends. Code that parses for a keep rather than a
//! kernel (a decoded snapshot, a kernel-free document) opens a [`Batch`]
//! itself, closed when it has made its keep. A sweep frees no slot interned
//! since the oldest open batch began, for up to [`SPAN`] interns, however
//! many colours the batch parses; a batch left open longer (a kernel that
//! parsed and never committed) protects no more than that. Anything else (a
//! thread with no holder, an id handed to another thread, another kernel's
//! commit on the same thread closing a batch first) is protected by
//! recency: a sweep frees no slot interned within the last [`GRACE`]
//! interns. An id in flight past all of these, unmarked, is a missed
//! holder.
//!
//! **The invariant**, then: outside an open batch, no path holds an id
//! unmarked across more than [`GRACE`] (1,024) interns. Whatever parses more
//! than that before it applies, off its kernel's thread or for no kernel,
//! must hold what it parsed under a [`Batch`] or a [`Keep`] until its commit
//! (or its keep) marks it. A live edit's batch or snapshot applied on its
//! kernel's thread (LLP 1121 stage 4, commit 4) is in that thread's batch.
//!
//! When fewer than a quarter of the slots are free, the table sweeps: it
//! frees every slot no mark holds and not interned recently, and bumps those
//! slots' generations. A full table of held colours refuses a new one.
//!
//! An id is its slot in the low 12 bits and the slot's generation in the
//! top 4, so an id a holder failed to mark reads as stale (`None`), never as
//! another colour; a stale read is a missed holder, a bug, loud in a
//! development build (`debug_assert`, one journal line per id). A missed
//! holder is a holder no visitor marks, or an id in flight past the
//! invariant above (past [`GRACE`] interns outside a batch, or past its
//! batch's [`SPAN`]). Generations wrap after 16 sweeps of one slot, when a
//! missed holder's id read only after the wrap aliases the slot's newest
//! colour: acceptable only because a missed holder of either kind is a bug
//! the stale read catches whenever it reads earlier, and tests exercise the
//! visitors and both bounds.

use super::{WideValue, WIDE_CAP};
use crate::style::ColorValue;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::thread::ThreadId;

const SLOT_BITS: u32 = 12;
const SLOT_MASK: u16 = (1 << SLOT_BITS) - 1;
const _: () = assert!(WIDE_CAP == 1 << SLOT_BITS);

/// The table sweeps when fewer than this many slots are free, so a colour
/// rarely meets a full table.
const LOW: usize = WIDE_CAP / 4;

/// A slot interned within this many interns is never swept: an id still in
/// flight to its holder, from a thread whose batches are not tracked.
pub const GRACE: u64 = (WIDE_CAP / 4) as u64;

/// The most interns an open batch protects: three quarters of the table,
/// more distinct colours than any one commit parses.
pub const SPAN: u64 = (WIDE_CAP - LOW) as u64;

struct Slot {
    value: Option<Arc<WideValue>>,
    generation: u8,
    /// The intern count when it was last interned.
    minted: u64,
}

pub(in crate::style) struct Table {
    slots: Vec<Slot>,
    free: Vec<u16>,
    by_text: HashMap<Box<str>, u16>,
    holders: HashMap<u64, Box<WideMark>>,
    next_holder: u64,
    /// Interns so far (and refusals, so a full table's recent slots age).
    interns: u64,
    /// Threads that made a live holder, and how many: their batches are
    /// tracked.
    holder_threads: HashMap<ThreadId, usize>,
    /// Each such thread's open batch: the intern count when it began.
    batches: HashMap<ThreadId, u64>,
    /// Open [`Batch`]es, by token: the intern count when each began.
    opened: HashMap<u64, u64>,
    next_batch: u64,
    /// A mark changed, or interns passed, since the last sweep: it may now
    /// free something.
    stirred: bool,
    /// Slots holding a colour.
    live: usize,
    /// Colours refused because every slot was held.
    refusals: u64,
    /// The table has refused since it last admitted a new colour: the
    /// journal has been told.
    told: bool,
}

static TABLE: LazyLock<Mutex<Table>> = LazyLock::new(|| {
    Mutex::new(Table {
        slots: Vec::new(),
        free: Vec::new(),
        by_text: HashMap::new(),
        holders: HashMap::new(),
        next_holder: 0,
        interns: 0,
        holder_threads: HashMap::new(),
        batches: HashMap::new(),
        opened: HashMap::new(),
        next_batch: 0,
        stirred: false,
        live: 0,
        refusals: 0,
        told: false,
    })
});

pub(in crate::style) fn table() -> Result<MutexGuard<'static, Table>, ()> {
    TABLE.lock().map_err(|_| ())
}

thread_local! {
    /// Closes this thread's batch, and forgets the thread, when it ends.
    static THREAD: ThreadGone = const { ThreadGone };
}

struct ThreadGone;

impl Drop for ThreadGone {
    fn drop(&mut self) {
        if let Ok(mut t) = table() {
            let me = std::thread::current().id();
            t.holder_threads.remove(&me);
            if t.batches.remove(&me).is_some() {
                t.stirred = true;
            }
        }
    }
}

fn id(slot: u16, generation: u8) -> u16 {
    slot | (u16::from(generation) << SLOT_BITS)
}

impl Table {
    pub(in crate::style) fn get(&self, id: u16) -> Option<Arc<WideValue>> {
        let slot = self.slots.get(usize::from(id & SLOT_MASK))?;
        (u16::from(slot.generation) == id >> SLOT_BITS)
            .then(|| slot.value.clone())
            .flatten()
    }

    fn available(&self) -> usize {
        self.free.len() + (WIDE_CAP - self.slots.len())
    }

    fn mint(&mut self, slot: u16) {
        self.tick();
        self.slots[usize::from(slot)].minted = self.interns;
        let me = std::thread::current().id();
        if self.holder_threads.contains_key(&me) {
            let at = self.interns;
            self.batches.entry(me).or_insert(at);
        }
    }

    fn tick(&mut self) {
        self.interns += 1;
        if self.interns.is_multiple_of(GRACE) {
            self.stirred = true;
        }
    }

    /// `me` made a holder: its batches are tracked until its last holder
    /// goes or it ends.
    fn holder_made(&mut self, me: ThreadId) {
        *self.holder_threads.entry(me).or_insert(0) += 1;
        let _ = THREAD.try_with(|_| {});
    }

    /// A holder `on` made went: with its last, `on`'s batch closes.
    fn holder_gone(&mut self, on: ThreadId) {
        if let Some(n) = self.holder_threads.get_mut(&on) {
            *n -= 1;
            if *n == 0 {
                self.holder_threads.remove(&on);
                self.batches.remove(&on);
            }
        }
    }

    /// Intern `value`: its id, or `None` when every slot is held.
    pub(in crate::style) fn intern(&mut self, value: WideValue) -> Option<u16> {
        if let Some(&slot) = self.by_text.get(&value.text) {
            self.mint(slot);
            return Some(id(slot, self.slots[usize::from(slot)].generation));
        }
        if self.available() <= LOW {
            self.sweep();
        }
        let slot = match self.free.pop() {
            Some(slot) => slot,
            None if self.slots.len() < WIDE_CAP => {
                self.slots.push(Slot {
                    value: None,
                    generation: 0,
                    minted: 0,
                });
                (self.slots.len() - 1) as u16
            }
            None => {
                self.refuse(&value.text);
                return None;
            }
        };
        self.told = false;
        let text = value.text.clone();
        let s = &mut self.slots[usize::from(slot)];
        s.value = Some(Arc::new(value));
        let generation = s.generation;
        self.by_text.insert(text, slot);
        self.live += 1;
        self.mint(slot);
        Some(id(slot, generation))
    }

    /// Free every slot no holder's mark holds, not interned since the oldest
    /// open batch began, and not within the last [`GRACE`] interns. With no
    /// holder registered (a compiler, a tool), nothing is swept: an id there
    /// may be in use where nothing marks it.
    fn sweep(&mut self) {
        if !self.stirred || self.holders.is_empty() {
            return;
        }
        self.stirred = false;
        let mut held = WideMark::default();
        for mark in self.holders.values() {
            held.union(mark);
        }
        let span = self.interns.saturating_sub(SPAN);
        let recent = self
            .batches
            .values()
            .chain(self.opened.values())
            .map(|&began| (began - 1).max(span))
            .fold(self.interns.saturating_sub(GRACE), u64::min);
        for (i, s) in self.slots.iter_mut().enumerate() {
            let Some(value) = &s.value else { continue };
            if held.has(i as u16) || s.minted > recent {
                continue;
            }
            self.by_text.remove(&value.text);
            self.live -= 1;
            s.value = None;
            s.generation = (s.generation + 1) & 0x0f;
            self.free.push(i as u16);
        }
    }

    /// A new colour refused, every slot held: counted for the caller to
    /// report (a live edit's build answers `unsupported`), and journaled
    /// once until the table admits a new colour again.
    fn refuse(&mut self, text: &str) {
        self.refusals += 1;
        self.tick();
        if !self.told {
            self.told = true;
            eprintln!(
                "exact: the wide colour table is full ({WIDE_CAP} colours held); {text} refused, and the row keeps its value (LLP 1121 stage 4)"
            );
        }
    }

    fn register(&mut self, mark: WideMark) -> u64 {
        self.next_holder += 1;
        self.holders.insert(self.next_holder, Box::new(mark));
        self.stirred = true;
        self.next_holder
    }
}

/// How many slots hold a colour now.
pub fn used() -> usize {
    table().map_or(0, |t| t.live)
}

/// How many new colours the table has refused because every slot was held:
/// a caller that compares it before and after a write knows to report one.
pub fn refusals() -> u64 {
    table().map_or(0, |t| t.refusals)
}

/// The slots one holder holds: a bit per slot.
#[derive(Clone, PartialEq, Eq)]
pub struct WideMark([u64; WIDE_CAP / 64]);

impl Default for WideMark {
    fn default() -> Self {
        WideMark([0; WIDE_CAP / 64])
    }
}

impl std::fmt::Debug for WideMark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n: u32 = self.0.iter().map(|w| w.count_ones()).sum();
        write!(f, "WideMark({n} slots)")
    }
}

impl WideMark {
    /// Mark the slot a colour holds, if it holds one.
    pub fn color(&mut self, c: ColorValue) {
        if let ColorValue::Wide(id) = c {
            self.set(id & SLOT_MASK);
        }
    }

    fn set(&mut self, slot: u16) {
        let slot = usize::from(slot);
        self.0[slot / 64] |= 1 << (slot % 64);
    }

    /// Mark a slot one of [`slots`](Self::slots) named.
    pub fn slot(&mut self, slot: u16) {
        self.set(slot & SLOT_MASK);
    }

    /// The slots marked, to mark again later without visiting.
    pub fn slots(&self) -> Vec<u16> {
        let mut out = Vec::new();
        for (i, &word) in self.0.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros();
                out.push((i * 64) as u16 + bit as u16);
                w &= w - 1;
            }
        }
        out
    }

    fn has(&self, slot: u16) -> bool {
        let slot = usize::from(slot);
        self.0[slot / 64] & (1 << (slot % 64)) != 0
    }

    /// Add `other`'s slots.
    pub fn union(&mut self, other: &WideMark) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a |= b;
        }
    }
}

/// One holder of ids that change (a kernel), registered while it lives,
/// holding nothing until it publishes.
#[derive(Debug)]
pub struct Holder {
    id: u64,
    /// The thread that made it, for a kernel's holder (not a keep's).
    thread: Option<ThreadId>,
}

impl Default for Holder {
    fn default() -> Self {
        Self::new()
    }
}

impl Holder {
    /// A holder holding nothing yet.
    pub fn new() -> Holder {
        let me = std::thread::current().id();
        let id = table().map_or(u64::MAX, |mut t| {
            t.holder_made(me);
            t.register(WideMark::default())
        });
        Holder {
            id,
            thread: Some(me),
        }
    }

    /// What it holds now, a superset of it: published whenever that may
    /// have changed, on the thread that made the change, which closes the
    /// thread's batch (what it parsed is in a mark now).
    pub fn publish(&self, mark: WideMark) {
        if let Ok(mut t) = table() {
            if let Some(m) = t.holders.get_mut(&self.id) {
                **m = mark;
            }
            t.batches.remove(&std::thread::current().id());
            t.stirred = true;
        }
    }

    /// Whether the table holds any wide colour: none, and a holder's mark
    /// needs no visit (it can hold no id).
    pub fn needed() -> bool {
        table().is_ok_and(|t| !t.by_text.is_empty())
    }
}

impl Drop for Holder {
    fn drop(&mut self) {
        if let Ok(mut t) = table() {
            t.holders.remove(&self.id);
            if let Some(on) = self.thread {
                t.holder_gone(on);
            }
            t.stirred = true;
        }
    }
}

/// A batch of colours parsed for a keep rather than a kernel (a decoded
/// snapshot, a kernel-free document): nothing interned while it is open is
/// swept (up to [`SPAN`] interns). Close it once the keep is made.
#[derive(Debug)]
pub struct Batch(u64);

impl Batch {
    /// Open a batch from the next intern on.
    pub fn open() -> Batch {
        Batch(table().map_or(u64::MAX, |mut t| {
            t.next_batch += 1;
            let (token, at) = (t.next_batch, t.interns + 1);
            t.opened.insert(token, at);
            token
        }))
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        if let Ok(mut t) = table() {
            if t.opened.remove(&self.0).is_some() {
                t.stirred = true;
            }
        }
    }
}

/// The ids in something that keeps them unchanged for as long as it lives
/// (a queued patch, a role override table, a decoded snapshot): marked once,
/// when made. A clone keeps the same; it is no part of the value beside it,
/// so any two compare equal.
#[derive(Debug, Default)]
pub struct Keep {
    _held: Option<Holder>,
}

impl Clone for Keep {
    fn clone(&self) -> Keep {
        let Some(held) = &self._held else {
            return Keep::default();
        };
        let id = table().map_or(u64::MAX, |mut t| {
            let mark = t.holders.get(&held.id).map(|m| (**m).clone());
            mark.map_or(u64::MAX, |m| t.register(m))
        });
        Keep {
            _held: Some(Holder { id, thread: None }),
        }
    }
}

impl PartialEq for Keep {
    fn eq(&self, _: &Keep) -> bool {
        true
    }
}

impl Keep {
    /// Keep each colour `colors` visits (none: no holder).
    pub fn of(colors: impl FnOnce(&mut WideMark)) -> Keep {
        let mut mark = WideMark::default();
        colors(&mut mark);
        if mark == WideMark::default() {
            return Keep { _held: None };
        }
        let id = table().map_or(u64::MAX, |mut t| t.register(mark));
        Keep {
            _held: Some(Holder { id, thread: None }),
        }
    }
}

/// A stale id read: a holder the marker missed.
pub(in crate::style) fn stale(id: u16) {
    static SEEN: Mutex<Vec<u16>> = Mutex::new(Vec::new());
    let first = SEEN.lock().is_ok_and(|mut seen| {
        let first = !seen.contains(&id);
        if first {
            seen.push(id);
        }
        first
    });
    if first {
        eprintln!(
            "exact: wide colour {id:#06x} read after its slot was reclaimed: a holder its mark missed (LLP 1121 stage 4)"
        );
    }
    debug_assert!(
        false,
        "wide colour {id:#06x} read after its slot was reclaimed: a holder the mark missed"
    );
}
