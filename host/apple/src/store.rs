//! The app's kept secrets on Apple platforms (LLP 1018 D6): `ibex2::host`'s
//! `Secrets` — the Keychain (ibex LLP 0069) — read into a snapshot before the
//! runner boots and written after each commit, on the main thread, never
//! through Swift. The runner's kept answers (LLP 1027 D4) go through
//! `ibex2`'s kv store, a file per answer, written on a thread of their own
//! (`flush_kept`).
//!
//! Agent mode (`EXACT_AGENT=1`, LLP 1012) gets memory stores unless
//! `EXACT_STORE=real` says otherwise: a scripted drive starts from nothing
//! and leaves nothing in a developer's keychain. A drive that names a scratch
//! store (`EXACT_AGENT_STORAGE`) keeps secrets in that tree instead, so a
//! reload reads them back (platformer R10). `EXACT_STORE=memory` asks for
//! memory stores outside agent mode — a test that needs the real filesystem
//! and database but must not raise the keychain's prompt.

use exact_runner::{Store, StoreWrite};
use ibex2::boundary::HostError;
use ibex2::host::{Bindings, Host, Kv, Secrets};

/// The kv scope the runner's kept answers live in (LLP 1027 D4): beside the
/// Keychain, not in it — a cache of settled data, any resource's name a
/// key, read in one listing at launch.
const KEPT: &str = "exact.kept";

/// The app's bindings from its grants (LLP 1016 D6), and the scope the
/// runner keeps answers in. Grants that do not parse are the error, which
/// the host journals; every request is then refused, naming it. An empty
/// app id, as a test that stands a platform store in, stays in memory.
pub fn endow(grants: &str) -> Result<Bindings, String> {
    endow_for(grants, "")
}

/// [`endow`] for `app_id`. A named agent drive keeps `secret.keep` in its
/// scratch tree (files, not the Keychain).
pub fn endow_for(grants: &str, app_id: &str) -> Result<Bindings, String> {
    let mut host = Host::new();
    let agent = std::env::var_os("EXACT_AGENT").is_some();
    let store = std::env::var("EXACT_STORE").unwrap_or_default();
    if (agent && store != "real") || store == "memory" {
        host = match crate::picker::agent_secret_root(app_id) {
            Some(root) => host
                .with_secret_store(Box::new(ibex2::secrets::FileStore::new(
                    root.join("secrets"),
                )))
                .with_kv_store(Box::new(ibex2::kv::FileStore::new(root.join("kv")))),
            None => host
                .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
                .with_kv_store(Box::new(ibex2::kv::MemoryStore::new())),
        };
    }
    #[cfg(test)]
    if let Some(secrets) = PLATFORM.with(|p| p.borrow().clone()) {
        host = host
            .with_secret_store(Box::new(Shared(secrets)))
            .with_kv_store(Box::new(ibex2::kv::MemoryStore::new()));
    }
    endow_in(host, grants)
}

/// [`endow_for`] for a boot. One that reads the store (`fresh`, not a
/// reload carrying memory) first empties a fresh drive's scratch tree, once
/// a process and before anything is written there (`picker::empty_fresh_tree`).
pub fn endow_bound(grants: &str, app_id: &str, fresh: bool) -> (Option<Bindings>, Option<String>) {
    if fresh {
        if let Err(e) = crate::picker::empty_fresh_tree(app_id) {
            return (None, Some(format!("{e:?}")));
        }
    }
    match endow_for(grants, app_id) {
        Ok(b) => (Some(b), None),
        Err(e) => (None, Some(e)),
    }
}

#[cfg(test)]
thread_local! {
    /// The platform's secret store as this thread's test stands it in: every
    /// `endow` on the thread shares it, as launches share the Keychain, so a
    /// test reads and writes the same store the bridge does without a
    /// developer's (possibly locked) keychain. The Keychain itself is
    /// `ibex2`'s `secrets::darwin` test.
    pub(crate) static PLATFORM: std::cell::RefCell<Option<std::sync::Arc<ibex2::secrets::MemoryStore>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
struct Shared(std::sync::Arc<ibex2::secrets::MemoryStore>);

#[cfg(test)]
impl ibex2::secrets::SecretStore for Shared {
    fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        self.0.get(name)
    }
    fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        self.0.set(name, value)
    }
    fn forget(&self, name: &str) -> Result<(), HostError> {
        self.0.forget(name)
    }
}

