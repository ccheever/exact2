//! What the host reads from and writes to the machine outside the tree: the
//! app's store files (a named agent drive's, or the app's own) and physical
//! memory.
use super::*;

/// What the app has kept — in a named agent drive's scratch tree, or outside
/// the agent in its own data root ([`crate::picker::secret_root`], LLP
/// 1027.007 D13) — for the runner's boot snapshot. A carried launch and a
/// nameless drive contribute nothing. A fresh drive's
/// tree is emptied first, once a process and before anything is written there
/// (`picker::empty_fresh_tree`); one that cannot be emptied gives nothing,
/// said at boot and again by the activation.
pub(super) fn store_snapshot(app_id: &str, carried: bool) -> Vec<(String, String)> {
    if carried {
        return Vec::new();
    }
    // A tree that could not be emptied is not read: a fresh drive starts
    // with nothing, never with what the last one left (b6 review C1).
    if let Err(e) = crate::picker::empty_fresh_tree(app_id) {
        eprintln!("exact: {e:?}");
        return Vec::new();
    }
    let Some(root) = crate::picker::secret_root(app_id) else {
        return Vec::new();
    };
    let secrets = ibex2::secrets::FileStore::new(root.join("secrets"));
    let kv = ibex2::kv::FileStore::new(root.join("kv"));
    // A leading dot is the store's temporary file, not a secret.
    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root.join("secrets")) {
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !name.starts_with('.') && ibex2::secrets::is_valid_name(&name) {
                names.push(name);
            }
        }
    }
    names.sort();
    let mut out = Vec::new();
    for name in names {
        if let Ok(Some(value)) = ibex2::secrets::SecretStore::get(&secrets, &name) {
            out.push((name, value));
        }
    }
    if let Ok(keys) = ibex2::kv::KvStore::keys(&kv, KEPT) {
        for key in keys {
            if let Ok(Some(bytes)) = ibex2::kv::KvStore::get(&kv, KEPT, &key) {
                if let Ok(value) = String::from_utf8(bytes) {
                    out.push((format!("{}{key}", exact_runner::Store::KEPT), value));
                }
            }
        }
    }
    out
}

/// Write one commit's store log where [`crate::picker::secret_root`] says: a
/// named drive's scratch tree, or the app's own data root. A nameless drive
/// drops the log. A failed write is a journal line.
pub(super) fn persist_store_writes(
    app_id: &str,
    writes: &[exact_runner::StoreWrite],
) -> Vec<String> {
    // Most commits write nothing: no directory is looked up for them.
    if writes.is_empty() {
        return Vec::new();
    }
    let Some(root) = crate::picker::secret_root(app_id) else {
        return Vec::new();
    };
    let secrets = ibex2::secrets::FileStore::new(root.join("secrets"));
    let kv = ibex2::kv::FileStore::new(root.join("kv"));
    let mut errors = Vec::new();
    for write in writes {
        let result = match write.name.strip_prefix(exact_runner::Store::KEPT) {
            Some(key) => match &write.value {
                Some(value) => ibex2::kv::KvStore::set(&kv, KEPT, key, value.as_bytes()),
                None => ibex2::kv::KvStore::delete(&kv, KEPT, key),
            },
            None => match &write.value {
                Some(value) => ibex2::secrets::SecretStore::set(&secrets, &write.name, value),
                None => ibex2::secrets::SecretStore::forget(&secrets, &write.name),
            },
        };
        if let Err(error) = result {
            errors.push(format!("store {} failed: {error}", write.name));
        }
    }
    errors
}

/// Physical memory in bytes, for the canvas budget (LLP 1056 D4): a quarter
/// of it, as WebKit charged on iOS. Linux reports it in `/proc/meminfo`;
/// elsewhere (the host run on a Mac) 8 GiB is assumed.
pub(super) fn physical_memory() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|m| {
            let line = m.lines().find(|l| l.starts_with("MemTotal:"))?;
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            Some(kb * 1024)
        })
        .unwrap_or(8 << 30)
}
