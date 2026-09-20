//! The store: durable client state as the runner holds it.
//!
//! @ref LLP 1018 D1 (snapshot in, writes out) / D3 (the grant is the load list)
//!
//! The host reads the app's kept secrets into a snapshot before boot and
//! persists the writes after each commit; the runner never reaches a
//! platform. A read is a map lookup; a write is a map update and a
//! [`StoreWrite`] for the host. The `secret.keep <name>` lines of the data
//! crate's grants say which names exist: an ungranted name reads as absent
//! and refuses a write, identically on every host.

use crate::runner::DataError;

/// Durable client state as the runner holds it (LLP 1018 D1): the host's
/// snapshot of the app's kept secrets, read before boot, and the writes
/// since, which the host persists after each commit. The runner never
/// reaches a platform — a read is a map lookup; a write is a map update and
/// a [`StoreWrite`] for the host. The grant (`secret.keep <name>` lines in
/// [`DataSource::grants`]) is the load list: an ungranted name reads as
/// absent and refuses a write, identically on every host.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Store {
    granted: Vec<String>,
    /// A child executor may see only the intersection with its own grants.
    restricted: bool,
    values: std::collections::BTreeMap<String, String>,
    writes: Vec<StoreWrite>,
    /// Monotonic within a transaction; restored with a refused transaction.
    revision: u64,
    /// Device-state observations: secrets and external storage both make
    /// resources device-dependent at bake (LLP 1018 D4 / LLP 1027 D4).
    reads: std::cell::Cell<usize>,
}

/// One write for the host to persist, in order: `value` `None` forgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreWrite {
    /// The secret's name.
    pub name: String,
    /// The value to keep, or `None` to forget.
    pub value: Option<String>,
}

/// Why the store refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The name is not in the app's `secret.keep` grants.
    Refused(String),
}

impl From<StoreError> for DataError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Refused(name) => {
                DataError::Unavailable(format!("secret {name} is not granted"))
            }
        }
    }
}

pub(crate) struct StoreCheckpoint {
    values: std::collections::BTreeMap<String, String>,
    revision: u64,
    /// How many writes stood at the checkpoint: the ones after it are new.
    pub(crate) writes: usize,
}

impl Store {
    /// The store for `grants` (its `secret.keep <name>` lines), filled from
    /// `snapshot`; an entry the grant does not name is dropped.
    pub fn new(grants: &str, snapshot: impl IntoIterator<Item = (String, String)>) -> Store {
        let granted: Vec<String> = grants
            .lines()
            .filter_map(|l| {
                let mut p = l.split_whitespace();
                (p.next()? == "secret.keep")
                    .then(|| p.next())
                    .flatten()
                    .map(str::to_string)
            })
            .collect();
        let values = snapshot
            .into_iter()
            .filter(|(n, _)| granted.iter().any(|g| g == n) || Store::is_kept(n))
            .collect();
        Store {
            granted,
            restricted: false,
            values,
            writes: Vec::new(),
            revision: 0,
            reads: std::cell::Cell::new(0),
        }
    }

    /// The names the grant allows, in declaration order.
    pub fn granted(&self) -> &[String] {
        &self.granted
    }