fn endow_in(host: Host, grants: &str) -> Result<Bindings, String> {
    // Appended, so an error's line number is the app's own.
    let spec = format!("{}\nstorage.kv {KEPT}", exact_runner::io_grants(grants));
    let set = ibex2::grant::GrantSet::parse(&spec)
        .map_err(|e| format!("the app's grants did not parse: {e}"))?;
    Ok(host.endow(set))
}

/// Where a commit's kept secrets and answers go (LLP 1018 D6): the
/// platform's stores, behind the I/O link (LLP 1047.001), so an archive that
/// links no I/O names neither them nor the Keychain and files under them.
pub trait KeptStore {
    /// Write one store change.
    fn write(&self, w: &StoreWrite) -> Result<(), String>;
    /// The kept answers the writer failed to write since the last call.
    fn kept_failures(&self) -> Vec<String>;
}

impl KeptStore for Platform {
    fn write(&self, w: &StoreWrite) -> Result<(), String> {
        Platform::write(self, w).map_err(|e| e.to_string())
    }
    fn kept_failures(&self) -> Vec<String> {
        Platform::kept_failures(self)
    }
}

/// The app's I/O as a boot finds it (LLP 1016 D6; LLP 1018 D6): its
/// bindings, the secrets and answers they kept, where commits keep more,
/// and why there are none when the grants did not bind.
pub struct Endowed {
    /// The bindings the executor takes.
    pub bindings: Option<Bindings>,
    /// What the platform's stores held, read before the runner boots.
    pub snapshot: Vec<(String, String)>,
    /// Where a commit's writes go.
    pub secrets: Option<Box<dyn KeptStore>>,
    /// Why the grants bound nothing.
    pub unbound: Option<String>,
}

impl Endowed {
    /// An archive that links no I/O: nothing bound, nothing kept.
    pub fn none() -> Endowed {
        Endowed {
            bindings: None,
            snapshot: Vec::new(),
            secrets: None,
            unbound: None,
        }
    }
}

/// [`endow_bound`] and, for a fresh session, what its stores held (a reload
/// carries the running store): [`crate::link::IoLinks`]'s endowment.
pub fn endowed(grants: &str, app_id: &str, fresh: bool) -> Endowed {
    let (bindings, unbound) = endow_bound(grants, app_id, fresh);
    Endowed {
        snapshot: if fresh {
            snapshot_of(bindings.as_ref())
        } else {
            Vec::new()
        },
        secrets: bindings
            .as_ref()
            .map(|b| Box::new(Platform::of(b)) as Box<dyn KeptStore>),
        bindings,
        unbound,
    }
}

/// Where a commit's store writes go (LLP 1018 D6): the app's secrets to the
/// platform's secret store, the runner's kept answers to its kv store.
#[derive(Clone)]
pub struct Platform {
    secrets: Secrets,
    kv: Kv,
    /// Kept answers the writer failed to write, for the journal.
    failed: std::sync::Arc<Failures>,
}

impl Platform {
    /// The stores `bindings` hold.
    pub fn of(bindings: &Bindings) -> Platform {
        Platform {
            secrets: bindings.secrets.clone(),
            kv: bindings.kv.clone(),
            failed: Default::default(),
        }
    }

    /// Keep or forget one write. A kept answer is handed to the writer and
    /// its failure, if any, is told later (`kept_failures`).
    pub fn write(&self, w: &StoreWrite) -> Result<(), HostError> {
        match (w.name.strip_prefix(Store::KEPT), &w.value) {
            (Some(key), value) => {
                let write = Kept {
                    kv: self.kv.clone(),
                    value: value.clone(),
                    failed: self.failed.clone(),
                };
                keep(
                    (
                        std::sync::Arc::as_ptr(&self.failed) as usize,
                        key.to_string(),
                    ),
                    write,
                );
                Ok(())
            }
            (None, Some(v)) => self.secrets.set(&w.name, v),
            (None, None) => self.secrets.forget(&w.name),
        }
    }

    /// The kept answers the writer failed to write since the last call, as
    /// journal lines: this platform's, and those of platforms dropped since
    /// (a reload's predecessor).
    pub fn kept_failures(&self) -> Vec<String> {
        let mut lines = std::mem::take(&mut *self.failed.0.lock().expect("kept"));
        lines.append(&mut ORPHANED.lock().expect("kept"));
        lines
    }
}

/// A platform's failed kept writes. Those still untold when the last
/// reference goes (its host replaced, its writes done) pass to the next
/// platform that asks.
#[derive(Default)]
struct Failures(std::sync::Mutex<Vec<String>>);

