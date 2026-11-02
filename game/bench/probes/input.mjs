// One scheduled item / delivered record is ONE key edge, never a whole press.
export const keys = Object.freeze({
  Enter: [13, 'Enter'], KeyW: [87, 'w'], KeyD: [68, 'd'],
  KeyS: [83, 's'], KeyA: [65, 'a'], Space: [32, ' '],
});
export function edge(code, down, trial = -1) {
  const vk = typeof code === 'string' ? keys[code]?.[0] : code;
  if (!Object.values(keys).some(k => k[0] === vk) || typeof down !== 'boolean'
      || !Number.isInteger(trial) || trial < -1) throw new Error('Invalid feel key edge');
  return { code: vk, down, trial };
}

// Latency starts at listener delivery (now), not event.timeStamp.
// Shared by both browser observers. No debouncing/deduplication: unexpected
// delivered edges are evidence of contamination, and must invalidate the run.
export function inputRecorder(target = globalThis, now = () => performance.now()) {
  let events, stamps, count = 0, trial = -1, active = false, overflow = false;
  const codes = Object.fromEntries(Object.entries(keys).map(([name, [vk]]) => [name, edge(vk, false).code]));
  function input(e) {
    if (!active || !Object.hasOwn(codes, e.code) || e.repeat) return;
    if ((count + 1) * 4 > events.length) { overflow = true; return; }
    const i = count * 4;
    stamps[count++] = e.timeStamp;
    events[i] = now(); events[i + 1] = codes[e.code];
    events[i + 2] = +(e.type === 'keydown'); events[i + 3] = trial;
    trial = -1;
  }
  target.addEventListener('keydown', input, true);
  target.addEventListener('keyup', input, true);
  return {
    begin() {
      events = new Float64Array(128 * 4); stamps = new Float64Array(128);
      count = 0; trial = -1; overflow = false; active = true;
    },
    arm(value) { trial = value; },
    end() {
      active = false;
      return { events: Array.from(events.subarray(0, count * 4)),
        event_stamps_ms: Array.from(stamps.subarray(0, count)), overflow };
    },
    dispose() {
      active = false;
      target.removeEventListener('keydown', input, true);
      target.removeEventListener('keyup', input, true);
    },
  };
}
