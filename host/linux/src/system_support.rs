//! What the host reads from and writes to the machine outside the tree: the
//! app's store files (a named agent drive's, or the app's own) and physical
//! memory.

/// The kv scope the runner's kept answers live in, beside secrets
/// (LLP 1027 D4). The same scope Apple's store writes.
const KEPT: &str = "exact.kept";

/// The kv scope persisted states live in (LLP 1116 D5): a preference, never
/// a secret. The same scope Apple's agent stores write.
const STATE: &str = "exact.state";

/// What the app has kept — in a named agent drive's scratch tree, or outside
/// the agent in its own data root ([`crate::picker::secret_root`], LLP
/// 1027.007 D13) — for the runner's boot snapshot. A carried launch and a
/// nameless drive contribute nothing. A fresh drive's
/// tree is emptied first, once a process and before anything is written there
/// (`picker::empty_fresh_tree`); one that cannot be emptied gives nothing,
/// said at boot and again by the activation.
pub(super) fn store_snapshot(app_id: &str, carried: bool) -> Vec<(String, Vec<u8>)> {
    if carried {
        return Vec::new();
    }
    // A tree that could not be emptied is not read: a fresh drive starts
    // with nothing, never with what the last one left (b6 review C1).
    if let Err(e) = crate::picker::empty_fresh_tree(app_id) {
        eprintln!("exact: {e:?}");
        return Vec::new();
    }
    let Ok(Some(root)) = crate::picker::secret_root(app_id) else {
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
            out.push((name, value.into_bytes()));
        }
    }
    // The runner's own: kept answers and persisted states (LLP 1116 D5),
    // each a kv scope of its own beside the secrets. No more kept answers
    // are read than the runner keeps together (LLP 1027 D4).
    let mut left = exact_runner::Store::KEPT_TOTAL_BYTES;
    for (scope, prefix) in [
        (KEPT, exact_runner::Store::KEPT),
        (STATE, exact_runner::Store::STATE),
    ] {
        for key in ibex2::kv::KvStore::keys(&kv, scope).unwrap_or_default() {
            if let Ok(Some(value)) = ibex2::kv::KvStore::get(&kv, scope, &key) {
                if scope == KEPT {
                    let Some(rest) = left.checked_sub(value.len()) else {
                        continue;
                    };
                    left = rest;
                }
                out.push((format!("{prefix}{key}"), value));
            }
        }
    }
    out
}

#[derive(Default)]
pub(super) struct StoreWrites(std::collections::BTreeMap<String, PendingWrite>);

struct PendingWrite {
    value: Option<Vec<u8>>,
    warned: bool,
}

impl StoreWrites {
    /// Retry at each commit, with newer writes replacing pending values or removals.
    pub(super) fn flush(
        &mut self,
        app_id: &str,
        writes: &[exact_runner::StoreWrite],
    ) -> Vec<String> {
        for write in writes {
            self.0.insert(
                write.name.clone(),
                PendingWrite {
                    value: write.value.clone(),
                    warned: false,
                },
            );
        }
        if self.0.is_empty() {
            return Vec::new();
        }
        let root = match crate::picker::secret_root(app_id) {
            Ok(None) => {
                self.0.clear();
                return Vec::new();
            }
            Ok(Some(root)) => Ok(root),
            Err(error) => Err(error),
        };
        let stores = root.map(|root| {
            (
                ibex2::secrets::FileStore::new(root.join("secrets")),
                ibex2::kv::FileStore::new(root.join("kv")),
            )
        });
        let mut errors = Vec::new();
        self.0.retain(|name, pending| {
            let result = stores
                .as_ref()
                .map_err(|e| format!("{e:?}"))
                .and_then(|(secrets, kv)| {
                    let owned = [
                        (KEPT, exact_runner::Store::KEPT),
                        (STATE, exact_runner::Store::STATE),
                    ]
                    .into_iter()
                    .find_map(|(scope, prefix)| Some((scope, name.strip_prefix(prefix)?)));
                    match owned {
                        Some((scope, key)) => match &pending.value {
                            Some(value) => ibex2::kv::KvStore::set(kv, scope, key, value),
                            None => ibex2::kv::KvStore::delete(kv, scope, key),
                        },
                        None => match &pending.value {
                            Some(value) => ibex2::secrets::SecretStore::set(
                                secrets,
                                name,
                                &String::from_utf8_lossy(value),
                            ),
                            None => ibex2::secrets::SecretStore::forget(secrets, name),
                        },
                    }
                    .map_err(|e| e.to_string())
                });
            if let Err(error) = result {
                if !pending.warned {
                    errors.push(format!("store {name} failed: {error}"));
                    pending.warned = true;
                }
                true
            } else {
                false
            }
        });
        errors
    }
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
