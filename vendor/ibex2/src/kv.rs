//! KV: durable, unremarkable state behind one trait.
//!
//! What a consumer keeps that is not a secret: a cursor, a cache of settled
//! data, a preference — durable client state (exact2 LLP 1018) that a plan or
//! a log may show. The platform owns where it lives — a directory the
//! platform gives the app — and Rust owns the semantics: bytes under a key,
//! keys under a scope, and the grant a binding carries (`storage.kv <scope>`,
//! LLP 0070 §1) is checked in `host::Kv` before any store is asked.
//!
//! Synchronous, as every binding is (LLP 0068 §2): a read is one local file,
//! and the consumer's executor is its own.
//!
//! @ref LLP 0070#3-the-backends — a file per key, memory

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::boundary::HostError;

/// The platform half: bytes under a key, keys under a scope. Scopes have
/// already been admitted by the binding, and scope and key both revalidated.
pub trait KvStore: Send + Sync {
    /// The kept value, `None` when nothing is kept under `key`.
    fn get(&self, scope: &str, key: &str) -> Result<Option<Vec<u8>>, HostError>;
    /// Keep `value` under `key`, replacing what was there.
    fn set(&self, scope: &str, key: &str, value: &[u8]) -> Result<(), HostError>;
    /// Delete `key`; deleting what was never kept is not an error.
    fn delete(&self, scope: &str, key: &str) -> Result<(), HostError>;
    /// Every key kept under `scope`, in a stable order.
    fn keys(&self, scope: &str) -> Result<Vec<String>, HostError>;
}

/// Whether `scope` is a scope name. The grammar is the secret-name grammar
/// (`[a-z0-9._-]{1,64}`, never only dots) — deliberately one rule for every
/// name that becomes a path component, so it cannot drift on one family.
/// Lowercase matters here more than anywhere: an exact grant on a scope must
/// stay exact on a case-insensitive filesystem, where `state` and `State`
/// would be one directory.
pub fn is_valid_scope(scope: &str) -> bool {
    crate::secrets::is_valid_name(scope)
}

pub(crate) fn check_scope(scope: &str) -> Result<(), HostError> {
    if is_valid_scope(scope) {
        Ok(())
    } else {
        Err(HostError::InvalidArgument(format!(
            "`{scope}` is not a kv scope ([a-z0-9._-]{{1,64}})"
        )))
    }
}

/// Keys are any UTF-8, one to 128 bytes. The bound is the file backend's
/// name budget — a key is stored under its base32 spelling, 205 characters
/// at most, which must fit a 255-byte filename everywhere — and it is a v1
/// rule, not a tunable: a consumer with longer keys hashes them itself and
/// owns the collision story.
pub const MAX_KEY_BYTES: usize = 128;

pub(crate) fn check_key(key: &str) -> Result<(), HostError> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(HostError::InvalidArgument(format!(
            "a kv key is 1..={MAX_KEY_BYTES} bytes, got {}",
            key.len()
        )));
    }
    Ok(())
}

/// A map behind a mutex: tests, and a consumer that must leave no trace.
#[derive(Debug, Default)]
pub struct MemoryStore {
    values: Mutex<BTreeMap<(String, String), Vec<u8>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// With these values already kept, as `(scope, key, value)`. Panics on
    /// a scope or key the family refuses: this constructor takes handed
    /// literals (tests, trace-free consumers), and refusing loudly at
    /// construction beats a store whose `keys()` lists what `get` refuses.
    pub fn with(values: impl IntoIterator<Item = (String, String, Vec<u8>)>) -> Self {
        Self {
            values: Mutex::new(
                values
                    .into_iter()
                    .map(|(scope, key, value)| {
                        check_scope(&scope)
                            .and_then(|()| check_key(&key))
                            .unwrap_or_else(|e| panic!("MemoryStore::with: {e}"));
                        ((scope, key), value)
                    })
                    .collect(),
            ),
        }
    }
}

impl KvStore for MemoryStore {
    fn get(&self, scope: &str, key: &str) -> Result<Option<Vec<u8>>, HostError> {
        check_scope(scope)?;
        check_key(key)?;
        Ok(self
            .values
            .lock()
            .expect("kv")
            .get(&(scope.to_string(), key.to_string()))
            .cloned())
    }

