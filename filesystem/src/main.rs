// Tooling-only filesystem operations; never linked by an app. @ref LLP 1030.002.
mod directory;
use base64::{engine::general_purpose::STANDARD, Engine};
use directory::{refuse, Directory};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

fn field<'a>(input: &'a Value, key: &str) -> io::Result<&'a str> {
    input[key]
        .as_str()
        .ok_or_else(|| refuse(format!("missing {key}")))
}
fn bytes(input: &Value) -> io::Result<Vec<u8>> {
    STANDARD
        .decode(field(input, "bytes")?)
        .map_err(|_| refuse("invalid base64"))
}
fn tree(
    dir: &Directory,
    prefix: &str,
    out: &mut serde_json::Map<String, Value>,
    read: bool,
) -> io::Result<()> {
    for leaf in dir.names()? {
        let path = format!("{prefix}{leaf}");
        match dir.kind(&leaf)?.st_mode & libc::S_IFMT {
            libc::S_IFDIR => tree(&dir.child(&leaf, false)?, &format!("{path}/"), out, read)?,
            libc::S_IFREG => {
                out.insert(
                    path,
                    if read {
                        Value::String(STANDARD.encode(dir.read(&leaf)?))
                    } else {
                        Value::Null
                    },
                );
            }
            _ => {
                return Err(refuse(
                    "static app files must be regular files or directories",
                ))
            }
        }
    }
    Ok(())
}

// Retained dev generations are ordinary immutable files. Quota admission and
// publication share one OS lock; exhaustion never evicts an existing reader.
fn retained_bytes(dir: &Directory) -> io::Result<u64> {
    let mut bytes = 0u64;
    for leaf in dir.names()? {
        let info = dir.kind(&leaf)?;
        let size = match info.st_mode & libc::S_IFMT {
            libc::S_IFDIR => retained_bytes(&dir.child(&leaf, false)?)?,
            libc::S_IFREG => {
                u64::try_from(info.st_size).map_err(|_| refuse("invalid file size"))?
            }
            _ => {
                return Err(refuse(
                    "retained files must be regular files or directories",
                ))
            }
        };
        bytes = bytes
            .checked_add(size)
            .ok_or_else(|| refuse("retained size overflow"))?;
    }
    Ok(bytes)
}

