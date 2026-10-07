// @ref llp/1107.010-mobile-browser-devices.decision.md#connection-and-command-ownership
// T3 Code365aa87982 (MIT): apps/mobile/src/features/devices/shakeDetector.ts
// Original SHA256 d7b818271c67251243ddb3e1eda629adf33ab20b2c4c980ab6ed63cbe439819c; type import adapted only.
export interface AccelerationSample {
  /** Acceleration per axis in g, gravity included. */
  readonly x: number;
  readonly y: number;
  readonly z: number;
  /** Milliseconds. */
  readonly timestamp: number;
}

/**
 * Reports a shake after two strong jolts within a short window. A single bump,
 * setting the phone down, or walking stays below the threshold or the count.
 */
export function createShakeDetector({
  threshold = 1.8,
  window = 600,
  cooldown = 1_000,
}: { threshold?: number; window?: number; cooldown?: number } = {}) {
  let jolts: number[] = [];
  let lastShake = Number.NEGATIVE_INFINITY;
  return (sample: AccelerationSample) => {
    if (sample.timestamp - lastShake < cooldown) return false;
    if (Math.hypot(sample.x, sample.y, sample.z) < threshold) return false;
    jolts = [...jolts.filter((time) => sample.timestamp - time <= window), sample.timestamp];
    if (jolts.length < 2) return false;
    jolts = [];
    lastShake = sample.timestamp;
    return true;
  };
}