    fn set(&self, scope: &str, key: &str, value: &[u8]) -> Result<(), HostError> {
        check_scope(scope)?;
        check_key(key)?;
        self.values
            .lock()
            .expect("kv")
            .insert((scope.to_string(), key.to_string()), value.to_vec());
        Ok(())
    }

    fn delete(&self, scope: &str, key: &str) -> Result<(), HostError> {
        check_scope(scope)?;
        check_key(key)?;
        self.values
            .lock()
            .expect("kv")
            .remove(&(scope.to_string(), key.to_string()));
        Ok(())
    }

    fn keys(&self, scope: &str) -> Result<Vec<String>, HostError> {
        check_scope(scope)?;
        Ok(self
            .values
            .lock()
            .expect("kv")
            .keys()
            .filter(|(s, _)| s == scope)
            .map(|(_, key)| key.clone())
            .collect())
    }
}

/// The store `Host::new()` uses when no durable base directory can be
/// derived (no `HOME`, no `XDG_DATA_HOME`): every operation refuses, loudly.
/// Landing "durable" state in a temp directory a reboot may clear would be
/// worse than the error.
#[derive(Debug)]
pub struct UnavailableStore;

impl UnavailableStore {
    fn refuse<T>(&self) -> Result<T, HostError> {
        Err(HostError::Failed(
            "kv: no durable base directory (neither HOME nor XDG_DATA_HOME provides \
             an absolute path); construct FileStore::new(dir) with a directory of \
             your own"
                .into(),
        ))
    }
}

impl KvStore for UnavailableStore {
    fn get(&self, _scope: &str, _key: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.refuse()
    }
    fn set(&self, _scope: &str, _key: &str, _value: &[u8]) -> Result<(), HostError> {
        self.refuse()
    }
    fn delete(&self, _scope: &str, _key: &str) -> Result<(), HostError> {
        self.refuse()
    }
    fn keys(&self, _scope: &str) -> Result<Vec<String>, HostError> {
        self.refuse()
    }
}