fn retain(root: &Directory, input: &Value) -> io::Result<Value> {
    let token = field(input, "token")?;
    if token.len() != 48 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(refuse("invalid temporary-file token"));
    }
    let lock = Lock::acquire(root, ".retained/.lock", token)?;
    let quota = input["quota"]
        .as_u64()
        .ok_or_else(|| refuse("missing quota"))?;
    let prefix = field(input, "path")?;
    let (epoch, seq) = prefix
        .split_once('/')
        .ok_or_else(|| refuse("invalid dev generation path"))?;
    if epoch.len() != 32
        || !epoch.bytes().all(|byte| byte.is_ascii_hexdigit())
        || seq.is_empty()
        || !seq.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(refuse("invalid dev generation path"));
    }
    let files = input["files"]
        .as_object()
        .ok_or_else(|| refuse("missing files"))?;
    if !files.contains_key("exact.json") {
        return Err(refuse("retained generation needs an envelope"));
    }
    let mut total = retained_bytes(root)?;
    let mut candidate = Vec::new();
    let mut parents = HashMap::new();
    for (name, encoded) in files {
        let path = format!("{prefix}/{name}");
        let body = STANDARD
            .decode(encoded.as_str().ok_or_else(|| refuse("invalid bytes"))?)
            .map_err(|_| refuse("invalid base64"))?;
        let previous = root
            .parent(&path, false)
            .and_then(|(parent, leaf)| parent.read(&leaf).map(|bytes| (parent, bytes)));
        match previous {
            Ok((parent, old)) if old == body => {
                // A prior failed batch may have linked this immutable payload
                // without syncing its directory. A retry must finish that work.
                let identity = parent.0.metadata()?;
                parents.insert((identity.dev(), identity.ino()), parent);
                continue;
            }
            Ok(_) => return Err(refuse("immutable retained file changed")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        total = total
            .checked_add(body.len() as u64)
            .ok_or_else(|| refuse("retained size overflow"))?;
        candidate.push((name == "exact.json", path, body));
    }
    if total > quota {
        return Err(refuse("retained generation cache quota exceeded"));
    }
    // Independent payload flushes can overlap, but the envelope remains the
    // completion marker: every payload must be durable before it is published.
    candidate.sort_by_key(|(envelope, _, _)| *envelope);
    let envelope = if candidate.last().is_some_and(|(envelope, _, _)| *envelope) {
        candidate.pop()
    } else {
        None
    };
    let workers = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(4);
    let chunk_size = candidate.len().div_ceil(workers).max(1);
    let lock = &lock;
    std::thread::scope(|scope| -> io::Result<()> {
        let handles: Vec<_> = candidate
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || -> io::Result<HashMap<(u64, u64), Directory>> {
                    let mut parents = HashMap::new();
                    for (_, path, body) in chunk {
                        let (parent, leaf) = root.parent(path, true)?;
                        // The caller's random token still owns the temporary name;
                        // each concurrent sibling needs a distinct derivative of it.
                        let temporary = format!("{:x}", Sha256::digest(format!("{token}/{path}")));
                        parent.write_retained_payload(&leaf, body, &temporary[..48], || {
                            lock.verify(root)
                        })?;
                        let identity = parent.0.metadata()?;
                        parents.insert((identity.dev(), identity.ino()), parent);
                    }
                    Ok(parents)
                })
            })
            .collect();
        for handle in handles {
            parents.extend(
                handle
                    .join()
                    .map_err(|_| refuse("retained payload writer failed"))??,
            );
        }
        Ok(())
    })?;
    for parent in parents.into_values() {
        parent.0.sync_all()?;
    }
    if let Some((_, path, body)) = envelope {
        let (parent, leaf) = root.parent(&path, true)?;
        parent.write_checked(&leaf, &body, true, token, || lock.verify(root))?;
    }
    Ok(json!(total))
}

