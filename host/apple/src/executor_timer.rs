//! The machine's one timer (LLP 1120 D3): a heap of deadlines, earliest
//! first, on a thread of its own that only claims, posts and aborts.
use super::Run;
use ibex2::stdlib::abort::AbortController;
use std::collections::BinaryHeap;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// One deadline: when, whose, and what aborts it.
pub(super) struct Entry {
    at: Instant,
    seq: u64,
    pub(super) lane: usize,
    pub(super) run: Run,
    pub(super) ms: u32,
    pub(super) abort: AbortController,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        (self.at, self.seq) == (other.at, other.seq)
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    /// Earliest first, as a max-heap pops it.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (other.at, other.seq).cmp(&(self.at, self.seq))
    }
}

/// The executor's one timer thread's heap (D3): it only claims, posts and
/// aborts.
#[derive(Default)]
pub(super) struct Timer {
    heap: Mutex<(BinaryHeap<Entry>, u64, bool)>,
    changed: Condvar,
}

impl Timer {
    pub(super) fn arm(&self, ms: u32, lane: usize, run: Run, abort: AbortController) {
        let mut heap = self.heap.lock().unwrap();
        heap.1 += 1;
        let seq = heap.1;
        heap.0.push(Entry {
            at: Instant::now() + Duration::from_millis(ms.into()),
            seq,
            lane,
            run,
            ms,
            abort,
        });
        self.changed.notify_all();
    }

    /// Remove `run`'s deadline: at its terminal transition, or a park.
    pub(super) fn remove(&self, run: Run) {
        let mut heap = self.heap.lock().unwrap();
        heap.0.retain(|e| e.run != run);
    }

    pub(super) fn stop(&self) {
        self.heap.lock().unwrap().2 = true;
        self.changed.notify_all();
    }

    /// The next deadline to pass, waiting for it; `None` once stopped.
    pub(super) fn next(&self) -> Option<Entry> {
        let mut heap = self.heap.lock().unwrap();
        loop {
            if heap.2 {
                return None;
            }
            let now = Instant::now();
            match heap.0.peek().map(|e| e.at) {
                Some(at) if at <= now => return heap.0.pop(),
                Some(at) => heap = self.changed.wait_timeout(heap, at - now).unwrap().0,
                None => heap = self.changed.wait(heap).unwrap(),
            }
        }
    }
}
