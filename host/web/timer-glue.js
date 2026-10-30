import { createWaitObserver } from './timer-waits-glue.js';
// @ref LLP 1043.000 §3 D8, §6 ruling 5 — one clock owner and one advance
// per frame; ordered catch-up (up to 4096 commits) belongs to the runner.
// Loaded after first paint, also shared by the optional flow executor.
export function timerWake(due, now, agent = false, tolerance = 0) {
  if (agent || due == null) return { kind: 'none' };
  const ms = Math.max(0, due - now);
  return ms <= 8 * 1000 / 60 && tolerance === 0 ? { kind: 'frame' } : { kind: 'timeout', ms: Math.min(ms, 2147483647) };
}

// @ref LLP 1073 D5 — while the batch says `frames`, every animation frame is
// `present(timestamp)` (timers, then frame tasks); timeouts still wake timers.
export function createTimerScheduler({ now, advance, present = null, release = null, agentMode = false, paint = () => {},
  raf = requestAnimationFrame, cancel = cancelAnimationFrame, delay = setTimeout, clearDelay = clearTimeout }) {
  let due = null, frames = false, frame = null, timeout = null, requested = false, running = false, disposed = false;
  let waits = [], admission = null, earliest = null, tolerance = 0;
  const presenting = () => frames && present && !agentMode;
  function schedule() {
    if (running || disposed) return;
    if (timeout !== null) clearDelay(timeout); timeout = null;
    const wake = timerWake(due, now(), agentMode, tolerance), each = presenting();
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
    try { advanceDue(); admission?.check(true); if (presenting()) present(timestamp); paint(); }
    finally { running = false; schedule(); }
  }
  function onTimeout() {
    timeout = null; running = true;
    try { advanceDue(); admission?.check(); }
    finally { running = false; schedule(); }
  }
  return {
    update(deadline, wantsFrames = false, waiting = [], slack = 0) {
      earliest = deadline ?? null; tolerance = slack; due = deadline == null ? null : deadline + slack; frames = !!wantsFrames; waits = waiting;
      if (waiting.length && release && !admission && typeof document !== 'undefined') {
        admission = createWaitObserver({ now, read: () => waits, release, demand: () => frames, next: () => earliest, requestFrame: () => { requested = true; schedule(); } });
      }
      admission?.update(); schedule();
    },
    requestFrame() { requested = true; schedule(); },
    reset() {
      if (frame !== null) cancel(frame);
      if (timeout !== null) clearDelay(timeout);
      admission?.dispose(); admission = null; waits = [];
      frame = timeout = due = null; requested = frames = false;
    },
    dispose() { this.reset(); disposed = true; },
  };
}
if (globalThis.exact) globalThis.exact.createTimerScheduler = createTimerScheduler;
