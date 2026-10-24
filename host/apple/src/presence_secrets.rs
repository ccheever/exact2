//! The presence backend's vault under the agent (LLP 1133.010 §7): the
//! agent's own stores, never a real keychain item. A presence name's plain
//! copy is the drive's secret store's (memory, or the named drive's scratch
//! tree; the Keychain under `EXACT_STORE=real`), its record and its locked
//! items the drive's kv store's, which holds nothing a person's check gates:
//! under the agent the check is a ticket.

use exact_runner::presence::{Backend, Vault};
use ibex2::host::{Bindings, Kv, Secrets};

/// The kv scope of the backend's records (D4 step 1).
pub const RECORDS: &str = "exact.presence";
/// The kv scope of the agent's locked items, `<name>#<generation>`.
pub const LOCKED: &str = "exact.presence.agent";

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
    format!("storage.kv {RECORDS}\nstorage.kv {LOCKED}")
}
