use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Instant;

#[derive(Default)]
struct Values {
    values: Mutex<BTreeMap<String, String>>,
    fail_read: AtomicBool,
    fail_write: AtomicBool,
}
impl Backend for Values {
    fn read(&self, name: &str) -> Result<Option<String>, String> {
        if self.fail_read.load(Ordering::SeqCst) {
            return Err("read failed".into());
        }
        Ok(self.values.lock().unwrap().get(name).cloned())
    }
    fn write(&self, name: &str, value: Option<&str>) -> Result<(), String> {
        if self.fail_write.load(Ordering::SeqCst) {
            return Err("write failed".into());
        }
        let mut values = self.values.lock().unwrap();
        if let Some(value) = value {
            values.insert(name.into(), value.into());
        } else {
            values.remove(name);
        }
        Ok(())
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "exact-secret-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn acquire(coordinator: &Coordinator) -> Arc<dyn SecretLease> {
    let (tx, rx) = mpsc::channel();
    let mut wait = coordinator.acquire(
        "oauth",
        Arc::new(move || {
            let _ = tx.send(());
        }),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    wait.take().unwrap().unwrap()
}
fn fixture() -> (Temp, Namespace, Arc<Values>, Coordinator) {
    let temp = Temp::new();
    let namespace = Namespace::disk(temp.0.clone());
    let backend = Arc::new(Values::default());
    backend.write("oauth", Some("old")).unwrap();
    let coordinator = Coordinator::new(namespace.clone(), backend.clone());
    (temp, namespace, backend, coordinator)
}

#[test]
fn independent_coordinators_serialize_and_read_the_completed_value() {
    let (temp, _, backend, first) = fixture();
    let second = Coordinator::new(Namespace::disk(temp.0.clone()), backend);
    let lease = acquire(&first);
    let (tx, rx) = mpsc::channel();
    let mut wait = second.acquire(
        "oauth",
        Arc::new(move || {
            let _ = tx.send(());
        }),
    );
    assert!(wait.take().is_none());
    lease.begin_effect().unwrap();
    lease.write(Some("rotated")).unwrap();
    lease.finish().unwrap();
    assert!(wait.take().is_none());
    drop(lease);
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let lease = wait.take().unwrap().unwrap();
    assert_eq!(lease.read().unwrap().as_deref(), Some("rotated"));
    lease.finish().unwrap();
}

#[test]
fn cancelling_read_only_lease_preserves_the_credential() {
    let (_temp, _, _, coordinator) = fixture();
    let lease = acquire(&coordinator);
    assert_eq!(lease.read().unwrap().as_deref(), Some("old"));
    drop(lease);
    assert_eq!(
        acquire(&coordinator).read().unwrap().as_deref(),
        Some("old")
    );
}

#[test]
fn abandoning_an_effect_invalidates_before_next_read() {
    let (_temp, _, backend, coordinator) = fixture();
    let lease = acquire(&coordinator);
    lease.begin_effect().unwrap();
    drop(lease);
    let next = acquire(&coordinator);
    assert_eq!(next.read().unwrap(), None);
    assert_eq!(backend.read("oauth").unwrap(), None);
    next.write(Some("signed-in")).unwrap();
    next.finish().unwrap();
    drop(next);
    assert_eq!(
        acquire(&coordinator).read().unwrap().as_deref(),
        Some("signed-in")
    );
}

#[test]
fn failed_refresh_write_never_exposes_the_old_token() {
    let (_temp, _, backend, coordinator) = fixture();
    let lease = acquire(&coordinator);
    backend.fail_write.store(true, Ordering::SeqCst);
    assert!(lease.write(Some("new")).is_err());
    assert!(lease.finish().is_err());
    assert!(lease.read().is_err());
    drop(lease);
    let (tx, rx) = mpsc::channel();
    let mut wait = coordinator.acquire(
        "oauth",
        Arc::new(move || {
            let _ = tx.send(());
        }),
    );
    rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(wait.take().unwrap().is_err());
    backend.fail_write.store(false, Ordering::SeqCst);
    assert_eq!(acquire(&coordinator).read().unwrap(), None);
}

#[test]
fn fresh_read_failure_is_not_a_snapshot_fallback() {
    let (_temp, _, backend, coordinator) = fixture();
    let lease = acquire(&coordinator);
    backend.fail_read.store(true, Ordering::SeqCst);
    assert!(lease.read().is_err());
}

#[test]
fn earlier_and_concurrent_ordinary_writes_cannot_replay_after_release() {
    let (_temp, namespace, backend, coordinator) = fixture();
    let earlier = namespace.stamp("oauth");
    let lease = acquire(&coordinator);
    let concurrent = namespace.stamp("oauth");
    lease.write(Some("new")).unwrap();
    lease.finish().unwrap();
    drop(lease);
    for stamp in [earlier, concurrent] {
        namespace
            .write("oauth", &stamp, || backend.write("oauth", Some("old")))
            .unwrap();
    }
    assert_eq!(backend.read("oauth").unwrap().as_deref(), Some("new"));
    let current = namespace.stamp("oauth");
    namespace
        .write("oauth", &current, || backend.write("oauth", Some("later")))
        .unwrap();
    assert_eq!(backend.read("oauth").unwrap().as_deref(), Some("later"));
}

#[test]
fn corrupt_marker_and_unusable_scope_fail_closed() {
    let (temp, namespace, _, coordinator) = fixture();
    let lease = acquire(&coordinator);
    drop(lease);
    std::fs::write(temp.0.join(filename("oauth")), b"corrupt").unwrap();
    assert!(namespace.lock("oauth").is_err());
    let bad = Temp::new();
    std::fs::write(&bad.0, b"not a directory").unwrap();
    assert!(Namespace::disk(bad.0.join("child")).lock("oauth").is_err());
    std::fs::remove_file(&bad.0).unwrap();
}

#[test]
fn cancelled_pending_wait_does_not_consume_a_lease_or_poison() {
    let (_temp, _, _, coordinator) = fixture();
    let first = acquire(&coordinator);
    let wait = coordinator.acquire("oauth", Arc::new(|| {}));
    drop(wait);
    drop(first);
    let lease = acquire(&coordinator);
    assert_eq!(lease.read().unwrap().as_deref(), Some("old"));
}

#[test]
fn process_lease_excludes_parent_and_survives_abandoned_effect() {
    const CHILD: &str = "EXACT_SECRET_LEASE_CHILD";
    if let Some(root) = std::env::var_os(CHILD) {
        let namespace = Namespace::disk(root.into());
        let coordinator = Coordinator::new(namespace, Arc::new(Values::default()));
        let lease = acquire(&coordinator);
        lease.begin_effect().unwrap();
        println!("lease armed");
        std::io::stdout().flush().unwrap();
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        std::process::exit(0); // Deliberately omit lease drop and finish.
    }
    let (temp, namespace, backend, coordinator) = fixture();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "secrets::tests::process_lease_excludes_parent_and_survives_abandoned_effect",
            "--nocapture",
        ])
        .env(CHILD, &temp.0)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        for line in std::io::BufReader::new(output).lines() {
            if line.unwrap().contains("lease armed") {
                let _ = tx.send(());
                break;
            }
        }
    });
    if rx.recv_timeout(Duration::from_secs(5)).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("child never acquired lease");
    }
    assert!(namespace.lock("oauth").unwrap().is_none());
    child.stdin.take().unwrap().write_all(b"exit\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not exit");
        }
        std::thread::yield_now();
    }
    let lease = acquire(&coordinator);
    assert_eq!(lease.read().unwrap(), None);
    assert_eq!(backend.read("oauth").unwrap(), None);
}

