//! The presence backend's vaults (LLP 1133.010 D4, D6, §7). Outside the
//! agent, the platform's keychain: locked items in the data-protection
//! keychain, reached through the hook ExactKit installs
//! (`exact_on_presence_keychain`, `Sources/ExactKit/Presence.swift`).
//! Under the agent, the agent's own stores, never a real keychain item: a
//! presence name's plain copy is the drive's secret store's (memory, or the
//! named drive's scratch tree; the Keychain under `EXACT_STORE=real`), its
//! record and its locked items the drive's kv store's, which holds nothing a
//! person's check gates: under the agent the check is a ticket.

use exact_runner::presence::{Backend, Vault};
use ibex2::host::{Bindings, Kv, Secrets};

/// The kv scope of the backend's records (D4 step 1).
pub const RECORDS: &str = "exact.presence";
/// The kv scope of the agent's locked items, `<name>#<generation>`.
pub const LOCKED: &str = "exact.presence.agent";
/// Candidate keychain accounts, with empty values: never secret bytes.
const INDEX: &str = "exact.presence.items";

/// The agent's vault over a drive's bindings.
pub struct AgentVault {
    secrets: Secrets,
    kv: Kv,
}

impl Vault for AgentVault {
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let value = self.secrets.get(name).map_err(|e| format!("{e:?}"))?;
        Ok(value.map(String::into_bytes))
    }
    fn put_plain(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), String> {
        match value {
            Some(v) => self.secrets.set(name, &String::from_utf8_lossy(v)),
            None => self.secrets.forget(name),
        }
        .map_err(|e| format!("{e:?}"))
    }
    fn record(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.kv.get(RECORDS, name).map_err(|e| format!("{e:?}"))
    }
    fn put_record(&mut self, name: &str, record: Option<&[u8]>) -> Result<(), String> {
        match record {
            Some(r) => self.kv.set(RECORDS, name, r),
            None => self.kv.delete(RECORDS, name),
        }
        .map_err(|e| format!("{e:?}"))
    }
    fn generations(&self, name: &str) -> Result<Vec<u64>, String> {
        let keys = self.kv.keys(LOCKED).map_err(|e| format!("{e:?}"))?;
        Ok(keys
            .iter()
            .filter_map(|k| k.strip_prefix(name)?.strip_prefix('#')?.parse().ok())
            .collect())
    }
    fn add_locked(
        &mut self,
        name: &str,
        generation: u64,
        _presence: &exact_runner::presence::Presence,
        value: &[u8],
    ) -> Result<(), String> {
        (self.kv)
            .set(LOCKED, &format!("{name}#{generation}"), value)
            .map_err(|e| format!("{e:?}"))
    }
    fn delete_locked(&mut self, name: &str, generation: u64) -> Result<(), String> {
        (self.kv)
            .delete(LOCKED, &format!("{name}#{generation}"))
            .map_err(|e| format!("{e:?}"))
    }
    fn read_locked(&mut self, name: &str, generation: u64) -> Result<Option<Vec<u8>>, String> {
        (self.kv)
            .get(LOCKED, &format!("{name}#{generation}"))
            .map_err(|e| format!("{e:?}"))
    }
}

/// The app's `secret.keep` names, each with its presence policy.
pub fn lines(grants: &str) -> Vec<(String, Option<exact_runner::presence::Presence>)> {
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

/// The backend under the agent, reconciled before boot, for an app with a
/// presence line; `None` outside the agent or for an app with none.
pub fn agent_backend(bindings: Option<&Bindings>, grants: &str, agent: bool) -> Option<Backend> {
    let bindings = bindings.filter(|_| agent)?;
    let lines = lines(grants);
    lines.iter().any(|(_, p)| p.is_some()).then(|| {
        let vault = AgentVault {
            secrets: bindings.secrets.clone(),
            kv: bindings.kv.clone(),
        };
        Backend::new(Box::new(vault), lines)
    })
}

/// The kv scopes the backend keeps, granted beside the app's own.
pub fn scopes() -> String {
    format!("storage.kv {RECORDS}\nstorage.kv {LOCKED}\nstorage.kv {INDEX}")
}

/// The platform's keychain for locked items (LLP 1133.010 D6), reached
/// through the hook ExactKit installs (`exact_on_presence_keychain`): one
/// generic-password item per name and generation in the data-protection
/// keychain, `kSecAttrService` the app's id and `.presence`,
/// `kSecAttrAccount` `<name>#<generation>`, its access control the line's
/// policy. Each call returns once the keychain has answered, on the main
/// thread; none shows the person anything. Reading an item takes the
/// context the module's sheet evaluated, so it is ExactKit's alone (D13).
pub type KeychainFn = extern "C" fn(
    op: u32,
    json: *const u8,
    len: usize,
    value: *const u8,
    value_len: usize,
    reply: *mut std::ffi::c_void,
    done: extern "C" fn(*mut std::ffi::c_void, i32, *const u8, usize),
);

static KEYCHAIN: std::sync::Mutex<Option<KeychainFn>> = std::sync::Mutex::new(None);

/// Install the keychain hook (`exact_on_presence_keychain`), once a process.
pub fn on_keychain(f: Option<KeychainFn>) {
    *KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner()) = f;
}

