use super::{Data, DataError, Reader};

/// Maximum entity slots accepted by a world save: 16 million, including dead slots.
pub const MAX_LOAD_ENTITIES: usize = 16 * 1024 * 1024;
/// Maximum decoded UTF-8 bytes in any one string: 64 MiB.
pub const MAX_LOAD_STRING: usize = 64 * 1024 * 1024;
/// Maximum input bytes and accounted decoded allocations per decoder: 2 GiB.
pub const MAX_LOAD_BYTES: usize = 2 * 1024 * 1024 * 1024;

pub(crate) struct Budget(usize);
impl Default for Budget {
    fn default() -> Self {
        Self(MAX_LOAD_BYTES)
    }
}
impl Budget {
    pub(crate) fn new(bytes: usize) -> Self {
        Self(bytes)
    }
    pub(crate) fn check(&self, bytes: usize) -> Result<(), DataError> {
        if bytes > self.0 {
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
        self.0 = self
            .0
            .checked_sub(bytes)
            .ok_or_else(|| DataError::new("decoded size exceeds load budget"))?;
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
        let mut item = T::default();
        item.read(r).map_err(|e| e.at(v.len()))?;
        v.push(item);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn total_budget_refuses_before_allocation() {
        let mut budget = Budget::default();
        budget.claim(MAX_LOAD_BYTES - 1).unwrap();
        assert!(budget.claim(2).is_err());
        let bytes = crate::bin::to_vec(&vec![1u64]);
        let mut r = crate::bin::Decoder::new(&bytes);
        r.claim(MAX_LOAD_BYTES - 1024).unwrap();
        let mut values = Vec::<u64>::new();
        r.begin_seq().unwrap();
        // The remaining budget cannot hold this destination allocation.
        assert!(reserve(&mut r, &mut values, 1024).is_err());
        assert_eq!(values.capacity(), 0);
    }
    #[test]
    fn declared_nested_collection_storage_is_checked_before_any_element() {
        use crate::{bin, Value, Writer};
        // A compact, malformed sequence whose first element must never be read.
        let mut w = bin::Encoder::default();
        w.begin_seq(1024);
        let mut bytes = w.finish();
        bytes.extend(std::iter::repeat_n(255, 1024));
        let mut r = bin::Decoder::with_budget(&bytes, 4096);
        let mut values = Vec::<Value>::new();
        assert!(values.read(&mut r).unwrap_err().message.contains("budget"));
        assert_eq!(values.capacity(), 0);

        // The outer vector fits; the nested vector is checked before its first
        // element's read/default/reservation, not after traversing the payload.
        let mut w = bin::Encoder::default();
        w.begin_seq(1);
        let mut nested = w.finish();
        nested.extend_from_slice(&bytes);
        let mut r = bin::Decoder::with_budget(&nested, 4096);
        let mut values = Vec::<Vec<Value>>::new();
        let error = values.read(&mut r).unwrap_err();
        assert!(error.message.contains("budget"), "{error}");
        assert_eq!(error.path, "0");
        assert!(values.is_empty());

        // Choosing a local budget does not constrain ordinary save decoding.
        let original = vec![Value::Unit; 1024];
        let bytes = bin::to_vec(&original);
        assert_eq!(bin::from_slice::<Vec<Value>>(&bytes).unwrap().len(), 1024);

        // Normal saves may reuse existing destination capacity; only the scoped
        // importing decoder adds the conservative declared-storage preflight.
        let mut original = vec![0u64; 1024];
        let bytes = bin::to_vec(&original);
        let mut r = bin::Decoder::new(&bytes);
        r.claim(MAX_LOAD_BYTES - 4096).unwrap();
        original.read(&mut r).unwrap();
        r.finish().unwrap();
    }
}
