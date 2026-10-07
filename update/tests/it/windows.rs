//! LLP 1026 D11a: unqualified durability must refuse before touching the store.
use super::support::{embedded, open_store, Temp};
use exact_update::Trust;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn refuse(path: &Path, trust: Trust) {
    let mut facts = embedded(&[]);
    facts.trust = trust;
    let error = match open_store(path, facts) {
        Ok(_) => panic!("Windows durable storage must not open"),
        Err(error) => error,
    };
    assert!(error.contains("Windows durable update storage is unsupported"));
    assert!(error.contains("no store files were touched"));
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, dir: &Path, entries: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let key = path.strip_prefix(root).unwrap().to_owned();
            if entry.file_type().unwrap().is_dir() {
                entries.insert(key, None);
                visit(root, &path, entries);
            } else {
                entries.insert(key, Some(std::fs::read(path).unwrap()));
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

#[test]
fn unsupported_durability_does_not_create_a_missing_store() {
    let temp = Temp::new("windows-missing-no-touch");
    let missing = temp.path().join("absent parent").join("store");
    for trust in [Trust::Production, Trust::Development] {
        refuse(&missing, trust);
        assert!(!missing.parent().unwrap().exists());
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
}

#[test]
fn unsupported_durability_leaves_every_existing_store_byte_and_name_unchanged() {
    let temp = Temp::new("windows-existing-no-touch");
    for directory in ["entries/.tmp-entry", "blobs", "untouched empty"] {
        std::fs::create_dir_all(temp.path().join(directory)).unwrap();
    }
    for (name, bytes) in [
        (
            "record.json",
            b"{\"codec\":2,\"selected\":\"damaged\"}".as_slice(),
        ),
        (".tmp-record", b"rollback floor awaiting recovery"),
        ("entries/.tmp-entry/app.plan", b"incomplete generation"),
        ("blobs/.tmp-blob", b"an interrupted download"),
        ("owner.lock", b"existing owner bytes"),
    ] {
        std::fs::write(temp.path().join(name), bytes).unwrap();
    }
    let before = snapshot(temp.path());
    // Even with the ownership file held, the platform refusal precedes trying
    // to acquire it; it must not be misreported as an ownership conflict.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(temp.path().join("owner.lock"))
        .unwrap();
    for trust in [Trust::Production, Trust::Development] {
        lock.try_lock().unwrap();
        refuse(temp.path(), trust);
        lock.unlock().unwrap();
        assert_eq!(snapshot(temp.path()), before);
    }
    drop(lock);
}
