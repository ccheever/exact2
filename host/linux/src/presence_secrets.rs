//! The presence backend's vault on this host and Android's reader (LLP
//! 1133.010 D4, D6, §7). The moves are the runner's ([`Backend`]); the vault
//! keeps a name's plain copy where `secret.keep` keeps any (`<root>/secrets`),
//! its record in the kv store (`exact.presence`), and its locked items:
//!
//! - under the agent, in the drive's kv store (`exact.presence.agent`), or in
//!   memory for a drive that names no scratch store: never a Keystore key,
//!   and the check is a ticket;
//! - on Android, as the reader's Keystore keeps them (`Presence.kt`): a file
//!   per name and generation in a directory of its own, outside the plain
//!   secrets' and out of backups (`$HOME/exact-presence/<app id>`), sealed
//!   under a Keystore key that needs the person for every use. The reader
//!   adds an item through a hook ([`on_keystore`]); listing and deleting are
//!   file operations; the read is the reader's own, after its check;
//! - elsewhere (Linux, Windows) nowhere: a mandatory name is refused and an
//!   optional one stays plain (D15).

use exact_runner::presence::{Backend, MemoryVault, Presence, Vault};
use ibex2::kv::KvStore;
use ibex2::secrets::SecretStore;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The kv scope of the backend's records (D4 step 1).
const RECORDS: &str = "exact.presence";
/// The kv scope of the agent's locked items, `<name>#<generation>`.
const LOCKED: &str = "exact.presence.agent";

/// The reader's Keystore (`dev.exact.canvas.Presence.keystore`): op 1 adds a
/// locked item (`{"dir", "account", "policy"}` and the value), op 5 says
/// whether one can be kept now (`"1"`); `Err` carries the refusal.
pub type KeystoreFn = fn(op: u32, request: &str, value: &str) -> Result<String, String>;

static KEYSTORE: OnceLock<KeystoreFn> = OnceLock::new();

/// Install the reader's Keystore hook, once a process (the JNI attach).
pub fn on_keystore(keystore: KeystoreFn) {
    let _ = KEYSTORE.set(keystore);
}

/// Where locked items are.
enum Locked {
    /// The drive's kv store.
    Agent,
    /// The reader's Keystore, its files in this directory.
    Keystore(PathBuf),
    /// Nowhere: this host keeps no locked secret.
    None,
}

struct HostVault {
    secrets: ibex2::secrets::FileStore,
    kv: ibex2::kv::FileStore,
    locked: Locked,
}

fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

impl Vault for HostVault {
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.secrets.get(name).map_err(err)?.map(String::into_bytes))
    }
    fn put_plain(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), String> {
        match value {
            Some(v) => self.secrets.set(name, &String::from_utf8_lossy(v)),
            None => self.secrets.forget(name),
        }
        .map_err(err)
    }
    fn record(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.kv.get(RECORDS, name).map_err(err)
    }
    fn put_record(&mut self, name: &str, record: Option<&[u8]>) -> Result<(), String> {
        match record {
            Some(r) => self.kv.set(RECORDS, name, r),
            None => self.kv.delete(RECORDS, name),
        }
        .map_err(err)
    }
    fn generations(&self, name: &str) -> Result<Vec<u64>, String> {
        let accounts = match &self.locked {
            Locked::Agent => self.kv.keys(LOCKED).map_err(err)?,
            Locked::Keystore(dir) => match std::fs::read_dir(dir) {
                Ok(entries) => (entries.flatten())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
                Err(e) => return Err(err(e)),
            },
            Locked::None => Vec::new(),
        };
        Ok(accounts
            .iter()
            .filter_map(|a| a.strip_prefix(name)?.strip_prefix('#')?.parse().ok())
            .collect())
    }
    fn add_locked(
        &mut self,
        name: &str,
        generation: u64,
        presence: &Presence,
        value: &[u8],
    ) -> Result<(), String> {
        let account = format!("{name}#{generation}");
        match &self.locked {
            Locked::Agent => self.kv.set(LOCKED, &account, value).map_err(err),
            Locked::Keystore(dir) => {
                let keystore = KEYSTORE.get().ok_or("the reader installed no Keystore")?;
                let request = serde_json::json!({ "dir": dir, "account": account,
                    "policy": presence.line_words() });
                keystore(1, &request.to_string(), &String::from_utf8_lossy(value)).map(|_| ())
            }
            Locked::None => Err("this host keeps no locked secret".into()),
        }
    }
    fn delete_locked(&mut self, name: &str, generation: u64) -> Result<(), String> {
        let account = format!("{name}#{generation}");
        match &self.locked {
            Locked::Agent => self.kv.delete(LOCKED, &account).map_err(err),
            Locked::Keystore(dir) => match std::fs::remove_file(dir.join(&account)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(err(e)),
                _ => Ok(()),
            },
            Locked::None => Ok(()),
        }
    }
    fn read_locked(&mut self, name: &str, generation: u64) -> Result<Option<Vec<u8>>, String> {
        match &self.locked {
            Locked::Agent => self
                .kv
                .get(LOCKED, &format!("{name}#{generation}"))
                .map_err(err),
            // The reader reads after its check (`Checked::Read`).
            _ => Err("a locked item is read by the reader after its check".into()),
        }
    }
}

