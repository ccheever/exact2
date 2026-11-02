//! Shared-secret leases and fences for native persistence queues.

use exact_runner::{SecretLease, SecretWait, SharedSecrets};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

/// The host's actual credential backend. Writes acknowledge durable storage.
pub trait Backend: Send + Sync {
    fn read(&self, name: &str) -> Result<Option<String>, String>;
    fn write(&self, name: &str, value: Option<&str>) -> Result<(), String>;
}

#[derive(Clone)]
pub struct Namespace(Arc<Scope>);
enum Scope {
    Disk(PathBuf),
    Memory(Mutex<BTreeMap<String, Arc<Mutex<Memory>>>>),
}
#[derive(Default)]
struct Memory {
    held: bool,
    marker: Marker,
}

/// Captured when an ordinary write enters a host's queue, never at retry time.
#[derive(Clone)]
pub struct Stamp(Result<Option<(u64, bool)>, String>);
impl Stamp {
    /// A queue whose backend could not be resolved. It may recover only in a
    /// namespace which has never granted an exclusive transaction.
    pub fn unresolved() -> Self {
        Self(Err("secret backend was unresolved".into()))
    }
}

impl Namespace {
    /// A dedicated directory beside the backend, shared by every process that
    /// accesses that backend. Lock files must never be renamed or removed.
    pub fn disk(directory: PathBuf) -> Self {
        Self(Arc::new(Scope::Disk(directory)))
    }

    pub fn named(directory: PathBuf, identity: &str) -> Self {
        Self::disk(directory.join(filename(identity)))
    }

    /// Whether this namespace belongs to an ephemeral backend.
    pub fn is_memory(&self) -> bool {
        matches!(self.0.as_ref(), Scope::Memory(_))
    }

    /// A private ephemeral backend; clones share the same namespace.
    pub fn memory() -> Self {
        Self(Arc::new(Scope::Memory(Mutex::new(BTreeMap::new()))))
    }

    pub fn stamp(&self, name: &str) -> Stamp {
        Stamp((|| {
            let marker = match self.0.as_ref() {
                Scope::Memory(entries) => entries
                    .lock()
                    .unwrap()
                    .get(name)
                    .map(|entry| entry.lock().unwrap().marker)
                    .unwrap_or_default(),
                Scope::Disk(directory) => match File::open(directory.join(filename(name))) {
                    Ok(mut file) => Marker::read(&mut file)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Marker::default(),
                    Err(error) => return Err(error.to_string()),
                },
            };
            if marker.poison {
                return Ok(None);
            }
            Ok(Some((marker.epoch, marker.active)))
        })())
    }

    /// Discard superseded writes permanently. A current write that fails stays
    /// eligible for its host's ordinary retry policy.
    pub fn write(
        &self,
        name: &str,
        stamp: &Stamp,
        write: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let (epoch, was_active) = match &stamp.0 {
            Ok(Some(epoch)) => *epoch,
            Ok(None) => return Ok(()),
            Err(_) => {
                let Some(guard) = self.lock(name)? else {
                    return Err("secret has an exclusive transaction".into());
                };
                if guard.marker.epoch != 0 || guard.marker.poison {
                    return Ok(());
                }
                return write();
            }
        };
        let Some(mut guard) = self.lock(name)? else {
            return Err("secret has an exclusive transaction".into());
        };
        if guard.marker.epoch != epoch
            || guard.marker.poison
            || (was_active && !guard.marker.active)
        {
            return Ok(());
        }
        if guard.marker.active {
            // The OS lock is free: recover a dead, unarmed holder on the
            // persistence worker, never on the thread capturing a stamp.
            guard.marker.active = false;
            guard.save()?;
        }
        write()
    }

    fn lock(&self, name: &str) -> Result<Option<Guard>, String> {
        match self.0.as_ref() {
            Scope::Memory(entries) => {
                let entry = entries
                    .lock()
                    .unwrap()
                    .entry(name.into())
                    .or_default()
                    .clone();
                let mut memory = entry.lock().unwrap();
                if memory.held {
                    return Ok(None);
                }
                memory.held = true;
                let marker = memory.marker;
                drop(memory);
                Ok(Some(Guard {
                    storage: Storage::Memory(entry),
                    marker,
                    exclusive: false,
                }))
            }
            Scope::Disk(directory) => {
                ensure_directory(directory)?;
                let filename = filename(name);
                let mut options = OpenOptions::new();
                options.read(true).write(true).create(true).truncate(false);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
                }
                let path = directory.join(filename);
                let existed = path.exists();
                let mut file = options.open(path).map_err(|e| e.to_string())?;
                match file.try_lock() {
                    Ok(()) => {}
                    Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
                    Err(error) => return Err(error.to_string()),
                }
                #[cfg(unix)]
                if !existed {
                    File::open(directory)
                        .and_then(|f| f.sync_all())
                        .map_err(|e| e.to_string())?;
                }
                let marker = Marker::read(&mut file)?;
                Ok(Some(Guard {
                    storage: Storage::Disk(file),
                    marker,
                    exclusive: false,
                }))
            }
        }
    }
}

