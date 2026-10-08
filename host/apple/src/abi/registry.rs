//! The runtimes an app's exports address by handle (LLP 1031 D2): each an
//! entry of its bridge and the hooks it was created with.

use super::{escape, Bridge, Hooks};
use exact_runner::DataSource;
use std::cell::RefCell;
use std::rc::Rc;

/// One runtime the registry holds: its bridge and the hooks it was created
/// with (the measurer and the wake, passed at every boot).
pub struct Entry<D: DataSource> {
    /// The bridge.
    pub bridge: Bridge<D>,
    /// The callbacks given at `exact_create`.
    pub hooks: Hooks,
}

/// Every live runtime on this thread, by handle (LLP 1031 D2). Handles come
/// from one process-wide counter — never 0, never reused, unique across
/// threads even though each thread keeps its own registry — so a late call
/// on a destroyed runtime is refused, never confused with a successor, and
/// a handle from another thread never resolves here by coincidence.
pub struct Registry<D: DataSource> {
    entries: std::collections::HashMap<u32, Rc<RefCell<Entry<D>>>>,
}

/// The process-wide handle counter (see [`Registry`]).
static NEXT_HANDLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

impl<D: DataSource> Default for Registry<D> {
    fn default() -> Self {
        Registry {
            entries: std::collections::HashMap::new(),
        }
    }
}

impl<D: DataSource> Registry<D> {
    /// A new runtime with no callbacks yet; its handle. Exhaustion of the
    /// counter (four billion runtimes) is a `0` the caller must refuse.
    pub fn create(&mut self) -> u32 {
        self.create_linked(crate::link::Links::ALL)
    }

    /// Create a runtime linking what `links` names (`host!`'s entry const).
    pub fn create_linked(&mut self, links: crate::link::Links<D>) -> u32 {
        let rt = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if rt == 0 || rt == u32::MAX {
            return 0;
        }
        self.entries.insert(
            rt,
            Rc::new(RefCell::new(Entry {
                bridge: Bridge::with_links(links),
                hooks: Hooks::none(),
            })),
        );
        rt
    }

    /// Drop a runtime: its runner, executor sender, buffers, and journal go
    /// with it (LLP 1031 D2). `false` when there was no such runtime.
    pub fn destroy(&mut self, rt: u32) -> bool {
        self.entries.remove(&rt).is_some()
    }

    /// The runtime, if it lives.
    pub fn get(&self, rt: u32) -> Option<Rc<RefCell<Entry<D>>>> {
        self.entries.get(&rt).cloned()
    }

    /// How many runtimes live.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether none lives.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

thread_local! {
    /// The refusal a call on a dead or busy runtime answers with: a batch
    /// whose `error` names it, in a buffer no runtime owns.
    pub(crate) static REFUSAL: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Record a refusal for a call that reached no runtime — a handle nobody
/// holds, or one busy with another call — and return its length; the bytes
/// are at [`refusal_ptr`].
pub fn refuse(rt: u32, why: &str) -> u32 {
    REFUSAL.with(|r| {
        let mut r = r.borrow_mut();
        *r = format!(
            "{{\"ops\":[],\"timers\":false,\"motion\":false,\"error\":\"runtime {rt}: {}\"}}",
            escape(why)
        )
        .into_bytes();
        r.len() as u32
    })
}

/// The same refusal in the agent API's shape (`{"error":…}`, LLP 1012),
/// for `exact_agent` on a dead or busy runtime.
pub fn refuse_agent(rt: u32, why: &str) -> u32 {
    REFUSAL.with(|r| {
        let mut r = r.borrow_mut();
        *r = exact_runner::agent::error(&format!("runtime {rt}: {why}")).into_bytes();
        r.len() as u32
    })
}

/// The last refusal's bytes.
pub fn refusal_ptr() -> *const u8 {
    REFUSAL.with(|r| r.borrow().as_ptr())
}

/// Run `f` on runtime `rt`'s bridge, or refuse: no such runtime, or one
/// already inside a call on this thread (`busy`). `refused` gets the
/// refusal's length; `agent` chooses the agent API's `{"error":…}` shape
/// over a batch's.
pub fn with_runtime<D: DataSource, T>(
    registry: &'static std::thread::LocalKey<RefCell<Registry<D>>>,
    rt: u32,
    agent: bool,
    f: impl FnOnce(&mut Bridge<D>, Hooks) -> T,
    refused: impl FnOnce(u32) -> T,
) -> T {
    let refusal = |why: &str| {
        if agent {
            refuse_agent(rt, why)
        } else {
            refuse(rt, why)
        }
    };
    let entry = registry.with(|r| r.borrow().get(rt));
    let Some(entry) = entry else {
        return refused(refusal("no such runtime (destroyed, or never created)"));
    };
    let mut guard = match entry.try_borrow_mut() {
        Ok(guard) => guard,
        Err(_) => {
            return refused(refusal(
                "busy: a call is already in progress on this runtime",
            ))
        }
    };
    let hooks = guard.hooks;
    let out = f(&mut guard.bridge, hooks);
    drop(guard);
    out
}

/// Run `f` on runtime `rt`'s entry (a setter); silently nothing for a dead
/// or busy runtime — a setter returns nothing, and the next call says why.
pub fn with_entry<D: DataSource>(
    registry: &'static std::thread::LocalKey<RefCell<Registry<D>>>,
    rt: u32,
    f: impl FnOnce(&mut Entry<D>),
) {
    let entry = registry.with(|r| r.borrow().get(rt));
    if let Some(entry) = entry {
        if let Ok(mut e) = entry.try_borrow_mut() {
            f(&mut e);
        }
    }
}
