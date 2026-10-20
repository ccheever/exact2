//! The device's GPU tier (LLP 1139.003 D2): `low`, `mid` or `high`, chosen
//! once, before the device is made, from what the adapter says it is, else
//! from the size of the screen. Never from a clock: a frame drawn under the
//! agent must not depend on how loaded the machine is. A surface reads it
//! when it makes its renderer; a game's quality ladder reads it as
//! `Quality::Auto`.
//!
//! The web host chooses it in the page (`host/web/gpu-tier.js`, from
//! `GPUAdapterInfo`, which wgpu's WebGPU backend does not carry) and tells
//! the module with `gpu_tier` before `gpu_load`; a native host that tells
//! nothing gets [`for_adapter`] on the adapter the module requests. Under the
//! agent every host tells `high`, or what `prefer gpu-tier` set.

use std::sync::atomic::{AtomicU8, Ordering};

/// How much GPU work a frame can afford on this device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuTier {
    /// A phone GPU of the Adreno 600 class or older, or a software adapter.
    Low,
    /// A recent phone or tablet GPU.
    Mid,
    /// A desktop or laptop GPU, and the agent's.
    #[default]
    High,
}

impl GpuTier {
    /// The page facts' word for it.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
        }
    }

    /// The tier by its word.
    pub fn from_keyword(word: &str) -> Option<Self> {
        match word {
            "low" => Some(Self::Low),
            "mid" => Some(Self::Mid),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    /// The hosts' wire code (`gpu_tier`, and bits 10–11 of the page facts
    /// word): 0 high, 1 mid, 2 low, so a host that says nothing says `high`.
    pub fn code(self) -> u32 {
        match self {
            Self::High => 0,
            Self::Mid => 1,
            Self::Low => 2,
        }
    }

    /// The tier by its wire code; 3 is `low`.
    pub fn from_code(code: u32) -> Self {
        match code & 3 {
            0 => Self::High,
            1 => Self::Mid,
            _ => Self::Low,
        }
    }
}

const UNSET: u8 = u8::MAX;
static TIER: AtomicU8 = AtomicU8::new(UNSET);

thread_local! {
    static THREAD: std::cell::Cell<Option<GpuTier>> = const { std::cell::Cell::new(None) };
}

/// The tier this module's surfaces use: this thread's ([`set_thread_tier`]),
/// else what the host told, else what the module chose when it made the
/// device, else `high`.
pub fn tier() -> GpuTier {
    if let Some(tier) = THREAD.with(std::cell::Cell::get) {
        return tier;
    }
    match TIER.load(Ordering::Acquire) {
        UNSET => GpuTier::High,
        code => GpuTier::from_code(u32::from(code)),
    }
}

/// A tier for this thread alone, over the process's: a test's, which must
/// not retier the other tests in its binary. `None` hands it back.
pub fn set_thread_tier(tier: Option<GpuTier>) {
    THREAD.with(|t| t.set(tier));
}

/// The host's answer (`gpu_tier`), which stands over the module's own.
pub fn set_tier(tier: GpuTier) {
    TIER.store(tier.code() as u8, Ordering::Release);
}

/// Whether a host has told the tier or the module has chosen one. A device
/// a fixture or a headless world's look frame makes never chooses it.
#[doc(hidden)]
pub fn chosen() -> bool {
    TIER.load(Ordering::Acquire) != UNSET
}

/// The tier from an adapter's own description: its vendor, its architecture
/// or model name, and whether it is a CPU (software) adapter. `handheld` is
/// a phone-sized screen, which is never `high`. A GPU the table does not
/// know is `low` on a handheld and `high` elsewhere. The web host's table
/// (`host/web/gpu-tier.js`) is this one over `GPUAdapterInfo`'s words.
pub fn classify(vendor: &str, architecture: &str, software: bool, handheld: bool) -> GpuTier {
    let vendor = vendor.to_ascii_lowercase();
    let architecture = architecture.to_ascii_lowercase();
    // The number after `prefix` in the architecture or model name: "adreno-6xx"
    // and "Adreno (TM) 620" both give 620 or 6, "Apple A14 GPU" 14 for "apple a".
    let number = |prefix: &str| -> Option<u32> {
        let at = architecture.find(prefix)? + prefix.len();
        let digits: String = architecture[at..]
            .trim_start_matches(|c: char| !c.is_ascii_digit())
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    };
    let leading = |n: u32| if n >= 100 { n / 100 } else { n };
    let known = if software {
        Some(GpuTier::Low)
    } else if vendor.contains("qualcomm") || architecture.contains("adreno") {
        // Adreno 600 and older (the Pixel 5's 620 among them) are low.
        Some(match number("adreno").map(leading) {
            Some(g) if g >= 7 => GpuTier::Mid,
            _ => GpuTier::Low,
        })
    } else if vendor.contains("arm") || architecture.contains("mali") {
        // Valhall and later are mid; Midgard (Mali-T) and Bifrost (Mali-G31,
        // G51, G52, G71, G72, G76) are low, by family word or model name.
        let bifrost = number("mali-g").is_some_and(|g| [31, 51, 52, 71, 72, 76].contains(&g));
        Some(
            if architecture.contains("midgard")
                || architecture.contains("bifrost")
                || architecture.contains("mali-t")
                || bifrost
            {
                GpuTier::Low
            } else {
                GpuTier::Mid
            },
        )
    } else if vendor.contains("imagination") || architecture.contains("powervr") {
        Some(GpuTier::Low)
    } else if vendor.contains("samsung") || architecture.contains("xclipse") {
        Some(GpuTier::Mid)
    } else if vendor.contains("apple") && number("apple a").is_some_and(|chip| chip < 14) {
        // An iPhone's Metal device names its chip: A13 and older are low.
        Some(GpuTier::Low)
    } else if ["apple", "nvidia", "amd", "intel"]
        .iter()
        .any(|v| vendor.contains(v))
    {
        Some(GpuTier::High)
    } else {
        None
    };
    match (known, handheld) {
        (Some(tier), true) => tier.min(GpuTier::Mid),
        (Some(tier), false) => tier,
        (None, true) => GpuTier::Low,
        (None, false) => GpuTier::High,
    }
}

impl crate::Module {
    /// Every canvas takes the current tier: one that released its renderer
    /// for it is prepared again on the same device, its state untouched.
    pub fn retier(&mut self) {
        for inst in self.instances.values_mut() {
            if !inst.surface.retier() {
                continue;
            }
            if let Some(gpu) = &self.gpu {
                inst.surface.device_ready(gpu.device.features());
                if let Some(config) = &inst.config {
                    inst.surface.prepare_assets(
                        &gpu.device,
                        &gpu.queue,
                        crate::view_format(config),
                    );
                }
            }
        }
    }
}

/// The tier a native module chooses from the adapter it requested, when its
/// host told none: a phone's or a tablet's system is a handheld.
pub fn for_adapter(info: &wgpu::AdapterInfo) -> GpuTier {
    let vendor = match info.vendor {
        0x5143 => "qualcomm",
        0x13B5 => "arm",
        0x1010 => "imagination",
        0x144D => "samsung",
        0x106B => "apple",
        0x10DE => "nvidia",
        0x1002 => "amd",
        0x8086 => "intel",
        _ if info.name.starts_with("Apple") => "apple",
        _ => "",
    };
    let handheld = cfg!(any(target_os = "ios", target_os = "android"));
    classify(
        vendor,
        &info.name,
        info.device_type == wgpu::DeviceType::Cpu,
        handheld,
    )
}

/// Record the module's own choice, unless the host has told one.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn choose(info: &wgpu::AdapterInfo) {
    if !chosen() {
        let _ = TIER.compare_exchange(
            UNSET,
            for_adapter(info).code() as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, GpuTier};

    #[test]
    fn the_table_knows_phone_gpus_and_caps_a_handheld_at_mid() {
        // Chrome 155 on a Pixel 5: GPUAdapterInfo { vendor: "qualcomm", architecture: "adreno-6xx" }.
        assert_eq!(
            classify("qualcomm", "adreno-6xx", false, true),
            GpuTier::Low
        );
        assert_eq!(
            classify("qualcomm", "adreno-7xx", false, true),
            GpuTier::Mid
        );
        assert_eq!(classify("arm", "valhall", false, true), GpuTier::Mid);
        assert_eq!(classify("arm", "bifrost", false, true), GpuTier::Low);
        // Vulkan's model names: Bifrost and Midgard low, Valhall mid.
        assert_eq!(classify("arm", "Mali-G76", false, true), GpuTier::Low);
        assert_eq!(classify("arm", "Mali-T880", false, true), GpuTier::Low);
        assert_eq!(classify("arm", "Mali-G78", false, true), GpuTier::Mid);
        assert_eq!(
            classify("qualcomm", "adreno-10xx", false, true),
            GpuTier::Mid
        );
        assert_eq!(classify("apple", "metal-3", false, false), GpuTier::High);
        assert_eq!(classify("apple", "metal-3", false, true), GpuTier::Mid);
        assert_eq!(classify("nvidia", "ampere", false, false), GpuTier::High);
        assert_eq!(classify("google", "swiftshader", true, false), GpuTier::Low);
        assert_eq!(classify("", "", false, true), GpuTier::Low);
        assert_eq!(classify("", "", false, false), GpuTier::High);
        // Native names: Vulkan's on a Pixel 5, Metal's on an iPhone.
        assert_eq!(
            classify("qualcomm", "Adreno (TM) 620", false, true),
            GpuTier::Low
        );
        assert_eq!(
            classify("qualcomm", "Adreno (TM) 740", false, true),
            GpuTier::Mid
        );
        assert_eq!(
            classify("apple", "Apple A13 GPU", false, true),
            GpuTier::Low
        );
        assert_eq!(
            classify("apple", "Apple A17 Pro GPU", false, true),
            GpuTier::Mid
        );
        assert_eq!(
            classify("apple", "Apple M3 Max", false, false),
            GpuTier::High
        );
    }

    #[test]
    fn codes_round_trip_and_zero_is_high() {
        for tier in [GpuTier::Low, GpuTier::Mid, GpuTier::High] {
            assert_eq!(GpuTier::from_code(tier.code()), tier);
            assert_eq!(GpuTier::from_keyword(tier.keyword()), Some(tier));
        }
        assert_eq!(GpuTier::from_code(0), GpuTier::High);
        assert_eq!(GpuTier::from_keyword("ultra"), None);
    }
}
