//! The app's kept secrets on Apple platforms (LLP 1018 D6): `ibex2::host`'s
//! `Secrets` — the Keychain (ibex LLP 0069) — read into a snapshot before the
//! runner boots and written after each commit, on the main thread, never
//! through Swift. The runner's kept answers (LLP 1027 D4) travel the same
//! way through `ibex2`'s kv store, a file per answer.
//!
//! Agent mode (`EXACT_AGENT=1`, LLP 1012) gets memory stores unless
//! `EXACT_STORE=real` says otherwise: a scripted drive starts from nothing
//! and leaves nothing in a developer's keychain. `EXACT_STORE=memory` asks
//! for the same memory stores outside agent mode — a test that needs the real
//! filesystem and database but must not raise the keychain's prompt.

use exact_runner::{Store, StoreWrite};
use ibex2::boundary::HostError;
use ibex2::host::{Bindings, Host, Kv, Secrets};

/// The kv scope the runner's kept answers live in (LLP 1027 D4): beside the
/// Keychain, not in it — a cache of settled data, any resource's name a
/// key, read in one listing at launch.
const KEPT: &str = "exact.kept";

/// The app's bindings from its grants (LLP 1016 D6), and the scope the
/// runner keeps answers in. Grants that do not parse are the error, which
/// the host journals; every request is then refused, naming it.
pub fn endow(grants: &str) -> Result<Bindings, String> {
    let mut host = Host::new();
    let agent = std::env::var_os("EXACT_AGENT").is_some();
    let store = std::env::var("EXACT_STORE").unwrap_or_default();
    if (agent && store != "real") || store == "memory" {
        host = host
            .with_secret_store(Box::new(ibex2::secrets::MemoryStore::new()))
            .with_kv_store(Box::new(ibex2::kv::MemoryStore::new()));
    }
    #[cfg(test)]
    if let Some(secrets) = PLATFORM.with(|p| p.borrow().clone()) {
        host = host
            .with_secret_store(Box::new(Shared(secrets)))
            .with_kv_store(Box::new(ibex2::kv::MemoryStore::new()));
    }
    endow_in(host, grants)
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

/// Where a commit's store writes go (LLP 1018 D6): the app's secrets to the
/// platform's secret store, the runner's kept answers to its kv store.
#[derive(Clone)]
pub struct Platform {
    secrets: Secrets,
    kv: Kv,
}

impl Platform {
    /// The stores `bindings` hold.
    pub fn of(bindings: &Bindings) -> Platform {
        Platform {
            secrets: bindings.secrets.clone(),
            kv: bindings.kv.clone(),
        }
    }

    /// Keep or forget one write.
    pub fn write(&self, w: &StoreWrite) -> Result<(), HostError> {
        match (w.name.strip_prefix(Store::KEPT), &w.value) {
            (Some(key), Some(v)) => self.kv.set_text(KEPT, key, v),
            (Some(key), None) => self.kv.delete(KEPT, key),
            (None, Some(v)) => self.secrets.set(&w.name, v),
            (None, None) => self.secrets.forget(&w.name),
        }
    }
}

/// What the store holds under the granted names — the runner's snapshot.
/// A name the store cannot read is absent, as an ungranted one is.
pub fn snapshot_of(bindings: Option<&Bindings>) -> Vec<(String, String)> {
    let Some(b) = bindings else {
        return Vec::new();
    };
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
}
