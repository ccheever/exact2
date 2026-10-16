//! The store: durable client state as the runner holds it.
//!
//! @ref LLP 1018 D1 (snapshot in, writes out) / D3 (the grant is the load list)
//!
//! The host reads the app's kept secrets into a snapshot before boot and
//! persists the writes after each commit; the runner never reaches a
//! platform. A read is a map lookup; a write is a map update and a
//! [`StoreWrite`] for the host. The `secret.keep <name>` lines of the data
//! crate's grants say which names exist: an ungranted name reads as absent
//! and refuses a write, identically on every host. The grants are read by
//! one parse of the whole set ([`crate::grants`]): a set that does not parse
//! names nothing, as a native host holds nothing, and each refusal says why.

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
    scope_reader: ScopeReader,
    /// Why nothing is granted: the grants did not parse.
    unparsed: Option<String>,
    /// A child executor may see only the intersection with its own grants.
    restricted: bool,
    values: exact_kernel::SortedMap<String, Vec<u8>>,
    /// The snapshot it opened with held a granted secret (LLP 1027 D4,
    /// 2026-10-10): a launch's fact, unchanged by later writes.
    opened_with_secrets: bool,
    writes: Vec<StoreWrite>,
    /// Monotonic within a transaction; restored with a refused transaction.
    revision: u64,
    /// Device-state observations: secrets and external storage both make
    /// resources device-dependent at bake (LLP 1018 D4 / LLP 1027 D4).
    reads: std::cell::Cell<usize>,
    /// Of those, draws of secure randomness (LLP 1069.005 D2).
    entropy: std::cell::Cell<usize>,
    /// Device topics the answer being made watches (`native.watch`, LLP
    /// 1016.002): taken by the runner after each answer.
    topics: std::cell::RefCell<Vec<String>>,
    /// Socket operations the mutation being answered staged, by the index
    /// of the resource that owns each socket (LLP 1016.000 D7): taken by
    /// the runner after each mutation's answer, which publishes them only
    /// if the answer stands.
    sockets: Vec<(usize, crate::SocketOp)>,
    /// Each open socket's buffered bytes at the start of the turn, by its
    /// resource's index: what its handle's `bufferedAmount` starts from.
    socket_buffered: Vec<(usize, u64)>,
    /// Each entry's value before its first write since the open checkpoint:
    /// what a refused transaction puts back. A checkpoint costs what the
    /// transaction writes, not the store's size. Checkpoints do not nest.
    undo: Vec<(String, Option<Vec<u8>>)>,
    /// Bytes copied into `undo` since the store was made.
    copied: usize,
}

// A module mirror must remain importless. Its host already enforced the whole
// grant set; scopes there read only secret names. Ordinary stores retain the
// strict reader, including for nested child scopes.
// The strict reader reads through the archive's grant grammar (LLP
// 1047.001), so a store names none when the archive links no I/O.
#[derive(Debug, Clone, Copy, Default)]
enum ScopeReader {
    #[default]
    Module,
    Strict(crate::grants::GrantsLink),
}
impl ScopeReader {
    fn read(self, grants: &str) -> Vec<String> {
        match self {
            ScopeReader::Module => module_secret_names(grants),
            ScopeReader::Strict(link) => crate::grants::parse_with(grants, link)
                .map(|set| set.secrets().map(str::to_string).collect())
                .unwrap_or_default(),
        }
    }
}
impl PartialEq for ScopeReader {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ScopeReader::Module, ScopeReader::Module) => true,
            (ScopeReader::Strict(a), ScopeReader::Strict(b)) => match (a, b) {
                (Some(a), Some(b)) => std::ptr::fn_addr_eq(*a, *b),
                (None, None) => true,
                _ => false,
            },
            _ => false,
        }
    }
}
fn module_secret_names(grants: &str) -> Vec<String> {
    grants
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if words.next()? != "secret.keep" {
                return None;
            }
            let name = words.next()?;
            (exact_grants::valid_name(name) && words.next().is_none()).then(|| name.to_string())
        })
        .collect()
}

