// The device's GPU tier on the web (LLP 1139.003 D2), chosen once by
// gpu-glue.js before the GPU module makes its device: from GPUAdapterInfo's
// `vendor` and `architecture`, else the screen's class (navigation.js
// `handheld`). Never from a clock. Under the agent it is the drive's (`high`
// until `prefer gpu-tier`), and no adapter is requested for it. The table is
// gpu/src/tier.rs `classify`, which native modules use on their own adapter.
import { handheld } from "./navigation.js";

/** The tiers by wire code (`gpu_tier`, the page facts' bits 11–12). */
export const TIERS = ["high", "mid", "low"];

/** gpu/src/tier.rs `classify` over GPUAdapterInfo's words. */
export function classify(vendor = "", architecture = "", software = false, small = false) {
  vendor = vendor.toLowerCase(); architecture = architecture.toLowerCase();
  const model = Number(/adreno\D*(\d+)/.exec(architecture)?.[1] ?? 0), generation = model >= 100 ? Math.floor(model / 100) : model;
  const bifrost = [31, 51, 52, 71, 72, 76].includes(Number(/mali-g(\d+)/.exec(architecture)?.[1]));
  const known = software ? "low"
    : vendor.includes("qualcomm") || architecture.includes("adreno") ? (generation >= 7 ? "mid" : "low")
    : vendor.includes("arm") || architecture.includes("mali") ? (/midgard|bifrost|mali-t/.test(architecture) || bifrost ? "low" : "mid")
    : vendor.includes("imagination") || architecture.includes("powervr") ? "low"
    : vendor.includes("samsung") || architecture.includes("xclipse") ? "mid"
    : vendor.includes("apple") && Number(/apple a(\d+)/.exec(architecture)?.[1] ?? 99) < 14 ? "low"
    : ["apple", "nvidia", "amd", "intel"].some(v => vendor.includes(v)) ? "high"
    : null;
  if (known) return small && known === "high" ? "mid" : known;
  return small ? "low" : "high";
}

/** A refusal gpu-glue.js reports as the host's existing `no-adapter`. */
const noAdapter = (why) => Object.assign(new Error(`no adapter: ${why}`), { code: "no-adapter" });

/** The tier, told to the page's facts; throws `no-adapter` when this browser has no WebGPU or no adapter. */
export async function chooseTier(page, platform = globalThis) {
  const gpu = platform.navigator?.gpu;
  if (!gpu) throw noAdapter("this browser has no WebGPU (navigator.gpu is absent)");
  if (page?.agent) return page.read()["gpu-tier"] ?? "high";
  const adapter = await gpu.requestAdapter();
  if (!adapter) throw noAdapter("requestAdapter returned null");
  const info = adapter.info ?? {};
  const tier = classify(info.vendor, info.architecture, info.isFallbackAdapter ?? adapter.isFallbackAdapter ?? false, handheld(platform));
  page?.tell?.(tier); page?.changed?.();
  return tier;
}
