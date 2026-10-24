//! The store's half of presence secrets (LLP 1133.010 D5): a presence
//! name's occupancy, never its bytes while it is locked or pending.
//!
//! Occupancy is a runner-owned entry, `exact.presence.<name>`, beside the
//! app's values, so the boot snapshot, a reload's carry and every per-call
//! snapshot carry it as an entry of its own: an empty value is not a locked
//! name. `exact.presence.` alone says the host keeps locked secrets now.

use super::{Store, StoreError, StoreWrite};
use crate::presence::Occupancy;
use exact_grants::Presence;

pub(super) fn locked_message(name: &str) -> String {
    format!(
        "InvalidAccessError: secret {name} is locked: a locked secret is read with store.unlock"
    )
}

pub(super) fn unsupported_message(name: &str) -> String {
    format!(
        "NotSupportedError: secret {name} is kept locked, and this device keeps no locked secret now (no device credential, or a target without them)"
    )
}

/// Whether `name` is a presence entry these lines admit: the capability, or
/// the occupancy of one of their names.
pub(super) fn presence_entry(name: &str, presence: &[(String, Presence)]) -> bool {
    match name.strip_prefix(Store::PRESENCE) {
        Some("") => true,
        Some(n) => presence.iter().any(|(p, _)| p == n),
        None => false,
    }
}

/// Whether a snapshot entry may enter a store of these lines: an app value
/// is checked against the grant elsewhere; of the runner's own, a presence
/// entry only for a name these lines carry.
pub(super) fn admits_entry(name: &str, presence: &[(String, Presence)]) -> bool {
    !name.starts_with(Store::PRESENCE) || presence_entry(name, presence)
}

fn entry_name(name: &str) -> String {
    format!("{}{name}", Store::PRESENCE)
}

/// Make `values` hold no bytes for a locked or pending name: a mandatory
/// name's plain bytes with no occupancy said are pending (a host without a
/// backend, or one that did not reconcile), and dropped. Whether any name
/// is locked or pending.
pub(super) fn settle_entries(
    values: &mut exact_kernel::SortedMap<String, Vec<u8>>,
    presence: &[(String, Presence)],
) -> bool {
    let mut locked = false;
    for (name, line) in presence {
        let key = entry_name(name);
        let said = values.get(&key).and_then(|w| Occupancy::from_word(w));
        let occupancy = match said {
            Some(o) => o,
            None if values.contains_key(name) && !line.optional => Occupancy::Pending,
            None if values.contains_key(name) => Occupancy::Plain,
            None => Occupancy::Absent,
        };
        values.insert(key, occupancy.word().as_bytes().to_vec());
        if occupancy.locked() {
            values.remove(name);
            locked = true;
        }
    }
    locked
}

impl Store {
    /// The runner's own prefix for presence occupancy (LLP 1133.010 D4); the
    /// prefix alone is the entry that says the host keeps locked secrets.
    pub const PRESENCE: &'static str = exact_grants::PRESENCE_PREFIX;

    /// `name`'s presence policy, when its line carries one in this scope.
    pub fn presence_of(&self, name: &str) -> Option<&Presence> {
        (self.presence.iter())
            .find(|(n, _)| n == name)
            .map(|(_, p)| p)
    }

    /// The names with a presence policy in this scope, in declaration order.
    pub fn presence_names(&self) -> Vec<(&str, &Presence)> {
        (self.presence.iter())
            .map(|(n, p)| (n.as_str(), p))
            .collect()
    }

    /// `name`'s occupancy, uncounted; `None` for a name with no policy.
    pub fn occupancy(&self, name: &str) -> Option<Occupancy> {
        self.presence_of(name)?;
        let said = self.values.get(&entry_name(name));
        Some(said.and_then(|w| Occupancy::from_word(w)).unwrap_or(
            if self.values.contains_key(name) {
                Occupancy::Plain
            } else {
                Occupancy::Absent
            },
        ))
    }

    /// Whether this host keeps locked secrets now: it has a presence backend,
    /// and a device credential is set (D5).
    pub fn presence_capable(&self) -> bool {
        self.values.get(Store::PRESENCE).is_some_and(|v| v == b"1")
    }

    /// Say whether this host keeps locked secrets now (the runner, as the
    /// device's credential comes and goes). Neither a write nor a revision.
    pub fn set_presence_capable(&mut self, capable: bool) {
        if capable {
            self.values
                .insert(Store::PRESENCE.to_owned(), b"1".to_vec());
        } else {
            self.values.remove(Store::PRESENCE);
        }
    }

    /// Whether a value is kept under `name`, locked or not: a device read.
    pub fn has(&self, name: &str) -> bool {
        self.reads.set(self.reads.get() + 1);
        if !self.is_granted(name) {
            return false;
        }
        match self.occupancy(name) {
            Some(o) => o != Occupancy::Absent,
            None => self.values.contains_key(name),
        }
    }