/// The hook's answer: the `Option` the call below waits on.
#[allow(unsafe_code)] // One slot crosses the C seam and is answered before the call returns.
extern "C" fn answered(reply: *mut std::ffi::c_void, status: i32, bytes: *const u8, len: usize) {
    // SAFETY: `reply` is `call`'s slot, alive for its synchronous hook call.
    let slot = unsafe { &mut *(reply as *mut Option<(i32, Vec<u8>)>) };
    let bytes = match bytes.is_null() || len == 0 {
        true => Vec::new(),
        // SAFETY: the hook passes `len` readable bytes.
        false => unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec(),
    };
    *slot = Some((status, bytes));
}

/// One keychain call: its status (0 done, 1 not found, below 0 failed) and
/// the bytes it answered (a failure's message).
fn call(op: u32, request: serde_json::Value, value: &[u8]) -> Result<(i32, Vec<u8>), String> {
    let Some(f) = *KEYCHAIN.lock().unwrap_or_else(|e| e.into_inner()) else {
        return Err("this build's host installed no keychain for locked secrets".into());
    };
    let json = request.to_string();
    let mut slot: Option<(i32, Vec<u8>)> = None;
    let reply = &mut slot as *mut Option<(i32, Vec<u8>)> as *mut std::ffi::c_void;
    f(
        op,
        json.as_ptr(),
        json.len(),
        value.as_ptr(),
        value.len(),
        reply,
        answered,
    );
    let (status, bytes) = slot.ok_or("the keychain did not answer")?;
    if status < 0 {
        return Err(String::from_utf8_lossy(&bytes).into_owned());
    }
    Ok((status, bytes))
}

/// The service locked items live under: the app's store service and `.presence`.
pub fn locked_service(app_id: &str) -> String {
    format!("{app_id}.presence")
}

/// The real vault: plain copies in the app's Keychain items (`Secrets`), the
/// records in its kv store, locked items through the keychain hook.
pub struct KeychainVault {
    secrets: Secrets,
    kv: Kv,
    service: String,
}

impl KeychainVault {
    fn account(name: &str, generation: u64) -> String {
        format!("{name}#{generation}")
    }
}

