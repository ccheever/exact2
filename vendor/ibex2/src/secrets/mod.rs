//! Secrets: the platform's credential store behind one trait.
//!
//! A secret is small, per-app, kept across launches, and never shown: a
//! session token is the first one (exact2 LLP 1018). The platform owns the
//! mechanism — the Keychain on Apple platforms, a `0600` file elsewhere —
//! and Rust owns the semantics: a name is kept, replaced, or forgotten, and
//! the grant a binding carries (`secret.keep <name>`, LLP 0069 §1) is checked
//! in `host::Secrets` before any store is asked.
//!
//! Synchronous, as every binding is (LLP 0068 §2): a Keychain read is bounded
//! and local, and the consumer that needs a secret needs it before its first
//! frame.
//!
//! @ref LLP 0069#3-the-backends — the Keychain, a file per secret, memory

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::boundary::HostError;

#[cfg(target_vendor = "apple")]
pub mod darwin;
#[cfg(target_vendor = "apple")]
pub use darwin::KeychainStore;

/// The platform half: keep, read back, forget. Names have already been
/// admitted by the binding; values are UTF-8 strings (a record is JSON, as
/// the web's `localStorage` would hold it).
pub trait SecretStore: Send + Sync {
    /// The kept value, `None` when nothing is kept under `name`.
    fn get(&self, name: &str) -> Result<Option<String>, HostError>;
    /// Keep `value` under `name`, replacing what was there.
    fn set(&self, name: &str, value: &str) -> Result<(), HostError>;
    /// Forget `name`; forgetting what was never kept is not an error.
    fn forget(&self, name: &str) -> Result<(), HostError>;
}

/// Whether `name` is a secret name: `[a-z0-9._-]{1,64}`, and never only
/// dots — `.` and `..` are directory hops, not names. Lowercase, because a
/// name becomes a path component and a case-insensitive filesystem would
/// fold two exactly-granted case variants into one file; bounded, because a
/// filesystem bounds its components. Refused at grant parse and again at
/// the store, so a name can never spell a path.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
        && !name.bytes().all(|b| b == b'.')
}

fn check_name(name: &str) -> Result<(), HostError> {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(HostError::InvalidArgument(format!(
            "`{name}` is not a secret name ([a-z0-9._-]{{1,64}})"
        )))
    }
}

/// A map behind a mutex: tests, and the store a consumer selects when it must
/// leave no trace (a scripted drive against real credentials).
#[derive(Debug, Default)]
pub struct MemoryStore {
    values: Mutex<BTreeMap<String, String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// With these values already kept.
    pub fn with(values: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            values: Mutex::new(values.into_iter().collect()),
        }
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        check_name(name)?;
        Ok(self.values.lock().expect("secrets").get(name).cloned())
    }

    fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        check_name(name)?;
        self.values
            .lock()
            .expect("secrets")
            .insert(name.to_string(), value.to_string());
        Ok(())
    }

    fn forget(&self, name: &str) -> Result<(), HostError> {
        check_name(name)?;
        self.values.lock().expect("secrets").remove(name);
        Ok(())
    }
}

/// A file per secret under one directory: the file `0600`, the directory
/// `0700`, each write whole to a temporary file renamed into place. Not
/// encrypted — the platform's own secret service arrives as a second
/// `SecretStore` with the consumer that needs it (LLP 0069 §3).
#[derive(Debug)]
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    /// Secrets under `dir`, created on the first write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `$XDG_DATA_HOME/<app>/secrets` (`~/.local/share` when unset).
    pub fn for_app(app: &str) -> Self {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))
            .unwrap_or_else(std::env::temp_dir);
        Self::new(base.join(app).join("secrets"))
    }

    fn path(&self, name: &str) -> Result<PathBuf, HostError> {
        check_name(name)?;
        Ok(self.dir.join(name))
    }

    fn ensure_dir(&self) -> Result<(), HostError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| failed("create", &self.dir, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700));
        }
        Ok(())
    }
}

fn failed(what: &str, path: &Path, e: std::io::Error) -> HostError {
    HostError::Failed(format!("secrets: {what} {}: {e}", path.display()))
}

