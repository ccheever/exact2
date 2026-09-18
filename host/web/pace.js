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
  let deltas = [];
  // The paced clock never runs backwards: a callback that lands before the last
  // slot (a callback delivered early, a bootstrap sample) redraws at that slot.
  return now => paced = Math.max(paced, step(now));
  function step(now) {
    if (last === null) { last = origin = now; return now; }
    const delta = now - last;
    last = now;
    if (period === null) {
      deltas.push(delta);
      if (deltas.length < 16) return now;
      period = median(deltas);
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
    if (misfit > 1 / 6) { period = null; misfit = 0; origin = now; return now; }
    if (k === 1) {
      deltas.push(delta);
      if (deltas.length > window) deltas.shift();
      period = deltas.reduce((sum, d) => sum + d, 0) / deltas.length;
    }
    // A young estimate drifts faster than a small gain can follow; let the phase
    // move freely until the window has filled enough to trust the period.
    origin = slot + residual * Math.max(gain, 1 / (deltas.length + 1));
    return slot;
  }
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b), mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}
