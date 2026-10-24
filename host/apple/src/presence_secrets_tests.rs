//! The keychain vault over the hook ExactKit installs (LLP 1133.010 D6): a
//! keychain in memory stands for it, as `Presence.swift` answers each op.

use super::*;
use exact_runner::presence::{Checked, Done, Env, Event, Faults, Occupancy, Op};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// (service, account) → (the line's words, the value).
type Items = BTreeMap<(String, String), (String, Vec<u8>)>;
static ITEMS: Mutex<Items> = Mutex::new(BTreeMap::new());

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
        2 => match items.remove(&(text("service"), text("account"))) {
            Some(_) => (0, Vec::new()),
            None => (1, Vec::new()),
        },
        3 => {
            let accounts: Vec<&String> = (items.keys())
                .filter(|(s, _)| *s == text("service"))
                .map(|(_, a)| a)
                .collect();
            (0, serde_json::to_vec(&accounts).unwrap())
        }
        4 => {
            items.retain(|(s, _), _| *s != text("service"));
            (0, Vec::new())
        }
        _ => (0, Vec::new()),
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
    let env = Env {
        can_lock: true,
        moves: true,
        now_ms: 0.0,
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

    // A relaunch finds the newest generation by an attributes-only listing.
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
}
