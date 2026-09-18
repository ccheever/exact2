// Frame pacing for the live frame clock (LLP 1041.001 D3: the host owns the clock).
//
// Chrome's requestAnimationFrame timestamp wanders around the display's vsync grid
// under load while the presented frames land on it (measured on the 120 Hz display:
// deltas of 7.8–8.9 ms with a lag-1 autocorrelation of −0.54; the Beacons player,
// drawn exactly at that clock, moved with a per-frame displacement CV of 0.106). A
// world interpolates between its last two ticks at the time the frame is *shown*, so
// the clock the host hands the module must be that time: each callback is snapped to
// the nearest slot of a lattice `origin + k·period` locked to the callbacks
// themselves. The period is the mean of recent consecutive deltas (which telescopes
// to the span, so its error falls as 1/window); the phase follows the callbacks with
// a small gain, so the constant callback lateness is invisible and the jitter is
// attenuated by that gain. A late callback snaps to its slot like any other — the
// frame it draws still lands on a vsync — and a gap of n slots advances the clock
// n periods. When the callbacks stop fitting the lattice (a refresh-rate change:
// the mean residual grows), the period is estimated again. The agent's clock never
// goes through here.
export function pacer({ window = 256, gain = 0.02 } = {}) {
  let origin = null, period = null, last = null, misfit = 0, paced = -Infinity;
  let displayPeriod = 0, skipped = 0;
  let deltas = [];
  // The paced clock never runs backwards: a callback that lands before the last
  // slot (a callback delivered early, a bootstrap sample) redraws at that slot.
  const pace = now => paced = Math.max(paced, step(now));
  // Publish a fitted display period, not a new lookahead on every noisy sample.
  // The 16-sample median bootstraps L; full rolling fits refine it. A stall can move the
  // lattice's origin without changing its rate: retain L while fitting again,
  // and replace it only when the new rate differs beyond sampling noise (1%).
  Object.defineProperty(pace, 'period_ms', { get: () => displayPeriod });
  return pace;
  function step(now) {
    if (last === null) { last = origin = now; return now; }
    const delta = now - last;
    if (delta <= 0) return paced;
    // Ignore duplicate/very early callbacks without contaminating the next delta.
    if (period !== null && delta < period * 0.25) return paced;
    last = now;
    // A half-period callback belongs to a faster display, not the next old slot.
    // Keep the published period while reacquiring from raw monotonic timestamps.
    if (period !== null && delta < period * 0.6) { acquire(now); return now; }
    if (period === null) {
      deltas.push(delta);
      if (deltas.length < 16) return now;
      period = median(deltas);
      publish();
      deltas = [];
      origin = now;
      return now;
    }
    const k = Math.round((now - origin) / period);
    if (k < 1) return now;
    const slot = origin + k * period, residual = now - slot;
    // A lattice that fits leaves residuals well inside a slot; one that does not
    // (callbacks at another rate) leaves them near ±period/2 on most frames.
    misfit += (Math.abs(residual) / period - misfit) / 8;
    skipped = k > 1 ? skipped + 1 : 0;
    // A trailing mean lags a gradual drift: its accumulated phase residual alone
    // must not keep resetting the window before it can publish. Reacquire only
    // when recent intervals also disagree with the fitted rate.
    const changed = deltas.length >= 16 && Math.abs(median(deltas.slice(-16)) - period) > period * 0.05;
    if ((misfit > 1 / 6 && changed) || skipped >= 32) { acquire(now); return now; }
    if (Math.abs(residual) <= period / 2) {
      // Include harmonic samples; a sustained k>1 run reacquires the raw cadence.
      // A lone missed slot remains a stall, not a slower display.
      deltas.push(delta / k);
      if (deltas.length > window) deltas.shift();
      period = deltas.reduce((sum, d) => sum + d, 0) / deltas.length;
      if (deltas.length === window) publish();
    }
    // A young estimate drifts faster than a small gain can follow; let the phase
    // move freely until the window has filled enough to trust the period.
    origin = slot + residual * Math.max(gain, 1 / (deltas.length + 1));
    return slot;
  }
  function publish() {
    if (!displayPeriod || Math.abs(period - displayPeriod) > displayPeriod * 0.01) displayPeriod = period;
  }
  function acquire(now) {
    period = null; deltas = []; misfit = skipped = 0; origin = now;
  }
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b), mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}