/// A directory per scope, a file per key — the key spelt in base32 so it can
/// never spell a path — the file `0600`, the directories `0700`, each write
/// whole to a temporary created fresh and renamed into place. The same
/// posture as the secrets `FileStore`, without the claim: this is owner-only,
/// not encrypted, and holds nothing a plan or a log may not show (LLP 0070
/// §4).
#[derive(Debug)]
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    /// State under `dir`, created on the first write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The platform's place for an app's durable state:
    /// `~/Library/Application Support/<app>/kv` on Apple platforms,
    /// `$XDG_DATA_HOME/<app>/kv` (`~/.local/share` when unset) elsewhere.
    /// `None` when no durable base can be derived — deliberately not a temp
    /// directory, which is where durable state goes to disappear.
    pub fn for_app(app: &str) -> Option<Self> {
        #[cfg(target_vendor = "apple")]
        let base = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|h| h.is_absolute())
            .map(|h| h.join("Library/Application Support"))?;
        #[cfg(not(target_vendor = "apple"))]
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|h| h.is_absolute())
                    .map(|h| h.join(".local/share"))
            })?;
        Some(Self::new(base.join(app).join("kv")))
    }

    /// The scope's directory. A symlink where the store root or the scope
    /// directory should be is refused — a kv grant must not be a way to
    /// read or write wherever a planted link points — and so is a scope
    /// directory stored under another spelling, which only a
    /// case-insensitive filesystem can produce (LLP 0070 §3).
    fn scope_dir(&self, scope: &str) -> Result<PathBuf, HostError> {
        check_scope(scope)?;
        if let Ok(meta) = std::fs::symlink_metadata(&self.dir) {
            if meta.file_type().is_symlink() {
                return Err(HostError::Failed(format!(
                    "kv: the store root {} is a symlink, refusing",
                    self.dir.display()
                )));
            }
        }
        let dir = self.dir.join(scope);
        if let Ok(meta) = std::fs::symlink_metadata(&dir) {
            if meta.file_type().is_symlink() {
                return Err(HostError::Failed(format!(
                    "kv: scope {scope} is a symlink, refusing"
                )));
            }
            if !occupies_canonical_name(&dir) {
                return Err(HostError::Failed(format!(
                    "kv: scope {scope} exists under another spelling, refusing"
                )));
            }
        }
        Ok(dir)
    }

    fn key_path(&self, scope: &str, key: &str) -> Result<PathBuf, HostError> {
        let dir = self.scope_dir(scope)?;
        check_key(key)?;
        Ok(dir.join(encode_key(key)))
    }

    fn ensure_dir(&self, scope_dir: &Path) -> Result<(), HostError> {
        // Find the deepest ancestor that already exists before creating
        // anything: first-write durability needs every *created* entry's
        // parent synced, all the way up — syncing only the leaves would let
        // a crash drop the whole new subtree while `set` had reported
        // success.
        let mut preexisting = scope_dir.to_path_buf();
        while !preexisting.exists() {
            match preexisting.parent() {
                Some(parent) => preexisting = parent.to_path_buf(),
                None => break,
            }
        }
        std::fs::create_dir_all(scope_dir).map_err(|e| failed("create", scope_dir, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for dir in [self.dir.as_path(), scope_dir] {
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }
        // Make the new directory entries themselves survive a crash, not
        // just the files later renamed into them: sync from the scope up to
        // and including the first directory that already existed (its entry
        // list changed). Best effort, like the permission bits: not every
        // filesystem lets a directory sync.
        if preexisting != *scope_dir {
            let mut dir = scope_dir.to_path_buf();
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

fn failed(what: &str, path: &Path, e: std::io::Error) -> HostError {
    HostError::Failed(format!("kv: {what} {}: {e}", path.display()))
}

/// Does the entry at `path` carry exactly `path`'s final component as its
/// stored spelling? On a case-insensitive filesystem a lookup resolves any
/// case variant of a name, so opening the canonical path can reach a file
/// `keys()` would refuse; the stored name is the one that decides
/// (LLP 0070 §3). An entry that vanished under us counts as canonical — the
/// caller's next step sees `NotFound` and answers for itself.
fn occupies_canonical_name(path: &Path) -> bool {
    match std::fs::canonicalize(path) {
        Ok(real) => real.file_name() == path.file_name(),
        Err(_) => true,
    }
}

/// Best-effort fsync of a directory, so a rename or unlink inside it is on
/// disk, not just in the page cache. Unix only; elsewhere the rename's
/// atomicity still holds and durability is the platform's.
#[cfg(unix)]
fn sync_dir(dir: &Path) {
    if let Ok(handle) = std::fs::File::open(dir) {
        let _ = handle.sync_all();
    }
}
#[cfg(not(unix))]
fn sync_dir(_dir: &Path) {}

/// Lowercase base32 (RFC 4648 alphabet, no padding): the one spelling of a
/// key that is a safe filename on every filesystem *including
/// case-insensitive ones* — a mixed-case encoding would fold two keys into
/// one file there — and decodes back to exactly the key for `keys()`.
const BASE32: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

fn encode_key(key: &str) -> String {
    let bytes = key.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32[(buffer >> bits) as usize & 0x1f] as char);
        }
    }
    if bits > 0 {
        out.push(BASE32[(buffer << (5 - bits)) as usize & 0x1f] as char);
    }
    out
}

fn decode_key(name: &str) -> Option<String> {
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    let mut bytes = Vec::with_capacity(name.len() * 5 / 8 + 1);
    for c in name.bytes() {
        let value = BASE32.iter().position(|&s| s == c)? as u32;
        buffer = (buffer << 5) | value;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    let key = String::from_utf8(bytes).ok()?;
    // Only the canonical spelling of an acceptable key is a key: a foreign
    // file whose name decodes but re-encodes differently — trailing bits
    // set, an impossible length — must not shadow or duplicate one, and
    // `keys()` must never list what `get` would refuse.
    (check_key(&key).is_ok() && encode_key(&key) == name).then_some(key)
}

impl KvStore for FileStore {
    fn get(&self, scope: &str, key: &str) -> Result<Option<Vec<u8>>, HostError> {
        let path = self.key_path(scope, key)?;
        // A key is a regular file this store wrote. A symlink here is a
        // plant, and following it would turn a kv grant into a read of
        // wherever it points.
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(failed("stat", &path, e)),
            Ok(meta) if !meta.file_type().is_file() => {
                return Err(HostError::Failed(format!(
                    "kv: {scope}/{key} is not a regular file, refusing"
                )));
            }
            Ok(_) => {}
        }
        // A case-variant occupant (case-insensitive filesystems only) is
        // not this key — `keys()` refuses its spelling, so `get` must not
        // read it.
        if !occupies_canonical_name(&path) {
            return Ok(None);
        }
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(failed("read", &path, e)),
        }
    }

    fn set(&self, scope: &str, key: &str, value: &[u8]) -> Result<(), HostError> {
        let path = self.key_path(scope, key)?;
        let scope_dir = self.dir.join(scope);
        self.ensure_dir(&scope_dir)?;
        // The temporary is created fresh (`create_new`: O_EXCL, which also
        // refuses to follow a planted symlink) under a name unique to this
        // call — pid, a timestamp, a counter — so concurrent writers, in
        // this process or another, can never truncate each other's file and
        // rename a torn value into place. A name somehow taken (a
        // PID-namespace twin, a crash leftover) is another writer's file and
        // never ours to delete: take the next name instead, and clean up
        // only the file this call created. Encoded names never start with
        // `.` (the alphabet has no dot), so a dotted temporary can never
        // collide with a key and `keys()` skips it.
        static NEXT_TMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let (mut file, tmp) = {
            let mut attempts = 0u32;
            loop {
                let tmp = scope_dir.join(format!(
                    ".tmp.{}.{}.{}",
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
            file.write_all(value).and_then(|()| file.sync_all())
        };
        drop(file);
        if let Err(e) = written {
            let error = failed("write", &tmp, e);
            let _ = std::fs::remove_file(&tmp);
            return Err(error);
        }
        // A case-variant occupant of this key's slot (case-insensitive
        // filesystems only) is evicted under its stored spelling first, so
        // the rename lands the canonical name rather than feeding the
        // occupant's — otherwise this very write could come back invisible
        // to `get`.
        if std::fs::symlink_metadata(&path).is_ok() && !occupies_canonical_name(&path) {
            if let Ok(real) = std::fs::canonicalize(&path) {
                let _ = std::fs::remove_file(&real);
            }
        }
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            failed("rename", &path, e)
        })?;
        sync_dir(&scope_dir);
        Ok(())
    }

    fn delete(&self, scope: &str, key: &str) -> Result<(), HostError> {
        let path = self.key_path(scope, key)?;
        // What occupies this slot under another spelling or as a non-file is
        // not this key (`keys()` refuses it): deleting the absent is not an
        // error, and a foreign occupant is not ours to remove.
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(failed("stat", &path, e)),
            Ok(meta) if !meta.file_type().is_file() => return Ok(()),
            Ok(_) if !occupies_canonical_name(&path) => return Ok(()),
            Ok(_) => {}
        }
        match std::fs::remove_file(&path) {
            Ok(()) => {
                sync_dir(&self.dir.join(scope));
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(failed("remove", &path, e)),
        }
    }

    fn keys(&self, scope: &str) -> Result<Vec<String>, HostError> {
        let dir = self.scope_dir(scope)?;
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(failed("list", &dir, e)),
        };
        let mut keys = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| failed("list", &dir, e))?;
            // Only a regular file under a canonical spelling is a key: not a
            // temporary, not a symlink or directory, not a foreign file whose
            // name is no encoding. (A regular foreign file *under* a
            // canonical name is indistinguishable by construction — the
            // directory is the store.)
            let file_type = entry.file_type().map_err(|e| failed("list", &dir, e))?;
            if !file_type.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if let Some(key) = decode_key(&name) {
                keys.push(key);
            }
        }
        keys.sort();
        Ok(keys)
    }
}

