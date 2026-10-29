//! The keychain vault over the hook ExactKit installs (LLP 1133.010 D6): a
//! keychain in memory stands for it, as `Presence.swift` answers each op.

use super::*;
use exact_runner::presence::{Checked, Done, Env, Event, Faults, Occupancy, Op};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// (service, account) → (the line's words, the value).
type Items = BTreeMap<(String, String), (String, Vec<u8>)>;
static ITEMS: Mutex<Items> = Mutex::new(BTreeMap::new());
static FAIL_PROBE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static FAIL_DELETE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static FAIL_CANDIDATE: Mutex<usize> = Mutex::new(0);

#[allow(unsafe_code)] // The hook's C shape: the request and value as pointers.
extern "C" fn keychain(
    op: u32,
    json: *const u8,
    len: usize,
    value: *const u8,
    value_len: usize,
    reply: *mut std::ffi::c_void,
    done: extern "C" fn(*mut std::ffi::c_void, i32, *const u8, usize),
) {
    // SAFETY: `call` passes its request's and value's bytes for the call.
    let (json, value) = unsafe {
        (
            std::slice::from_raw_parts(json, len),
            std::slice::from_raw_parts(value, value_len),
        )
    };
    let r: serde_json::Value = serde_json::from_slice(json).unwrap();
    let text = |k: &str| r[k].as_str().unwrap_or_default().to_owned();
    let mut items = ITEMS.lock().unwrap();
    let (status, out): (i32, Vec<u8>) = match op {
        1 => {
            let key = (text("service"), text("account"));
            match items.contains_key(&key) {
                true => (-1, b"keychain -25299: duplicate".to_vec()),
                false => {
                    items.insert(key, (text("policy"), value.to_vec()));
                    (0, Vec::new())
                }
            }
        }
        2 if FAIL_DELETE.load(std::sync::atomic::Ordering::Relaxed) => {
            (-1, b"keychain delete unavailable".to_vec())
        }
        2 => match items.remove(&(text("service"), text("account"))) {
            Some(_) => (0, Vec::new()),
            None => (1, Vec::new()),
        },
        3 => {
            assert!(
                !text("account").is_empty(),
                "a probe must name one indexed item"
            );
            let mut failures = FAIL_CANDIDATE.lock().unwrap();
            let failed_candidate = text("account") == "key#3" && *failures > 0;
            if failed_candidate {
                *failures -= 1;
            }
            if failed_candidate || FAIL_PROBE.load(std::sync::atomic::Ordering::Relaxed) {
                (-1, b"keychain unavailable".to_vec())
            } else {
                let present = items.contains_key(&(text("service"), text("account")));
                (if present { 0 } else { 1 }, Vec::new())
            }
        }
        4 => {
            items.retain(|(s, _), _| *s != text("service"));
            (0, Vec::new())
        }
        5 => (0, Vec::new()),
        _ => panic!("unexpected keychain operation {op}"),
    };
    done(reply, status, out.as_ptr(), out.len());
}

const GRANTS: &str = "secret.keep token user-presence optional\nsecret.keep key biometry-any apple.orWatch\nsecret.keep plain";

fn bindings() -> Bindings {
    let spec = format!("{GRANTS}\n{}", scopes());
    let host = ibex2::host::Host::new()
        .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
        .with_kv_store(Box::new(ibex2::kv::MemoryStore::new()));
    host.endow(ibex2::grant::GrantSet::parse(&spec).unwrap())
}

fn pump(backend: &mut Backend) -> Vec<Event> {
    pump_at(backend, 0.0)
}

fn pump_at(backend: &mut Backend, now_ms: f64) -> Vec<Event> {
    let env = Env {
        can_lock: true,
        moves: true,
        now_ms,
        faults: Faults::NONE,
    };
    backend.pump(&env)
}

fn item(account: &str) -> Option<(String, Vec<u8>)> {
    ITEMS
        .lock()
        .unwrap()
        .get(&("com.example.lock.presence".into(), account.into()))
        .cloned()
}

