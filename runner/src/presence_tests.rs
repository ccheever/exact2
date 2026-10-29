//! The backend's guarantees (LLP 1133.010 D4, D6): moves are the host's,
//! each recorded before it begins, a half-done one resolves toward locked at
//! the next boot, and one queue per name orders every operation.

use super::*;
use std::cell::RefCell;

fn line(words: &str) -> Presence {
    exact_grants::line_presence(&format!("secret.keep a {words}"))
        .expect("a presence line")
        .1
}

fn optional() -> Presence {
    line("user-presence optional")
}

fn mandatory() -> Presence {
    line("user-presence")
}

/// A pump's environment, with faults armed by name: `fail` ones stop a move
/// once each; `hold` ones hold it while they are listed.
struct Drive {
    can_lock: bool,
    moves: bool,
    now_ms: f64,
    fail: RefCell<Vec<String>>,
    hold: RefCell<Vec<String>>,
}

impl Drive {
    fn new() -> Drive {
        Drive {
            can_lock: true,
            moves: true,
            now_ms: 0.0,
            fail: RefCell::default(),
            hold: RefCell::default(),
        }
    }

    fn pump(&self, backend: &mut Backend) -> Vec<Event> {
        let fail = |p: &str| {
            let mut fail = self.fail.borrow_mut();
            let hit = fail.iter().position(|f| f == p);
            hit.map(|i| fail.remove(i)).is_some()
        };
        let hold = |p: &str| self.hold.borrow().iter().any(|h| h == p);
        backend.pump(&Env {
            can_lock: self.can_lock,
            moves: self.moves,
            now_ms: self.now_ms,
            faults: Faults {
                fail: &fail,
                hold: &hold,
            },
        })
    }
}

fn boot(vault: &MemoryVault, presence: Presence) -> Backend {
    Backend::new(Box::new(vault.clone()), vec![("a".into(), Some(presence))])
}

fn done(events: &[Event], id: u64) -> Option<Done> {
    events.iter().find_map(|e| match e {
        Event::Done { id: i, done } if *i == id => Some(done.clone()),
        _ => None,
    })
}

fn asked(events: &[Event], id: u64) -> bool {
    (events.iter()).any(|e| matches!(e, Event::Check { id: i, .. } if *i == id))
}

/// Never both copies, and never a plain copy while the record says locked
/// once a boot has reconciled.
fn one_copy(vault: &MemoryVault) {
    let m = vault.memory();
    let locked = m.locked.keys().any(|(n, _)| n == "a");
    assert!(
        !(locked && m.plain.contains_key("a")),
        "both copies of a: {m:?}"
    );
}

#[test]
fn protect_records_its_intent_then_adds_then_deletes_and_unprotect_reverses_it() {
    let vault = MemoryVault::default();
    vault.clone().put_plain("a", Some(b"token")).unwrap();
    let mut backend = boot(&vault, optional());
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Plain));
    let drive = Drive::new();
    vault.0.lock().unwrap().calls.clear();
    backend.enqueue("a", Op::Protect, Some(1));
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 1), Some(Done::Ok));
    assert_eq!(
        vault.memory().calls,
        [
            "put_record a locked user-presence optional",
            "add_locked a#1",
            "put_plain a false",
        ]
    );
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Locked));
    assert_eq!(backend.plain_value("a"), None);
    one_copy(&vault);

    // Unprotect asks the person first, and moves nothing before the answer.
    vault.0.lock().unwrap().calls.clear();
    backend.enqueue("a", Op::Unprotect, Some(2));
    let events = drive.pump(&mut backend);
    assert!(asked(&events, 2));
    assert!(vault.memory().calls.is_empty());
    backend.checked(2, Checked::Passed);
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 2), Some(Done::Ok));
    assert_eq!(
        vault.memory().calls,
        [
            "put_record a plain",
            "put_plain a true",
            "delete_locked a#1"
        ]
    );
    assert_eq!(backend.plain_value("a").as_deref(), Some(&b"token"[..]));
}