/// The store this build uses by default: a file per key under the platform's
/// app-state directory, named by [`crate::secrets::app_identity`] — or, when
/// no durable base exists, a store that refuses every operation rather than
/// one that quietly lands in a temp directory.
pub fn default_store() -> Box<dyn KvStore> {
    match FileStore::for_app(&crate::secrets::app_identity()) {
        Some(store) => Box::new(store),
        None => Box::new(UnavailableStore),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ibex2-kv-{}-{}",
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
    fn keys_round_trip_through_their_filename_spelling() {
        for key in [
            "a",
            "cursor",
            "castle.rooms?page=2",
            "args:{\"id\":7,\"q\":\"ünïcode ☃\"}",
            "/looks/like/a/path/../..",
            &"k".repeat(MAX_KEY_BYTES),
        ] {
            let encoded = encode_key(key);
            assert!(
                encoded.bytes().all(|b| BASE32.contains(&b)),
                "{encoded} is a safe, single-case filename"
            );
            assert!(encoded.len() <= 205, "fits a filename budget");
            assert_eq!(decode_key(&encoded).as_deref(), Some(key));
        }
        // Fixed vectors, so the encoding cannot drift silently: "a" is
        // 01100001, whose 5-bit groups are 01100 001(00) — "me"; the rest
        // are RFC 4648 §10's own vectors, lowercased and unpadded, decoded
        // independently of the encoder so a shared multi-byte defect cannot
        // hide behind a round trip.
        assert_eq!(encode_key("a"), "me");
        assert_eq!(decode_key("me").as_deref(), Some("a"));
        for (key, spelling) in [
            ("f", "my"),
            ("fo", "mzxq"),
            ("foo", "mzxw6"),
            ("foob", "mzxw6yq"),
            ("fooba", "mzxw6ytb"),
            ("foobar", "mzxw6ytboi"),
        ] {
            assert_eq!(encode_key(key), spelling);
            assert_eq!(decode_key(spelling).as_deref(), Some(key));
        }
        // Non-canonical spellings — trailing bits set — are not keys:
        // "mf" also decodes to the byte of "a", and must be refused.
        assert_eq!(decode_key("mf"), None);
        // Case variants are not keys either; on a case-insensitive
        // filesystem they would be the same file as the canonical name.
        assert_eq!(decode_key("ME"), None);
        assert_eq!(decode_key("not!base32"), None);
        assert_eq!(decode_key("m"), None); // an impossible length
        assert_eq!(decode_key(""), None); // the empty key is no key
    }

    #[test]
    fn a_memory_store_keeps_scopes_apart() {
        let store = MemoryStore::new();
        store.set("a", "k", b"one").unwrap();
        store.set("b", "k", b"two").unwrap();
        assert_eq!(store.get("a", "k").unwrap().as_deref(), Some(&b"one"[..]));
        assert_eq!(store.get("b", "k").unwrap().as_deref(), Some(&b"two"[..]));
        store.delete("a", "k").unwrap();
        store.delete("a", "k").unwrap();
        assert_eq!(store.get("a", "k").unwrap(), None);
        assert_eq!(store.keys("b").unwrap(), vec!["k"]);
        assert!(store.keys("a").unwrap().is_empty());
    }

    #[test]
    fn a_file_store_round_trips_with_owner_only_permissions() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        assert_eq!(store.get("state", "cursor").unwrap(), None);
        store.set("state", "cursor", b"41").unwrap();
        store.set("state", "cursor", b"42").unwrap();
        store
            .set("state", "args:{\"id\":7}", b"\x00\x01\xff")
            .unwrap();
        assert_eq!(
            store.get("state", "cursor").unwrap().as_deref(),
            Some(&b"42"[..])
        );
        assert_eq!(
            store.get("state", "args:{\"id\":7}").unwrap().as_deref(),
            Some(&b"\x00\x01\xff"[..])
        );
        assert_eq!(
            store.keys("state").unwrap(),
            vec!["args:{\"id\":7}".to_string(), "cursor".to_string()]
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = dir.join("state").join(encode_key("cursor"));
            assert_eq!(
                std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
            for d in [&dir, &dir.join("state")] {
                assert_eq!(
                    std::fs::metadata(d).unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
        }
        // Nothing but the keys is left behind: no temporary file.
        assert_eq!(std::fs::read_dir(dir.join("state")).unwrap().count(), 2);
        store.delete("state", "cursor").unwrap();
        store.delete("state", "cursor").unwrap();
        assert_eq!(store.get("state", "cursor").unwrap(), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_scope_or_key_that_breaks_the_rules_is_refused_and_creates_nothing() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        let overlong_scope = "a".repeat(65);
        for bad_scope in [
            "",
            ".",
            "..",
            "...",
            "a/b",
            "a b",
            "State",
            overlong_scope.as_str(),
            "ünïcode",
        ] {
            assert!(!is_valid_scope(bad_scope), "{bad_scope:?}");
            assert!(matches!(
                store.get(bad_scope, "k"),
                Err(HostError::InvalidArgument(_))
            ));
            assert!(matches!(
                store.set(bad_scope, "k", b"v"),
                Err(HostError::InvalidArgument(_))
            ));
        }
        for bad_key in ["", &"k".repeat(MAX_KEY_BYTES + 1)] {
            assert!(matches!(
                store.set("state", bad_key, b"v"),
                Err(HostError::InvalidArgument(_))
            ));
            assert!(matches!(
                store.get("state", bad_key),
                Err(HostError::InvalidArgument(_))
            ));
        }
        assert!(!dir.exists(), "a refused name creates nothing");
        let memory = MemoryStore::new();
        assert!(matches!(
            memory.set("..", "k", b"v"),
            Err(HostError::InvalidArgument(_))
        ));
    }

    #[test]
    fn listing_an_absent_scope_is_empty_and_foreign_files_are_not_keys() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        assert!(store.keys("state").unwrap().is_empty());
        store.set("state", "real", b"v").unwrap();
        std::fs::write(dir.join("state").join(".DS_Store"), b"junk").unwrap();
        std::fs::write(dir.join("state").join("not!akey"), b"junk").unwrap();
        // A directory under a canonical spelling is not a key either.
        std::fs::create_dir(dir.join("state").join(encode_key("fake-dir"))).unwrap();
        assert_eq!(store.keys("state").unwrap(), vec!["real"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_never_followed_as_a_scope_or_a_key() {
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        store.set("state", "real", b"v").unwrap();

        // A planted symlink under a canonical key name: not listed, and a
        // read refuses rather than following it out of the store.
        let outside = dir.join("outside.txt");
        std::fs::write(&outside, b"the file a kv grant must not reach").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("state").join(encode_key("stolen"))).unwrap();
        assert_eq!(store.keys("state").unwrap(), vec!["real"]);
        assert!(matches!(
            store.get("state", "stolen"),
            Err(HostError::Failed(_))
        ));

        // A symlink where a scope directory should be: every operation on
        // that scope refuses.
        let elsewhere = temp_dir();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, dir.join("linked")).unwrap();
        assert!(store.get("linked", "k").is_err());
        assert!(store.set("linked", "k", b"v").is_err());
        assert!(store.keys("linked").is_err());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }

    #[test]
    fn concurrent_writers_never_leave_a_torn_value() {
        let dir = temp_dir();
        let store = std::sync::Arc::new(FileStore::new(&dir));
        let mut handles = Vec::new();
        for writer in 0..8u32 {
            let store = std::sync::Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                for i in 0..25u32 {
                    // Every value spells out its writer and fills a fixed
                    // width, so a torn or interleaved file cannot decode.
                    let value = format!("writer-{writer:02}-iteration-{i:04}");
                    store.set("state", "contended", value.as_bytes()).unwrap();
                    store
                        .set("state", &format!("own-{writer}"), value.as_bytes())
                        .unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }
        let last = store.get("state", "contended").unwrap().unwrap();
        let text = String::from_utf8(last).expect("a whole value, not a torn one");
        assert!(
            text.starts_with("writer-") && text.ends_with("-iteration-0024"),
            "the last write is one writer's whole final value: {text}"
        );
        for writer in 0..8u32 {
            let own = store
                .get("state", &format!("own-{writer}"))
                .unwrap()
                .unwrap();
            assert_eq!(
                String::from_utf8(own).unwrap(),
                format!("writer-{writer:02}-iteration-0024")
            );
        }
        // No temporary survived the storm.
        let leftovers: Vec<_> = std::fs::read_dir(dir.join("state"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_case_variant_occupant_is_never_this_key_and_set_reclaims_the_slot() {
        // "a" canonically encodes as "me". Plant "ME": on a case-insensitive
        // filesystem the canonical path resolves it; on a case-sensitive one
        // it is simply another file. The observable behavior must be the
        // same on both: not a key, not read, not deleted — and a `set`
        // reclaims the slot under the canonical spelling.
        let dir = temp_dir();
        let store = FileStore::new(&dir);
        store.set("state", "anchor", b"x").unwrap(); // creates the scope dir
        std::fs::write(dir.join("state").join("ME"), b"foreign").unwrap();
        assert_eq!(store.keys("state").unwrap(), vec!["anchor"]);
        assert_eq!(store.get("state", "a").unwrap(), None);
        store.delete("state", "a").unwrap();
        store.set("state", "a", b"mine").unwrap();
        assert_eq!(
            store.get("state", "a").unwrap().as_deref(),
            Some(&b"mine"[..])
        );
        assert_eq!(
            store.keys("state").unwrap(),
            vec!["a".to_string(), "anchor".to_string()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_unavailable_store_refuses_every_operation() {
        let store = UnavailableStore;
        assert!(store.get("state", "k").is_err());
        assert!(store.set("state", "k", b"v").is_err());
        assert!(store.delete("state", "k").is_err());
        assert!(store.keys("state").is_err());
    }
}