    /// Whether `name` is kept under its presence policy, or awaits a move to
    /// it: a device read.
    pub fn locked(&self, name: &str) -> bool {
        self.reads.set(self.reads.get() + 1);
        self.is_granted(name) && self.occupancy(name).is_some_and(Occupancy::locked)
    }

    /// Whether a write of `name` is kept locked: it is locked or pending, or
    /// absent under a mandatory line.
    pub(super) fn writes_locked(&self, name: &str) -> bool {
        match (self.presence_of(name), self.occupancy(name)) {
            (Some(p), Some(Occupancy::Absent)) => !p.optional,
            (_, Some(o)) => o.locked(),
            _ => false,
        }
    }

    /// A locked write: refused at once when this device keeps no locked
    /// secret; otherwise a write for the host's backend, which keeps the
    /// value, and no bytes here.
    pub(super) fn set_locked(&mut self, name: &str, value: &str) -> Result<(), StoreError> {
        if !self.presence_capable() {
            return Err(StoreError::NotSupported(name.to_owned()));
        }
        let key = entry_name(name);
        self.remember(name);
        self.remember(&key);
        self.values.remove(name);
        if self.occupancy(name) == Some(Occupancy::Absent) {
            self.values
                .insert(key, Occupancy::Locked.word().as_bytes().to_vec());
        }
        self.revision += 1;
        self.writes.push(StoreWrite {
            name: name.to_string(),
            value: Some(value.into()),
        });
        Ok(())
    }

    /// A forgotten presence name reads absent.
    pub(super) fn absent(&mut self, name: &str) {
        if self.presence_of(name).is_some() {
            let key = entry_name(name);
            self.remember(&key);
            self.values
                .insert(key, Occupancy::Absent.word().as_bytes().to_vec());
        }
    }

    /// Adopt the backend's occupancy of `name` (the runner, at the start of a
    /// commit): a locked or pending name's bytes go; a plain one takes
    /// `plain`, its plain copy. A move the app asked for (`protect`,
    /// `unprotect`) is told by its call's answer, after which the app asks
    /// again what it shows; asking every store reader again would also ask
    /// sources that only touched the device (a socket's resource, which
    /// would reconnect). Only a value gone from under the app (a pending
    /// copy that could not be locked, an item the platform dropped) asks
    /// the store's readers again. Whether it does.
    pub fn adopt_occupancy(
        &mut self,
        name: &str,
        occupancy: Occupancy,
        plain: impl FnOnce() -> Option<Vec<u8>>,
    ) -> bool {
        let Some(before) = self.occupancy(name) else {
            return false;
        };
        let key = entry_name(name);
        let bytes = self.values.contains_key(name);
        if before == occupancy && (occupancy != Occupancy::Plain || bytes) {
            return false;
        }
        self.remember(name);
        self.remember(&key);
        self.values
            .insert(key, occupancy.word().as_bytes().to_vec());
        match occupancy {
            Occupancy::Plain if !bytes => {
                if let Some(value) = plain() {
                    self.values.insert(name.to_owned(), value);
                }
            }
            Occupancy::Plain => {}
            _ => {
                self.values.remove(name);
            }
        }
        let gone = before != Occupancy::Absent && occupancy == Occupancy::Absent;
        if gone {
            self.revision += 1;
        }
        gone
    }

    /// The snapshot a child executor takes for one call (a logic module, a
    /// worker): the values of `granted`, the occupancy of those with a
    /// presence policy and whether locked secrets are kept, never the
    /// runner's kept answers or persisted states.
    pub fn call_snapshot(&self, granted: &[String]) -> Vec<(String, Vec<u8>)> {
        let named = |n: &str| granted.iter().any(|g| g == n);
        (self.snapshot().into_iter())
            .filter(|(name, _)| match name.strip_prefix(Store::PRESENCE) {
                Some(n) => n.is_empty() || named(n),
                None => !Store::runner_owned(name) && named(name),
            })
            .collect()
    }

    /// Whether the launch opened with a presence name locked or pending: it
    /// shows no kept answer (D5).
    pub fn opened_locked(&self) -> bool {
        self.opened_locked
    }

