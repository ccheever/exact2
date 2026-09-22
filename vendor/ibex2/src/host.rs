//! The standard library for a consumer with no engine.
//!
//! `rules/NOT-DOING.md` sets the bar: a no-JS consumer — Exact 2's plan
//! runner is one — gets the same standard library with no engine in the
//! process at all. This is that surface. It is not a second implementation:
//! every function here is the one the JavaScript bindings call, behind the
//! same `boundary::admit`, and the grants are the same `GrantSet` a manifest
//! section parses to. What is stated in Rust is LLP 0067's model: a consumer
//! is *endowed* with bindings that carry their grant — the module parameter
//! list, as a struct — and a request is checked against the grant the binding
//! carries, never against who is calling.
//!
//! Synchronous, deliberately. These are the primitives; an executor is the
//! consumer's, and a runner with its own loop puts them on its own workers.
//!
//! @ref LLP 0068#1-the-shape — bindings that carry their grant
//! @ref LLP 0067#3-the-check — one chokepoint, the grant the binding carries

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::boundary::HostError;
use crate::grant::{GrantSet, Operation};
use crate::kv::{self, KvStore};
use crate::secrets::{self, SecretStore};
use crate::stdlib::fetch::{self, Request, Response, Transport};
use crate::stdlib::fs::{self, FsOp, FsResult, Stat};

/// The host: the platform's transport, its secret store, and its kv store,
/// and nothing else. One per process is the expected shape; it is the
/// analogue of the runtime, without an engine.
pub struct Host {
    transport: Arc<dyn Transport>,
    secrets: Arc<dyn SecretStore>,
    kv: Arc<dyn KvStore>,
    app_directories: Option<Arc<crate::stdlib::app_fs::AppDirectories>>,
    sqlite_provider: Option<Arc<dyn crate::stdlib::sqlite::Provider>>,
}

impl Host {
    /// The platform's transport (`NSURLSession` on Apple platforms), its
    /// secret store (the Keychain there; a file per secret elsewhere), and
    /// its kv store (a file per key under the app-state directory).
    pub fn new() -> Self {
        Self::with_transport(crate::transport::default_transport())
    }

    pub fn with_transport(transport: Box<dyn Transport>) -> Self {
        Self {
            transport: Arc::from(transport),
            secrets: Arc::from(secrets::default_store()),
            kv: Arc::from(kv::default_store()),
            app_directories: None,
            sqlite_provider: None,
        }
    }

    /// This host over another secret store — the memory store for a consumer
    /// that must leave no trace, a test's own (LLP 0069 §3).
    pub fn with_secret_store(mut self, store: Box<dyn SecretStore>) -> Self {
        self.secrets = Arc::from(store);
        self
    }

    /// This host over another kv store — the memory store, or a directory the
    /// consumer chose (LLP 0070 §3).
    pub fn with_kv_store(mut self, store: Box<dyn KvStore>) -> Self {
        self.kv = Arc::from(store);
        self
    }

    pub fn with_app_directories(
        mut self,
        directories: crate::stdlib::app_fs::AppDirectories,
    ) -> Self {
        self.app_directories = Some(Arc::new(directories));
        self
    }
    pub fn with_sqlite_provider(
        mut self,
        provider: Arc<dyn crate::stdlib::sqlite::Provider>,
    ) -> Self {
        self.sqlite_provider = Some(provider);
        self
    }

    /// Endow a consumer with `grants`: the bindings a module receives as
    /// parameters, each carrying this grant set for its whole life.
    pub fn endow(&self, grants: GrantSet) -> Bindings {
        let grants = Arc::new(grants);
        Bindings {
            fetch: Fetch {
                transport: Arc::clone(&self.transport),
                grants: Arc::clone(&grants),
            },
            fs: Fs {
                grants: Arc::clone(&grants),
                directories: self.app_directories.clone(),
            },
            sqlite: Sqlite {
                grants: Arc::clone(&grants),
                directories: self.app_directories.clone(),
                provider: self.sqlite_provider.clone(),
            },
            secrets: Secrets {
                store: Arc::clone(&self.secrets),
                grants: Arc::clone(&grants),
            },
            kv: Kv {
                store: Arc::clone(&self.kv),
                grants: Arc::clone(&grants),
            },
            env: Env { grants },
        }
    }
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

/// What a consumer holds: `fetch`, `fs`, `secrets`, `kv`, and `process.env`,
/// as a module has them, over one grant set.
pub struct Bindings {
    pub fetch: Fetch,
    pub fs: Fs,
    pub sqlite: Sqlite,
    pub secrets: Secrets,
    pub kv: Kv,
    pub env: Env,
}

/// The secrets a consumer keeps across launches, carrying their grant
/// (`secret.keep <name>`, LLP 0069). Every call admits the name first; a
/// denial never reaches the platform's store.
#[derive(Clone)]
pub struct Secrets {
    store: Arc<dyn SecretStore>,
    grants: Arc<GrantSet>,
}

impl Secrets {
    fn admit(&self, name: &str) -> Result<(), HostError> {
        crate::boundary::admit(
            &self.grants,
            &Operation::SecretKeep {
                name: name.to_string(),
            },
        )
    }