#[test]
fn ordinary_stamp_does_not_create_persistence_files() {
    let temp = Temp::new();
    let namespace = Namespace::disk(temp.0.clone());
    assert!(matches!(namespace.stamp("oauth").0, Ok(Some((0, false)))));
    assert!(!temp.0.exists());
}

#[test]
fn shared_secret_broker_bounds_waiters_and_releases_cancelled_slots() {
    const CHILD: &str = "EXACT_SECRET_BROKER_LIMIT_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "secrets::tests::shared_secret_broker_bounds_waiters_and_releases_cancelled_slots",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let coordinator = Coordinator::new(Namespace::memory(), Arc::new(Values::default()));
    let holder = acquire(&coordinator);
    let pending: Vec<_> = (1..MAX_LEASES)
        .map(|_| coordinator.acquire("oauth", Arc::new(|| {})))
        .collect();
    let mut refused = coordinator.acquire("oauth", Arc::new(|| {}));
    assert!(refused.take().unwrap().is_err());
    drop(pending);
    drop(holder);
    let lease = acquire(&coordinator);
    assert_eq!(lease.read().unwrap(), None);
}

#[cfg(not(unix))]
#[test]
fn file_secret_durability_is_refused_without_an_acknowledged_platform_primitive() {
    assert!(sync_directory_chain(std::path::Path::new("scratch/secrets")).is_err());
}
