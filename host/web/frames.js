// The web's frame sampler (LLP 1079 D3–D4), for both targets: a development
// page's own requestAnimationFrame loop, run only inside active segments. It
// watches; it never keeps the app's frame source running, so the measured
// build schedules frames as the shipped one does. Loaded after first paint
// (glue.js `loadAfterPaint`, perf.js), never in a production build.
//
// A segment starts on activity — input, a scroll, a commit or batch the host
// reports, the page becoming visible, the sampler's own start — and ends
// 500 ms after the last, unless an animation is still running, or when the
// page hides. Its first callback is the baseline, never a sample. The period
// is the segment's `floor`: the lowest median of 8 consecutive intervals,
// never raised by a slower phase inside the segment (pace.js's fitted period
// would adopt sustained jank), estimated afresh each segment, since the
// display's rate may have changed between. A segment's samples before its
// floor exists are classified once it does; a segment too short to set one
// uses the last segment's.
const RING = 600, LATE = 100, QUIET = 500;
const r2 = x => Math.round(x * 100) / 100;

/** `origin`: performance.now() at the runner's clock zero. `log(line)`: a
 * journal line. `gather()`: the rest of a trace (D5) — `{journal, perf}` —
 * which ⌥⇧T posts with the frames to the dev server's `/__exact/trace`. */
export function createFrameSampler({ origin, log, gather, covers, target }) {
  const ring = [], late = [];
  // (`reset()`, below the functions, sets these.)
  let lifetime, dropped, floor, prior, recent, waiting, pending, raf = 0, last = null, active = -Infinity, loafUnmatched;
  const empty = () => ({ first: null, last: null, batches: 0, apply: 0 });
  /** Forget every sample, and the callback in flight: a new runner's
   * transactions are numbered afresh (a dev restart). A segment starts over. */
  function reset() {
    cancelAnimationFrame(raf); raf = 0; last = null;
    ring.length = late.length = 0;
    lifetime = { presented: 0, late: 0, missed: 0, segments: 0 };
    dropped = loafUnmatched = 0; floor = prior = Infinity; recent = []; waiting = []; pending = empty();
  }
  const running = () => document.getAnimations?.().some(a => a.playState === 'running');
  function tick(ts) {
    raf = 0;
    if (last != null) sample(ts - last, ts);
    last = ts;
    if (document.visibilityState !== 'visible' || performance.now() - active > QUIET && !running()) return end();
    raf = requestAnimationFrame(tick);
  }
  function end() {
    cancelAnimationFrame(raf); raf = 0; last = null;
    for (const r of waiting.splice(0)) if (isFinite(prior)) classify(r, prior);
    if (isFinite(floor)) prior = floor;
  }
  function activity() {
    active = performance.now();
    if (!raf && document.visibilityState === 'visible') { lifetime.segments++; last = null; floor = Infinity; recent = []; raf = requestAnimationFrame(tick); }
  }
  function classify(record, period) {
    record.missed = Math.max(0, Math.round(record.interval / period) - 1);
    if (!record.missed) return;
    lifetime.late++; lifetime.missed += record.missed;
    late.push(record);
    if (late.length > LATE) late.shift();
    log(`frame late at ${record.t}: ${record.missed} missed (${record.interval} ms / ${r2(period)} floor)${record.seq ? ` seq ${record.seq[0]}..${record.seq[1]}` : ''} apply ${record.apply}`);
  }
  function sample(interval, ts) {
    recent.push(interval);
    if (recent.length > 8) recent.shift();
    if (recent.length === 8) floor = Math.min(floor, [...recent].sort((a, b) => a - b)[4]);
    const p = pending, record = { t: r2(ts - origin()), interval: r2(interval), missed: null, seq: p.first == null ? null : [p.first, p.last], batches: p.batches, apply: r2(p.apply) };
    pending = empty();
    ring.push(record);
    if (ring.length > RING) { ring.shift(); dropped++; }
    lifetime.presented++;
    if (!isFinite(floor)) { waiting.push(record); return; }
    for (const r of waiting.splice(0)) classify(r, floor);
    classify(record, floor);
  }
  // A long animation frame (Chrome's LoAF, frames over 50 ms) annotates the
  // retained sample whose interval overlaps it most, once; one that overlaps
  // none is counted, never invented.
  try {
    new PerformanceObserver(list => {
      for (const e of list.getEntries()) {
        const from = e.startTime - origin(), to = from + e.duration;
        let r = null, most = 0;
        for (const x of ring) { const o = Math.min(to, x.t) - Math.max(from, x.t - x.interval); if (!x.loaf && o > most) { most = o; r = x; } }
        if (!r) { loafUnmatched++; continue; }
        r.loaf = { script: r2((e.scripts ?? []).reduce((n, s) => n + s.duration, 0)), ...(e.styleAndLayoutStart ? { styleLayout: r2(e.startTime + e.duration - e.styleAndLayoutStart) } : {}) };
      }
    }).observe({ type: 'long-animation-frame' });
  } catch {}
  for (const kind of ['pointerdown', 'pointermove', 'keydown', 'wheel', 'touchstart', 'touchmove', 'scroll']) addEventListener(kind, activity, { capture: true, passive: true });
  document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') activity(); else end(); });
  const save = async () => {
    try {
      const identity = { host: 'web', target, userAgent: navigator.userAgent, url: location.href, devicePixelRatio, viewport: [innerWidth, innerHeight], bootWall: Math.round(performance.timeOrigin + origin()) };
      // Each timing's provenance (LLP 1041 §5): what the numbers are proxies for.
      const proxies = { period: 'floor: the segment\'s lowest median of 8 consecutive requestAnimationFrame intervals', interval: 'requestAnimationFrame callback gap', covers, loaf: PerformanceObserver.supportedEntryTypes?.includes('long-animation-frame') ? 'long-animation-frame entries, frames over 50 ms' : 'unsupported by this browser' };
      const body = JSON.stringify({ identity, proxies, ...await gather(), frames: sampler.reply(LATE, true) });
      const r = await (await fetch('/__exact/trace', { method: 'POST', headers: { 'content-type': 'application/json' }, body })).json();
      log(r.path ? `trace saved: ${r.path}` : `trace refused: ${r.error}`);
      console.info(r.path ? `exact: trace saved to ${r.path}` : `exact: trace refused: ${r.error}`);
    } catch (e) { log(`trace refused: ${e.message}`); }
  };
  addEventListener('keydown', e => { if (e.altKey && e.shiftKey && e.code === 'KeyT') { e.preventDefault(); save(); } }, true);
  const sampler = {
    /** Save Trace, as ⌥⇧T asks for it (a hatch's `diagnostics.saveTrace()`, LLP 1075.003.000.001 §3.3). */
    save,
    /** A commit or batch the page applied: `seq` its transactions' range (or one number), `ms` the time applying it. */
    batch(seq, ms) {
      const [a, b] = Array.isArray(seq) ? seq : seq == null ? [null, null] : [seq, seq];
      // A nested apply reports before the batch around it: the range is the span.
      if (a != null) { pending.first = Math.min(pending.first ?? a, a); pending.last = Math.max(pending.last ?? b, b); }
      pending.batches++; pending.apply += ms;
      activity();
    },
    /** `{"op":"perf","frames":true[,"late":N]}` (D4); `all` adds every retained record (a trace). */
    reply(n = 20, all = false) {
      const xs = ring.map(r => r.interval).sort((a, b) => a - b), q = f => xs.length ? xs[Math.min(xs.length - 1, Math.floor(f * xs.length))] : null;
      const period = isFinite(floor) ? floor : prior;
      return { period: { ms: isFinite(period) ? r2(period) : null, source: 'floor' }, covers, lifetime: { ...lifetime },
        window: { from: ring[0]?.t ?? null, to: ring.at(-1)?.t ?? null, samples: ring.length, dropped, p50: q(0.5), p95: q(0.95), p99: q(0.99), max: xs.at(-1) ?? null },
        late: n > 0 ? late.slice(-Math.min(LATE, n)) : [], loafUnmatched, ...(all ? { records: ring.slice() } : {}) };
    },
    reset() { reset(); activity(); },
    /** Something the page draws on its own is moving (a GPU canvas's loop): keep the segment open. */
    activity,
  };
  reset();
  activity(); // a page already animating is measured from the start
  return sampler;
}