/// The app's `secret.keep` names, each with its presence policy.
fn lines(grants: &str) -> Vec<(String, Option<Presence>)> {
    let Ok(set) = exact_runner::grants::parse(grants) else {
        return Vec::new();
    };
    let presence: Vec<_> = set.presence().collect();
    (set.secrets())
        .map(|name| {
            let policy = presence
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, p)| p.clone());
            (name.to_owned(), policy)
        })
        .collect()
}

/// What a boot installs: the backend, reconciled; where the host reads a
/// locked item after its check (the reader's directory, none under the agent
/// or where nothing is kept); and whether a locked item can be kept now.
pub struct Installed {
    /// The backend, for `Runner::install_presence`.
    pub backend: Backend,
    /// The directory the reader reads items from.
    pub keystore: Option<String>,
    /// The boot snapshot's occupancy entries, the capability among them.
    pub snapshot: Vec<(String, Vec<u8>)>,
}

/// The backend for an app with a presence line, else `None`. A drive with no
/// scratch store keeps its locked items in this process's memory.
pub fn backend(app_id: &str, grants: &str) -> Option<Installed> {
    let lines = lines(grants);
    if !lines.iter().any(|(_, p)| p.is_some()) {
        return None;
    }
    // A drive that asks for the platform's stores (`EXACT_STORE=real`) keeps
    // locked items in the Keystore, behind the prompt it binds: the
    // conformance suite's mode, as on Apple.
    let real = std::env::var("EXACT_STORE").as_deref() == Ok("real");
    let agent = std::env::var("EXACT_AGENT").as_deref() == Ok("1") && !real;
    let (backend, keystore, capable) = match crate::picker::secret_root(app_id) {
        Ok(Some(root)) => {
            let reader = !agent && KEYSTORE.get().is_some();
            let dir = std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join("exact-presence").join(app_id));
            let (locked, capable) = match (agent, reader, dir) {
                (true, ..) => (Locked::Agent, true),
                (false, true, Some(dir)) => {
                    let capable = KEYSTORE
                        .get()
                        .is_some_and(|k| k(5, "{}", "").is_ok_and(|s| s == "1"));
                    (Locked::Keystore(dir), capable)
                }
                _ => (Locked::None, false),
            };
            let keystore = match &locked {
                Locked::Keystore(dir) => Some(dir.to_string_lossy().into_owned()),
                _ => None,
            };
            let vault = HostVault {
                secrets: ibex2::secrets::FileStore::new(root.join("secrets")),
                kv: ibex2::kv::FileStore::new(root.join("kv")),
                locked,
            };
            (Backend::new(Box::new(vault), lines), keystore, capable)
        }
        _ if agent => {
            static MEMORY: OnceLock<MemoryVault> = OnceLock::new();
            let vault = MEMORY.get_or_init(MemoryVault::default).clone();
            (Backend::new(Box::new(vault), lines), None, true)
        }
        _ => return None,
    };
    let snapshot = backend.snapshot_entries(capable);
    Some(Installed {
        backend,
        keystore,
        snapshot,
    })
}

#[cfg(test)]
#[path = "presence_secrets_tests.rs"]
mod tests;