#[test]
fn each_interrupted_move_reconciles_toward_locked_and_never_keeps_both_copies() {
    // (the fault point, the occupancy the next boot reads)
    for (point, after) in [
        ("protect.record", Occupancy::Pending),
        ("protect.write", Occupancy::Locked),
        ("unprotect.record", Occupancy::Locked),
        ("unprotect.write", Occupancy::Locked),
    ] {
        let vault = MemoryVault::default();
        vault.clone().put_plain("a", Some(b"token")).unwrap();
        let mut backend = boot(&vault, optional());
        let drive = Drive::new();
        if point.starts_with("unprotect") {
            backend.enqueue("a", Op::Protect, Some(1));
            drive.pump(&mut backend);
        }
        drive
            .fail
            .borrow_mut()
            .push(format!("exact-presence:{point}"));
        let op = if point.starts_with("unprotect") {
            Op::Unprotect
        } else {
            Op::Protect
        };
        backend.enqueue("a", op, Some(2));
        let mut events = drive.pump(&mut backend);
        backend.checked(2, Checked::Passed);
        events.extend(drive.pump(&mut backend));
        assert!(
            matches!(done(&events, 2), Some(Done::Failed(code, _)) if code == "InvalidStateError"),
            "{point}: {events:?}"
        );
        // Until a relaunch the name takes nothing, as after a crash.
        backend.enqueue("a", Op::Unlock, Some(3));
        let events = drive.pump(&mut backend);
        assert!(
            matches!(done(&events, 3), Some(Done::Failed(..))),
            "{point}"
        );

        let mut relaunched = boot(&vault, optional());
        assert_eq!(relaunched.occupancy("a"), Some(after), "{point}");
        if after == Occupancy::Locked {
            one_copy(&vault);
            let record = vault.memory().records.get("a").cloned().unwrap_or_default();
            assert!(record.starts_with(b"locked"), "{point}: the lock stays on");
        }
        // An unlock reads the value; a pending name's waits for its move.
        relaunched.enqueue("a", Op::Unlock, Some(4));
        let events = drive.pump(&mut relaunched);
        assert!(asked(&events, 4), "{point}: {events:?}");
        assert_eq!(
            relaunched.occupancy("a"),
            Some(Occupancy::Locked),
            "{point}"
        );
        one_copy(&vault);
        relaunched.checked(4, Checked::Passed);
        let events = drive.pump(&mut relaunched);
        assert_eq!(
            done(&events, 4),
            Some(Done::Value(Some(b"token".to_vec()))),
            "{point}"
        );
        // An unlock never removes a lock: the next boot still reads locked.
        assert_eq!(
            boot(&vault, optional()).occupancy("a"),
            Some(Occupancy::Locked),
            "{point}"
        );
    }
}

#[test]
fn a_pending_move_waits_for_the_module_and_deletes_the_copy_when_it_cannot_lock() {
    let vault = MemoryVault::default();
    vault.clone().put_plain("a", Some(b"token")).unwrap();
    // A mandatory line over a plain copy: an update added the lock.
    let mut backend = boot(&vault, mandatory());
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Pending));
    let mut drive = Drive::new();
    drive.moves = false;
    backend.enqueue("a", Op::Unlock, Some(1));
    let events = drive.pump(&mut backend);
    assert!(events.is_empty(), "nothing moves before the module loads");
    assert!(vault.memory().plain.contains_key("a"));
    drive.moves = true;
    drive.can_lock = false;
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 1), Some(Done::Value(None)));
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Absent));
    let m = vault.memory();
    assert!(
        m.plain.is_empty() && m.locked.is_empty() && m.records.is_empty(),
        "{m:?}"
    );
}

#[test]
fn reconciliation_keeps_the_newest_generation_and_forgets_a_changed_policy_unread() {
    let vault = MemoryVault::default();
    {
        let mut v = vault.clone();
        v.add_locked("a", 3, &optional(), b"old").unwrap();
        v.add_locked("a", 4, &optional(), b"new").unwrap();
        v.put_plain("a", Some(b"stale")).unwrap();
        v.put_record("a", Some(b"plain")).unwrap();
    }
    let mut backend = boot(&vault, optional());
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Locked));
    let m = vault.memory();
    assert_eq!(m.locked.keys().collect::<Vec<_>>(), [&("a".to_owned(), 4)]);
    assert!(
        !m.plain.contains_key("a"),
        "a plain copy never outlives a locked one"
    );
    assert_eq!(m.records["a"], b"locked user-presence optional");
    backend.enqueue("a", Op::Unlock, Some(1));
    let drive = Drive::new();
    drive.pump(&mut backend);
    backend.checked(1, Checked::Passed);
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 1), Some(Done::Value(Some(b"new".to_vec()))));

    // Another policy: the item is forgotten, never read, and the name is absent.
    vault.0.lock().unwrap().calls.clear();
    let backend = boot(&vault, line("biometry-current-set optional"));
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Absent));
    let m = vault.memory();
    assert!(m.locked.is_empty() && m.records.is_empty(), "{m:?}");
    assert!(!m.calls.iter().any(|c| c.starts_with("read")));

    // A line that lost its policy: the same, and the name is no presence name.
    vault.clone().add_locked("a", 1, &optional(), b"x").unwrap();
    vault
        .clone()
        .put_record("a", Some(b"locked user-presence optional"))
        .unwrap();
    let backend = Backend::new(Box::new(vault.clone()), vec![("a".into(), None)]);
    assert_eq!(backend.occupancy("a"), None);
    assert!(vault.memory().locked.is_empty());
}

