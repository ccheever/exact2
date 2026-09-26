//! Keying an answer that shares objects with the previous one (LLP 1053 §0
//! G8): a live insert keys the rows it added, not the list.
//!
//! A row's key is its key expression over its item; while the key's other
//! inputs are unchanged, the same item object has the same key. So an item
//! that is the same object as a previous item ([`crate::compare::shared`])
//! takes that item's key and identity text without evaluating anything, and
//! only the others are keyed. The result equals a full re-key: when the
//! previous keys repeated, or a new key repeats one the new list keeps, the
//! full path decides the repeats' order instead.
use super::*;

#[cfg(test)]
thread_local! {
    /// Tests: take the full path, the reference the shared path must equal.
    pub(super) static FULL_REKEY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

impl Collection {
    /// Re-key `items` from the previous keys where items are the same
    /// objects. `false` (nothing changed) when the full path must run.
    pub(super) fn rekey_shared(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key: exact_plan::Code,
        items: &[Value],
    ) -> Result<bool, InstanceError> {
        #[cfg(test)]
        if FULL_REKEY.with(|full| full.get()) {
            return Ok(false);
        }
        if !self.dups.is_empty() {
            return Ok(false);
        }
        let shared = crate::compare::shared(&self.items, items, true);
        let start = shared.prefix;
        let old_end = self.items.len() - shared.suffix;
        // Mostly new objects: the full path is as cheap, and it is the
        // reference.
        let found = shared.middle.iter().filter(|m| m.is_some()).count();
        if 2 * (start + shared.suffix + found) < items.len() {
            return Ok(false);
        }
        let mut kept = vec![false; old_end - start];
        let mut keys = Vec::with_capacity(shared.middle.len());
        let mut idents: Vec<(Rc<str>, Option<usize>)> = Vec::with_capacity(shared.middle.len());
        let mut fresh = Vec::new();
        let mut inner = frames.to_vec();
        inner.push(Frame::default());
        for (k, found) in shared.middle.iter().enumerate() {
            match *found {
                Some(old) => {
                    if std::mem::replace(&mut kept[old - start], true) {
                        // One object at two new positions: a repeated key.
                        return Ok(false);
                    }
                    keys.push(self.keys[old].clone());
                    idents.push((self.index.shared_key(old).unwrap().clone(), Some(old)));
                }
                None => {
                    u.work.rows_keyed += 1;
                    inner.last_mut().unwrap().item = Some(items[start + k].clone());
                    let value = u.eval(key, &inner)?;
                    let text = key_text(&value).ok_or(InstanceError::KeyKind {
                        region: self.region,
                    })?;
                    fresh.push(k);
                    keys.push(value);
                    idents.push((Rc::from(text), None));
                }
            }
        }
        // A new key must not repeat a kept one or another new one; then no
        // key repeats and every identity is its key's text.
        let mut seen = std::collections::BTreeSet::new();
        for &k in &fresh {
            let ident = &*idents[k].0;
            let repeats_kept = self
                .index
                .position(ident)
                .is_some_and(|old| old < start || old >= old_end || kept[old - start]);
            if repeats_kept || !seen.insert(ident) {
                return Ok(false);
            }
        }
        self.index
            .splice_keys(start, old_end, idents)
            .map_err(index_error)?;
        self.keys.splice(start..old_end, keys);
        self.string_keys = self.keys.iter().all(|key| key.as_str().is_some());
        Ok(true)
    }
}