#[test]
fn locked_items_are_the_keychain_s_by_name_and_generation_and_never_plain() {
    on_keychain(Some(keychain));
    let b = bindings();
    let (mut backend, service) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    assert_eq!(service, "com.example.lock.presence");
    assert!(keychain_capable());

    // A mandatory name's write: one item under its first generation, its
    // access control the line's words, and no plain copy.
    backend.enqueue("key", Op::Write(Some(b"secret-1".to_vec())), None);
    pump(&mut backend);
    assert_eq!(
        item("key#1"),
        Some(("biometry-any apple.orWatch".into(), b"secret-1".to_vec()))
    );
    assert_eq!(b.secrets.get("key").unwrap(), None);
    assert_eq!(backend.occupancy("key"), Some(Occupancy::Locked));

    // An overwrite adds the next generation before deleting the last (D6).
    backend.enqueue("key", Op::Write(Some(b"secret-2".to_vec())), None);
    pump(&mut backend);
    assert_eq!(item("key#1"), None);
    assert_eq!(item("key#2").map(|i| i.1), Some(b"secret-2".to_vec()));

    // A relaunch probes indexed generations without reading their values.
    let (mut again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    assert_eq!(again.occupancy("key"), Some(Occupancy::Locked));
    assert_eq!(again.generation("key"), 2);

    // An unlock waits on the person; the host's read after the check answers it.
    again.enqueue("key", Op::Unlock, Some(7));
    let events = pump(&mut again);
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::Check { id: 7, .. })));
    again.checked(7, Checked::Read(Some(b"secret-2".to_vec())));
    let events = pump(&mut again);
    assert!(events.iter().any(
        |e| matches!(e, Event::Done { id: 7, done: Done::Value(Some(v)) } if v == b"secret-2")
    ));

    // An optional name's plain write is the app's Keychain item, and
    // `protect` moves it into a locked one, deleting the plain copy.
    again.enqueue("token", Op::Write(Some(b"open".to_vec())), None);
    pump(&mut again);
    assert_eq!(b.secrets.get("token").unwrap().as_deref(), Some("open"));
    again.enqueue("token", Op::Protect, Some(8));
    pump(&mut again);
    assert_eq!(item("token#1").map(|i| i.1), Some(b"open".to_vec()));
    assert_eq!(b.secrets.get("token").unwrap(), None);

    // A crash between indexing and adding leaves no durable item. A crash
    // after deleting leaves a stale index entry. Both are removed by probing.
    b.kv.set(INDEX, "key#3", &[]).unwrap();
    b.kv.set(INDEX, "key#1", &[]).unwrap();
    let (again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    assert_eq!(again.generation("key"), 2);
    assert_eq!(b.kv.keys(INDEX).unwrap(), vec!["key#2", "token#1"]);
    for account in b.kv.keys(INDEX).unwrap() {
        assert_eq!(b.kv.get(INDEX, &account).unwrap(), Some(Vec::new()));
    }

    // A crash before the add leaves an unconfirmed candidate. Neither a
    // transient probe failure nor repeated failures may select it. Recovery
    // and the queued unlock use this backend, without another launch.
    for failures in [1, 2] {
        b.kv.set(INDEX, "key#3", &[]).unwrap();
        *FAIL_CANDIDATE.lock().unwrap() = failures;
        let (mut recovering, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
        assert_eq!(item("key#2").map(|i| i.1), Some(b"secret-2".to_vec()));
        assert_eq!(recovering.generation("key"), 0, "no candidate selected");
        assert_eq!(recovering.occupancy("key"), Some(Occupancy::Pending));
        assert_eq!(b.kv.get(INDEX, "key#3").unwrap(), Some(Vec::new()));
        recovering.enqueue("key", Op::Unlock, Some(9));
        // A reply before any check must not invalidate the real credential.
        recovering.checked(9, Checked::Read(None));
        let mut events = pump(&mut recovering);
        if failures == 2 {
            assert!(events.is_empty(), "no authentication during a failed probe");
            assert_eq!(recovering.generation("key"), 0);
            assert!(pump_at(&mut recovering, 999.0).is_empty());
            events = pump_at(&mut recovering, 1000.0);
        }
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Check { id: 9, .. })));
        assert_eq!(recovering.generation("key"), 2);
        let value = item(&format!("key#{}", recovering.generation("key"))).map(|i| i.1);
        assert_eq!(value.as_deref(), Some(b"secret-2".as_slice()));
        recovering.checked(9, Checked::Read(value));
        assert!(pump_at(&mut recovering, 1000.0).iter().any(
            |e| matches!(e, Event::Done { id: 9, done: Done::Value(Some(v)) } if v == b"secret-2")
        ));
        assert_eq!(recovering.occupancy("key"), Some(Occupancy::Locked));
        assert_eq!(item("key#2").map(|i| i.1), Some(b"secret-2".to_vec()));
        assert_eq!(b.kv.get(INDEX, "key#3").unwrap(), None);
    }

    // A crash after adding the new item but before deleting the old one
    // keeps both indexed. Relaunch selects the newest and retires the old.
    let mut vault = KeychainVault {
        secrets: b.secrets.clone(),
        kv: b.kv.clone(),
        service: service.clone(),
    };
    let policy = lines(GRANTS)
        .into_iter()
        .find(|(name, _)| name == "key")
        .unwrap()
        .1
        .unwrap();
    vault.add_locked("key", 3, &policy, b"secret-3").unwrap();
    *FAIL_CANDIDATE.lock().unwrap() = 1;
    let (mut again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    assert_eq!(again.occupancy("key"), Some(Occupancy::Pending));
    assert!(item("key#2").is_some() && item("key#3").is_some());
    assert!(
        pump(&mut again).is_empty(),
        "recovery must not authenticate"
    );
    assert_eq!(again.generation("key"), 3);
    assert_eq!(item("key#2"), None);
    assert_eq!(item("key#3").map(|i| i.1), Some(b"secret-3".to_vec()));

    // Platform invalidation removes the item. The next launch removes its
    // candidate and reports absence, with no authentication to discover it.
    ITEMS
        .lock()
        .unwrap()
        .remove(&(service.clone(), "key#3".into()));
    let (again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    assert_eq!(again.occupancy("key"), Some(Occupancy::Absent));
    assert_eq!(b.kv.get(INDEX, "key#3").unwrap(), None);

    // New calls retry immediately, even inside a failed pump's retry interval.
    FAIL_PROBE.store(true, std::sync::atomic::Ordering::Relaxed);
    let (mut again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    assert_eq!(again.occupancy("token"), Some(Occupancy::Pending));
    assert_eq!(again.plain_value("token"), None);
    assert!(again
        .snapshot_entries(true)
        .contains(&("exact.presence.token".into(), b"pending".to_vec())));
    assert!(pump(&mut again).is_empty());
    FAIL_PROBE.store(false, std::sync::atomic::Ordering::Relaxed);
    again.enqueue("token", Op::Unlock, Some(10));
    assert!(pump_at(&mut again, 1.0)
        .iter()
        .any(|e| matches!(e, Event::Check { id: 10, .. })));
    again.cancel(10);
    again.checked(10, Checked::Read(None));
    assert!(pump_at(&mut again, 1.0).is_empty());
    assert!(item("token#1").is_some());

    // Writes and an intentional forget stay ordered behind reconciliation.
    // A failed probe must neither overwrite the old item nor lose the forget.
    FAIL_PROBE.store(true, std::sync::atomic::Ordering::Relaxed);
    let (mut again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
    again.enqueue("token", Op::Write(Some(b"replacement".to_vec())), None);
    again.enqueue("token", Op::Write(None), None);
    again.enqueue("token", Op::Unlock, Some(11));
    assert!(pump(&mut again).is_empty());
    assert_eq!(item("token#1").map(|i| i.1), Some(b"open".to_vec()));
    assert!(b.kv.get(RECORDS, "token").unwrap().is_some());
    FAIL_PROBE.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        pump_at(&mut again, 1000.0),
        vec![Event::Done {
            id: 11,
            done: Done::Value(None)
        }]
    );
    assert_eq!(again.occupancy("token"), Some(Occupancy::Absent));
    assert_eq!(b.secrets.get("token").unwrap(), None);
    assert_eq!(b.kv.get(RECORDS, "token").unwrap(), None);
    assert!(b.kv.keys(INDEX).unwrap().is_empty());
    assert!(ITEMS.lock().unwrap().is_empty());

    // Forget and authenticated invalidation delete candidates unread, even
    // when an existence probe would fail. A failed deletion keeps its intent.
    for invalidate in [false, true] {
        let (mut again, _) = keychain_backend(Some(&b), GRANTS, "com.example.lock").unwrap();
        again.enqueue("key", Op::Write(Some(b"last".to_vec())), None);
        pump(&mut again);
        b.kv.set(INDEX, "key#2", &[]).unwrap();
        FAIL_PROBE.store(true, std::sync::atomic::Ordering::Relaxed);
        FAIL_DELETE.store(true, std::sync::atomic::Ordering::Relaxed);
        if invalidate {
            again.enqueue("key", Op::Unlock, Some(12));
            assert!(pump(&mut again)
                .iter()
                .any(|e| matches!(e, Event::Check { id: 12, .. })));
            again.checked(12, Checked::Invalidated);
            assert!(pump(&mut again).iter().any(|e| matches!(
                e, Event::Done { id: 12, done: Done::Failed(code, _) } if code == "UnknownError"
            )));
        } else {
            again.enqueue("key", Op::Write(None), None);
            assert!(pump(&mut again).is_empty());
        }
        assert_eq!(again.occupancy("key"), Some(Occupancy::Locked));
        assert!(item("key#1").is_some());
        assert!(b.kv.get(RECORDS, "key").unwrap().is_some());
        FAIL_DELETE.store(false, std::sync::atomic::Ordering::Relaxed);
        if invalidate {
            again.enqueue("key", Op::Unlock, Some(13));
            pump(&mut again);
            again.checked(13, Checked::Invalidated);
            assert_eq!(
                pump(&mut again),
                vec![Event::Done {
                    id: 13,
                    done: Done::Value(None)
                }]
            );
        } else {
            assert!(pump_at(&mut again, 1000.0).is_empty());
        }
        assert_eq!(again.occupancy("key"), Some(Occupancy::Absent));
        assert!(ITEMS.lock().unwrap().is_empty());
        assert!(b.kv.keys(INDEX).unwrap().is_empty());
        assert_eq!(b.kv.get(RECORDS, "key").unwrap(), None);
        FAIL_PROBE.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}