    /// The names this binding may keep, in a stable order — what a host
    /// reads into a snapshot before its consumer boots.
    pub fn names(&self) -> Vec<&str> {
        self.grants.kept_secrets()
    }

    /// The kept value under `name`; `None` when nothing is kept.
    pub fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        self.admit(name)?;
        self.store.get(name)
    }

    /// Keep `value` under `name`, replacing what was there.
    pub fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        self.admit(name)?;
        self.store.set(name, value)
    }

    /// Forget `name`; forgetting what was never kept is not an error.
    pub fn forget(&self, name: &str) -> Result<(), HostError> {
        self.admit(name)?;
        self.store.forget(name)
    }
}

/// Durable state a consumer keeps that is not a secret, carrying its grant
/// (`storage.kv <scope>`, LLP 0070). Every call admits the scope first; a
/// denial never reaches the store.
#[derive(Clone)]
pub struct Kv {
    store: Arc<dyn KvStore>,
    grants: Arc<GrantSet>,
}

impl Kv {
    /// Admission first — an ungranted scope is `Denied` whatever else is
    /// wrong with the call — then the family grammar, here rather than in
    /// each store, so a consumer-supplied `KvStore` never sees a scope or
    /// key the family would refuse (a `GrantSet` built with `with` rather
    /// than parsed can hold a scope `parse` would not admit).
    fn admit(&self, scope: &str, key: Option<&str>) -> Result<(), HostError> {
        crate::boundary::admit(
            &self.grants,
            &Operation::StorageKv {
                scope: scope.to_string(),
            },
        )?;
        kv::check_scope(scope)?;
        if let Some(key) = key {
            kv::check_key(key)?;
        }
        Ok(())
    }

    /// The scopes this binding may use, in a stable order.
    pub fn scopes(&self) -> Vec<&str> {
        self.grants.kv_scopes()
    }

    /// The kept bytes under `key`; `None` when nothing is kept.
    pub fn get(&self, scope: &str, key: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.admit(scope, Some(key))?;
        self.store.get(scope, key)
    }

    /// The kept value as text; an error if what is kept is not UTF-8.
    pub fn get_text(&self, scope: &str, key: &str) -> Result<Option<String>, HostError> {
        match self.get(scope, key)? {
            None => Ok(None),
            Some(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| HostError::Failed(format!("kv: {scope}/{key} is not UTF-8"))),
        }
    }

    /// Keep `value` under `key`, replacing what was there.
    pub fn set(&self, scope: &str, key: &str, value: &[u8]) -> Result<(), HostError> {
        self.admit(scope, Some(key))?;
        self.store.set(scope, key, value)
    }

    /// Keep `value` under `key` as UTF-8 bytes.
    pub fn set_text(&self, scope: &str, key: &str, value: &str) -> Result<(), HostError> {
        self.set(scope, key, value.as_bytes())
    }

    /// Delete `key`; deleting what was never kept is not an error.
    pub fn delete(&self, scope: &str, key: &str) -> Result<(), HostError> {
        self.admit(scope, Some(key))?;
        self.store.delete(scope, key)
    }

    /// Every key kept under `scope`, in a stable order.
    pub fn keys(&self, scope: &str) -> Result<Vec<String>, HostError> {
        self.admit(scope, None)?;
        self.store.keys(scope)
    }
}

/// `fetch`, carrying its grant. Rust owns redirects, header folding, and the
/// error taxonomy; the platform owns the connection.
#[derive(Clone)]
pub struct Fetch {
    transport: Arc<dyn Transport>,
    grants: Arc<GrantSet>,
}

