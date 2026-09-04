use super::*;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "exact-fs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn dir(&self, name: &str) -> Directory {
        Directory::root(self.path(name).to_str().unwrap(), true).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
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
