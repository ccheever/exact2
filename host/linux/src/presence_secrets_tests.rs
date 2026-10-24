//! The vault over the reader's Keystore and where none is (LLP 1133.010 D6,
//! D15): a hook that writes the file as `Presence.kt` does stands for the
//! reader.

use super::*;
use exact_runner::presence::{Checked, Done, Env, Event, Faults, Occupancy, Op};

const GRANTS: &str =
    "secret.keep token user-presence optional\nsecret.keep key biometry-any android.strongBox";

/// The reader's op 1, as `Presence.add` keeps it: the item under its
/// generation's name in the directory, the line's words recorded beside.
fn keystore(op: u32, request: &str, value: &str) -> Result<String, String> {
    let r: serde_json::Value = serde_json::from_str(request).unwrap();
    match op {
        1 => {
            let dir = PathBuf::from(r["dir"].as_str().unwrap());
            std::fs::create_dir_all(&dir).unwrap();
            let sealed = format!("{}|{value}", r["policy"].as_str().unwrap());
            std::fs::write(dir.join(r["account"].as_str().unwrap()), sealed).unwrap();
            Ok(String::new())
        }
        _ => Ok("1".into()),
    }
}

fn vault(root: &std::path::Path, locked: Locked) -> HostVault {
    HostVault {
        secrets: ibex2::secrets::FileStore::new(root.join("secrets")),
        kv: ibex2::kv::FileStore::new(root.join("kv")),
        locked,
    }
}

fn pump(backend: &mut Backend, can_lock: bool) -> Vec<Event> {
    backend.pump(&Env {
        can_lock,
        moves: true,
        now_ms: 0.0,
        faults: Faults::NONE,
    })
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("exact-presence-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_reader_s_items_are_files_by_name_and_generation_never_plain() {
    on_keystore(keystore);
    let root = scratch("reader");
    let dir = root.join("exact-presence");
    let make = || {
        Backend::new(
            Box::new(vault(&root, Locked::Keystore(dir.clone()))),
            lines(GRANTS),
        )
    };
    let mut backend = make();
    backend.enqueue("key", Op::Write(Some(b"secret-1".to_vec())), None);
    pump(&mut backend, true);
    let item = |account: &str| std::fs::read_to_string(dir.join(account)).ok();
    assert_eq!(
        item("key#1").as_deref(),
        Some("biometry-any android.strongBox|secret-1")
    );
    assert!(!root.join("secrets/key").exists());
    // An overwrite adds the next generation before the last is deleted.
    backend.enqueue("key", Op::Write(Some(b"secret-2".to_vec())), None);
    pump(&mut backend, true);
    assert_eq!((item("key#1"), item("key#2").is_some()), (None, true));
    // A relaunch lists the directory, and the reader's read answers an unlock.
    let mut again = make();
    assert_eq!(
        (again.occupancy("key"), again.generation("key")),
        (Some(Occupancy::Locked), 2)
    );
    again.enqueue("key", Op::Unlock, Some(3));
    assert!(pump(&mut again, true)
        .iter()
        .any(|e| matches!(e, Event::Check { id: 3, .. })));
    again.checked(3, Checked::Read(Some(b"secret-2".to_vec())));
    let events = pump(&mut again, true);
    assert!(events.iter().any(
        |e| matches!(e, Event::Done { id: 3, done: Done::Value(Some(v)) } if v == b"secret-2")
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn where_nothing_is_kept_locked_an_optional_name_stays_plain() {
    let root = scratch("none");
    let mut backend = Backend::new(Box::new(vault(&root, Locked::None)), lines(GRANTS));
    backend.enqueue("token", Op::Write(Some(b"open".to_vec())), None);
    pump(&mut backend, false);
    assert_eq!(backend.occupancy("token"), Some(Occupancy::Plain));
    assert_eq!(
        std::fs::read_to_string(root.join("secrets/token"))
            .ok()
            .as_deref(),
        Some("open")
    );
    // A protect there is refused, and the plain copy stays.
    backend.enqueue("token", Op::Protect, Some(1));
    let events = pump(&mut backend, false);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Done {
                id: 1,
                done: Done::Failed(..)
            }
        )),
        "{events:?}"
    );
    assert_eq!(backend.occupancy("token"), Some(Occupancy::Plain));
    let _ = std::fs::remove_dir_all(&root);
}