impl Vault for KeychainVault {
    fn plain(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let value = self.secrets.get(name).map_err(|e| format!("{e:?}"))?;
        Ok(value.map(String::into_bytes))
    }
    fn put_plain(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), String> {
        match value {
            Some(v) => self.secrets.set(name, &String::from_utf8_lossy(v)),
            None => self.secrets.forget(name),
        }
        .map_err(|e| format!("{e:?}"))
    }
    fn record(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        self.kv.get(RECORDS, name).map_err(|e| format!("{e:?}"))
    }
    fn put_record(&mut self, name: &str, record: Option<&[u8]>) -> Result<(), String> {
        match record {
            Some(r) => self.kv.set(RECORDS, name, r),
            None => self.kv.delete(RECORDS, name),
        }
        .map_err(|e| format!("{e:?}"))
    }
    fn generations(&self, name: &str) -> Result<Vec<u64>, String> {
        let accounts = self.kv.keys(INDEX).map_err(|e| format!("{e:?}"))?;
        let mut generations = Vec::new();
        for account in accounts {
            let Some(generation) = account
                .strip_prefix(name)
                .and_then(|a| a.strip_prefix('#'))
                .and_then(|g| g.parse().ok())
            else {
                continue;
            };
            let request = serde_json::json!({ "service": self.service, "account": account });
            // A failed probe leaves the candidate indexed, but cannot
            // establish which generation may replace the last durable one.
            match call(3, request, &[])? {
                (0, _) => generations.push(generation),
                (1, _) => {
                    let _ = self.kv.delete(INDEX, &account);
                }
                _ => return Err("the keychain returned an unknown probe status".into()),
            }
        }
        Ok(generations)
    }
    fn add_locked(
        &mut self,
        name: &str,
        generation: u64,
        presence: &exact_runner::presence::Presence,
        value: &[u8],
    ) -> Result<(), String> {
        let account = Self::account(name, generation);
        // Index first: a crash after the add leaves a discoverable item;
        // before it, the next probe removes the empty candidate.
        self.kv
            .set(INDEX, &account, &[])
            .map_err(|e| format!("{e:?}"))?;
        let request = serde_json::json!({ "service": self.service,
            "account": account, "policy": presence.line_words() });
        if let Err(error) = call(1, request, value) {
            let _ = self.kv.delete(INDEX, &account);
            return Err(error);
        }
        Ok(())
    }
    fn delete_locked(&mut self, name: &str, generation: u64) -> Result<(), String> {
        let account = Self::account(name, generation);
        let request = serde_json::json!({ "service": self.service, "account": account });
        call(2, request, &[])?;
        self.kv
            .delete(INDEX, &account)
            .map_err(|e| format!("{e:?}"))
    }
    fn delete_all_locked(&mut self, name: &str) -> Result<(), String> {
        let accounts = self.kv.keys(INDEX).map_err(|e| format!("{e:?}"))?;
        for account in accounts {
            if let Some(generation) = account
                .strip_prefix(name)
                .and_then(|a| a.strip_prefix('#'))
                .and_then(|g| g.parse().ok())
            {
                self.delete_locked(name, generation)?;
            }
        }
        Ok(())
    }
    fn read_locked(&mut self, _: &str, _: u64) -> Result<Option<Vec<u8>>, String> {
        // The host reads with the context the sheet evaluated (`Checked::Read`).
        Err("a locked item is read by the host after its check".into())
    }
}

/// Whether this build keeps locked secrets: the hook is installed and says
/// so (iOS; macOS only with a provisioned keychain group, D7).
pub fn keychain_capable() -> bool {
    call(5, serde_json::json!({}), &[]).is_ok_and(|(status, _)| status == 0)
}

/// The backend outside the agent, for an app with a presence line, and the
/// keychain service its locked items live under.
pub fn keychain_backend(
    bindings: Option<&Bindings>,
    grants: &str,
    app_id: &str,
) -> Option<(Backend, String)> {
    let bindings = bindings?;
    let lines = lines(grants);
    if !lines.iter().any(|(_, p)| p.is_some()) || app_id.is_empty() {
        return None;
    }
    let service = locked_service(app_id);
    first_launch(app_id, &service, &bindings.kv);
    let vault = KeychainVault {
        secrets: bindings.secrets.clone(),
        kv: bindings.kv.clone(),
        service: service.clone(),
    };
    Some((Backend::new(Box::new(vault), lines), service))
}

/// iOS keeps keychain items after an app is deleted, so a reinstalled app
/// would find the last install's token (LLP 1018 D7). The first launch of an
/// install, which a marker in Application Support (deleted with the app)
/// tells, deletes every item under the app's two services. A container that
/// already holds the backend's records or kept answers is an update, not an
/// install: a build that first carries the marker signs no one out.
fn first_launch(app_id: &str, service: &str, kv: &Kv) {
    if !cfg!(target_os = "ios") {
        return;
    }
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let marker = std::path::Path::new(&home).join("Library/Application Support/exact/installed");
    if marker.exists() {
        return;
    }
    let empty = |scope: &str| kv.keys(scope).is_ok_and(|k| k.is_empty());
    if empty(RECORDS) && empty(crate::store::KEPT) {
        for service in [app_id, service] {
            let _ = call(4, serde_json::json!({ "service": service }), &[]);
        }
    }
    if let Some(dir) = marker.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&marker, b"");
}

#[cfg(test)]
#[path = "presence_secrets_tests.rs"]
mod tests;
