// @ref LLP 1043.000 §3 D8, §6 ruling 5 — one clock owner and one advance
// per frame; ordered catch-up (up to 4096 commits) belongs to the runner.
// Loaded after first paint, also shared by the optional flow executor.
export function timerWake(due, now, agent = false) {
  if (agent || due == null) return { kind: 'none' };
  const ms = Math.max(0, due - now);
  return ms <= 8 * 1000 / 60 ? { kind: 'frame' } : { kind: 'timeout', ms: Math.min(ms, 2147483647) };
}

export function createTimerScheduler({ now, advance, agentMode = false, paint = () => {},
  raf = requestAnimationFrame, cancel = cancelAnimationFrame, delay = setTimeout, clearDelay = clearTimeout }) {
  let due = null, frame = null, timeout = null, requested = false, running = false, disposed = false;
  function schedule() {
    if (running || disposed) return;
    if (timeout !== null) clearDelay(timeout); timeout = null;
    const wake = timerWake(due, now(), agentMode);
    if (requested || wake.kind === 'frame') {
      if (frame === null) frame = raf(onFrame);
    } else {
      if (frame !== null) cancel(frame); frame = null;
      if (wake.kind === 'timeout') timeout = delay(onTimeout, wake.ms);
    }
  }
  function advanceDue() {
    const time = now();
    if (!agentMode && due != null && due <= time) advance(time);
  }
  function onFrame() {
    frame = null; requested = false; running = true;
    try { advanceDue(); paint(); }
    finally { running = false; schedule(); }
  }
  function onTimeout() {
    timeout = null; running = true;
    try { advanceDue(); }
    finally { running = false; schedule(); }
  }
  return {
    update(deadline) { due = deadline ?? null; schedule(); },
    requestFrame() { requested = true; schedule(); },
    reset() {
      if (frame !== null) cancel(frame);
      if (timeout !== null) clearDelay(timeout);
      frame = timeout = due = null; requested = false;
    },
    dispose() { this.reset(); disposed = true; },
  };
}
if (globalThis.exact) globalThis.exact.createTimerScheduler = createTimerScheduler;