impl Fetch {
    pub fn stream(
        &self,
        request: Request,
        signal: &crate::stdlib::abort::AbortSignal,
    ) -> Result<crate::stdlib::fetch::StreamingResponse, HostError> {
        fetch::fetch_stream(self.transport.as_ref(), &self.grants, request, signal)
    }

    pub fn send(&self, request: Request) -> Result<Response, HostError> {
        fetch::fetch(self.transport.as_ref(), &self.grants, request)
    }

    pub fn get(&self, url: &str) -> Result<Response, HostError> {
        self.send(Request::get(url))
    }
}

/// `fs`, carrying its grant. Paths are absolute, normalized lexically and
/// checked as spelt and as the filesystem will really resolve them
/// (LLP 0067 §3); a two-path operation needs read on the source and write on
/// the destination.
#[derive(Clone)]
pub struct Fs {
    grants: Arc<GrantSet>,
    directories: Option<Arc<crate::stdlib::app_fs::AppDirectories>>,
}

impl Fs {
    fn run(
        &self,
        op: FsOp,
        path: &str,
        destination: Option<&str>,
        data: Option<&[u8]>,
    ) -> Result<FsResult, HostError> {
        fs::run(
            &self.grants,
            self.directories.as_deref(),
            op,
            path,
            destination,
            data,
        )
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, HostError> {
        match self.run(FsOp::ReadFile, path, None, None)? {
            FsResult::Bytes(bytes) => Ok(bytes),
            FsResult::Text(text) => Ok(text.into_bytes()),
            other => Err(unexpected(other)),
        }
    }

    pub fn write_file(&self, path: &str, data: &[u8]) -> Result<(), HostError> {
        self.run(FsOp::WriteFile, path, None, Some(data))
            .map(|_| ())
    }

    pub fn atomic_write_file(&self, path: &str, data: &[u8]) -> Result<(), HostError> {
        self.run(FsOp::AtomicWriteFile, path, None, Some(data))
            .map(|_| ())
    }

    pub fn append_file(&self, path: &str, data: &[u8]) -> Result<(), HostError> {
        self.run(FsOp::AppendFile, path, None, Some(data))
            .map(|_| ())
    }

    pub fn read_dir(&self, path: &str) -> Result<Vec<String>, HostError> {
        match self.run(FsOp::ReadDir, path, None, None)? {
            FsResult::Names(names) => Ok(names),
            other => Err(unexpected(other)),
        }
    }

    pub fn mkdir(&self, path: &str) -> Result<(), HostError> {
        self.run(FsOp::Mkdir, path, None, None).map(|_| ())
    }

    pub fn remove(&self, path: &str) -> Result<(), HostError> {
        self.run(FsOp::Remove, path, None, None).map(|_| ())
    }

    pub fn stat(&self, path: &str) -> Result<Stat, HostError> {
        match self.run(FsOp::Stat, path, None, None)? {
            FsResult::Stat(stat) => Ok(stat),
            other => Err(unexpected(other)),
        }
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<(), HostError> {
        self.run(FsOp::Rename, from, Some(to), None).map(|_| ())
    }

    pub fn copy_file(&self, from: &str, to: &str) -> Result<(), HostError> {
        self.run(FsOp::CopyFile, from, Some(to), None).map(|_| ())
    }

    pub fn realpath(&self, path: &str) -> Result<String, HostError> {
        match self.run(FsOp::Realpath, path, None, None)? {
            FsResult::Text(text) => Ok(text),
            FsResult::Bytes(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
            other => Err(unexpected(other)),
        }
    }
}

fn unexpected(result: FsResult) -> HostError {
    HostError::Failed(format!("unexpected filesystem result: {result:?}"))
}

/// `process.env`, carrying its grant: a snapshot of exactly the variables the
/// grant names. An ungranted variable is absent — not refused, absent — which
/// is the whole capability model in one object (LLP 0059.000 §3.8).
#[derive(Clone)]
pub struct Env {
    grants: Arc<GrantSet>,
}

impl Env {
    pub fn get(&self, name: &str) -> Option<String> {
        if !self.grants.readable_env().contains(&name) {
            return None;
        }
        std::env::var(name).ok()
    }

    pub fn snapshot(&self) -> BTreeMap<String, String> {
        self.grants
            .readable_env()
            .into_iter()
            .filter_map(|name| {
                std::env::var(name)
                    .ok()
                    .map(|value| (name.to_string(), value))
            })
            .collect()
    }
}

/// A database opener carrying its provider, directory mounts, and path grants.
#[derive(Clone)]
pub struct Sqlite {
    grants: Arc<GrantSet>,
    directories: Option<Arc<crate::stdlib::app_fs::AppDirectories>>,
    provider: Option<Arc<dyn crate::stdlib::sqlite::Provider>>,
}
impl Sqlite {
    pub fn open(&self, path: &str) -> Result<crate::stdlib::sqlite::Database, HostError> {
        let path =
            crate::stdlib::app_fs::resolve_sqlite(&self.grants, self.directories.as_deref(), path)?;
        let provider = self
            .provider
            .as_ref()
            .ok_or_else(|| HostError::Failed("SQLite provider is not installed".into()))?;
        Ok(crate::stdlib::sqlite::Database::new(
            provider.open(crate::stdlib::sqlite::Location { path })?,
        ))
    }
}

#[cfg(test)]
mod secrets_tests {
    use super::*;
    use crate::secrets::MemoryStore;
    use crate::stdlib::fetch::Transport;

    /// A transport that is never reached.
    struct NoTransport;
    impl Transport for NoTransport {
        fn open(
            &self,
            _request: &Request,
            _: &crate::stdlib::abort::AbortSignal,
        ) -> Result<crate::stdlib::fetch::StreamingResponse, HostError> {
            Err(HostError::Failed("no transport in this test".into()))
        }
    }

    fn host() -> Host {
        Host::with_transport(Box::new(NoTransport)).with_secret_store(Box::new(MemoryStore::new()))
    }

    #[test]
    fn a_granted_secret_is_kept_read_and_forgotten() {
        let app = host().endow(GrantSet::parse("secret.keep castle.session\n").unwrap());
        assert_eq!(app.secrets.names(), vec!["castle.session"]);
        assert_eq!(app.secrets.get("castle.session").unwrap(), None);
        app.secrets.set("castle.session", "t0k").unwrap();
        assert_eq!(
            app.secrets.get("castle.session").unwrap().as_deref(),
            Some("t0k")
        );
        app.secrets.forget("castle.session").unwrap();
        assert_eq!(app.secrets.get("castle.session").unwrap(), None);
    }

    #[test]
    fn an_ungranted_secret_is_denied_before_any_store_is_asked() {
        let store = Box::new(MemoryStore::with([("other".to_string(), "v".to_string())]));
        let app = host()
            .with_secret_store(store)
            .endow(GrantSet::parse("secret.keep castle.session\n").unwrap());
        for result in [
            app.secrets.get("other").map(|_| ()),
            app.secrets.set("other", "x"),
            app.secrets.forget("other"),
        ] {
            assert_eq!(
                result,
                Err(HostError::Denied {
                    capability: "secret.keep"
                })
            );
        }
        // Two consumers over one host share the store but not the authority:
        // the granted one keeps a value in the very store the ungranted one
        // is denied.
        let shared = host();
        let keeper = shared.endow(GrantSet::parse("secret.keep castle.session\n").unwrap());
        keeper.secrets.set("castle.session", "t0k").unwrap();
        let other = shared.endow(GrantSet::none());
        assert!(other.secrets.names().is_empty());
        assert_eq!(
            other.secrets.get("castle.session"),
            Err(HostError::Denied {
                capability: "secret.keep"
            })
        );
        assert_eq!(
            keeper.secrets.get("castle.session").unwrap().as_deref(),
            Some("t0k")
        );
    }
}

#[cfg(test)]
mod kv_tests {
    use super::*;
    use crate::kv::MemoryStore;
    use crate::stdlib::fetch::Transport;

    struct NoTransport;
    impl Transport for NoTransport {
        fn open(
            &self,
            _request: &Request,
            _: &crate::stdlib::abort::AbortSignal,
        ) -> Result<crate::stdlib::fetch::StreamingResponse, HostError> {
            Err(HostError::Failed("no transport in this test".into()))
        }
    }

    fn host() -> Host {
        Host::with_transport(Box::new(NoTransport)).with_kv_store(Box::new(MemoryStore::new()))
    }

    /// A store that must never be reached: every method is a test failure.
    struct UntouchableStore;
    impl KvStore for UntouchableStore {
        fn get(&self, s: &str, k: &str) -> Result<Option<Vec<u8>>, HostError> {
            panic!("a denied call reached the store: get {s}/{k}")
        }
        fn set(&self, s: &str, k: &str, _v: &[u8]) -> Result<(), HostError> {
            panic!("a denied call reached the store: set {s}/{k}")
        }
        fn delete(&self, s: &str, k: &str) -> Result<(), HostError> {
            panic!("a denied call reached the store: delete {s}/{k}")
        }
        fn keys(&self, s: &str) -> Result<Vec<String>, HostError> {
            panic!("a denied call reached the store: keys {s}")
        }
    }

    #[test]
    fn a_granted_scope_is_read_written_listed_and_deleted() {
        let app = host().endow(GrantSet::parse("storage.kv castle.state\n").unwrap());
        assert_eq!(app.kv.scopes(), vec!["castle.state"]);
        assert_eq!(app.kv.get("castle.state", "cursor").unwrap(), None);
        app.kv.set("castle.state", "cursor", b"42").unwrap();
        app.kv.set_text("castle.state", "rooms", "[1,2]").unwrap();
        assert_eq!(
            app.kv.get("castle.state", "cursor").unwrap().as_deref(),
            Some(&b"42"[..])
        );
        assert_eq!(
            app.kv.get_text("castle.state", "rooms").unwrap().as_deref(),
            Some("[1,2]")
        );
        assert_eq!(
            app.kv.keys("castle.state").unwrap(),
            vec!["cursor".to_string(), "rooms".to_string()]
        );
        app.kv.delete("castle.state", "cursor").unwrap();
        assert_eq!(app.kv.get("castle.state", "cursor").unwrap(), None);
        // An empty value is a kept value, not a missing one.
        app.kv.set("castle.state", "empty", b"").unwrap();
        assert_eq!(
            app.kv.get("castle.state", "empty").unwrap().as_deref(),
            Some(&b""[..])
        );
        // Kept bytes that are not UTF-8 are an error as text, not a value.
        app.kv.set("castle.state", "raw", b"\xff\xfe").unwrap();
        assert!(app.kv.get_text("castle.state", "raw").is_err());
    }

    #[test]
    fn an_ungranted_scope_is_denied_and_no_store_is_ever_asked() {
        // The store panics on any touch, so this test fails if admission is
        // ever moved after dispatch — not only if a denial's value leaks.
        let app = host()
            .with_kv_store(Box::new(UntouchableStore))
            .endow(GrantSet::parse("storage.kv castle.state\n").unwrap());
        for result in [
            app.kv.get("other", "k").map(|_| ()),
            app.kv.set("other", "k", b"x"),
            app.kv.delete("other", "k"),
            app.kv.keys("other").map(|_| ()),
        ] {
            assert_eq!(
                result,
                Err(HostError::Denied {
                    capability: "storage.kv"
                })
            );
        }
        // Ungranted wins over ill-formed: the denial reveals nothing, not
        // even that the scope could never have been granted.
        assert_eq!(
            app.kv.get("../escape", "k"),
            Err(HostError::Denied {
                capability: "storage.kv"
            })
        );
        // A granted scope with a key the family refuses is invalid before
        // the store sees it — the panic store proves the store never does.
        assert!(matches!(
            app.kv.get("castle.state", ""),
            Err(HostError::InvalidArgument(_))
        ));
    }

    #[test]
    fn two_consumers_over_one_host_share_the_store_but_not_the_authority() {
        let shared = host();
        let app = shared.endow(GrantSet::parse("storage.kv castle.state\n").unwrap());
        app.kv.set("castle.state", "cursor", b"42").unwrap();
        // Same host, so the same store: the granted consumer sees the value,
        // the ungranted one is denied the very same scope.
        let other = shared.endow(GrantSet::none());
        assert!(other.kv.scopes().is_empty());
        assert_eq!(
            other.kv.get("castle.state", "cursor"),
            Err(HostError::Denied {
                capability: "storage.kv"
            })
        );
        assert_eq!(
            app.kv.get("castle.state", "cursor").unwrap().as_deref(),
            Some(&b"42"[..])
        );
    }
}