static ORPHANED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

impl Drop for Failures {
    fn drop(&mut self) {
        let lines = std::mem::take(&mut *self.0.lock().expect("kept"));
        ORPHANED.lock().expect("kept").extend(lines);
    }
}

/// The runner's kept answers are a cache of settled data (LLP 1027 D4), and
/// a commit wrote each one to disk before it returned: a file, a rename and
/// a sync per answer, while the main thread waited on the commit (62 ms of
/// a chat's opening frame on the Signal clone). One thread writes them
/// instead. What waits is one value per platform and key, the last kept
/// (a later write replaces a waiting one), so a store that changes faster
/// than the disk holds no more than its keys, and the last kept is the last
/// written. A launch's snapshot and the process's orderly exit wait for the
/// writer (`flush_kept`). An answer a crash loses is fetched again.
struct Kept {
    kv: Kv,
    value: Option<String>,
    failed: std::sync::Arc<Failures>,
}

type KeptKey = (usize, String);

#[derive(Default)]
struct Writer {
    order: std::collections::VecDeque<KeptKey>,
    waiting: std::collections::HashMap<KeptKey, Kept>,
    busy: bool,
}

static WRITER: std::sync::Mutex<Option<Writer>> = std::sync::Mutex::new(None);
static WRITTEN: std::sync::Condvar = std::sync::Condvar::new();

fn keep(key: KeptKey, write: Kept) {
    let mut guard = WRITER.lock().expect("kept");
    let writer = guard.get_or_insert_with(|| {
        std::thread::Builder::new()
            .name("exact.kept".into())
            .spawn(write_kept)
            .expect("the kept answers' writer thread");
        extern "C" fn drain() {
            // Everything kept until now is written, and nothing after: the
            // lock is held through the rest of the exit, so no write starts
            // that this drain would not see.
            std::mem::forget(wait_until_written(WRITER.lock().expect("kept")));
        }
        extern "C" {
            fn atexit(f: extern "C" fn()) -> i32;
        }
        // The process's orderly exit (an app quitting, the agent's `exit`)
        // writes what waits first. SAFETY: `drain` is a plain function,
        // registered once, that lives as long as the process.
        #[allow(unsafe_code)]
        unsafe {
            atexit(drain)
        };
        Writer::default()
    });
    // The last kept is the last written: a key kept again (by this platform
    // or another on the same storage) waits behind everything kept before.
    if writer.waiting.insert(key.clone(), write).is_some() {
        writer.order.retain(|k| *k != key);
    }
    writer.order.push_back(key);
    WRITTEN.notify_all();
}

/// The writer thread: the oldest waiting key's last value, one at a time.
fn write_kept() {
    let mut guard = WRITER.lock().expect("kept");
    loop {
        let writer = guard.as_mut().expect("the writer's state");
        let Some(key) = writer.order.pop_front() else {
            guard = WRITTEN.wait(guard).expect("kept");
            continue;
        };
        let write = writer.waiting.remove(&key).expect("a waiting kept answer");
        writer.busy = true;
        drop(guard);
        write_one(&key.1, write);
        guard = WRITER.lock().expect("kept");
        guard.as_mut().expect("the writer's state").busy = false;
        WRITTEN.notify_all();
    }
}

fn write_one(key: &str, write: Kept) {
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &write.value {
        Some(v) => write.kv.set_text(KEPT, key, v),
        None => write.kv.delete(KEPT, key),
    }));
    let why = match done {
        Ok(Ok(())) => return,
        Ok(Err(e)) => e.to_string(),
        Err(_) => "the store panicked".to_string(),
    };
    let line = format!("store {}{key} failed: {why}", Store::KEPT);
    write.failed.0.lock().expect("kept").push(line);
}

/// Wait until nothing is waiting to be written and the writer is idle.
pub fn flush_kept() {
    drop(wait_until_written(WRITER.lock().expect("kept")));
}

fn wait_until_written(
    mut guard: std::sync::MutexGuard<'static, Option<Writer>>,
) -> std::sync::MutexGuard<'static, Option<Writer>> {
    while guard
        .as_ref()
        .is_some_and(|w| w.busy || !w.order.is_empty())
    {
        guard = WRITTEN.wait(guard).expect("kept");
    }
    guard
}

