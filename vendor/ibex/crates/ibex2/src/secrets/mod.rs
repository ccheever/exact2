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
use std::ffi::OsString;
use std::io::Read as _;
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

/// The store `Host::new()` uses off Apple when no durable base directory can
/// be derived: every operation refuses, loudly. A temporary directory is not
/// durable storage and may be pre-owned by another local user.
#[derive(Debug)]
pub struct UnavailableStore;

impl UnavailableStore {
    fn refuse<T>(&self) -> Result<T, HostError> {
        Err(HostError::Failed(
            "secrets: no durable base directory (neither HOME nor XDG_DATA_HOME provides \
             an absolute path); construct FileStore::new(dir) with a directory of \
             your own"
                .into(),
        ))
    }
}

impl SecretStore for UnavailableStore {
    fn get(&self, _name: &str) -> Result<Option<String>, HostError> {
        self.refuse()
    }

    fn set(&self, _name: &str, _value: &str) -> Result<(), HostError> {
        self.refuse()
    }

    fn forget(&self, _name: &str) -> Result<(), HostError> {
        self.refuse()
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
    /// `None` when neither variable supplies an absolute durable base.
    pub fn for_app(app: &str) -> Option<Self> {
        durable_base(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
            .map(|base| Self::new(base.join(app).join("secrets")))
    }

    fn path(&self, name: &str) -> Result<PathBuf, HostError> {
        check_name(name)?;
        Ok(self.dir.join(name))
    }

    /// Refuse an existing non-directory or symlink in the store-root slot.
    /// This is deliberately a path check, not a claim of handle-relative
    /// protection against a same-user race in an ancestor directory.
    fn check_dir(&self) -> Result<(), HostError> {
        match std::fs::symlink_metadata(&self.dir) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(failed("stat", &self.dir, e)),
            Ok(meta) if meta.file_type().is_symlink() || is_reparse_point(&meta) => {
                Err(HostError::Failed(format!(
                    "secrets: the store root {} is a link or reparse point, refusing",
                    self.dir.display()
                )))
            }
            Ok(meta) if !meta.file_type().is_dir() => Err(HostError::Failed(format!(
                "secrets: the store root {} is not a directory, refusing",
                self.dir.display()
            ))),
            Ok(_) => Ok(()),
        }
    }

    fn ensure_dir(&self) -> Result<(), HostError> {
        self.check_dir()?;
        // Sync every directory entry created by the first write, through the
        // first ancestor that already existed. Directory sync is best effort
        // because not every filesystem supports it.
        let mut preexisting = self.dir.clone();
        while !preexisting.exists() {
            match preexisting.parent() {
                Some(parent) => preexisting = parent.to_path_buf(),
                None => break,
            }
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| failed("create", &self.dir, e))?;
        self.check_dir()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| failed("secure", &self.dir, e))?;
        }
        if preexisting != self.dir {
            let mut dir = self.dir.clone();
            loop {
                sync_dir(&dir);
                if dir == preexisting {
                    break;
                }
                match dir.parent() {
                    Some(parent) => dir = parent.to_path_buf(),
                    None => break,
                }
            }
        }
        Ok(())
    }
}

fn durable_base(xdg: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    xdg.map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".local/share"))
        })
}

fn failed(what: &str, path: &Path, e: std::io::Error) -> HostError {
    HostError::Failed(format!("secrets: {what} {}: {e}", path.display()))
}

fn non_regular(name: &str) -> HostError {
    HostError::Failed(format!("secrets: {name} is not a regular file, refusing"))
}

#[cfg(windows)]
fn is_reparse_point(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &std::fs::Metadata) -> bool {
    false
}

/// Open a secret without following a final-component symlink, validate the
/// opened handle, and return that same handle to the reader. The store-root
/// check remains path based; this closes the file check/open gap without
/// claiming handle-relative traversal of the parent directory.
fn open_secret(path: &Path, name: &str) -> Result<Option<std::fs::File>, HostError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NONBLOCK keeps a planted FIFO from hanging before the handle's
        // file type can be checked. It does not change regular-file reads.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            if std::fs::symlink_metadata(path)
                .map(|meta| !meta.file_type().is_file())
                .unwrap_or(false)
            {
                return Err(non_regular(name));
            }
            return Err(failed("read", path, e));
        }
    };
    let meta = file.metadata().map_err(|e| failed("stat", path, e))?;
    if !meta.file_type().is_file() || is_reparse_point(&meta) {
        return Err(non_regular(name));
    }
    Ok(Some(file))
}

fn ensure_regular_or_absent(path: &Path, name: &str) -> Result<bool, HostError> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(failed("stat", path, e)),
        Ok(meta) if !meta.file_type().is_file() || is_reparse_point(&meta) => {
            Err(non_regular(name))
        }
        Ok(_) => Ok(true),
    }
}

/// Best-effort fsync of a directory after an entry is created, replaced, or
/// removed. Some filesystems do not permit directory sync.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

