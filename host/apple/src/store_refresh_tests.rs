use super::*;
use ibex2::{kv::KvStore, secrets::SecretStore};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Default)]
struct Faults {
    mode: AtomicUsize,
    secrets: ibex2::secrets::MemoryStore,
    kv: ibex2::kv::MemoryStore,
}
struct FaultSecrets(Arc<Faults>);
impl SecretStore for FaultSecrets {
    fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        if self.0.mode.load(Ordering::SeqCst) & 1 != 0 {
            return Err(HostError::Failed("locked".into()));
        }
        self.0.secrets.get(name)
    }
    fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        self.0.secrets.set(name, value)
    }
    fn forget(&self, name: &str) -> Result<(), HostError> {
        self.0.secrets.forget(name)
    }
}
struct FaultKv(Arc<Faults>);
impl KvStore for FaultKv {
    fn get(&self, scope: &str, key: &str) -> Result<Option<Vec<u8>>, HostError> {
        if self.0.mode.load(Ordering::SeqCst) & 2 != 0 {
            return Err(HostError::Failed("read".into()));
        }
        self.0.kv.get(scope, key)
    }
    fn set(&self, scope: &str, key: &str, value: &[u8]) -> Result<(), HostError> {
        self.0.kv.set(scope, key, value)
    }
    fn delete(&self, scope: &str, key: &str) -> Result<(), HostError> {
        self.0.kv.delete(scope, key)
    }
    fn keys(&self, scope: &str) -> Result<Vec<String>, HostError> {
        if self.0.mode.load(Ordering::SeqCst) & 4 != 0 {
            return Err(HostError::Failed("listing".into()));
        }
        self.0.kv.keys(scope)
    }
}

#[test]
fn refresh_keeps_unreadable_values_and_cap_omissions_but_deletes_confirmed_absences() {
    let faults = Arc::new(Faults::default());
    let host = Host::new()
        .with_secret_store(Box::new(FaultSecrets(faults.clone())))
        .with_kv_store(Box::new(FaultKv(faults.clone())));
    let bindings = endow_in(host, "secret.keep favourite").unwrap();
    let platform = Platform::of(&bindings);
    faults.secrets.set("favourite", "Palo Alto").unwrap();
    faults.kv.set(STATE, "starred", b"true").unwrap();
    faults.kv.set(KEPT, "feed", b"kept").unwrap();
    let previous = platform.snapshot(Vec::new()).values;
    faults.secrets.forget("favourite").unwrap();
    for mode in [1, 3, 5] {
        faults.mode.store(mode, Ordering::SeqCst);
        let read = platform.snapshot(previous.clone());
        assert!(!read.complete);
        assert_eq!(
            read.values, previous,
            "a failed get or listing is no deletion"
        );
    }
    faults.mode.store(0, Ordering::SeqCst);
    faults.kv.delete(STATE, "starred").unwrap();
    faults
        .kv
        .set(KEPT, "feed", &vec![0; Store::KEPT_TOTAL_BYTES + 1])
        .unwrap();
    let read = platform.snapshot(previous);
    assert!(read.complete);
    assert_eq!(read.omitted, vec!["exact.kept.feed"]);
    faults.kv.set(KEPT, "feed", b"small").unwrap();
    faults.mode.store(5, Ordering::SeqCst); // unrelated secrets and listings still fail
    let retry = platform.retry_snapshot(read.values.clone(), &read.omitted);
    assert!(retry.complete && retry.omitted.is_empty());
    assert_eq!(
        retry.values,
        vec![("exact.kept.feed".into(), b"small".to_vec())]
    );
    assert_eq!(read.values, vec![("exact.kept.feed".into(), b"kept".to_vec())], "only the confirmed secret and state absences delete; the cap retains the previous kept answer");
}
