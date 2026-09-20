use super::{Data, DataError, Reader};
use std::{cell::Cell, rc::Rc};

/// Maximum entity slots accepted by a world save: 200,000, including dead slots.
pub const MAX_LOAD_ENTITIES: usize = crate::MAX_ENTITIES;
/// Maximum decoded UTF-8 bytes in any one string: 1 MiB.
pub const MAX_LOAD_STRING: usize = 1024 * 1024;
/// Maximum input bytes and accounted decoded allocations per decoder: 256 MiB.
pub const MAX_LOAD_BYTES: usize = 256 * 1024 * 1024;

/// One explicit cumulative allowance across nested binary/JSON/world decoders.
/// Cloning the handle shares consumption; it never replenishes the allowance.
#[derive(Clone)]
pub struct LoadBudget(Rc<Cell<usize>>);
impl LoadBudget {
    /// Set the total accounted allocation allowance for cooperating decoders.
    pub fn new(bytes: usize) -> Self {
        Self(Rc::new(Cell::new(bytes)))
    }
}

pub(crate) enum Budget {
    Local(usize),
    Shared(LoadBudget),
}
impl Default for Budget {
    fn default() -> Self {
        Self::Local(MAX_LOAD_BYTES)
    }
}
impl Budget {
    pub(crate) fn new(bytes: usize) -> Self {
        Self::Local(bytes)
    }
    pub(crate) fn shared(budget: &LoadBudget) -> Self {
        Self::Shared(budget.clone())
    }
    fn remaining(&self) -> usize {
        match self {
            Self::Local(bytes) => *bytes,
            Self::Shared(budget) => budget.0.get(),
        }
    }
    pub(crate) fn check(&self, bytes: usize) -> Result<(), DataError> {
        if bytes > self.remaining() {
            Err(DataError::new("decoded size exceeds load budget"))
        } else {
            Ok(())
        }
    }
    pub fn text(&mut self, s: &str) -> Result<String, DataError> {
        if s.len() > MAX_LOAD_STRING {
            return Err(DataError::new("string exceeds load limit"));
        }
        self.claim(s.len())?;
        let mut out = String::new();
        out.try_reserve_exact(s.len()).map_err(allocation)?;
        out.push_str(s);
        Ok(out)
    }
    pub fn reserve<T>(&mut self, v: &mut Vec<T>) -> Result<(), DataError> {
        if v.len() == v.capacity() {
            let capacity = v.capacity().saturating_mul(2).max(4);
            self.claim(
                (capacity - v.capacity())
                    .checked_mul(std::mem::size_of::<T>())
                    .ok_or_else(|| DataError::new("allocation size overflow"))?,
            )?;
            v.try_reserve_exact(capacity - v.len())
                .map_err(allocation)?;
        }
        Ok(())
    }
    pub fn claim(&mut self, bytes: usize) -> Result<(), DataError> {
        let remaining = self
            .remaining()
            .checked_sub(bytes)
            .ok_or_else(|| DataError::new("decoded size exceeds load budget"))?;
        match self {
            Self::Local(bytes) => *bytes = remaining,
            Self::Shared(budget) => budget.0.set(remaining),
        }
        Ok(())
    }
}
pub(crate) fn allocation(_: std::collections::TryReserveError) -> DataError {
    DataError::new("cannot allocate decoded value")
}
pub(crate) fn reserve<T>(
    r: &mut dyn Reader,
    v: &mut Vec<T>,
    extra: usize,
) -> Result<(), DataError> {
    let need = v
        .len()
        .checked_add(extra)
        .ok_or_else(|| DataError::new("length overflow"))?;
    if need > v.capacity() {
        let capacity = need.max(v.capacity().saturating_mul(2)).max(4);
        r.claim(
            (capacity - v.capacity())
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| DataError::new("allocation size overflow"))?,
        )?;
        v.try_reserve_exact(capacity - v.len())
            .map_err(allocation)?;
    }
    Ok(())
}
pub(crate) fn read_vec<T: Data>(
    r: &mut dyn Reader,
    v: &mut Vec<T>,
    limit: usize,
) -> Result<(), DataError> {
    r.begin_seq()?;
    if r.sequence_len().is_some_and(|n| n > limit) {
        return Err(DataError::new("sequence count exceeds load limit"));
    }
    if let Some(n) = r.sequence_len() {
        r.check_allocation(
            n.checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| DataError::new("allocation size overflow"))?,
        )?;
    }
    v.clear();
    while r.item()? {
        if v.len() == limit {
            return Err(DataError::new("sequence count exceeds load limit"));
        }
        reserve(r, v, 1)?;
        let item = T::read_new(r).map_err(|e| e.at(v.len()))?;
        v.push(item);
    }
    Ok(())
}
