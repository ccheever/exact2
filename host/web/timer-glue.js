// @ref LLP 1043.000 §3 D8, §6 ruling 5 — one clock owner and one advance
// per frame; ordered catch-up (up to 4096 commits) belongs to the runner.
// Loaded after first paint, also shared by the optional flow executor.
export function timerWake(due, now, agent = false) {
  if (agent || due == null) return { kind: 'none' };
  const ms = Math.max(0, due - now);
  return ms <= 8 * 1000 / 60 ? { kind: 'frame' } : { kind: 'timeout', ms: Math.min(ms, 2147483647) };
}

// @ref LLP 1073 D5 — while the batch says `frames`, every animation frame is
// `present(timestamp)` (timers, then frame tasks); timeouts still wake timers.
export function createTimerScheduler({ now, advance, present = null, agentMode = false, paint = () => {},
  raf = requestAnimationFrame, cancel = cancelAnimationFrame, delay = setTimeout, clearDelay = clearTimeout }) {
  let due = null, frames = false, frame = null, timeout = null, requested = false, running = false, disposed = false;
  const presenting = () => frames && present && !agentMode;
  function schedule() {
    if (running || disposed) return;
    if (timeout !== null) clearDelay(timeout); timeout = null;
    const wake = timerWake(due, now(), agentMode), each = presenting();
    if (requested || each || wake.kind === 'frame') {
      if (frame === null) frame = raf(onFrame);
    } else {
      if (frame !== null) cancel(frame); frame = null;
    }
    if (wake.kind === 'timeout' && (each || frame === null)) timeout = delay(onTimeout, wake.ms);
  }
  function advanceDue() {
    const time = now();
    if (!agentMode && due != null && due <= time) advance(time);
  }
  function onFrame(timestamp) {
    frame = null; requested = false; running = true;
    try { if (presenting()) present(timestamp); else advanceDue(); paint(); }
    finally { running = false; schedule(); }
  }
  function onTimeout() {
    timeout = null; running = true;
    try { advanceDue(); }
    finally { running = false; schedule(); }
  }
  return {
    update(deadline, wantsFrames = false) { due = deadline ?? null; frames = !!wantsFrames; schedule(); },
    requestFrame() { requested = true; schedule(); },
    reset() {
      if (frame !== null) cancel(frame);
      if (timeout !== null) clearDelay(timeout);
      frame = timeout = due = null; requested = frames = false;
    },
    dispose() { this.reset(); disposed = true; },
  };
}
if (globalThis.exact) globalThis.exact.createTimerScheduler = createTimerScheduler;