fn operate(root: &Directory, input: &Value, locked: Option<&Lock>) -> io::Result<Value> {
    if let Some(lock) = locked {
        lock.verify(root)?;
    }
    let op = field(input, "op")?;
    if op == "retain" {
        return retain(root, input);
    }
    if op == "tree" || op == "names" || op == "copy" {
        let mut entries = serde_json::Map::new();
        tree(root, "", &mut entries, op != "names")?;
        if op == "copy" {
            let target = Directory::root(field(input, "target")?, true)?;
            for (path, data) in &entries {
                let (parent, leaf) = target.parent(path, true)?;
                let bytes = STANDARD
                    .decode(data.as_str().unwrap())
                    .map_err(|_| refuse("invalid bytes"))?;
                parent.write(&leaf, &bytes, false, field(input, "token")?)?;
            }
            return Ok(Value::Bool(true));
        }
        return Ok(Value::Object(entries));
    }
    let path = field(input, "path")?;
    let parts: Vec<_> = path.split('/').collect();
    if op == "put"
        && parts.len() == 4
        && parts[0] == ".exact"
        && matches!(parts[3], "exact.json" | ".lock")
    {
        return Err(refuse(
            "stream heads require putHead; stream locks are never replaced",
        ));
    }
    let (dir, leaf) = root.parent(path, op == "put" || op == "head")?;
    match op {
        "get" => Ok(Value::String(STANDARD.encode(dir.read(&leaf)?))),
        "list" => Ok(json!(dir.child(&leaf, false)?.names()?)),
        "put" | "head" => {
            let commit = || {
                if let Some(lock) = locked {
                    lock.verify(root)?;
                }
                if op == "head" {
                    let lock =
                        locked.ok_or_else(|| refuse("head requires the held stream lock"))?;
                    if path != lock.head {
                        return Err(refuse("head does not belong to the held stream lock"));
                    }
                    let digest = match dir.read(&leaf) {
                        Ok(bytes) => Value::String(format!("{:x}", Sha256::digest(bytes))),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => Value::Null,
                        Err(e) => return Err(e),
                    };
                    if digest != input["previousDigest"] {
                        return Err(refuse("the head changed underneath; classify again"));
                    }
                }
                Ok(())
            };
            Ok(json!(dir.write_checked(
                &leaf,
                &bytes(input)?,
                input["immutable"] == true,
                field(input, "token")?,
                commit
            )?))
        }

        _ => Err(refuse("unknown filesystem operation")),
    }
}
struct Lock {
    file: File,
    dir: Directory,
    leaf: String,
    token: Vec<u8>,
    head: String,
    path: String,
}
impl Lock {
    fn acquire(root: &Directory, path: &str, token: &str) -> io::Result<Self> {
        let (dir, leaf) = root.parent(path, true)?;
        let mut file = dir.file(&leaf, libc::O_RDWR | libc::O_CREAT)?;
        // Locks are host-independent OS ownership, not wall-clock/PID leases.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(refuse("stream is locked by another publisher"));
        }
        let old = file.metadata()?;
        let named = dir.kind(&leaf)?;
        if i128::from(old.dev()) != i128::from(named.st_dev) || old.ino() != named.st_ino {
            return Err(refuse("lock changed while acquiring"));
        }
        file.set_len(0)?;
        file.write_all(token.as_bytes())?;
        // This token identifies a live flock holder, not recovery state. Reads
        // see the write immediately; process exit/reboot releases the OS lock.
        // Persisting the token adds no ownership guarantee.
        Ok(Self {
            file,
            dir,
            leaf,
            token: token.as_bytes().to_vec(),
            path: path.to_owned(),
            head: format!(
                "{}/exact.json",
                path.strip_suffix("/.lock")
                    .ok_or_else(|| refuse("invalid stream lock path"))?
            ),
        })
    }
    fn verify(&self, root: &Directory) -> io::Result<()> {
        let (current, _) = root.parent(&self.path, false)?;
        let opened_dir = self.dir.0.metadata()?;
        let current_dir = current.0.metadata()?;
        if opened_dir.dev() != current_dir.dev() || opened_dir.ino() != current_dir.ino() {
            return Err(refuse("locked stream directory was replaced"));
        }
        let old = self.file.metadata()?;
        let named = self.dir.kind(&self.leaf)?;
        if i128::from(old.dev()) != i128::from(named.st_dev)
            || old.ino() != named.st_ino
            || self.dir.read(&self.leaf)? != self.token
        {
            return Err(refuse(
                "stream lock was replaced; refusing publisher operation",
            ));
        }
        Ok(())
    }
    // Drop closes the owned fd. The permanent lock file is never unlinked,
    // so an old owner cannot remove a successor's lock on release.
}
fn response(result: io::Result<Value>, missing: bool) -> Value {
    match result {
        Ok(value) => json!({"value": value}),
        Err(error) if missing && error.kind() == io::ErrorKind::NotFound => json!({"value":null}),
        Err(error) => json!({"error":error.to_string(), "errno":error.raw_os_error()}),
    }
}
fn emit(value: Value) -> io::Result<()> {
    println!("{value}");
    io::stdout().flush()
}
fn main() -> io::Result<()> {
    let mut lines = io::stdin().lock().lines();
    let first: Value =
        serde_json::from_str(&lines.next().ok_or_else(|| refuse("missing request"))??)?;
    let session = first["op"] == "lock";
    let write = session || first["op"] == "put" || first["op"] == "retain";
    let root = match Directory::root(field(&first, "root")?, write) {
        Ok(root) => root,
        Err(error) => {
            emit(response(
                Err(error),
                first["optionalRoot"] == true
                    || matches!(first["op"].as_str(), Some("get" | "list")),
            ))?;
            return Ok(());
        }
    };
    if !session {
        emit(response(
            operate(&root, &first, None),
            matches!(first["op"].as_str(), Some("get" | "list")),
        ))?;
        return Ok(());
    }
    let lock = match Lock::acquire(&root, field(&first, "path")?, field(&first, "token")?) {
        Ok(lock) => lock,
        Err(error) => {
            emit(response(Err(error), false))?;
            return Ok(());
        }
    };
    emit(json!({"value":true}))?;
    for line in lines {
        let input: Value = serde_json::from_str(&line?)?;
        emit(response(
            operate(&root, &input, Some(&lock)),
            matches!(input["op"].as_str(), Some("get" | "list")),
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
