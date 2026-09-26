//! An allocation owns this account, never a session, mailbox or payload.
use crate::{Refusal, Stats};
use std::sync::{Arc, Condvar, Mutex, Weak};

#[derive(Default)]
pub(crate) struct Wake {
    pub sequence: Mutex<u64>,
    pub changed: Condvar,
}
impl Wake {
    pub fn notify(&self) {
        let mut sequence = self.sequence.lock().unwrap();
        *sequence = sequence.wrapping_add(1);
        self.changed.notify_all();
    }
}

pub(crate) struct BudgetAccount {
    pub usage: Mutex<Stats>,
    /// Decoded bytes this session may hold: pinned, cold, retiring, reserved.
    pub budget: u64,
    wake: Weak<Wake>,
}
impl BudgetAccount {
    pub fn new(wake: &Arc<Wake>, budget: u64) -> Arc<Self> {
        Arc::new(Self {
            usage: Mutex::new(Stats::default()),
            budget,
            wake: Arc::downgrade(wake),
        })
    }
    pub fn available(&self) -> u64 {
        let s = self.usage.lock().unwrap();
        self.budget - s.resident_bytes - s.reserved_bytes
    }
    pub fn reserve(self: &Arc<Self>, bytes: u64) -> Result<AllocationReservation, Refusal> {
        if bytes > self.budget {
            return Err(Refusal::TooLarge);
        }
        let mut usage = self.usage.lock().unwrap();
        if bytes > self.budget - usage.resident_bytes - usage.reserved_bytes {
            return Err(Refusal::Budget);
        }
        usage.reserved_bytes += bytes;
        usage.peak_bytes = usage
            .peak_bytes
            .max(usage.resident_bytes + usage.reserved_bytes);
        drop(usage);
        Ok(AllocationReservation {
            charge: AllocationCharge(Arc::new(Allocation {
                account: self.clone(),
                state: Mutex::new(AllocationState {
                    bytes,
                    resident: false,
                    cached: false,
                    pins: 0,
                }),
            })),
        })
    }
    fn notify(&self) {
        if let Some(wake) = self.wake.upgrade() {
            wake.notify();
        }
    }
}

#[derive(Clone, Copy)]
struct AllocationState {
    bytes: u64,
    resident: bool,
    cached: bool,
    pins: usize,
}
impl AllocationState {
    fn subtract(self, s: &mut Stats) {
        if !self.resident {
            s.reserved_bytes -= self.bytes;
            return;
        }
        s.resident_bytes -= self.bytes;
        if self.pins > 0 {
            s.pinned_bytes -= self.bytes;
        } else if self.cached {
            s.cold_bytes -= self.bytes;
        } else {
            s.retiring_bytes -= self.bytes;
        }
    }
    fn add(self, s: &mut Stats) {
        if !self.resident {
            s.reserved_bytes += self.bytes;
            return;
        }
        s.resident_bytes += self.bytes;
        if self.pins > 0 {
            s.pinned_bytes += self.bytes;
        } else if self.cached {
            s.cold_bytes += self.bytes;
        } else {
            s.retiring_bytes += self.bytes;
        }
    }
}
struct Allocation {
    account: Arc<BudgetAccount>,
    state: Mutex<AllocationState>,
}
impl Allocation {
    fn change(&self, f: impl FnOnce(&mut AllocationState)) {
        let mut state = self.state.lock().unwrap();
        let mut usage = self.account.usage.lock().unwrap();
        let before = *state;
        state.subtract(&mut usage);
        f(&mut state);
        state.add(&mut usage);
        let capacity =
            state.bytes < before.bytes || (before.pins > 0 && state.pins == 0 && state.cached);
        drop(usage);
        drop(state);
        if capacity {
            self.account.notify();
        }
    }
}
impl Drop for Allocation {
    fn drop(&mut self) {
        self.state
            .get_mut()
            .unwrap()
            .subtract(&mut self.account.usage.lock().unwrap());
        self.account.notify();
    }
}

/// One physical allocation's debit. Cloning shares a debit, never accounts for
/// a new copy. Safe to retain in a native provider beyond runtime destruction.
#[derive(Clone)]
pub struct AllocationCharge(Arc<Allocation>);
impl std::fmt::Debug for AllocationCharge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AllocationCharge")
            .field("bytes", &self.bytes())
            .finish()
    }
}
impl AllocationCharge {
    pub fn bytes(&self) -> u64 {
        self.0.state.lock().unwrap().bytes
    }
    pub(crate) fn pins(&self) -> usize {
        self.0.state.lock().unwrap().pins
    }
    pub(crate) fn cached(&self, cached: bool) {
        self.0.change(|s| s.cached = cached);
    }
    pub(crate) fn pin(&self) {
        self.0.change(|s| s.pins += 1);
    }
    pub(crate) fn unpin(&self) {
        self.0.change(|s| s.pins -= 1);
    }
    fn promote(&self, actual_bytes: u64) -> Result<(), Refusal> {
        if actual_bytes > self.bytes() {
            return Err(Refusal::ActualExceedsReservation);
        }
        self.0.change(|s| {
            s.bytes = actual_bytes;
            s.resident = true;
        });
        Ok(())
    }
}

/// Preallocation ownership for an additional native/GPU CPU copy. The backing
/// retains `charge()` BEFORE allocation. Drop does not refund surviving clones.
#[derive(Debug)]
pub struct AllocationReservation {
    pub(crate) charge: AllocationCharge,
}
impl AllocationReservation {
    pub fn charge(&self) -> AllocationCharge {
        self.charge.clone()
    }
    /// The caller has already kept allocation within the admitted upper bound.
    pub fn commit(self, actual_bytes: u64) -> Result<AllocationCharge, Refusal> {
        self.charge.promote(actual_bytes)?;
        Ok(self.charge)
    }
}