    /// Run one child executor with only its admitted secret grants. Values,
    /// observations, and writes remain in this transaction; even unwinding
    /// restores the caller's grant scope. Nested scopes can only narrow access.
    pub fn with_grants<T>(&mut self, grants: &str, call: impl FnOnce(&mut Store) -> T) -> T {
        struct Scope<'a> {
            store: &'a mut Store,
            granted: Vec<String>,
            restricted: bool,
        }
        impl Drop for Scope<'_> {
            fn drop(&mut self) {
                self.store.granted = std::mem::take(&mut self.granted);
                self.store.restricted = self.restricted;
            }
        }
        let admitted = Store::new(grants, []).granted;
        let narrowed = self
            .granted
            .iter()
            .filter(|name| admitted.contains(name))
            .cloned()
            .collect();
        let granted = std::mem::replace(&mut self.granted, narrowed);
        let restricted = std::mem::replace(&mut self.restricted, true);
        let scope = Scope {
            store: self,
            granted,
            restricted,
        };
        call(scope.store)
    }

    fn is_granted(&self, name: &str) -> bool {
        !Self::is_kept(name) && self.granted.iter().any(|g| g == name)
    }

    /// The runner's own names (LLP 1027 D4): a kept answer for a
    /// store-reading resource lives beside the app's secrets under this
    /// prefix, persisted by the host like any write, never granted to the
    /// app — `get` sees nothing there, `set` refuses — and never counted as
    /// a read or a revision.
    pub const KEPT: &'static str = "exact.kept.";

    fn is_kept(name: &str) -> bool {
        name.starts_with(Store::KEPT)
    }

    /// A kept answer, by the runner (uncounted, ungated).
    pub(crate) fn kept(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// Keep an answer for the next boot, by the runner: a write for the host
    /// to persist that bumps no revision, so store-reading resources do not
    /// re-answer because one of them was answered.
    pub(crate) fn keep(&mut self, name: &str, value: &str) {
        debug_assert!(Store::is_kept(name));
        if self.values.get(name).map(String::as_str) == Some(value) {
            return;
        }
        self.values.insert(name.to_string(), value.to_string());
        self.writes.push(StoreWrite {
            name: name.to_string(),
            value: Some(value.to_string()),
        });
    }

    /// Discard an incompatible runner-owned answer without dirtying app secrets.
    pub(crate) fn forget_kept(&mut self, name: &str) {
        debug_assert!(Store::is_kept(name));
        if self.values.remove(name).is_some() {
            self.writes.push(StoreWrite {
                name: name.to_string(),
                value: None,
            });
        }
    }

    /// The kept value under `name` — `None` when nothing is kept, or when
    /// `name` is not granted (the same fact, as `process.env` has it).
    pub fn get(&self, name: &str) -> Option<&str> {
        if Store::is_kept(name) {
            return None;
        }
        self.reads.set(self.reads.get() + 1);
        if !self.is_granted(name) {
            return None;
        }
        self.values.get(name).map(String::as_str)
    }

    /// Keep `value` under `name`, replacing what was there; refused outside
    /// the grant.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), StoreError> {
        if !self.is_granted(name) {
            return Err(StoreError::Refused(name.to_string()));
        }
        self.values.insert(name.to_string(), value.to_string());
        self.revision += 1;
        self.writes.push(StoreWrite {
            name: name.to_string(),
            value: Some(value.to_string()),
        });
        Ok(())
    }

    /// Forget `name` (the host forgets it too, whether or not anything was
    /// kept); refused outside the grant.
    pub fn forget(&mut self, name: &str) -> Result<(), StoreError> {
        if !self.is_granted(name) {
            return Err(StoreError::Refused(name.to_string()));
        }
        self.values.remove(name);
        self.revision += 1;
        self.writes.push(StoreWrite {
            name: name.to_string(),
            value: None,
        });
        Ok(())
    }

    /// The names with a kept value, sorted — never the values.
    pub fn names(&self) -> Vec<&str> {
        self.values
            .keys()
            .filter(|name| !self.restricted || self.is_granted(name))
            .map(String::as_str)
            .collect()
    }

    /// Everything kept: what a reload carries (`Carried::store`).
    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.values
            .iter()
            .filter(|(name, _)| !self.restricted || self.is_granted(name))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// The writes since the last take, in order.
    pub fn take_writes(&mut self) -> Vec<StoreWrite> {
        if !self.restricted {
            return std::mem::take(&mut self.writes);
        }
        // A child cannot inspect another child's pending secret values.
        let (visible, hidden) = std::mem::take(&mut self.writes)
            .into_iter()
            .partition(|write: &StoreWrite| self.is_granted(&write.name));
        self.writes = hidden;
        visible
    }

    /// Observe app filesystem or database access without reading a secret.
    /// The existing resource dependency marker also governs external storage:
    /// bake emits a placeholder, then the host refreshes after first pixel.
    /// This changes neither the store revision nor its persisted values.
    pub fn observe_external_read(&self) {
        self.reads.set(self.reads.get() + 1);
    }

    /// How many device-state observations so far, including secret reads.
    pub fn reads(&self) -> usize {
        self.reads.get()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn checkpoint(&self) -> StoreCheckpoint {
        StoreCheckpoint {
            values: self.values.clone(),
            revision: self.revision,
            writes: self.writes.len(),
        }
    }

    pub(crate) fn restore(&mut self, c: StoreCheckpoint) {
        self.values = c.values;
        self.revision = c.revision;
        self.writes.truncate(c.writes);
    }
}

impl Store {
    /// The writes recorded so far, in order (the runner journals the new
    /// ones once a commit stands).
    pub(crate) fn writes(&self) -> &[StoreWrite] {
        &self.writes
    }
}
