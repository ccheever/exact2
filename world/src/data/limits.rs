use super::{Data, DataError, Reader};
use std::{cell::Cell, rc::Rc};

pub const MAX_LOAD_ENTITIES: usize = crate::MAX_ENTITIES;
pub const MAX_LOAD_STRING: usize = 1024 * 1024;
pub const MAX_LOAD_BYTES: usize = 256 * 1024 * 1024;

/// One explicit cumulative allowance across nested binary/JSON/world decoders.
/// Cloning the handle shares consumption; it never replenishes the allowance.
#[derive(Clone)]
pub struct LoadBudget(Rc<Cell<usize>>);
impl LoadBudget {
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
        grow(v, 1, 32, |bytes| self.claim(bytes))
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
pub(crate) fn reserve<T: Data>(
    r: &mut dyn Reader,
    v: &mut Vec<T>,
    extra: usize,
) -> Result<(), DataError> {
    grow(v, extra, T::default_size(), |bytes| r.claim(bytes))
}
fn grow<T>(
    v: &mut Vec<T>,
    extra: usize,
    unit: usize,
    mut claim: impl FnMut(usize) -> Result<(), DataError>,
) -> Result<(), DataError> {
    let need = v
        .len()
        .checked_add(extra)
        .ok_or_else(|| DataError::new("length overflow"))?;
    if need > v.capacity() {
        let capacity = need.max(v.capacity().saturating_mul(2)).max(4);
        claim(
            (capacity - v.capacity())
                .checked_mul(unit)
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
    v.clear();
    if let Some(n) = r.sequence_len() {
        reserve(r, v, n)?;
    }
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

pub(crate) fn read_map<T: Data, K: Ord + for<'a> From<&'a str>>(
    r: &mut dyn Reader,
    values: &mut std::collections::BTreeMap<K, T>,
    limit: usize,
    key_bytes: usize,
) -> Result<(), DataError> {
    r.begin_struct()?;
    values.clear();
    while let Some(key) = r.field()? {
        if values.len() == limit || key.len() > key_bytes {
            return Err(DataError::new("map count/key limit"));
        }
        r.claim(
            64usize
                .saturating_add(key.len())
                .saturating_add(T::default_size()),
        )?;
        let value = T::read_new(r).map_err(|e| e.at(key))?;
        values.insert(key.into(), value);
    }
    Ok(())
}
