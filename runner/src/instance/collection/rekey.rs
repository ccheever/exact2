//! Keying an answer that shares objects with the previous one (LLP 1053 §0
//! G8): a live insert keys the rows it added, not the list.
//!
//! A row's key is its key expression over its item; while the key's other
//! inputs and its index (if read) are unchanged, the same item has the same
//! key. An item that is the same object as a previous item ([`crate::compare::shared`])
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
    /// Key every item, reusing a key only where the list kept its length and an item its
    /// position ([`Self::rekey_shared`] handles the rest). Whether any key changed.
    pub(super) fn rekey_full(
        &mut self,
        u: &mut Update<'_>,
        frames: &[Frame],
        key_code: exact_plan::Code,
        items: &[Value],
        compare_previous: bool,
        keys_stale: bool,
    ) -> Result<bool, InstanceError> {
        let reuse_items = compare_previous && !keys_stale;
        // No candidate N-vector or canonical strings until an actual key
        // mismatch. All equal results reuse the existing uniqueness proof.
        let mut changed = !compare_previous;
        let mut keys = Vec::new();
        let mut text_keys = Vec::new();
        if changed {
            keys.reserve(items.len());
            text_keys.reserve(items.len());
        }
        // Membership only: hashed, and holding the identities the index
        // keeps (no copy of each key's text).
        // Each identity's position: the index takes it as its key map.
        let mut unique: std::collections::HashMap<Rc<str>, usize> = Default::default();
        if changed {
            unique.reserve(items.len());
        }
        let mut dups = BTreeMap::new();
        // Each key's text, written here before its identity is made.
        let mut text = String::new();
        let mut inner = frames.to_vec();
        inner.push(Frame::default());
        for (position, item) in items.iter().enumerate() {
            let key = if reuse_items && crate::compare::same(item, &self.items[position]) {
                if !changed {
                    continue;
                }
                self.keys[position].clone()
            } else {
                u.work.rows_keyed += 1;
                inner.last_mut().unwrap().item = Some(item.clone());
                inner.last_mut().unwrap().index = Some(position);
                u.eval(key_code, &inner)?
            };
            if !changed && same_key(&key, &self.keys[position]) {
                continue;
            }
            if !changed {
                // Seed only the already-validated prefix. Checking each
                // following row now preserves duplicate-before-later-trap.
                keys.reserve(items.len());
                text_keys.reserve(items.len());
                unique.reserve(items.len());
                for prefix in 0..position {
                    let text = self.index.shared_key(prefix).unwrap().clone();
                    unique.insert(text.clone(), prefix);
                    text_keys.push(text);
                    keys.push(self.keys[prefix].clone());
                }
                dups.extend(self.dups.range(..position).map(|(p, d)| (*p, *d)));
                changed = true;
            }
            text.clear();
            if !super::key_text_into(&key, &mut text) {
                return Err(InstanceError::KeyKind {
                    region: self.region,
                });
            }
            // A repeated key is the data's error: the repeat takes the
            // next identity in order (as a `for` does).
            let mut dup = 0;
            let ident: Rc<str> = if unique.contains_key(text.as_str()) {
                let mut ident = text.clone();
                while unique.contains_key(ident.as_str()) {
                    dup += 1;
                    ident = super::disambiguate(text.clone(), dup);
                }
                Rc::from(ident)
            } else {
                Rc::from(text.as_str())
            };
            unique.insert(ident.clone(), position);
            if dup > 0 {
                dups.insert(position, dup);
            }
            keys.push(key);
            text_keys.push(ident);
        }
        if changed {
            self.index
                .replace_keys_indexed(text_keys, unique)
                .map_err(index_error)?;
            self.string_keys = keys.iter().all(|key| key.as_str().is_some());
            self.keys = keys;
            self.dups = dups;
        }
        Ok(changed)
    }
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
        // Shared objects can move. A positional key must be evaluated at
        // the new position, including the otherwise-reused suffix.
        if vm::instructions(u.env.plan.code(key))
            .any(|i| i.is_ok_and(|i| i.op == exact_plan::Opcode::LoadIndex))
        {
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
                    inner.last_mut().unwrap().index = Some(start + k);
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
