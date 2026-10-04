use super::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(unix)]
use std::sync::Arc;

pub(super) const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";
pub(super) struct Fixture(PathBuf);
impl Fixture {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "exact-fs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    pub(super) fn dir(&self, name: &str) -> Directory {
        Directory::root(self.path(name).to_str().unwrap(), true).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
#[cfg(unix)]
fn a_static_inventory_refuses_a_fifo() {
    let fixture = Fixture::new();
    let root = fixture.dir("assets");
    let path = std::ffi::CString::new(fixture.path("assets/pipe").to_str().unwrap()).unwrap();
    // SAFETY: path is a live, NUL-terminated name in this test's private directory.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let error = operate(&root, &json!({"op":"tree"}), None).unwrap_err();
    assert!(error.to_string().contains("regular files"), "{error}");
}

#[test]
fn retained_generation_quota_is_atomic_and_never_prunes() {
    let fixture = Fixture::new();
    let root = fixture.dir("cache");
    let request = |seq, quota| {
        json!({"op":"retain", "path":format!("{}/{seq}", "a".repeat(32)),
        "quota":quota, "token":TOKEN,
        "files":{"app.plan":STANDARD.encode("plan"), "deck/nested/page.html":STANDARD.encode("old guest"), "exact.json":STANDARD.encode("envelope")}})
    };
    let first = request(1, 1024);
    let used = operate(&root, &first, None).unwrap()["bytes"]
        .as_u64()
        .unwrap();
    assert_eq!(used, 68 + 4 + 9 + 8);
    assert_eq!(
        operate(&root, &request(1, used), None).unwrap()["bytes"],
        used
    );
    assert!(operate(&root, &request(2, used + 1), None)
        .unwrap_err()
        .to_string()
        .contains("quota exceeded"));
    assert!(!fixture
        .path(&format!("cache/{}/2", "a".repeat(32)))
        .exists());
    assert_eq!(
        fs::read(fixture.path(&format!("cache/{}/1/deck/nested/page.html", "a".repeat(32))))
            .unwrap(),
        b"old guest"
    );
    let mut changed = first.clone();
    changed["files"]["deck/nested/page.html"] = json!(STANDARD.encode("other"));
    assert!(operate(&root, &changed, None).is_err());
    let lock = Lock::acquire(&root, ".retained/.lock", TOKEN).unwrap();
    assert!(operate(&root, &request(2, 1024), None).is_err());
    drop(lock);
    assert!(operate(&root, &request(2, 1024), None).is_ok());
}

#[test]
fn retained_payload_failure_never_publishes_the_envelope() {
    let fixture = Fixture::new();
    let root = fixture.dir("cache");
    let epoch = "c".repeat(32);
    let request = |sequence, files| {
        json!({"op":"retain", "path":format!("{epoch}/{sequence}"),
        "quota":4096, "token":TOKEN, "files":files})
    };
    let good = json!({"a":STANDARD.encode("first"), "b":STANDARD.encode("second"),
        "c":STANDARD.encode("third"), "d":STANDARD.encode("fourth"),
        "exact.json":STANDARD.encode("complete")});
    operate(&root, &request(0, good.clone()), None).unwrap();
    // Both absent names pass quota admission. Publication must fail because
    // one payload needs a directory where the other needs a regular file.
    let bad = json!({"collision":STANDARD.encode("file"),
        "collision/child":STANDARD.encode("child"), "independent":STANDARD.encode("partial"),
        "exact.json":STANDARD.encode("must not publish")});
    assert!(operate(&root, &request(1, bad), None).is_err());
    assert!(!fixture
        .path(&format!("cache/{epoch}/1/exact.json"))
        .exists());
    assert_eq!(
        fs::read(fixture.path(&format!("cache/{epoch}/0/exact.json"))).unwrap(),
        b"complete"
    );
    assert_eq!(
        fs::read(fixture.path(&format!("cache/{epoch}/0/b"))).unwrap(),
        b"second"
    );
    // Failure releases the single publisher lock, retains the partial quota
    // usage, and leaves every temporary name cleaned up before returning.
    let used = operate(&root, &request(2, good.clone()), None).unwrap();
    assert_eq!(used["bytes"], retained_bytes(&root).unwrap());
    assert_eq!(operate(&root, &request(2, good), None).unwrap(), used);
    // Reuse an already-linked payload from the failed batch. The retry must
    // include its held directory in durability completion before its envelope.
    let recovered = json!({"independent":STANDARD.encode("partial"),
        "exact.json":STANDARD.encode("recovered")});
    operate(&root, &request(1, recovered), None).unwrap();
    assert_eq!(
        fs::read(fixture.path(&format!("cache/{epoch}/1/exact.json"))).unwrap(),
        b"recovered"
    );
    let mut names = serde_json::Map::new();
    tree(&root, "", &mut names, false).unwrap();
    assert!(names
        .keys()
        .all(|path| !path.split('/').any(|part| part.starts_with(".tmp-"))));
}

#[test]
#[cfg(unix)]
fn retained_completion_counts_require_the_last_successful_holder() {
    let fixture = Fixture::new();
    let root = fixture.dir("cache");
    let epoch = "d".repeat(32);
    let other = "abcdef0123456789abcdef0123456789abcdef0123456789";
    let request = |seq, token, previous| {
        json!({"op":"retain", "path":format!("{epoch}/{seq}"),
        "quota":4096, "token":token, "previousToken":previous,
        "files":{"payload":STANDARD.encode("payload"), "exact.json":STANDARD.encode("complete")}})
    };
    let first = operate(&root, &request(0, TOKEN, Value::Null), None).unwrap();
    assert_eq!(first["bytes"], retained_bytes(&root).unwrap());
    assert_eq!(first["token"], TOKEN);
    // A different publisher completes under the same lock. Its extra files
    // must count even when the original publisher supplies its stale token.
    operate(&root, &request(1, other, Value::Null), None).unwrap();
    let next = operate(&root, &request(2, TOKEN, json!(TOKEN)), None).unwrap();
    assert_eq!(next["bytes"], retained_bytes(&root).unwrap());
    let held = Lock::acquire(&root, ".retained/.lock", other).unwrap();
    assert!(operate(&root, &request(3, TOKEN, json!(TOKEN)), None).is_err());
    drop(held);
    let next = operate(&root, &request(3, TOKEN, json!(TOKEN)), None).unwrap();
    assert_eq!(next["bytes"], retained_bytes(&root).unwrap());
    // Failure after a payload write invalidates the count, even if a caller
    // reuses the same token. A fresh scan includes the abandoned partial bytes.
    let mut failed = request(4, TOKEN, json!(TOKEN));
    failed["files"] = json!({"a-partial":STANDARD.encode("partial"),
        "collision":STANDARD.encode("file"), "collision/child":STANDARD.encode("child"),
        "exact.json":STANDARD.encode("absent")});
    assert!(operate(&root, &failed, None).is_err());
    assert_eq!(
        fs::metadata(fixture.path("cache/.retained/.lock"))
            .unwrap()
            .len(),
        48
    );
    let next = operate(&root, &request(5, TOKEN, json!(TOKEN)), None).unwrap();
    assert_eq!(next["bytes"], retained_bytes(&root).unwrap());
    let mut quota_refusal = request(6, TOKEN, json!(TOKEN));
    quota_refusal["quota"] = json!(next["bytes"].as_u64().unwrap());
    assert!(operate(&root, &quota_refusal, None).is_err());
    assert_eq!(
        fs::metadata(fixture.path("cache/.retained/.lock"))
            .unwrap()
            .len(),
        48
    );
    // A malformed fixed-width completion cannot be used as accounting.
    fs::write(
        fixture.path("cache/.retained/.lock"),
        format!("{TOKEN}{}", "x".repeat(20)),
    )
    .unwrap();
    let next = operate(&root, &request(6, TOKEN, json!(TOKEN)), None).unwrap();
    assert_eq!(next["bytes"], retained_bytes(&root).unwrap());
    // Untouched historical corruption is discovered on reads or the next full
    // scan, not by a matching completion. New writes still use nofollow handles.
    let linked = fixture.path(&format!("cache/{epoch}/0/corrupt"));
    symlink(fixture.path("outside"), &linked).unwrap();
    operate(&root, &request(7, TOKEN, json!(TOKEN)), None).unwrap();
    assert!(operate(&root, &request(8, TOKEN, Value::Null), None).is_err());
    assert!(root
        .parent(&format!("{epoch}/0/corrupt"), false)
        .unwrap()
        .0
        .read("corrupt")
        .is_err());
    fs::remove_file(linked).unwrap();
    let next = operate(&root, &request(8, TOKEN, json!(TOKEN)), None).unwrap();
    assert_eq!(next["bytes"], retained_bytes(&root).unwrap());
}

#[test]
#[cfg(unix)]
fn retained_generations_cannot_bypass_heads_or_symlink_gate() {
    let fixture = Fixture::new();
    let root = fixture.dir("cache");
    fixture.dir("outside");
    let epoch = "a".repeat(32);
    symlink(
        fixture.path("outside"),
        fixture.path(&format!("cache/{epoch}")),
    )
    .unwrap();
    for path in [
        format!("{epoch}/1"),
        ".exact/prod/cohort".into(),
        "../escape".into(),
    ] {
        assert!(operate(
            &root,
            &json!({"op":"retain", "path":path, "quota":4096, "token":TOKEN,
            "files":{"exact.json":STANDARD.encode("head")}}),
            None
        )
        .is_err());
    }
    assert!(fs::read_dir(fixture.path("outside"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
#[cfg(unix)]
fn held_parent_cannot_be_redirected_after_validation() {
    let fixture = Fixture::new();
    let root = fixture.dir("origin");
    let outside = fixture.dir("outside");
    outside.write("secret", b"private", false, TOKEN).unwrap();
    let (parent, leaf) = root.parent("nested/value", true).unwrap();
    fs::rename(fixture.path("origin/nested"), fixture.path("origin/held")).unwrap();
    symlink(fixture.path("outside"), fixture.path("origin/nested")).unwrap();
    parent.write(&leaf, b"public", false, TOKEN).unwrap();
    assert_eq!(
        fs::read(fixture.path("origin/held/value")).unwrap(),
        b"public"
    );
    assert!(!fixture.path("outside/value").exists());
    assert!(root.parent("nested/secret", false).is_err());
    assert_eq!(outside.read("secret").unwrap(), b"private");
}

#[test]
#[cfg(unix)]
fn replaced_lock_and_head_are_refused_before_commit() {
    let fixture = Fixture::new();
    let root = fixture.dir("origin");
    let lock = Lock::acquire(&root, ".exact/prod/cohort/.lock", TOKEN).unwrap();
    assert!(Lock::acquire(&root, ".exact/prod/cohort/.lock", TOKEN).is_err());
    let (dir, leaf) = root.parent(&lock.head, false).unwrap();
    dir.write(&leaf, b"old", false, TOKEN).unwrap();
    let result = dir.write_checked(&leaf, b"candidate", false, TOKEN, || {
        fs::write(fixture.path("origin/.exact/prod/cohort/exact.json"), b"other publisher").unwrap();
        let request = json!({"op":"head", "path":lock.head, "bytes":STANDARD.encode("candidate"),
            "previousDigest":format!("{:x}", Sha256::digest(b"old")), "token":"abcdef0123456789abcdef0123456789abcdef0123456789"});
        operate(&root, &request, Some(&lock)).map(|_| ())
    });
    assert!(result.is_err());
    assert_eq!(dir.read(&leaf).unwrap(), b"other publisher");
    let lock_path = fixture.path("origin/.exact/prod/cohort/.lock");
    fs::rename(&lock_path, fixture.path("old-lock")).unwrap();
    let successor = Lock::acquire(&root, ".exact/prod/cohort/.lock", "successor").unwrap();
    assert!(lock.verify(&root).is_err());
    drop(lock);
    assert!(successor.verify(&root).is_ok());
    assert!(Lock::acquire(&root, ".exact/prod/cohort/.lock", TOKEN).is_err());
    drop(successor);
    assert!(Lock::acquire(&root, ".exact/prod/cohort/.lock", TOKEN).is_ok());
}

// Atomic exchange gives the adversary a continuously present path: no test
// can pass merely because every operation caught a missing-name gap.
#[cfg(unix)]
fn exchange(left: &Path, right: &Path) {
    use std::ffi::CString;
    let left = CString::new(left.to_str().unwrap()).unwrap();
    let right = CString::new(right.to_str().unwrap()).unwrap();
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            left.as_ptr(),
            libc::AT_FDCWD,
            right.as_ptr(),
            libc::RENAME_SWAP,
        )
    };
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            left.as_ptr(),
            libc::AT_FDCWD,
            right.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    };
    assert_eq!(result, 0, "{}", io::Error::last_os_error());
}

#[test]
#[cfg(unix)]
fn concurrent_intermediate_symlinks_never_supply_or_receive_bytes() {
    let fixture = Fixture::new();
    let root = fixture.dir("origin");
    let legitimate = fixture.dir("origin/.exact");
    legitimate.write("value", b"public", false, TOKEN).unwrap();
    let outside = fixture.dir("outside");
    outside.write("value", b"secret", false, TOKEN).unwrap();
    outside.child("private-only", true).unwrap();
    symlink(fixture.path("outside"), fixture.path("swap")).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let ready = Arc::new(AtomicBool::new(false));
    let adversary = {
        let stop = stop.clone();
        let ready = ready.clone();
        let left = fixture.path("origin/.exact");
        let right = fixture.path("swap");
        std::thread::spawn(move || {
            let mut exchanges = 0;
            while !stop.load(Ordering::Relaxed) {
                exchange(&left, &right);
                exchange(&left, &right);
                exchanges += 2;
                ready.store(true, Ordering::Release);
            }
            exchanges
        })
    };
    while !ready.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    let mut reads = 0;
    for _ in 0..2000 {
        if let Ok(value) = operate(&root, &json!({"op":"get","path":".exact/value"}), None) {
            assert_eq!(value, STANDARD.encode("public"));
            reads += 1;
        }
        if let Ok(names) = operate(&root, &json!({"op":"list","path":".exact"}), None) {
            assert!(!names.as_array().unwrap().contains(&json!("private-only")));
        }
        let _ = operate(
            &root,
            &json!({"op":"put","path":".exact/new/leaf", "bytes":STANDARD.encode("safe"), "token":TOKEN}),
            None,
        );
        if let Ok(source) = Directory::root(fixture.path("origin/.exact").to_str().unwrap(), false)
        {
            let captured = operate(&source, &json!({"op":"tree"}), None).unwrap();
            assert_eq!(captured["value"], STANDARD.encode("public"));
            assert!(captured.get("private-only").is_none());
        }
        if let Ok(lock) = Lock::acquire(&root, ".exact/prod/cohort/.lock", TOKEN) {
            let _ = operate(
                &root,
                &json!({"op":"head","path":lock.head,"bytes":STANDARD.encode("head"),"previousDigest":null,"token":TOKEN}),
                Some(&lock),
            );
        }
    }
    stop.store(true, Ordering::Relaxed);
    assert!(adversary.join().unwrap() > 0);
    assert!(
        reads > 0,
        "the race must also exercise successfully opened owned handles"
    );
    assert_eq!(outside.names().unwrap(), vec!["private-only", "value"]);
    assert_eq!(outside.read("value").unwrap(), b"secret");
}

#[test]
#[cfg(unix)]
fn resident_reader_reopens_roots_and_refuses_mutations() {
    let fixture = Fixture::new();
    let path = fixture.path("root");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("value"), "before").unwrap();
    let input = json!({"op":"get","root":path,"path":"value"});
    assert_eq!(read_request(&input).unwrap(), STANDARD.encode("before"));
    fs::rename(&path, fixture.path("old")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("value"), "after").unwrap();
    assert_eq!(read_request(&input).unwrap(), STANDARD.encode("after"));
    assert!(read_request(&json!({"op":"put","root":path,"path":"value"})).is_err());
    fs::remove_file(path.join("value")).unwrap();
    symlink(fixture.path("old/value"), path.join("value")).unwrap();
    assert!(read_request(&input).is_err());
}