fn filename(name: &str) -> String {
    Sha256::digest(name.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn ensure_directory(directory: &std::path::Path) -> Result<(), String> {
    match std::fs::symlink_metadata(directory) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => return Ok(()),
        Ok(_) => return Err("secret lease directory is not a directory".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    let mut absent = Vec::new();
    let mut path = directory;
    while !path.exists() {
        absent.push(path.to_owned());
        path = path
            .parent()
            .ok_or("secret lease directory has no parent")?;
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    if std::fs::symlink_metadata(directory)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("secret lease directory is a symbolic link".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        for path in absent
            .iter()
            .rev()
            .chain(std::iter::once(&directory.to_owned()))
        {
            File::open(path)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            if let Some(parent) = path.parent() {
                File::open(parent)
                    .and_then(|f| f.sync_all())
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Acknowledge file-backed credential renames/removals and their directory
/// ancestry. A backend's best-effort directory sync is insufficient for leases.
pub fn sync_directory_chain(directory: &std::path::Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let mut path = Some(directory);
        while let Some(directory) = path {
            match File::open(directory) {
                Ok(file) => file.sync_all().map_err(|error| error.to_string())?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
            path = directory.parent();
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Err("durable file-backed secret leases are unavailable on this platform".into())
    }
}

#[derive(Clone, Copy, Default)]
struct Marker {
    revision: u64,
    epoch: u64,
    poison: bool,
    active: bool,
}
impl Marker {
    // Two checksummed slots preserve the last durable record across torn writes.
    fn read(file: &mut File) -> Result<Self, String> {
        let mut bytes = [0u8; 64];
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        let mut len = 0;
        while len < bytes.len() {
            let count = file.read(&mut bytes[len..]).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            len += count;
        }
        if len == 0 {
            return Ok(Self::default());
        }
        bytes[..len]
            .chunks_exact(32)
            .filter_map(|slot| {
                if Sha256::digest(&slot[..17])[..15] != slot[17..] || slot[16] > 3 {
                    return None;
                }
                Some(Self {
                    revision: u64::from_le_bytes(slot[..8].try_into().unwrap()),
                    epoch: u64::from_le_bytes(slot[8..16].try_into().unwrap()),
                    poison: slot[16] & 1 != 0,
                    active: slot[16] & 2 != 0,
                })
            })
            .max_by_key(|m| m.revision)
            .ok_or_else(|| "secret lease marker is corrupt".into())
    }
    fn bytes(self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&self.revision.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.epoch.to_le_bytes());
        bytes[16] = u8::from(self.poison) | (u8::from(self.active) << 1);
        let hash = Sha256::digest(&bytes[..17]);
        bytes[17..].copy_from_slice(&hash[..15]);
        bytes
    }
}

enum Storage {
    Disk(File),
    Memory(Arc<Mutex<Memory>>),
}
struct Guard {
    exclusive: bool,
    storage: Storage,
    marker: Marker,
}
impl Guard {
    fn save(&mut self) -> Result<(), String> {
        self.marker.revision = self
            .marker
            .revision
            .checked_add(1)
            .ok_or("secret lease revision overflow")?;
        match &mut self.storage {
            Storage::Memory(memory) => memory.lock().unwrap().marker = self.marker,
            Storage::Disk(file) => {
                file.seek(SeekFrom::Start((self.marker.revision % 2) * 32))
                    .map_err(|e| e.to_string())?;
                file.write_all(&self.marker.bytes())
                    .and_then(|()| file.sync_all())
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.exclusive {
            self.marker.active = false;
            let _ = self.save();
        }
        if let Storage::Memory(memory) = &self.storage {
            memory.lock().unwrap().held = false;
        }
    }
}

pub struct Coordinator {
    namespace: Namespace,
    backend: Arc<dyn Backend>,
}
impl Coordinator {
    pub fn new(namespace: Namespace, backend: Arc<dyn Backend>) -> Self {
        Self { namespace, backend }
    }
}

struct Lease {
    backend: Arc<dyn Backend>,
    name: String,
    state: Mutex<LeaseState>,
    _permit: Permit,
}
struct LeaseState {
    guard: Guard,
    finished: bool,
    failed: bool,
}
impl Lease {
    fn usable(state: &LeaseState) -> Result<(), String> {
        if state.finished {
            Err("secret transaction already finished".into())
        } else if state.failed {
            Err("secret transaction failed".into())
        } else {
            Ok(())
        }
    }
    fn arm(state: &mut LeaseState) -> Result<(), String> {
        Self::usable(state)?;
        if !state.guard.marker.poison {
            state.guard.marker.poison = true;
            if let Err(error) = state.guard.save() {
                state.failed = true;
                return Err(error);
            }
        }
        Ok(())
    }
}
impl SecretLease for Lease {
    fn read(&self) -> Result<Option<String>, String> {
        let state = self.state.lock().unwrap();
        Self::usable(&state)?;
        self.backend.read(&self.name)
    }
    fn begin_effect(&self) -> Result<(), String> {
        Self::arm(&mut self.state.lock().unwrap())
    }
    fn write(&self, value: Option<&str>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        Self::arm(&mut state)?;
        if let Err(error) = self.backend.write(&self.name, value) {
            state.failed = true;
            return Err(error);
        }
        Ok(())
    }
    fn finish(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        Self::usable(&state)?;
        if state.guard.marker.poison {
            state.guard.marker.poison = false;
            if let Err(error) = state.guard.save() {
                state.failed = true;
                return Err(error);
            }
        }
        state.finished = true;
        Ok(())
    }
}

const MAX_LEASES: usize = 256;
struct Broker {
    queue: Mutex<(Vec<Job>, usize)>,
    wake: Condvar,
}
struct Permit(&'static Broker);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.queue.lock().unwrap().1 -= 1;
        self.0.wake.notify_one();
    }
}
struct Job {
    namespace: Namespace,
    backend: Arc<dyn Backend>,
    name: String,
    wait: Arc<Mutex<Waiting>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    permit: Permit,
}
#[derive(Default)]
struct Waiting {
    cancelled: bool,
    result: Option<Result<Arc<dyn SecretLease>, String>>,
}
struct Wait(Arc<Mutex<Waiting>>);
impl SecretWait for Wait {
    fn take(&mut self) -> Option<Result<Arc<dyn SecretLease>, String>> {
        self.0.lock().unwrap().result.take()
    }
}
impl Drop for Wait {
    fn drop(&mut self) {
        let result = {
            let mut state = self.0.lock().unwrap();
            state.cancelled = true;
            state.result.take()
        };
        drop(result);
        broker().wake.notify_one();
    }
}
fn broker() -> &'static Broker {
    static BROKER: OnceLock<&'static Broker> = OnceLock::new();
    BROKER.get_or_init(|| {
        let broker: &'static Broker = Box::leak(Box::new(Broker {
            queue: Mutex::new((Vec::new(), 0)),
            wake: Condvar::new(),
        }));
        std::thread::Builder::new()
            .name("exact secrets".into())
            .spawn(move || run(broker))
            .expect("secret broker thread starts");
        broker
    })
}
impl SharedSecrets for Coordinator {
    fn acquire(&self, name: &str, wake: Arc<dyn Fn() + Send + Sync>) -> Box<dyn SecretWait> {
        let wait = Arc::new(Mutex::new(Waiting::default()));
        let broker = broker();
        let mut queue = broker.queue.lock().unwrap();
        if queue.1 == MAX_LEASES {
            wait.lock().unwrap().result = Some(Err("too many shared secret transactions".into()));
            drop(queue);
            wake();
        } else {
            queue.1 += 1;
            queue.0.push(Job {
                namespace: self.namespace.clone(),
                backend: self.backend.clone(),
                name: name.into(),
                wait: wait.clone(),
                wake,
                permit: Permit(broker),
            });
            broker.wake.notify_one();
        }
        Box::new(Wait(wait))
    }
}
fn run(broker: &'static Broker) {
    loop {
        let jobs = {
            let mut queue = broker.queue.lock().unwrap();
            while queue.0.is_empty() {
                queue = broker.wake.wait(queue).unwrap();
            }
            std::mem::take(&mut queue.0)
        };
        let mut pending = Vec::new();
        for job in jobs {
            if job.wait.lock().unwrap().cancelled {
                continue;
            }
            let acquired = match job.namespace.lock(&job.name) {
                Ok(None) => {
                    pending.push(job);
                    continue;
                }
                Ok(Some(mut guard)) => {
                    let prepared = (|| {
                        guard.exclusive = true;
                        guard.marker.active = true;
                        guard.marker.epoch = guard
                            .marker
                            .epoch
                            .checked_add(1)
                            .ok_or("secret epoch overflow")?;
                        guard.save()?;
                        if guard.marker.poison {
                            job.backend.write(&job.name, None)?;
                            guard.marker.poison = false;
                            guard.save()?;
                        }
                        Ok(())
                    })();
                    prepared.map(|()| guard)
                }
                Err(error) => Err(error),
            };
            let result = acquired.map(|guard| {
                Arc::new(Lease {
                    backend: job.backend,
                    name: job.name,
                    state: Mutex::new(LeaseState {
                        guard,
                        finished: false,
                        failed: false,
                    }),
                    _permit: job.permit,
                }) as Arc<dyn SecretLease>
            });
            let mut wait = job.wait.lock().unwrap();
            if wait.cancelled {
                drop(wait);
                drop(result);
            } else {
                wait.result = Some(result);
                drop(wait);
                (job.wake)();
            }
        }
        let mut queue = broker.queue.lock().unwrap();
        queue.0.extend(pending);
        if !queue.0.is_empty() {
            drop(
                broker
                    .wake
                    .wait_timeout(queue, Duration::from_millis(20))
                    .unwrap(),
            );
        }
    }
}

#[cfg(test)]
mod tests;