#[test]
fn one_queue_orders_a_write_behind_a_held_move_and_keeps_it_locked() {
    let vault = MemoryVault::default();
    vault.clone().put_plain("a", Some(b"old")).unwrap();
    let mut backend = boot(&vault, optional());
    let drive = Drive::new();
    drive
        .hold
        .borrow_mut()
        .push("exact-presence:hold.protect.record".into());
    backend.enqueue("a", Op::Protect, Some(1));
    assert!(drive.pump(&mut backend).is_empty(), "held after its record");
    // The gateway rotates the token while the person turns the lock on.
    backend.enqueue("a", Op::Write(Some(b"new".to_vec())), None);
    backend.enqueue("a", Op::Unlock, Some(2));
    assert!(drive.pump(&mut backend).is_empty());
    assert_eq!(vault.memory().plain["a"], b"old", "the write waits");
    drive.hold.borrow_mut().clear();
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 1), Some(Done::Ok));
    assert!(
        asked(&events, 2),
        "the unlock waited for the move: {events:?}"
    );
    let m = vault.memory();
    assert!(
        !m.plain.contains_key("a"),
        "the write never gained a plain copy"
    );
    assert_eq!(m.locked.values().collect::<Vec<_>>(), [&b"new".to_vec()]);
    backend.checked(2, Checked::Passed);
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 2), Some(Done::Value(Some(b"new".to_vec()))));
}

#[test]
fn a_refused_locked_write_leaves_the_previous_value_until_its_retry_lands() {
    let vault = MemoryVault::default();
    let mut backend = boot(&vault, mandatory());
    let mut drive = Drive::new();
    backend.enqueue("a", Op::Write(Some(b"one".to_vec())), None);
    drive.pump(&mut backend);
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Locked));
    drive.fail.borrow_mut().push("exact-presence:set".into());
    backend.enqueue("a", Op::Write(Some(b"two".to_vec())), None);
    drive.pump(&mut backend);
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Locked));
    let unlock = |backend: &mut Backend, drive: &Drive, id| {
        backend.enqueue("a", Op::Unlock, Some(id));
        drive.pump(backend);
        backend.checked(id, Checked::Passed);
        done(&drive.pump(backend), id)
    };
    assert_eq!(
        unlock(&mut backend, &drive, 1),
        Some(Done::Value(Some(b"one".to_vec())))
    );
    drive.now_ms = 1000.0;
    drive.pump(&mut backend);
    assert_eq!(
        unlock(&mut backend, &drive, 2),
        Some(Done::Value(Some(b"two".to_vec())))
    );
    assert_eq!(vault.memory().locked.len(), 1, "the old generation went");
}

#[test]
fn a_check_that_fails_or_finds_the_item_gone_moves_nothing_or_reads_absent() {
    let vault = MemoryVault::default();
    let mut backend = boot(&vault, mandatory());
    let drive = Drive::new();
    backend.enqueue("a", Op::Write(Some(b"t".to_vec())), None);
    drive.pump(&mut backend);
    backend.enqueue("a", Op::Unlock, Some(1));
    drive.pump(&mut backend);
    backend.checked(1, Checked::Failed("USER_CANCEL".into(), "cancelled".into()));
    let events = drive.pump(&mut backend);
    assert_eq!(
        done(&events, 1),
        Some(Done::Failed("USER_CANCEL".into(), "cancelled".into()))
    );
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Locked));
    // A mandatory name is never unlocked for good.
    backend.enqueue("a", Op::Unprotect, Some(2));
    let events = drive.pump(&mut backend);
    assert!(matches!(done(&events, 2), Some(Done::Failed(code, _)) if code == "InvalidStateError"));
    backend.enqueue("a", Op::Unlock, Some(3));
    drive.pump(&mut backend);
    backend.checked(3, Checked::Invalidated);
    let events = drive.pump(&mut backend);
    assert_eq!(done(&events, 3), Some(Done::Value(None)));
    assert_eq!(backend.occupancy("a"), Some(Occupancy::Absent));
    assert!(vault.memory().locked.is_empty());
    // A call let go while queued leaves the queue, and is never answered.
    backend.enqueue("a", Op::Write(Some(b"t".to_vec())), None);
    drive.pump(&mut backend);
    backend.enqueue("a", Op::Unlock, Some(4));
    backend.enqueue("a", Op::Unlock, Some(5));
    drive.pump(&mut backend);
    backend.cancel(5);
    backend.checked(4, Checked::Passed);
    let events = drive.pump(&mut backend);
    assert!(done(&events, 4).is_some() && done(&events, 5).is_none() && !asked(&events, 5));
}

#[derive(Clone)]
pub(crate) struct UnavailableVault {
    pub(crate) memory: MemoryVault,
    pub(crate) failure: std::sync::Arc<std::sync::Mutex<Option<&'static str>>>,
}

