//! The standard library with no engine in the process (LLP 0068).
//!
//! This file has no `hermes` gate on purpose. Run it with the feature off —
//!
//!     cargo test -p ibex2 --test rust_consumer
//!
//! — and nothing of Hermes is linked; that is the bar `rules/NOT-DOING.md`
//! sets for the no-JS consumer. With the feature on it is the same code.
use ibex2::boundary::HostError;
use ibex2::grant::GrantSet;
use ibex2::host::Host;

/// The same standard library, the same grant grammar, the same refusals —
/// through bindings that carry their grant, as a module's parameters do.
#[test]
fn a_rust_consumer_gets_the_same_standard_library_under_the_same_grants() {
    let dir = std::env::temp_dir().join(format!("ibex2-rust-consumer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("allowed")).unwrap();
    let dir = dir.canonicalize().unwrap();
    std::fs::write(dir.join("outside.txt"), "OUTSIDE").unwrap();
    std::fs::write(dir.join("allowed/inside.txt"), "INSIDE").unwrap();
    let allowed = dir.join("allowed");

    let host = Host::new();
    let bindings = host.endow(
        GrantSet::parse(&format!(
            "net.fetch https://example.com\nfs.read {a}\nfs.write {a}\nenv.read HOME\n",
            a = allowed.display()
        ))
        .unwrap(),
    );

    // fetch: the granted origin, and a refusal for any other, from Rust's
    // own redirect and header logic over the platform's connection.
    let response = bindings.fetch.get("https://example.com/").unwrap();
    assert!(response.ok(), "{}", response.status);
    assert!(response.text().contains("Example"));
    assert!(response.headers.get("content-type").is_some());
    assert!(matches!(
        bindings.fetch.get("https://example.org/"),
        Err(HostError::Denied {
            capability: "net.fetch"
        })
    ));

    // fs: inside the prefix, and refused outside it; a write, a listing, a stat.
    let inside = allowed.join("inside.txt");
    assert_eq!(
        bindings.fs.read_file(inside.to_str().unwrap()).unwrap(),
        b"INSIDE"
    );
    assert!(matches!(
        bindings
            .fs
            .read_file(dir.join("outside.txt").to_str().unwrap()),
        Err(HostError::Denied {
            capability: "fs.read"
        })
    ));
    let written = allowed.join("written.txt");
    bindings
        .fs
        .write_file(written.to_str().unwrap(), b"hello")
        .unwrap();
    assert_eq!(
        bindings.fs.read_file(written.to_str().unwrap()).unwrap(),
        b"hello"
    );
    let mut names = bindings.fs.read_dir(allowed.to_str().unwrap()).unwrap();
    names.sort();
    assert_eq!(names, vec!["inside.txt", "written.txt"]);
    assert!(bindings.fs.stat(inside.to_str().unwrap()).unwrap().is_file);
    // Traversal is lexical and refused before the filesystem is asked.
    let climb = format!("{}/../outside.txt", allowed.display());
    assert!(matches!(
        bindings.fs.read_file(&climb),
        Err(HostError::Denied { .. })
    ));

    // env: a snapshot of exactly the granted names.
    assert!(bindings.env.get("HOME").is_some());
    assert_eq!(
        bindings.env.get("PATH"),
        None,
        "not granted: absent, not refused"
    );
    assert_eq!(
        bindings.env.snapshot().keys().collect::<Vec<_>>(),
        vec!["HOME"]
    );

    // The pure tier is plain Rust, the same functions the bindings wrap.
    assert_eq!(
        ibex2::stdlib::url::parse("/settings", Some("https://exact.local"))
            .unwrap()
            .pathname,
        "/settings"
    );
    assert_eq!(ibex2::stdlib::base64::btoa("hi").unwrap(), "aGk=");

    let _ = std::fs::remove_dir_all(&dir);
}

/// A consumer granted nothing holds bindings that refuse everything — not
/// absent bindings, so the failure is a denial rather than a panic.
#[test]
fn an_ungranted_consumer_is_refused_not_absent() {
    let host = Host::new();
    let bindings = host.endow(GrantSet::none());
    assert!(matches!(
        bindings.fetch.get("https://example.com/"),
        Err(HostError::Denied { .. })
    ));
    assert!(matches!(
        bindings.fs.read_file("/etc/hosts"),
        Err(HostError::Denied { .. })
    ));
    assert_eq!(bindings.env.get("HOME"), None);
}