impl SecretStore for FileStore {
    fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        let path = self.path(name)?;
        match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| HostError::Failed(format!("secrets: {name} is not UTF-8"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(failed("read", &path, e)),
        }
    }

    fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        let path = self.path(name)?;
        self.ensure_dir()?;
        // Unique per call — pid and a counter — so concurrent writers of one
        // name can never truncate each other's temporary and rename a torn
        // value into place.
        static NEXT_TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = self.dir.join(format!(
            ".{name}.{}.{}",
            std::process::id(),
            NEXT_TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            use std::io::Write as _;
            let mut file = options.open(&tmp).map_err(|e| failed("write", &tmp, e))?;
            file.write_all(value.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|e| failed("write", &tmp, e))?;
        }
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            failed("rename", &path, e)
        })
    }

    fn forget(&self, name: &str) -> Result<(), HostError> {
        let path = self.path(name)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(failed("remove", &path, e)),
        }
    }
}

/// What the platform knows this process as: the main bundle's identifier on
/// Apple platforms when there is a bundle, else the executable's file name.
/// The Keychain service and the XDG directory are named by it.
pub fn app_identity() -> String {
    #[cfg(target_vendor = "apple")]
    if let Some(id) = darwin::bundle_identifier() {
        return id;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "ibex2".to_string())
}

/// The store this build uses by default: the Keychain on Apple platforms, a
/// file per secret elsewhere, both under [`app_identity`].
pub fn default_store() -> Box<dyn SecretStore> {
    #[cfg(target_vendor = "apple")]
    {
        Box::new(KeychainStore::new(app_identity()))
    }
    #[cfg(not(target_vendor = "apple"))]
    {
        Box::new(FileStore::for_app(&app_identity()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ibex2-secrets-{}-{}",
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
    fn a_memory_store_keeps_and_forgets() {
        let store = MemoryStore::new();
        assert_eq!(store.get("a").unwrap(), None);
        store.set("a", "one").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("one"));
        store.set("a", "two").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some("two"));
        store.forget("a").unwrap();
        store.forget("a").unwrap();
        assert_eq!(store.get("a").unwrap(), None);
    }

    #[test]
    fn a_file_store_round_trips_with_owner_only_permissions() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        assert_eq!(store.get("castle.session").unwrap(), None);
        store
            .set("castle.session", r#"{"token":"t0k","username":"ada"}"#)
            .unwrap();
        assert_eq!(
            store.get("castle.session").unwrap().as_deref(),
            Some(r#"{"token":"t0k","username":"ada"}"#)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join("castle.session"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "the file is the owner's alone");
            let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700, "so is the directory");
        }
        store.set("castle.session", "replaced").unwrap();
        assert_eq!(
            store.get("castle.session").unwrap().as_deref(),
            Some("replaced")
        );
        // Nothing but the secret is left behind: no temporary file.
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["castle.session".to_string()]);
        store.forget("castle.session").unwrap();
        store.forget("castle.session").unwrap();
        assert_eq!(store.get("castle.session").unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_that_could_spell_a_path_is_refused_everywhere() {
        assert!(is_valid_name("castle.session"));
        assert!(is_valid_name("a_b-c.9"));
        let overlong = "a".repeat(65);
        for bad in [
            "",
            ".",
            "..",
            "...",
            "../x",
            "a/b",
            "a b",
            "Upper",
            overlong.as_str(),
            "ünïcode",
            "a\n",
        ] {
            assert!(!is_valid_name(bad), "{bad:?}");
            let dir = temp_dir();
            let store = FileStore::new(&dir);
            assert!(matches!(store.get(bad), Err(HostError::InvalidArgument(_))));
            assert!(matches!(
                store.set(bad, "v"),
                Err(HostError::InvalidArgument(_))
            ));
            assert!(!dir.exists(), "a refused name creates nothing");
            let memory = MemoryStore::new();
            assert!(matches!(
                memory.set(bad, "v"),
                Err(HostError::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn the_app_identity_is_never_empty() {
        assert!(!app_identity().is_empty());
    }
}