let windowSampler = null;
/** `{"op":"perf","frames":true,"live":ms}` (D4's live window; the platformer's
 * diary, R11): under the agent's clock no frame is presented, so for `ms` of
 * wall time the clock is lent to the wall. Each animation frame advances the
 * runner to the wall's time (`advance`); a GPU canvas draws on its own frame
 * loop with its perf armed (`gpu.live`); a sampler of the window's own
 * measures what was presented. The reply is D4's, with `live` (the window and
 * the clock it moved) and each world's perf. The world ticks on the wall in
 * the window, so its hash afterwards is not a seeked run's; headless Chrome's
 * frames are its own clock's, not a display's. */
async function liveFrames({ ms, late, origin, log, clock, advance, gpu }) {
  if (windowSampler?.open) throw new Error('perf frames live: a live window is already open');
  windowSampler ??= createFrameSampler({ origin, log, covers: ['canvas'], target: 'wasm', gather: async () => ({}) });
  const sampler = windowSampler, outer = globalThis.exact.frames, from = clock(), w0 = performance.now();
  sampler.open = true; sampler.reset(); globalThis.exact.frames = sampler;
  gpu?.live?.(true);
  let worlds = [];
  try {
    await new Promise((done, fail) => {
      const step = () => { try {
        const elapsed = Math.min(performance.now() - w0, ms);
        advance(from + elapsed); sampler.activity();
        if (elapsed < ms) requestAnimationFrame(step); else done();
      } catch (e) { fail(e); } };
      requestAnimationFrame(step);
    });
  } finally { worlds = gpu?.live?.(false) ?? []; globalThis.exact.frames = outer; sampler.open = false; }
  return { ...sampler.reply(late ?? 20), live: { ms, from, to: clock() }, ...(worlds.length ? { world: worlds } : {}) };
}
globalThis.exact = Object.assign(globalThis.exact ?? {}, { createFrameSampler, liveFrames });