    /// The locked name an answer read with `get` since the last take: its
    /// answer fails, as data code's `store.get` throws.
    pub fn take_locked_read(&self) -> Option<String> {
        self.locked_read.borrow_mut().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::DataError;

    const LINES: &str = "secret.keep token user-presence optional\nsecret.keep key user-presence\nsecret.keep plain";

    fn occupancy(name: &str, o: Occupancy) -> (String, Vec<u8>) {
        (entry_name(name), o.word().as_bytes().to_vec())
    }

    /// No locked name's bytes anywhere a value of the store can go.
    fn holds_no(store: &Store, secret: &[u8]) {
        let snapshot = store.snapshot();
        assert!(
            !snapshot
                .iter()
                .any(|(_, v)| v.windows(secret.len()).any(|w| w == secret)),
            "{snapshot:?}"
        );
        assert!(!store.names().iter().any(|n| n.contains("secret")));
    }

    #[test]
    fn a_locked_or_pending_name_holds_no_bytes_from_any_snapshot() {
        // A host that read a plain copy of a locked name, and one of a
        // mandatory name it never reconciled: both dropped.
        let store = Store::new(
            LINES,
            [
                ("token".into(), b"secret-1".to_vec()),
                ("key".into(), b"secret-2".to_vec()),
                ("plain".into(), b"open".to_vec()),
                occupancy("token", Occupancy::Locked),
                (Store::PRESENCE.into(), b"1".to_vec()),
            ],
        );
        holds_no(&store, b"secret");
        assert_eq!(store.occupancy("token"), Some(Occupancy::Locked));
        assert_eq!(store.occupancy("key"), Some(Occupancy::Pending));
        assert_eq!(store.occupancy("plain"), None);
        assert!(store.opened_locked() && store.opened_with_secrets());
        assert!(store.locked("token") && store.locked("key") && store.has("key"));
        assert!(!store.locked("plain") && store.has("plain"));
        assert_eq!(store.read("token"), Err(StoreError::Locked("token".into())));
        assert_eq!(store.get("token"), None);
        assert_eq!(store.take_locked_read().as_deref(), Some("token"));
        assert_eq!(store.get("plain"), Some("open"));
        assert_eq!(store.take_locked_read(), None);
        // A carry, a per-call mirror and a narrowed child scope see the
        // occupancy and no bytes.
        let carried = Store::new(LINES, store.snapshot());
        holds_no(&carried, b"secret");
        assert!(carried.locked("token") && carried.opened_locked());
        let mirror = Store::module_mirror(LINES, store.snapshot());
        assert!(mirror.locked("key"));
        assert_eq!(
            DataError::from(mirror.read("key").unwrap_err()),
            DataError::Unavailable(locked_message("key"))
        );
        let mut parent = Store::new(LINES, store.snapshot());
        parent.with_grants("secret.keep token user-presence optional", |child| {
            assert!(child.locked("token"));
            assert!(!child.has("key"));
            let seen = child.snapshot();
            assert!(seen.iter().any(|(n, _)| n == &entry_name("token")));
            assert!(
                !seen.iter().any(|(n, _)| n == &entry_name("key")),
                "{seen:?}"
            );
        });
        // A refresh from the host keeps the occupancy and drops bytes.
        let mut refreshed = Store::new(LINES, store.snapshot());
        refreshed.refresh_snapshot(vec![("token".into(), b"secret-3".to_vec())]);
        holds_no(&refreshed, b"secret");
        assert!(refreshed.locked("token"));
    }

    #[test]
    fn a_locked_write_goes_to_the_host_and_stays_out_of_the_store() {
        let mut store = Store::new(LINES, [(Store::PRESENCE.into(), b"1".to_vec())]);
        assert_eq!(store.occupancy("key"), Some(Occupancy::Absent));
        let before = store.revision();
        let c = store.checkpoint();
        store.set("key", "secret-1").unwrap();
        assert!(store.locked("key") && store.revision() > before);
        holds_no(&store, b"secret");
        assert_eq!(
            store.writes(),
            [StoreWrite {
                name: "key".into(),
                value: Some(b"secret-1".to_vec())
            }]
        );
        // A refused transaction restores the occupancy, and never bytes.
        store.restore(c);
        assert_eq!(store.occupancy("key"), Some(Occupancy::Absent));
        holds_no(&store, b"secret");
        // An optional name starts plain.
        store.set("token", "open").unwrap();
        assert_eq!(store.get("token"), Some("open"));
        // No locked secret here: a mandatory write is refused at once, and
        // an optional one stays plain.
        let mut web = Store::new(LINES, []);
        assert_eq!(
            web.set("key", "secret"),
            Err(StoreError::NotSupported("key".into()))
        );
        assert!(web.set("token", "open").is_ok());
        assert!(web.take_writes().iter().all(|w| w.name == "token"));
        // A forgotten name reads absent.
        store.set("key", "secret-2").unwrap();
        store.forget("key").unwrap();
        assert_eq!(store.occupancy("key"), Some(Occupancy::Absent));
        assert!(!store.has("key"));
    }

    #[test]
    fn the_backend_s_occupancy_is_adopted_with_the_plain_copy_only_when_plain() {
        let mut store = Store::new(LINES, [("token".into(), b"open".to_vec())]);
        assert_eq!(store.occupancy("token"), Some(Occupancy::Plain));
        // A move the app asked for asks no reader again: its call's answer tells it.
        let revision = store.revision();
        assert!(!store.adopt_occupancy("token", Occupancy::Locked, || None));
        assert!(store.locked("token"));
        assert_eq!(store.read("token"), Err(StoreError::Locked("token".into())));
        holds_no(&store, b"open");
        assert!(!store.adopt_occupancy("token", Occupancy::Plain, || Some(b"open".to_vec())));
        assert_eq!(store.get("token"), Some("open"));
        assert_eq!(store.revision(), revision);
        // A value gone from under the app does.
        assert!(store.adopt_occupancy("token", Occupancy::Absent, || None));
        assert!(!store.has("token") && store.revision() > revision);
    }
}