impl UnavailableVault {
    fn check(&self, operation: &str) -> Result<(), String> {
        if self.failure.lock().unwrap().as_ref() == Some(&operation) {
            Err(format!("{operation} unavailable"))
        } else {
            Ok(())
        }
    }
}

impl Vault for UnavailableVault {
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.check("plain")?;
        self.memory.plain(name)
    }
    fn put_plain(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), String> {
        self.memory.put_plain(name, value)
    }
    fn record(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.check("record")?;
        self.memory.record(name)
    }
    fn put_record(&mut self, name: &str, record: Option<&[u8]>) -> Result<(), String> {
        self.memory.put_record(name, record)
    }
    fn generations(&self, name: &str) -> Result<Vec<u64>, String> {
        self.check("generations")?;
        self.memory.generations(name)
    }
    fn add_locked(
        &mut self,
        name: &str,
        generation: u64,
        presence: &Presence,
        value: &[u8],
    ) -> Result<(), String> {
        self.check("add")?;
        self.memory.add_locked(name, generation, presence, value)
    }
    fn delete_locked(&mut self, name: &str, generation: u64) -> Result<(), String> {
        self.check("delete")?;
        self.memory.delete_locked(name, generation)
    }
    fn read_locked(&mut self, name: &str, generation: u64) -> Result<Option<Vec<u8>>, String> {
        self.memory.read_locked(name, generation)
    }
}

#[test]
fn unavailable_boot_reads_defer_the_move_and_keep_queued_unlocks_behind_it() {
    for operation in ["record", "generations", "plain"] {
        let memory = MemoryVault::default();
        memory.clone().put_plain("a", Some(b"token")).unwrap();
        memory
            .clone()
            .put_record("a", Some(b"locked user-presence optional"))
            .unwrap();
        memory.0.lock().unwrap().calls.clear();
        let failure = std::sync::Arc::new(std::sync::Mutex::new(Some(operation)));
        let vault = UnavailableVault {
            memory: memory.clone(),
            failure: failure.clone(),
        };
        let mut backend = Backend::new(Box::new(vault), vec![("a".into(), Some(optional()))]);
        assert_eq!(backend.occupancy("a"), Some(Occupancy::Pending));
        assert_eq!(backend.plain_value("a"), None);
        backend.enqueue("a", Op::Unlock, Some(1));
        let drive = Drive::new();
        assert!(drive.pump(&mut backend).is_empty());
        assert!(
            memory.memory().calls.is_empty(),
            "{operation}: nothing mutated"
        );
        assert_eq!(memory.memory().plain["a"], b"token");
        *failure.lock().unwrap() = None;
        // A newly queued call retries now, even before the retry interval.
        backend.enqueue("a", Op::Unlock, Some(2));
        let events = drive.pump(&mut backend);
        assert!(asked(&events, 1) && !asked(&events, 2));
        assert_eq!(backend.occupancy("a"), Some(Occupancy::Locked));
        assert!(
            memory.memory().plain.is_empty(),
            "the pending move ran first"
        );
        backend.checked(1, Checked::Passed);
        let events = drive.pump(&mut backend);
        assert_eq!(done(&events, 1), Some(Done::Value(Some(b"token".to_vec()))));
        assert!(asked(&events, 2));
        backend.checked(2, Checked::Passed);
        assert_eq!(
            done(&drive.pump(&mut backend), 2),
            Some(Done::Value(Some(b"token".to_vec())))
        );
    }
}

#[test]
fn deferred_reconciliation_still_forgets_removed_or_changed_policies_unread() {
    for presence in [None, Some(line("biometry-current-set optional"))] {
        let memory = MemoryVault::default();
        memory
            .clone()
            .add_locked("a", 1, &optional(), b"token")
            .unwrap();
        memory
            .clone()
            .put_record("a", Some(b"locked user-presence optional"))
            .unwrap();
        memory.0.lock().unwrap().calls.clear();
        let failure = std::sync::Arc::new(std::sync::Mutex::new(Some("generations")));
        let vault = UnavailableVault {
            memory: memory.clone(),
            failure: failure.clone(),
        };
        let mut backend = Backend::new(Box::new(vault), vec![("a".into(), presence.clone())]);
        let mut drive = Drive::new();
        assert!(drive.pump(&mut backend).is_empty());
        assert!(memory.memory().calls.is_empty());
        *failure.lock().unwrap() = None;
        drive.now_ms = 1000.0;
        assert!(
            drive.pump(&mut backend).is_empty(),
            "cleanup never authenticates"
        );
        assert_eq!(backend.occupancy("a"), presence.map(|_| Occupancy::Absent));
        assert!(memory.memory().locked.is_empty());
        assert!(memory.memory().records.is_empty());
        assert!(
            drive.pump(&mut backend).is_empty(),
            "removed names leave no retry"
        );
    }
}
