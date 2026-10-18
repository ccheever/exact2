//! Admission of one producer result, before creating its engine.
//! @ref LLP 1027 D4/D7 — first frame is data; plan and module travel together.

use crate::{Module, ABI, BYTECODE_VERSION};
use exact_plan::Plan;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A decoded plan and its *unloaded* data module, bound by the producer receipt.
/// Hashes establish pairing, not publisher authenticity. The caller must obtain
/// these bytes through its admitted dev origin or authenticated update channel.
pub struct Paired {
    /// The plan whose initial values were baked through this module.
    pub plan: Plan,
    /// No app code has run yet. Load after first pixel, or while preparing a
    /// replacement beside an already-running client.
    pub module: Module,
}

impl Paired {
    /// Verify the artifact pair against a client's already-admitted identity,
    /// grants, and this executor's ABI/bytecode version. All checks precede app
    /// execution; callers must not take `expected_grants` from the candidate.
    pub fn decode(
        receipt: &str,
        plan: &[u8],
        bytecode: Vec<u8>,
        expected_app: &str,
        expected_grants: &str,
    ) -> Result<Self, String> {
        let module = Self::module(receipt, bytecode, None, expected_app, expected_grants)?;
        let meta: Value = serde_json::from_str(receipt).map_err(|e| e.to_string())?;
        let card = &meta["plan"];
        if plan.len() > 32 << 20
            || card["file"] != "app.plan"
            || card["bytes"].as_u64() != Some(plan.len() as u64)
            || card["sha256"].as_str() != Some(&format!("{:x}", Sha256::digest(plan)))
        {
            return Err("module candidate plan does not match its receipt".into());
        }
        let plan = Plan::decode(plan).map_err(|e| format!("candidate plan: {e:?}"))?;
        if plan.app_id != expected_app {
            return Err("candidate plan and module name different apps".into());
        }
        Ok(Self { plan, module })
    }
    /// Admit bytecode from an already pinned widget generation without decoding
    /// the running app's plan. The generation pairs all its entry plans (D8).
    pub(crate) fn module(
        receipt: &str,
        bytecode: impl AsRef<[u8]> + Into<std::sync::Arc<[u8]>>,
        entry: Option<(&str, &[u8])>,
        expected_app: &str,
        expected_grants: &str,
    ) -> Result<Module, String> {
        let bytes = bytecode.as_ref();
        if receipt.len() > 1 << 20 || bytes.len() > 32 << 20 {
            return Err("module candidate exceeds its size limit".into());
        }
        let meta: Value = serde_json::from_str(receipt).map_err(|e| e.to_string())?;
        if meta["version"].as_u64() != Some(1)
            || !meta["abi"]
                .as_u64()
                .is_some_and(|abi| abi == 1 || abi == ABI as u64)
        {
            return Err("module candidate has an unsupported receipt or seam ABI".into());
        }
        if BYTECODE_VERSION == 0
            || meta["bytecodeVersion"].as_u64() != Some(BYTECODE_VERSION.into())
            || bytes.get(..12) != crate::PRELUDE.get(..12)
        {
            return Err("module candidate has an incompatible Hermes bytecode version".into());
        }
        if expected_app.is_empty() || meta["appId"].as_str() != Some(expected_app) {
            return Err("module candidate names another app".into());
        }
        if meta["grants"].as_str().map(str::trim) != Some(expected_grants.trim()) {
            return Err(
                "module candidate changes the client's admitted grants; rebuild the client".into(),
            );
        }
        let card = &meta["module"];
        if card["file"] != "app.hbc"
            || card["bytes"].as_u64() != Some(bytes.len() as u64)
            || card["sha256"].as_str() != Some(&format!("{:x}", Sha256::digest(bytes)))
        {
            return Err("module candidate module does not match its receipt".into());
        }
        if let Some((kind, plan)) = entry {
            let card = &meta["plans"][kind];
            if plan.len() > 32 << 20
                || card["file"] != format!("plans/{kind}.plan")
                || card["bytes"].as_u64() != Some(plan.len() as u64)
                || card["sha256"].as_str() != Some(&format!("{:x}", Sha256::digest(plan)))
            {
                return Err("snapshot entry does not match its module receipt".into());
            }
        }
        Ok(Module::new(bytecode, expected_app, expected_grants))
    }
}

/// The bytecode's SHA-256, hashed once per process for the same bytes: a
/// render server makes a module per request from one bytecode, and asks
/// each for its revision (the page's checkpoint names it). A few recent
/// bytecodes are kept, compared byte for byte.
pub(crate) fn revision_of(bytecode: &[u8]) -> String {
    static KNOWN: std::sync::Mutex<Vec<(Vec<u8>, String)>> = std::sync::Mutex::new(Vec::new());
    let mut known = KNOWN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some((_, hash)) = known.iter().find(|(bytes, _)| bytes.as_slice() == bytecode) {
        return hash.clone();
    }
    let hash = format!("{:x}", Sha256::digest(bytecode));
    if known.len() == 4 {
        known.remove(0);
    }
    known.push((bytecode.to_vec(), hash.clone()));
    hash
}
