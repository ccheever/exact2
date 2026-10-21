//! The GPU tier (LLP 1139.003 D2) between this host and its GPU modules:
//! the agent's told before a module makes any device, the module's answer
//! read back for the page facts, and `prefer gpu-tier` told to every module.
use super::*;

/// The drive's GPU tier (`prefer gpu-tier`), `high` (0) from launch.
static AGENT_TIER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Under the agent the GPU tier is the drive's, told before the module makes
/// any device, so a world's rung and look frames do not follow the machine
/// (LLP 1139.003 D2); otherwise the module chooses from its adapter.
pub(super) fn agent_tier(abi: &Abi) {
    if std::env::var("EXACT_AGENT").as_deref() == Ok("1") {
        abi.tier(AGENT_TIER.load(std::sync::atomic::Ordering::Relaxed));
    }
}

impl Abi {
    /// `gpu_tier(code)`: tells the tier (0 high, 1 mid, 2 low) or, with 3,
    /// asks; the module's answer. None from a module built without it.
    fn tier(&self, code: u32) -> Option<u32> {
        // SAFETY: `gpu_tier(u32) -> u32` as gpu/src/native.rs declares it.
        let tier = unsafe {
            self.library
                .get::<unsafe extern "C" fn(u32) -> u32>(b"gpu_tier")
        }
        .ok()?;
        Some(unsafe { tier(code) })
    }
}

impl Surfaces {
    /// The tier the primary module's surfaces use, for the page facts.
    pub(crate) fn tier(&self) -> Option<u8> {
        let abi = self.abis.get("").or_else(|| self.abis.values().next())?;
        abi.tier(3).map(|code| code as u8)
    }
    /// `prefer gpu-tier`: every module takes the drive's tier and makes its
    /// worlds' renderers again at the new rung, keeping the worlds.
    pub(crate) fn retier(&mut self, code: u8) {
        AGENT_TIER.store(u32::from(code), std::sync::atomic::Ordering::Relaxed);
        for abi in self.abis.values() {
            abi.tier(u32::from(code));
            // SAFETY: `gpu_retier()` as gpu/src/native.rs declares it.
            if let Ok(retier) = unsafe { abi.library.get::<unsafe extern "C" fn()>(b"gpu_retier") }
            {
                unsafe { retier() };
            }
        }
    }
}