/// One write for the host to persist, in order: `value` `None` forgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreWrite {
    /// The secret's name.
    pub name: String,
    /// The value to keep, or `None` to forget: a secret's or a state's text, a kept answer's canonical bytes.
    pub value: Option<Vec<u8>>,
}

/// Why the store refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The name is not in the app's `secret.keep` grants.
    Refused(String),
    /// The app's grants did not parse, so they name nothing: the name, then
    /// why ([`crate::grants::refusal`]).
    Unparsed(String, String),
}

impl From<StoreError> for DataError {
    fn from(e: StoreError) -> Self {
        DataError::Unavailable(match e {
            StoreError::Refused(name) => format!("secret {name} is not granted"),
            StoreError::Unparsed(name, why) => format!("secret {name} is not granted: {why}"),
        })
    }
}

pub(crate) struct StoreCheckpoint {
    revision: u64,
    /// How many writes stood at the checkpoint: the ones after it are new.
    pub(crate) writes: usize,
}

impl Store {
    /// The store for `grants` (its `secret.keep <name>` lines, when the whole
    /// set parses), filled from `snapshot`; an entry the grant does not name
    /// is dropped. A value is bytes: a secret's text, or the runner's own.
    pub fn new(grants: &str, snapshot: impl IntoIterator<Item = (String, Vec<u8>)>) -> Store {
        // One body for every caller's snapshot type (LLP 1047 §6).
        Store::from_snapshot(
            grants,
            snapshot.into_iter().collect(),
            Some(crate::grants::validate),
        )
    }

    /// [`Store::new`] reading I/O grants through what the archive links
    /// (LLP 1047.001).
    pub fn new_linked(
        grants: &str,
        snapshot: impl IntoIterator<Item = (String, Vec<u8>)>,
        link: crate::grants::GrantsLink,
    ) -> Store {
        Store::from_snapshot(grants, snapshot.into_iter().collect(), link)
    }

    fn from_snapshot(
        grants: &str,
        snapshot: Vec<(String, Vec<u8>)>,
        link: crate::grants::GrantsLink,
    ) -> Store {
        let (granted, unparsed): (Vec<String>, _) = match crate::grants::parse_with(grants, link) {
            Ok(set) => (set.secrets().map(str::to_string).collect(), None),
            Err(errors) => (Vec::new(), Some(crate::grants::refusal(&errors))),
        };
        Self::from_names(granted, unparsed, ScopeReader::Strict(link), snapshot)
    }

    /// Local mirror inside an importless logic module (LLP 1029): the host
    /// validates the complete declaration before loading the snapshot and
    /// atomically validates every returned write. This grants no host access.
    /// Host runners must use `new`, which validates the declaration itself.
    pub fn module_mirror(grants: &str, snapshot: Vec<(String, Vec<u8>)>) -> Store {
        Self::from_names(
            module_secret_names(grants),
            None,
            ScopeReader::default(),
            snapshot,
        )
    }

    fn from_names(
        granted: Vec<String>,
        unparsed: Option<String>,
        scope_reader: ScopeReader,
        snapshot: Vec<(String, Vec<u8>)>,
    ) -> Store {
        let values = snapshot
            .into_iter()
            .filter(|(n, _)| granted.iter().any(|g| g == n) || Store::is_kept(n))
            .collect::<exact_kernel::SortedMap<_, _>>();
        let opened_with_secrets = values.keys().any(|n| !Store::is_kept(n));
        Store {
            granted,
            scope_reader,
            unparsed,
            restricted: false,
            values,
            opened_with_secrets,
            writes: Vec::new(),
            revision: 0,
            reads: std::cell::Cell::new(0),
            entropy: std::cell::Cell::new(0),
            topics: Default::default(),
            sockets: Vec::new(),
            socket_buffered: Vec::new(),
            undo: Vec::new(),
            copied: 0,
        }
    }