impl SecretStore for FileStore {
    fn get(&self, name: &str) -> Result<Option<String>, HostError> {
        let path = self.path(name)?;
        self.check_dir()?;
        let Some(mut file) = open_secret(&path, name)? else {
            return Ok(None);
        };
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| failed("read", &path, e))?;
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| HostError::Failed(format!("secrets: {name} is not UTF-8")))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), HostError> {
        let path = self.path(name)?;
        self.ensure_dir()?;
        ensure_regular_or_absent(&path, name)?;
        // Create a fresh temporary for this call. A colliding name belongs to
        // another writer (or a crash) and is never ours to truncate or remove;
        // retry under a new pid/timestamp/counter spelling instead.
        static NEXT_TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let (mut file, tmp) = {
            let mut attempts = 0u32;
            loop {
                let tmp = self.dir.join(format!(
                    ".{name}.{}.{}.{}",
                    std::process::id(),
                    nanos,
                    NEXT_TMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                match options.open(&tmp) {
                    Ok(file) => break (file, tmp),
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempts < 16 => {
                        attempts += 1;
                    }
                    Err(e) => return Err(failed("write", &tmp, e)),
                }
            }
        };
        let written = {
            use std::io::Write as _;
            file.write_all(value.as_bytes())
                .and_then(|()| file.sync_all())
        };
        drop(file);
        if let Err(e) = written {
            let error = failed("write", &tmp, e);
            let _ = std::fs::remove_file(&tmp);
            sync_dir(&self.dir);
            return Err(error);
        }
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            sync_dir(&self.dir);
            failed("rename", &path, e)
        })?;
        sync_dir(&self.dir);
        Ok(())
    }

    fn forget(&self, name: &str) -> Result<(), HostError> {
        let path = self.path(name)?;
        self.check_dir()?;
        if !ensure_regular_or_absent(&path, name)? {
            return Ok(());
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                sync_dir(&self.dir);
                Ok(())
            }
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
        match FileStore::for_app(&app_identity()) {
            Some(store) => Box::new(store),
            None => Box::new(UnavailableStore),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static NEXT_TEMP_DIR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ibex2-secrets-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

    #[test]
    fn only_absolute_environment_paths_select_a_default_file_store() {
        let home = temp_dir();
        let xdg = home.join("xdg");
        assert_eq!(
            durable_base(
                Some(xdg.clone().into_os_string()),
                Some(home.clone().into_os_string())
            ),
            Some(xdg)
        );
        assert_eq!(
            durable_base(
                Some(OsString::from("relative-xdg")),
                Some(home.clone().into_os_string())
            ),
            Some(home.join(".local/share"))
        );
        assert_eq!(
            durable_base(
                Some(OsString::from("relative-xdg")),
                Some(OsString::from("relative-home"))
            ),
            None
        );
    }

    #[test]
    fn the_unavailable_store_refuses_every_operation() {
        let store = UnavailableStore;
        assert!(store.get("castle.session").is_err());
        assert!(store.set("castle.session", "value").is_err());
        assert!(store.forget("castle.session").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_writers_leave_one_whole_secret_and_no_temporary() {
        let dir = temp_dir();
        let store = std::sync::Arc::new(FileStore::new(&dir));
        let mut handles = Vec::new();
        for writer in 0..8u32 {
            let store = std::sync::Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                for iteration in 0..25u32 {
                    let value = format!("writer-{writer:02}-iteration-{iteration:04}");
                    store.set("contended", &value).unwrap();
                    store.set(&format!("writer-{writer}"), &value).unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let value = store.get("contended").unwrap().unwrap();
        assert!(
            value.starts_with("writer-") && value.ends_with("-iteration-0024"),
            "the last write is one writer's whole final value: {value}"
        );
        for writer in 0..8u32 {
            assert_eq!(
                store.get(&format!("writer-{writer}")).unwrap().unwrap(),
                format!("writer-{writer:02}-iteration-0024")
            );
        }
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    fn assert_non_regular_secret_is_refused(store: &FileStore, name: &str) {
        for result in [
            store.get(name).map(|_| ()),
            store.set(name, "replacement"),
            store.forget(name),
        ] {
            assert!(matches!(result, Err(HostError::Failed(_))));
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_never_read_replaced_or_removed_as_a_secret() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        store.set("anchor", "value").unwrap();

        let outside = dir.with_extension("outside");
        std::fs::write(&outside, b"outside").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("linked")).unwrap();
        assert_non_regular_secret_is_refused(&store, "linked");
        assert_eq!(std::fs::read(&outside).unwrap(), b"outside");

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&outside);
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_is_never_read_replaced_or_removed_as_a_secret() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        store.set("anchor", "value").unwrap();

        std::fs::create_dir(dir.join("directory")).unwrap();
        assert_non_regular_secret_is_refused(&store, "directory");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[cfg_attr(
        target_os = "android",
        ignore = "Android adb shell may deny FIFO fixture creation"
    )]
    #[test]
    fn a_fifo_is_never_read_replaced_or_removed_as_a_secret() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        store.set("anchor", "value").unwrap();

        use std::os::unix::ffi::OsStrExt as _;
        let fifo = dir.join("fifo");
        let fifo_path = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: `fifo_path` is a live, NUL-terminated path for this call.
        let result = unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) };
        assert_eq!(
            result,
            0,
            "FIFO fixture creation failed: {}",
            std::io::Error::last_os_error()
        );
        assert_non_regular_secret_is_refused(&store, "fifo");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_store_root_is_never_used() {
        let real_root = temp_dir();
        std::fs::create_dir_all(&real_root).unwrap();

        let linked_root = temp_dir();
        std::os::unix::fs::symlink(&real_root, &linked_root).unwrap();
        let linked_store = FileStore::new(&linked_root);
        assert!(matches!(
            linked_store.get("secret"),
            Err(HostError::Failed(_))
        ));
        assert!(matches!(
            linked_store.set("secret", "value"),
            Err(HostError::Failed(_))
        ));
        assert!(matches!(
            linked_store.forget("secret"),
            Err(HostError::Failed(_))
        ));

        let _ = std::fs::remove_file(&linked_root);
        let _ = std::fs::remove_dir_all(&real_root);
    }
}
