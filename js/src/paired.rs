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
        if receipt.len() > 1 << 20 || plan.len() > 32 << 20 || bytecode.len() > 32 << 20 {
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
            || bytecode.get(..12) != crate::PRELUDE.get(..12)
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
        for (key, name, bytes) in [
            ("plan", "app.plan", plan),
            ("module", "app.hbc", &bytecode[..]),
        ] {
            let card = &meta[key];
            let digest = format!("{:x}", Sha256::digest(bytes));
            if card["file"].as_str() != Some(name)
                || card["bytes"].as_u64() != Some(bytes.len() as u64)
                || card["sha256"].as_str() != Some(&digest)
            {
                return Err(format!("module candidate {key} does not match its receipt"));
            }
        }
        let plan = Plan::decode(plan).map_err(|e| format!("candidate plan: {e:?}"))?;
        if plan.app_id != expected_app {
            return Err("candidate plan and module name different apps".into());
        }
        Ok(Self {
            plan,
            module: Module::new(bytecode, expected_app, expected_grants),
        })
    }
}