    /// The names the grant allows, in declaration order.
    pub fn granted(&self) -> &[String] {
        &self.granted
    }

    /// Why no name is granted, when the grants did not parse: what a host
    /// journals once and each refusal carries.
    pub fn unparsed(&self) -> Option<&str> {
        self.unparsed.as_deref()
    }

    fn refused(&self, name: &str) -> StoreError {
        match &self.unparsed {
            Some(why) => StoreError::Unparsed(name.to_string(), why.clone()),
            None => StoreError::Refused(name.to_string()),
        }
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
        let admitted = self.scope_reader.read(grants);
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
    /// The most the kept answers hold together: the runner keeps no more,
    /// and a host's snapshot need read no more (LLP 1027 D4, 2026-10-16).
    pub const KEPT_TOTAL_BYTES: usize = crate::runner::kept::KEPT_TOTAL_BYTES;

    /// The runner's persisted states (LLP 1116 D5): `state … persist`'s
    /// value lives under this prefix and the state's name, as JSON text,
    /// owned by the runner as a kept answer is. A host keeps these in the
    /// platform's preference store, never its secret store.
    pub const STATE: &'static str = "exact.state.";

    fn is_kept(name: &str) -> bool {
        Store::runner_owned(name)
    }

    /// Whether `name` is the runner's own — a kept answer or a persisted
    /// state — which no grant reaches.
    pub fn runner_owned(name: &str) -> bool {
        name.starts_with(Store::KEPT) || name.starts_with(Store::STATE)
    }

    /// A kept answer, by the runner (uncounted, ungated).
    pub(crate) fn kept(&self, name: &str) -> Option<&[u8]> {
        self.values.get(name).map(Vec::as_slice)
    }

    /// Every kept answer's name and length, by the runner.
    pub(crate) fn kept_lengths(&self) -> Vec<(String, usize)> {
        self.values
            .iter()
            .filter(|(name, _)| Store::is_kept(name))
            .map(|(name, value)| (name.clone(), value.len()))
            .collect()
    }

    /// Keep an answer for the next boot, by the runner: a write for the host
    /// to persist that bumps no revision, so store-reading resources do not
    /// re-answer because one of them was answered.
    pub(crate) fn keep(&mut self, name: &str, value: Vec<u8>) {
        debug_assert!(Store::is_kept(name));
        if self.values.get(name) == Some(&value) {
            return;
        }
        self.remember(name);
        self.values.insert(name.to_string(), value.clone());
        self.writes.push(StoreWrite {
            name: name.to_string(),
            value: Some(value),
        });
    }

    /// Discard an incompatible runner-owned answer without dirtying app secrets.
    pub(crate) fn forget_kept(&mut self, name: &str) {
        debug_assert!(Store::is_kept(name));
        self.remember(name);
        if self.values.remove(name).is_some() {
            self.writes.push(StoreWrite {
                name: name.to_string(),
                value: None,
            });
        }
    }

    /// Whether the snapshot this store opened with held a granted secret:
    /// then an empty-store answer the bake computed does not describe this
    /// device (LLP 1027 D4, 2026-10-10). Uncounted, the runner's question.
    pub(crate) fn opened_with_secrets(&self) -> bool {
        self.opened_with_secrets
    }

    /// The kept value under `name` — `None` when nothing is kept, or when
    /// `name` is not granted (the same fact, as `process.env` has it), or
    /// when what is kept is not text.
    pub fn get(&self, name: &str) -> Option<&str> {
        if Store::is_kept(name) {
            return None;
        }
        self.reads.set(self.reads.get() + 1);
        if !self.is_granted(name) {
            return None;
        }
        self.values
            .get(name)
            .and_then(|v| std::str::from_utf8(v).ok())
    }

    /// Keep `value` under `name`, replacing what was there; refused outside
    /// the grant. Keeping what is already kept changes nothing, as Web
    /// Storage's `setItem` does nothing then: no revision and no write, so a
    /// source that writes as it answers does not make the store's readers
    /// stale with a value they already have.
    pub fn set(&mut self, name: &str, value: &str) -> Result<(), StoreError> {
        if !self.is_granted(name) {
            return Err(self.refused(name));
        }
        if self.values.get(name).map(Vec::as_slice) == Some(value.as_bytes()) {
            return Ok(());
        }
        self.remember(name);
        self.values.insert(name.to_string(), value.into());
        self.revision += 1;
        self.writes.push(StoreWrite {
            name: name.to_string(),
            value: Some(value.into()),
        });
        Ok(())
    }

    /// Forget `name` (the host forgets it too, whether or not anything was
    /// kept); refused outside the grant.
    pub fn forget(&mut self, name: &str) -> Result<(), StoreError> {
        if !self.is_granted(name) {
            return Err(self.refused(name));
        }
        self.remember(name);
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
    pub fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
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

    /// The answer being made drew secure randomness (LLP 1069.005 D2): a
    /// device read like any other, and one bake compiles no value for, since
    /// that value would be one draw shared by every install.
    pub fn observe_entropy(&self) {
        self.observe_external_read();
        self.entropy.set(self.entropy.get() + 1);
    }

    /// How many draws of secure randomness so far.
    pub fn entropy_draws(&self) -> usize {
        self.entropy.get()
    }

    /// The answer being made watches `topic`: the device announces when it
    /// changes, and the resource is asked again (LLP 1016.002). A watched
    /// answer is the device's, so it counts as a read too.
    pub fn observe_topic(&self, topic: &str) {
        self.observe_external_read();
        let mut topics = self.topics.borrow_mut();
        if !topics.iter().any(|t| t == topic) {
            topics.push(topic.to_owned());
        }
    }

    /// Stage `op` on the socket of resource `resource` for the mutation
    /// being answered (LLP 1016.000 D7).
    pub fn stage_socket(&mut self, resource: usize, op: crate::SocketOp) {
        self.sockets.push((resource, op));
    }

    /// Set the turn's socket snapshot (see `socket_buffered`).
    pub fn set_socket_buffered(&mut self, snapshot: Vec<(usize, u64)>) {
        self.socket_buffered = snapshot;
    }

    /// Each open socket's buffered bytes at the start of this turn, by the
    /// index of the resource that owns it.
    pub fn socket_buffered(&self) -> &[(usize, u64)] {
        &self.socket_buffered
    }

    /// The socket operations staged since the last take.
    pub fn take_sockets(&mut self) -> Vec<(usize, crate::SocketOp)> {
        std::mem::take(&mut self.sockets)
    }

    /// The topics watched since the last take.
    pub fn take_topics(&self) -> Vec<String> {
        std::mem::take(&mut self.topics.borrow_mut())
    }

    /// How many device-state observations so far, including secret reads.
    pub fn reads(&self) -> usize {
        self.reads.get()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// Open a checkpoint (the last one's undo entries are no longer needed).
    pub(crate) fn checkpoint(&mut self) -> StoreCheckpoint {
        self.undo.clear();
        StoreCheckpoint {
            revision: self.revision,
            writes: self.writes.len(),
        }
    }

    /// Put back every entry written since `c`, newest first.
    pub(crate) fn restore(&mut self, c: StoreCheckpoint) {
        while let Some((name, old)) = self.undo.pop() {
            match old {
                Some(value) => self.values.insert(name, value),
                None => self.values.remove(&name),
            };
        }
        self.revision = c.revision;
        self.writes.truncate(c.writes);
    }

    /// Record `name`'s value before a write, for `restore`.
    fn remember(&mut self, name: &str) {
        let old = self.values.get(name).cloned();
        self.copied += name.len() + old.as_ref().map_or(0, Vec::len);
        self.undo.push((name.to_string(), old));
    }

    /// Bytes copied to make transactions refusable, since the store was made.
    pub fn copied_bytes(&self) -> usize {
        self.copied
    }
}

impl Store {
    /// The writes recorded so far, in order (the runner journals the new
    /// ones once a commit stands).
    pub(crate) fn writes(&self) -> &[StoreWrite] {
        &self.writes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_mirror_has_only_its_host_endowed_secret_scope() {
        let mut mirror = Store::module_mirror(
            "secret.keep token\nnet.fetch https://example.test",
            vec![
                ("token".into(), "value".into()),
                ("other".into(), "hidden".into()),
            ],
        );
        assert_eq!(mirror.get("token"), Some("value"));
        assert_eq!(mirror.get("other"), None);
        mirror.with_grants("secret.keep other", |s| {
            assert!(s.set("token", "no").is_err())
        });
        assert!(mirror.set("token", "yes").is_ok());
        let mut mirror = Store::module_mirror(
            "secret.keep exact.kept.token",
            vec![("exact.kept.token".into(), "hidden".into())],
        );
        assert_eq!(mirror.get("exact.kept.token"), None);
        assert!(mirror.set("exact.kept.token", "no").is_err());
    }

    #[test]
    fn a_checkpoint_copies_what_the_transaction_writes_not_the_store() {
        let kept = format!("{}answer", Store::KEPT);
        let big = "x".repeat(1 << 20);
        let mut store = Store::new(
            "secret.keep token\n",
            [
                (kept.clone(), big.clone().into_bytes()),
                ("token".into(), "old".into()),
            ],
        );
        let c = store.checkpoint();
        store.set("token", "new").unwrap();
        assert!(store.copied_bytes() < 64, "{}", store.copied_bytes());
        store.restore(c);
        assert_eq!(store.get("token"), Some("old"));
        assert_eq!(store.kept(&kept), Some(big.as_bytes()));
        assert!(store.take_writes().is_empty());
    }

    #[test]
    fn grants_that_do_not_parse_name_no_secret_and_every_refusal_says_why() {
        // Crew's set (the port report of 2026-09-24, F1): one camelCase name
        // among good lines. A native host holds nothing; neither does this.
        let kept = format!("{}answer", Store::KEPT);
        let mut store = Store::new(
            "net.fetch https://crew.test\nsecret.keep crewHost\nsecret.keep token\n",
            [
                ("token".into(), "old".into()),
                ("crewHost".into(), "h".into()),
                (kept.clone(), "seed".into()),
            ],
        );
        assert!(store.granted().is_empty());
        assert_eq!(store.get("token"), None);
        assert_eq!(store.get("crewHost"), None);
        assert_eq!(store.names(), [kept.as_str()], "the runner's own stay");
        let why = "the app's grants did not parse: line 2: `crewHost` is not a secret name ([a-z0-9._-]{1,64})";
        assert_eq!(store.unparsed(), Some(why));
        for refused in [store.set("token", "new"), store.forget("token")] {
            assert_eq!(
                DataError::from(refused.unwrap_err()),
                DataError::Unavailable(format!("secret token is not granted: {why}"))
            );
        }
        assert!(store.take_writes().is_empty());
        // A child's scope reads the same parse.
        let mut parent = Store::new("secret.keep a\nsecret.keep b", []);
        parent.with_grants("secret.keep a\nsecret.keep B", |scope| {
            assert!(scope.set("a", "1").is_err());
        });
        assert_eq!(parent.unparsed(), None);
        assert!(parent.set("a", "1").is_ok());
    }

    #[test]
    fn keeping_what_is_kept_is_no_change() {
        let mut store = Store::new("secret.keep token\n", [("token".into(), "same".into())]);
        let before = store.revision();
        store.set("token", "same").unwrap();
        assert_eq!(store.revision(), before);
        assert!(store.take_writes().is_empty());
        store.set("token", "new").unwrap();
        assert_eq!(store.revision(), before + 1);
        assert_eq!(store.take_writes().len(), 1);
    }
}