/// What the store holds under the granted names — the runner's snapshot.
/// A name the store cannot read is absent, as an ungranted one is.
pub fn snapshot_of(bindings: Option<&Bindings>) -> Vec<(String, String)> {
    let Some(b) = bindings else {
        return Vec::new();
    };
    // What a runner before this one kept is on disk first.
    flush_kept();
    let kept =
        b.kv.keys(KEPT)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|key| {
                let value = b.kv.get_text(KEPT, &key).ok().flatten()?;
                Some((format!("{}{key}", Store::KEPT), value))
            });
    b.secrets
        .names()
        .iter()
        .filter_map(|n| b.secrets.get(n).ok().flatten().map(|v| (n.to_string(), v)))
        .chain(kept)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_answers_outlive_the_launch_beside_the_apps_secrets() {
        let dir = std::env::temp_dir().join(format!("exact-kept-{}", std::process::id()));
        let launch = || {
            let host = Host::new()
                .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
                .with_kv_store(Box::new(ibex2::kv::FileStore::new(&dir)));
            endow_in(host, "secret.keep app.token").unwrap()
        };
        let write = |name: &str, value: Option<&str>| StoreWrite {
            name: name.into(),
            value: value.map(str::to_string),
        };
        let first = Platform::of(&launch());
        // A resource's own name is the key, whatever its case.
        first
            .write(&write("exact.kept.crewState", Some("args|value")))
            .unwrap();
        first.write(&write("app.token", Some("t"))).unwrap();
        assert!(snapshot_of(Some(&launch()))
            .contains(&("exact.kept.crewState".into(), "args|value".into())));
        first.write(&write("exact.kept.crewState", None)).unwrap();
        assert!(!snapshot_of(Some(&launch()))
            .iter()
            .any(|(n, _)| n.starts_with(Store::KEPT)));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_kept_answer_is_written_off_the_commit_and_its_failure_told_later() {
        let host = Host::new()
            .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
            .with_kv_store(Box::new(ibex2::kv::UnavailableStore));
        let platform = Platform::of(&endow_in(host, "").unwrap());
        let kept = StoreWrite {
            name: "exact.kept.feed".into(),
            value: Some("v".into()),
        };
        assert!(
            platform.write(&kept).is_ok(),
            "the commit does not wait for it"
        );
        flush_kept();
        let failed = platform.kept_failures();
        assert!(
            failed.len() == 1 && failed[0].starts_with("store exact.kept.feed failed"),
            "{failed:?}"
        );
        assert!(platform.kept_failures().is_empty(), "told once");
    }

    /// A kv store whose writes wait for a gate, counting them.
    struct Gated {
        inner: ibex2::kv::MemoryStore,
        open: std::sync::Mutex<bool>,
        opened: std::sync::Condvar,
        sets: std::sync::atomic::AtomicUsize,
    }

    struct GatedStore(std::sync::Arc<Gated>);

    impl ibex2::kv::KvStore for GatedStore {
        fn get(&self, scope: &str, key: &str) -> Result<Option<Vec<u8>>, HostError> {
            self.0.inner.get(scope, key)
        }
        fn set(&self, scope: &str, key: &str, value: &[u8]) -> Result<(), HostError> {
            let mut open = self.0.open.lock().unwrap();
            while !*open {
                open = self.0.opened.wait(open).unwrap();
            }
            self.0
                .sets
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            self.0.inner.set(scope, key, value)
        }
        fn delete(&self, scope: &str, key: &str) -> Result<(), HostError> {
            self.0.inner.delete(scope, key)
        }
        fn keys(&self, scope: &str) -> Result<Vec<String>, HostError> {
            self.0.inner.keys(scope)
        }
    }

    #[test]
    fn kept_answers_are_written_in_order_after_the_commit_and_a_snapshot_waits() {
        let gated = std::sync::Arc::new(Gated {
            inner: ibex2::kv::MemoryStore::new(),
            open: std::sync::Mutex::new(false),
            opened: std::sync::Condvar::new(),
            sets: Default::default(),
        });
        let bindings = endow_in(
            Host::new()
                .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
                .with_kv_store(Box::new(GatedStore(gated.clone()))),
            "",
        )
        .unwrap();
        let platform = Platform::of(&bindings);
        // Opened however the test ends: a shut gate would hold every later
        // flush in the process.
        struct Opens(std::sync::Arc<Gated>);
        impl Drop for Opens {
            fn drop(&mut self) {
                *self.0.open.lock().unwrap() = true;
                self.0.opened.notify_all();
            }
        }
        let opens = Opens(gated.clone());
        let keep = |value: Option<&str>| StoreWrite {
            name: "exact.kept.feed".into(),
            value: value.map(str::to_string),
        };
        // The disk is shut: every write still returns.
        for value in [Some("1"), Some("2"), None, Some("3"), Some("4")] {
            platform.write(&keep(value)).unwrap();
        }
        let other = StoreWrite {
            name: "exact.kept.other".into(),
            value: Some("o".into()),
        };
        platform.write(&other).unwrap();
        assert_eq!(gated.sets.load(std::sync::atomic::Ordering::SeqCst), 0);
        // Another session on the same storage keeps the key, then this one
        // again: the last kept is the last written.
        let beside = Platform::of(
            &endow_in(
                Host::new()
                    .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
                    .with_kv_store(Box::new(GatedStore(gated.clone()))),
                "",
            )
            .unwrap(),
        );
        beside.write(&keep(Some("beside"))).unwrap();
        platform.write(&keep(Some("5"))).unwrap();
        let reader = std::thread::spawn(move || snapshot_of(Some(&bindings)));
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(!reader.is_finished(), "a snapshot waits for the queue");
        drop(opens);
        let snapshot = reader.join().unwrap();
        assert!(
            snapshot.contains(&("exact.kept.feed".into(), "5".into()))
                && snapshot.contains(&("exact.kept.other".into(), "o".into())),
            "{snapshot:?}"
        );
        // The first write may have been taken alone; the rest, waiting
        // behind it, are one write per platform and key.
        assert!(gated.sets.load(std::sync::atomic::Ordering::SeqCst) <= 4);
        assert!(platform.kept_failures().is_empty());
    }

    #[test]
    fn grants_that_do_not_parse_are_named() {
        let error = endow_in(
            Host::new(),
            "net.fetch https://a.example\nsecret.keep jwtToken",
        )
        .err()
        .unwrap();
        assert!(
            error.contains("line 2") && error.contains("jwtToken"),
            "{error}"
        );
    }

    /// A named agent drive keeps `secret.keep` in its scratch tree, and a
    /// fresh launch reads nothing back (platformer R10). A child, so the
    /// environment stays off this process.
    #[test]
    fn named_agent_storage_keeps_a_secret_across_a_relaunch() {
        const CHILD: &str = "EXACT_APPLE_SECRET_KEEP_TEST";
        if std::env::var_os(CHILD).is_none() {
            let home =
                std::env::temp_dir().join(format!("exact-apple-secrets-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&home);
            std::fs::create_dir_all(&home).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "store::tests::named_agent_storage_keeps_a_secret_across_a_relaunch",
                ])
                .env(CHILD, "1")
                .env("HOME", &home)
                .env("EXACT_AGENT", "1")
                .env("EXACT_AGENT_STORAGE", "s1")
                .env_remove("EXACT_AGENT_STORAGE_FRESH")
                .env_remove("EXACT_STORE")
                .output()
                .unwrap();
            let _ = std::fs::remove_dir_all(&home);
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let app = "test.exact.keep";
        let grants = "secret.keep platformer.best";
        let launch = || endow_for(grants, app).unwrap();
        let write = |name: &str, value: Option<&str>| StoreWrite {
            name: name.into(),
            value: value.map(str::to_string),
        };
        let first = Platform::of(&launch());
        first
            .write(&write("platformer.best", Some("22050")))
            .unwrap();
        first.write(&write("exact.kept.score", Some("9"))).unwrap();
        let again = snapshot_of(Some(&launch()));
        assert!(
            again.contains(&("platformer.best".into(), "22050".into())),
            "{again:?}"
        );
        assert!(
            again.contains(&("exact.kept.score".into(), "9".into())),
            "{again:?}"
        );
        let file = crate::picker::agent_secret_root(app)
            .unwrap()
            .join("secrets")
            .join("platformer.best");
        assert!(file.is_file(), "{}", file.display());
        // An empty app id is the test stand-in: memory, and a new endow
        // does not see the write or touch the scratch file.
        let memory = endow_for(grants, "").unwrap();
        Platform::of(&memory)
            .write(&write("platformer.best", Some("nope")))
            .unwrap();
        assert!(snapshot_of(Some(&endow_for(grants, "").unwrap())).is_empty());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "22050");
        std::env::set_var("EXACT_AGENT_STORAGE_FRESH", "1");
        let (fresh, err) = endow_bound(grants, app, true);
        assert!(err.is_none(), "{err:?}");
        assert!(
            snapshot_of(fresh.as_ref()).is_empty(),
            "a fresh launch reads nothing"
        );
        assert!(!file.exists());
    }
}
