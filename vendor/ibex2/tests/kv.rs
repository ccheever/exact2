//! The kv binding end-to-end with no engine: a real `Host`, the file store
//! on a real disk, grants parsed from the manifest grammar. LLP 0070; the
//! no-engine build shape is LLP 0068 §3 (`--no-default-features` runs this).

use ibex2::grant::GrantSet;
use ibex2::host::Host;
use ibex2::kv::FileStore;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "ibex2-kv-consumer-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn a_rust_consumer_keeps_durable_state_through_its_grant() {
    let dir = temp_dir("grant");
    let host = Host::new().with_kv_store(Box::new(FileStore::new(&dir)));
    let app = host.endow(GrantSet::parse("storage.kv castle.state\n").unwrap());

    assert_eq!(app.kv.scopes(), vec!["castle.state"]);
    assert_eq!(app.kv.get("castle.state", "cursor").unwrap(), None);
    app.kv.set("castle.state", "cursor", b"42").unwrap();
    app.kv
        .set_text("castle.state", "args:{\"id\":7}", "{\"rooms\":[1,2]}")
        .unwrap();
    assert_eq!(
        app.kv.get("castle.state", "cursor").unwrap().as_deref(),
        Some(&b"42"[..])
    );
    assert_eq!(
        app.kv
            .get_text("castle.state", "args:{\"id\":7}")
            .unwrap()
            .as_deref(),
        Some("{\"rooms\":[1,2]}")
    );
    assert_eq!(
        app.kv.keys("castle.state").unwrap(),
        vec!["args:{\"id\":7}".to_string(), "cursor".to_string()]
    );

    // A second endowment from the same host shares the disk and not the
    // authority.
    let other = host.endow(GrantSet::none());
    assert!(other.kv.get("castle.state", "cursor").is_err());

    // What was kept survives a new store over the same directory — the
    // "across launches" claim, as far as one process can witness it.
    let relaunched = Host::new().with_kv_store(Box::new(FileStore::new(&dir)));
    let again = relaunched.endow(GrantSet::parse("storage.kv castle.state\n").unwrap());
    assert_eq!(
        again.kv.get("castle.state", "cursor").unwrap().as_deref(),
        Some(&b"42"[..])
    );
    again.kv.delete("castle.state", "cursor").unwrap();
    assert_eq!(again.kv.get("castle.state", "cursor").unwrap(), None);

    let _ = std::fs::remove_dir_all(&dir);
}
